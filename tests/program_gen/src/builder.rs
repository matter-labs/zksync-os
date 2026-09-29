//! The program builder: it emits bytecode while keeping two invariants true by
//! construction, and the same file carries the independent replayers a test (or
//! a reviewer) uses to confirm those invariants held.
//!
//! Invariant one, stack validity: unless faults are explicitly requested, every
//! opcode is emitted only when the tracked stack has enough items for it, and
//! the tracked stack never passes 1024. To make this cheap to reason about, the
//! builder holds a stronger property: at every block boundary the stack is
//! empty, and every block leaves it empty again. A program is therefore a
//! sequence of self-contained blocks, which is also what lets a shrinker cut it
//! block by block.
//!
//! Invariant two, valid jumps: the only jumps the builder emits are a bounded
//! backward loop and a short forward skip, and both are written as a PUSH of a
//! real JUMPDEST offset immediately followed by JUMP or JUMPI. So every jump in
//! the output is preceded by a push of an offset at which the builder actually
//! placed a JUMPDEST.

use rand_chacha::ChaCha8Rng;

use crate::modes::Mode;
use crate::opcodes::{self, Fork, OpSpec};

/// The interpreter's hard stack ceiling.
pub const STACK_LIMIT: usize = 1024;

/// A self-imposed working ceiling kept below the hard limit. Because a single
/// opcode can only grow the stack by a bounded amount, staying under this while
/// building guarantees the hard limit is never crossed.
const WORKING_CEILING: usize = 1000;

/// A fixed memory word reserved for loop counters. Blocks that run as a loop
/// body never write to memory, so this word is never clobbered by a body and
/// the counter always decrements as intended.
const LOOP_COUNTER_OFFSET: u64 = 0x2000;

/// Gas forwarded to sub-calls. A constant is used (rather than the GAS opcode)
/// so calls can be emitted in every mode, including the gas-agnostic one. It
/// sits under the per-transaction gas cap.
const CALL_FORWARD_GAS: u64 = 0x00ff_ffff;

/// The four call-like opcodes, named so a block can pick one without juggling
/// raw bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CallKind {
    Call,
    CallCode,
    DelegateCall,
    StaticCall,
}

impl CallKind {
    fn opcode(self) -> u8 {
        match self {
            CallKind::Call => opcodes::CALL,
            CallKind::CallCode => opcodes::CALLCODE,
            CallKind::DelegateCall => opcodes::DELEGATECALL,
            CallKind::StaticCall => opcodes::STATICCALL,
        }
    }

    /// Whether this call kind takes a value argument on the stack.
    fn has_value(self) -> bool {
        matches!(self, CallKind::Call | CallKind::CallCode)
    }
}

/// A handle returned when a bounded loop is opened, passed back to close it.
#[derive(Clone, Copy, Debug)]
pub struct LoopHandle {
    top: usize,
}

/// A pending forward jump whose destination is filled in once known.
#[derive(Clone, Copy, Debug)]
pub struct BranchPatch {
    operand_pos: usize,
}

/// The byte range a single block occupies in the runtime code, with its name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockBoundary {
    pub name: &'static str,
    pub start: usize,
    pub end: usize,
}

/// Emits runtime bytecode while maintaining the stack and jump invariants.
pub struct ProgramBuilder {
    fork: Fork,
    mode: Mode,
    allow_stack_faults: bool,
    rng: ChaCha8Rng,
    code: Vec<u8>,
    /// Tracked stack height. Exact in the normal case; only approximate when
    /// faults are allowed, where the invariant is deliberately not upheld.
    stack: usize,
    max_stack: usize,
    boundaries: Vec<BlockBoundary>,
    notes: Vec<String>,
    open_block: Option<(&'static str, usize)>,
}

impl ProgramBuilder {
    pub fn new(fork: Fork, mode: Mode, rng: ChaCha8Rng) -> Self {
        Self {
            fork,
            mode,
            allow_stack_faults: false,
            rng,
            code: Vec::new(),
            stack: 0,
            max_stack: 0,
            boundaries: Vec::new(),
            notes: Vec::new(),
            open_block: None,
        }
    }

    /// Allow the builder to emit opcodes without enough operands, so a caller
    /// can deliberately produce a stack-underflowing program. Off by default.
    pub fn set_allow_stack_faults(&mut self, allow: bool) {
        self.allow_stack_faults = allow;
    }

    pub fn fork(&self) -> Fork {
        self.fork
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    pub fn rng(&mut self) -> &mut ChaCha8Rng {
        &mut self.rng
    }

    pub fn code_len(&self) -> usize {
        self.code.len()
    }

    pub fn current_stack(&self) -> usize {
        self.stack
    }

    /// Record a free-form note (for example the guarantees a block provides).
    pub fn note(&mut self, line: impl Into<String>) {
        self.notes.push(line.into());
    }

    /// Record one property that the current block guarantees.
    pub fn guarantee(&mut self, line: &str) {
        self.notes.push(format!("  guarantee: {line}"));
    }

    // --- block framing -----------------------------------------------------

    /// Open a block. In the normal case the stack must be empty here; that is
    /// the property that makes blocks independently truncatable.
    pub fn begin_block(&mut self, name: &'static str) {
        if !self.allow_stack_faults {
            debug_assert_eq!(
                self.stack, 0,
                "block {name} did not start from an empty stack"
            );
        }
        self.open_block = Some((name, self.code.len()));
    }

    /// Close a block, draining anything it left on the stack so the next block
    /// starts empty. Records the block's byte range.
    pub fn end_block(&mut self) {
        if !self.allow_stack_faults {
            while self.stack > 0 {
                self.op(opcodes::POP);
            }
        }
        self.finish_block();
    }

    /// Close a block that ends execution (RETURN, REVERT, INVALID,
    /// SELFDESTRUCT, STOP). Nothing is drained because nothing runs afterwards.
    pub fn end_terminal_block(&mut self) {
        self.finish_block();
    }

    fn finish_block(&mut self) {
        let (name, start) = self
            .open_block
            .take()
            .expect("end_block without a matching begin_block");
        let end = self.code.len();
        self.boundaries.push(BlockBoundary { name, start, end });
        self.notes
            .push(format!("block {name}: runtime bytes [{start}, {end})"));
    }

    // --- primitive emission ------------------------------------------------

    /// Emit one opcode that carries no immediate bytes, keeping the stack
    /// valid. PUSH must not go through here (it has immediate data); use the
    /// push helpers instead.
    pub fn op(&mut self, byte: u8) {
        let spec = opcodes::lookup(byte).expect("op called with a non-opcode byte");
        debug_assert_eq!(
            spec.immediate_bytes, 0,
            "op cannot emit an opcode with immediate data"
        );
        // The gas-agnostic mode forbids reading remaining gas. Enforce it at the
        // emit point so a stray GAS is caught here, not only by which blocks the
        // caller chose to run.
        debug_assert!(
            byte != opcodes::GAS || self.mode.allows_gas_reads(),
            "gas-agnostic mode must not emit the GAS opcode"
        );
        if !self.allow_stack_faults {
            self.require_depth(spec.stack_in as usize);
            let growth = spec.stack_out as i64 - spec.stack_in as i64;
            if growth > 0 {
                self.ensure_room(growth as usize);
            }
        }
        self.code.push(byte);
        self.apply_stack_effect(spec);
    }

    fn apply_stack_effect(&mut self, spec: OpSpec) {
        self.stack = self
            .stack
            .saturating_sub(spec.stack_in as usize)
            .saturating_add(spec.stack_out as usize);
        self.max_stack = self.max_stack.max(self.stack);
    }

    /// Ensure at least `n` items are on the stack by pushing zeros. Used only as
    /// a safety net; blocks normally push their own operands first.
    fn require_depth(&mut self, n: usize) {
        while self.stack < n {
            self.op(opcodes::PUSH0);
        }
    }

    /// Ensure `extra` more items can be pushed without passing the working
    /// ceiling, popping if needed.
    fn ensure_room(&mut self, extra: usize) {
        while self.stack + extra > WORKING_CEILING {
            self.op(opcodes::POP);
        }
    }

    /// Push a literal value given as big-endian bytes (0 to 32 of them). Empty
    /// input becomes PUSH0.
    pub fn push_bytes(&mut self, data: &[u8]) {
        assert!(data.len() <= 32, "a single push carries at most 32 bytes");
        if data.is_empty() {
            self.op(opcodes::PUSH0);
            return;
        }
        if !self.allow_stack_faults {
            self.ensure_room(1);
        }
        let push_op = opcodes::PUSH1 + (data.len() as u8 - 1);
        self.code.push(push_op);
        self.code.extend_from_slice(data);
        self.stack = self.stack.saturating_add(1);
        self.max_stack = self.max_stack.max(self.stack);
    }

    /// Push a `u64`, trimming leading zero bytes (they do not change the value
    /// the interpreter sees, which is left-padded to 256 bits).
    pub fn push_u64(&mut self, value: u64) {
        let bytes = value.to_be_bytes();
        let first = bytes.iter().position(|b| *b != 0).unwrap_or(bytes.len());
        self.push_bytes(&bytes[first..]);
    }

    /// Push a 32-byte word, trimming leading zeros.
    pub fn push_word(&mut self, word: &[u8; 32]) {
        let first = word.iter().position(|b| *b != 0).unwrap_or(word.len());
        self.push_bytes(&word[first..]);
    }

    /// Push a 20-byte address value, trimming leading zeros.
    pub fn push_address(&mut self, address: &[u8; 20]) {
        let first = address
            .iter()
            .position(|b| *b != 0)
            .unwrap_or(address.len());
        self.push_bytes(&address[first..]);
    }

    // --- jumps and loops ---------------------------------------------------

    /// Place a JUMPDEST and return its byte offset, so a later jump can target
    /// it.
    pub fn place_jumpdest(&mut self) -> usize {
        let offset = self.code.len();
        self.op(opcodes::JUMPDEST);
        offset
    }

    /// Open a loop that runs its body exactly `iterations` times.
    ///
    /// The counter is written to a reserved memory word, the loop top is a real
    /// JUMPDEST, and [`Self::end_loop`] closes the loop by decrementing the
    /// counter and jumping back while it is non-zero. The construct is stack
    /// neutral, so the body sees the same stack on every pass.
    pub fn begin_loop(&mut self, iterations: u64) -> LoopHandle {
        // Store the iteration count: push value, then offset on top, MSTORE.
        self.push_u64(iterations.max(1));
        self.push_u64(LOOP_COUNTER_OFFSET);
        self.op(opcodes::MSTORE);
        let top = self.place_jumpdest();
        LoopHandle { top }
    }

    /// Close a loop opened by [`Self::begin_loop`].
    pub fn end_loop(&mut self, handle: LoopHandle) {
        // Compute counter - 1: push 1, load the counter, subtract.
        self.push_u64(1);
        self.push_u64(LOOP_COUNTER_OFFSET);
        self.op(opcodes::MLOAD);
        self.op(opcodes::SUB);
        // Store the decremented counter back, keeping a copy on the stack.
        self.op(opcodes::DUP1);
        self.push_u64(LOOP_COUNTER_OFFSET);
        self.op(opcodes::MSTORE);
        // Jump back to the top while the counter is non-zero. The push of the
        // destination sits immediately before JUMPI, as the invariant requires.
        self.push_u64(handle.top as u64);
        self.op(opcodes::JUMPI);
    }

    /// Begin a forward skip conditioned on the current top of stack: if it is
    /// non-zero, execution jumps past the region emitted before
    /// [`Self::land_branch`] is called. Consumes the condition.
    pub fn branch_if_top(&mut self) -> BranchPatch {
        if !self.allow_stack_faults {
            self.require_depth(1);
            self.ensure_room(1);
        }
        // A two-byte destination placeholder, backpatched in land_branch.
        self.code.push(opcodes::PUSH2);
        let operand_pos = self.code.len();
        self.code.push(0x00);
        self.code.push(0x00);
        self.stack = self.stack.saturating_add(1);
        self.max_stack = self.max_stack.max(self.stack);
        self.op(opcodes::JUMPI);
        BranchPatch { operand_pos }
    }

    /// Land a forward branch: place the destination JUMPDEST and fill in the
    /// placeholder pushed earlier.
    pub fn land_branch(&mut self, patch: BranchPatch) {
        let dest = self.place_jumpdest();
        debug_assert!(dest < 0x1_0000, "jump destination exceeds a two-byte push");
        self.code[patch.operand_pos] = (dest >> 8) as u8;
        self.code[patch.operand_pos + 1] = (dest & 0xff) as u8;
    }

    // --- higher-level helpers ---------------------------------------------

    /// Write `data` into memory starting at `dest`, one 32-byte word at a time.
    /// The last word is zero-padded on the right; a caller that only reads
    /// `data.len()` bytes never sees the padding. Stack neutral.
    pub fn store_bytes_in_memory(&mut self, dest: u64, data: &[u8]) {
        for (i, chunk) in data.chunks(32).enumerate() {
            let mut word = [0u8; 32];
            word[..chunk.len()].copy_from_slice(chunk);
            self.push_word(&word);
            self.push_u64(dest + (i as u64) * 32);
            self.op(opcodes::MSTORE);
        }
    }

    /// Emit a call-like opcode against `target`, reading its arguments from the
    /// given memory region and ignoring any returned data. The success flag is
    /// popped, so the sequence is stack neutral.
    pub fn emit_call(
        &mut self,
        kind: CallKind,
        target: &[u8; 20],
        args_offset: u64,
        args_len: u64,
        value: u64,
    ) {
        // Arguments are pushed bottom to top, which is the reverse of the order
        // the opcode pops them in. Return data is discarded (zero length).
        self.push_u64(0); // return size
        self.push_u64(0); // return offset
        self.push_u64(args_len); // args size
        self.push_u64(args_offset); // args offset
        if kind.has_value() {
            self.push_u64(value); // value
        }
        self.push_address(target); // callee address
        self.push_u64(CALL_FORWARD_GAS); // forwarded gas
        self.op(kind.opcode());
        self.op(opcodes::POP); // drop the success flag
    }

    // --- finishing ---------------------------------------------------------

    /// Consume the builder and return the runtime code, the block boundaries and
    /// the notes. A trailing STOP is appended if the code does not already end
    /// at a halting opcode, so execution always terminates cleanly.
    pub fn finish(mut self) -> (Vec<u8>, Vec<BlockBoundary>, Vec<String>) {
        if !ends_with_halt(&self.code) {
            self.code.push(opcodes::STOP);
        }
        (self.code, self.boundaries, self.notes)
    }
}

/// Whether the code ends at an opcode that halts execution. This walks the code
/// so a halt byte that is really PUSH data is not mistaken for a terminator.
fn ends_with_halt(code: &[u8]) -> bool {
    let mut last: Option<u8> = None;
    let mut pc = 0;
    while pc < code.len() {
        let byte = code[pc];
        last = Some(byte);
        let imm = opcodes::lookup(byte)
            .map(|s| s.immediate_bytes)
            .unwrap_or(0);
        pc += 1 + imm as usize;
    }
    matches!(
        last,
        Some(
            opcodes::STOP
                | opcodes::RETURN
                | opcodes::REVERT
                | opcodes::INVALID
                | opcodes::SELFDESTRUCT
        )
    )
}

// --- independent verifiers -------------------------------------------------
//
// These do not share state with the builder; they re-derive the invariants from
// the finished bytes, which is exactly what a test needs to trust the output.

/// What a stack replay found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StackReport {
    pub max_depth: usize,
    pub final_depth: usize,
}

/// Why a stack replay rejected a program.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StackViolation {
    /// An opcode needed more items than were on the stack.
    Underflow {
        pc: usize,
        needed: usize,
        had: usize,
    },
    /// The stack grew past the 1024 item ceiling.
    Overflow { pc: usize, depth: usize },
    /// A byte that is not an opcode on this fork was reached as an instruction.
    NotAnOpcode { pc: usize, byte: u8 },
    /// A PUSH claimed more immediate bytes than remain in the code.
    TruncatedPush { pc: usize },
}

/// Replay `code` as a straight line of instructions, tracking stack depth.
///
/// Because the builder makes every block start and end at an empty stack, a
/// straight-line replay is enough to prove no path underflows or overflows: any
/// jump lands at a point whose depth matches the fall-through depth by
/// construction.
pub fn trace_stack(code: &[u8], fork: Fork) -> Result<StackReport, StackViolation> {
    let mut depth: usize = 0;
    let mut max_depth: usize = 0;
    let mut pc = 0;
    while pc < code.len() {
        let byte = code[pc];
        let spec = match opcodes::lookup(byte) {
            Some(spec) if opcodes::is_available(byte, fork) => spec,
            _ => return Err(StackViolation::NotAnOpcode { pc, byte }),
        };
        if depth < spec.stack_in as usize {
            return Err(StackViolation::Underflow {
                pc,
                needed: spec.stack_in as usize,
                had: depth,
            });
        }
        depth = depth - spec.stack_in as usize + spec.stack_out as usize;
        if depth > STACK_LIMIT {
            return Err(StackViolation::Overflow { pc, depth });
        }
        max_depth = max_depth.max(depth);

        let imm = spec.immediate_bytes as usize;
        if pc + 1 + imm > code.len() {
            return Err(StackViolation::TruncatedPush { pc });
        }
        pc += 1 + imm;
    }
    Ok(StackReport {
        max_depth,
        final_depth: depth,
    })
}

/// Why a jump-target check rejected a program.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JumpViolation {
    /// A JUMP or JUMPI was not immediately preceded by a push of its
    /// destination.
    NotPrecededByPush { pc: usize },
    /// The pushed destination does not land on a JUMPDEST opcode.
    NotAJumpdest { pc: usize, dest: u64 },
}

/// Confirm every JUMP and JUMPI targets a JUMPDEST that the code actually
/// contains, and is written as an immediately preceding push of that offset.
/// Returns the number of jumps checked.
pub fn verify_jump_targets(code: &[u8]) -> Result<usize, JumpViolation> {
    let jumpdests = jumpdest_offsets(code);

    let mut jumps_checked = 0;
    // The value of the immediately preceding push, if the previous instruction
    // was a push.
    let mut prev_push: Option<u64> = None;
    let mut pc = 0;
    while pc < code.len() {
        let byte = code[pc];
        let spec = match opcodes::lookup(byte) {
            Some(spec) => spec,
            None => {
                prev_push = None;
                pc += 1;
                continue;
            }
        };

        if byte == opcodes::JUMP || byte == opcodes::JUMPI {
            let dest = match prev_push {
                Some(dest) => dest,
                None => return Err(JumpViolation::NotPrecededByPush { pc }),
            };
            if !jumpdests.contains(&(dest as usize)) {
                return Err(JumpViolation::NotAJumpdest { pc, dest });
            }
            jumps_checked += 1;
        }

        let imm = spec.immediate_bytes as usize;
        if (opcodes::PUSH1..=opcodes::PUSH32).contains(&byte) {
            let end = (pc + 1 + imm).min(code.len());
            prev_push = Some(be_value(&code[pc + 1..end]));
        } else if byte == opcodes::PUSH0 {
            prev_push = Some(0);
        } else {
            prev_push = None;
        }

        pc += 1 + imm;
    }
    Ok(jumps_checked)
}

/// The opcode-aligned offsets at which a JUMPDEST appears (bytes inside push
/// data are skipped, so a JUMPDEST value hidden in push data is not counted).
fn jumpdest_offsets(code: &[u8]) -> std::collections::HashSet<usize> {
    let mut set = std::collections::HashSet::new();
    let mut pc = 0;
    while pc < code.len() {
        let byte = code[pc];
        if byte == opcodes::JUMPDEST {
            set.insert(pc);
        }
        let imm = opcodes::lookup(byte)
            .map(|s| s.immediate_bytes)
            .unwrap_or(0) as usize;
        pc += 1 + imm;
    }
    set
}

/// Interpret up to eight big-endian bytes as a value. Destinations always fit
/// in a small offset, so eight bytes is more than enough.
fn be_value(bytes: &[u8]) -> u64 {
    let mut v: u64 = 0;
    for b in bytes.iter().take(8) {
        v = (v << 8) | (*b as u64);
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    fn builder() -> ProgramBuilder {
        ProgramBuilder::new(
            Fork::Osaka,
            Mode::Unrestricted,
            ChaCha8Rng::seed_from_u64(1),
        )
    }

    #[test]
    fn a_bounded_loop_is_stack_valid_and_jumps_home() {
        let mut b = builder();
        b.begin_block("loop");
        let h = b.begin_loop(4);
        b.push_u64(7);
        b.op(opcodes::POP);
        b.end_loop(h);
        b.end_block();
        let (code, _, _) = b.finish();
        assert!(trace_stack(&code, Fork::Osaka).is_ok());
        assert_eq!(verify_jump_targets(&code).unwrap(), 1);
    }

    #[test]
    fn a_forward_branch_is_valid() {
        let mut b = builder();
        b.begin_block("branch");
        b.push_u64(1); // condition
        let p = b.branch_if_top();
        // A net-zero region that runs when the condition is false.
        b.push_u64(9);
        b.op(opcodes::POP);
        b.land_branch(p);
        b.end_block();
        let (code, _, _) = b.finish();
        assert!(trace_stack(&code, Fork::Osaka).is_ok());
        assert_eq!(verify_jump_targets(&code).unwrap(), 1);
    }

    #[test]
    fn the_verifier_catches_a_deliberate_underflow() {
        let mut b = builder();
        b.set_allow_stack_faults(true);
        b.begin_block("fault");
        b.push_u64(1);
        b.op(opcodes::ADD); // ADD needs two, only one is present
        b.end_terminal_block();
        let (code, _, _) = b.finish();
        assert!(matches!(
            trace_stack(&code, Fork::Osaka),
            Err(StackViolation::Underflow { .. })
        ));
    }

    #[test]
    fn store_bytes_round_trips_through_words() {
        let mut b = builder();
        b.begin_block("mem");
        b.store_bytes_in_memory(0, &[1, 2, 3, 4, 5]);
        b.end_block();
        let (code, _, _) = b.finish();
        assert!(trace_stack(&code, Fork::Osaka).is_ok());
    }
}
