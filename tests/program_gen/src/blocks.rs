//! The building blocks a program is assembled from.
//!
//! Each block is a function that appends a self-contained fragment to the
//! builder: it starts from an empty stack, leaves the stack empty again (except
//! for the terminal blocks, which end execution), records what it guarantees,
//! and records its byte range through the builder's block framing. Assembling a
//! program is then a matter of appending several body blocks and one terminal
//! block, and a shrinker can drop whole blocks because each stands on its own.

use rand::Rng;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

use crate::builder::{CallKind, ProgramBuilder};
use crate::initcode::wrap_runtime_as_initcode;
use crate::modes::{is_system_or_hook_address, Mode, CREATE2_FACTORY};
use crate::opcodes;

/// Cap on the length of any single precompile input we build in memory, so one
/// block never dominates the program's size.
const MAX_PRECOMPILE_INPUT: usize = 256;

/// Gas forwarded to a sub-call a block emits by hand (where the address is on
/// the stack rather than a literal, so `ProgramBuilder::emit_call` cannot be
/// used). A fixed value, mirroring the builder's own forwarding constant, keeps
/// these calls legal in gas-agnostic mode where the GAS opcode is forbidden.
const BLOCK_CALL_FORWARD_GAS: u64 = 0x00ff_ffff;
/// Gas forwarded on a reentrant self-call. Deliberately small: a self-call
/// re-runs this whole runtime, so with a large budget the 2-4 self-calls per
/// frame would branch into an exponential call tree that dominates run time
/// without adding coverage. A small budget keeps the reentrancy a few levels
/// deep, which is enough to exercise reentrant CALL/DELEGATECALL/STATICCALL.
const REENTRANCY_FORWARD_GAS: u64 = 0x0001_0000;

/// Scratch memory word `delegatecall_proxy` parks the freshly created proxy
/// address in. It sits above the small initcode written at offset 0 and below
/// the loop-counter word (0x2000), so it collides with neither.
const PROXY_ADDR_SCRATCH: u64 = 0x1000;

/// Memory offset `create2_collision` uses for its optional second initcode, kept
/// clear of the first initcode at offset 0 and of the loop-counter word.
const SECOND_INITCODE_OFFSET: u64 = 0x0800;

// --- small shared helpers --------------------------------------------------

fn fill_random(b: &mut ProgramBuilder, buf: &mut [u8]) {
    for byte in buf.iter_mut() {
        *byte = b.rng().random::<u8>();
    }
}

fn random_word(b: &mut ProgramBuilder) -> [u8; 32] {
    let mut word = [0u8; 32];
    fill_random(b, &mut word);
    word
}

/// Build a 20-byte address whose value is `low`.
fn address_from_low(low: u64) -> [u8; 20] {
    let mut address = [0u8; 20];
    address[12..20].copy_from_slice(&low.to_be_bytes());
    address
}

/// Choose an address for a call, code query or value transfer. In hook-free
/// mode a system or hook address is replaced with an ordinary one so a plain
/// EVM would reproduce the behavior; unrestricted mode may deliberately target
/// them.
fn pick_call_target(b: &mut ProgramBuilder) -> [u8; 20] {
    let fork = b.fork();
    let mode = b.mode();
    let choice = b.rng().random_range(0..100);
    let mut address = if choice < 40 {
        // A placeholder peer contract. The caller maps these onto real deployed
        // peers; on their own they are just ordinary low addresses.
        address_from_low(0x1000 + b.rng().random_range(0..16))
    } else if choice < 65 {
        // A real precompile on this fork.
        let table = opcodes::precompiles(fork);
        let picked = table[b.rng().random_range(0..table.len())];
        address_from_low(picked.address_low as u64)
    } else if choice < 90 {
        // A fully random address.
        let mut a = [0u8; 20];
        fill_random(b, &mut a);
        a
    } else {
        // A system or hook address, interesting to hit in unrestricted mode.
        address_from_low(0x8003)
    };
    if !mode.allows_system_addresses() && is_system_or_hook_address(&address) {
        address = address_from_low(0x1000);
    }
    address
}

/// Emit one call whose callee address is produced on the stack by `emit_addr`
/// (for example a CALLER or ADDRESS opcode, or an MLOAD of a stored address)
/// rather than pushed as a literal. This is the dynamic-target twin of
/// `ProgramBuilder::emit_call`: it lays out the same operands, forwards a fixed
/// amount of gas, reads no return data and pops the success flag, so the whole
/// sequence is stack neutral. Only plain CALL and CALLCODE carry a value.
fn emit_call_to_dynamic(
    b: &mut ProgramBuilder,
    kind: CallKind,
    emit_addr: impl FnOnce(&mut ProgramBuilder),
    args_offset: u64,
    args_len: u64,
    value: u64,
    forward_gas: u64,
) {
    let has_value = matches!(kind, CallKind::Call | CallKind::CallCode);
    let opcode = match kind {
        CallKind::Call => opcodes::CALL,
        CallKind::CallCode => opcodes::CALLCODE,
        CallKind::DelegateCall => opcodes::DELEGATECALL,
        CallKind::StaticCall => opcodes::STATICCALL,
    };
    // Operands are pushed bottom to top, the reverse of the pop order. The
    // address goes on last but one, immediately below the forwarded gas.
    b.push_u64(0); // return size
    b.push_u64(0); // return offset
    b.push_u64(args_len); // args size
    b.push_u64(args_offset); // args offset
    if has_value {
        b.push_u64(value); // value
    }
    emit_addr(b); // callee address, placed on the stack by the caller
    b.push_u64(forward_gas); // forwarded gas
    b.op(opcode);
    b.op(opcodes::POP); // drop the success flag
}

/// Emit a CREATE2 of the initcode at `[offset, offset+size)` with `salt`, then
/// drop the resulting address. Mirrors the operand order in `create_ops`
/// (salt deepest, then size, offset, value) and is stack neutral.
fn emit_create2(b: &mut ProgramBuilder, offset: u64, size: u64, salt: u64, value: u64) {
    b.push_u64(salt); // salt
    b.push_u64(size); // size
    b.push_u64(offset); // offset
    b.push_u64(value); // value
    b.op(opcodes::CREATE2);
    b.op(opcodes::POP);
}

// --- body blocks -----------------------------------------------------------

/// A run of arithmetic and bitwise opcodes over freshly pushed operands.
pub fn arithmetic_soup(b: &mut ProgramBuilder) {
    b.begin_block("arithmetic_soup");
    b.guarantee("stack neutral across the block");
    b.guarantee("does not read remaining gas or touch storage or memory");

    // Binary and unary opcodes drawn from below. CLZ is only added on forks
    // that have it.
    let binary = vec![
        opcodes::ADD,
        opcodes::MUL,
        opcodes::SUB,
        opcodes::DIV,
        opcodes::SDIV,
        opcodes::MOD,
        opcodes::SMOD,
        opcodes::EXP,
        opcodes::SIGNEXTEND,
        opcodes::LT,
        opcodes::GT,
        opcodes::SLT,
        opcodes::SGT,
        opcodes::EQ,
        opcodes::AND,
        opcodes::OR,
        opcodes::XOR,
        opcodes::BYTE,
        opcodes::SHL,
        opcodes::SHR,
        opcodes::SAR,
    ];
    let mut unary = vec![opcodes::ISZERO, opcodes::NOT];
    if opcodes::is_available(opcodes::CLZ, b.fork()) {
        unary.push(opcodes::CLZ);
    }
    // Seed two operands so the first binary op has inputs.
    let w0 = random_word(b);
    let w1 = random_word(b);
    b.push_word(&w0);
    b.push_word(&w1);

    let steps = b.rng().random_range(4..24);
    for _ in 0..steps {
        if b.rng().random::<bool>() {
            let op = binary[b.rng().random_range(0..binary.len())];
            if b.current_stack() < 2 {
                let w = random_word(b);
                b.push_word(&w);
            }
            b.op(op);
        } else {
            let op = unary[b.rng().random_range(0..unary.len())];
            if b.current_stack() < 1 {
                let w = random_word(b);
                b.push_word(&w);
            }
            b.op(op);
        }
    }
    b.end_block();
}

/// Memory writes and reads at deliberately large offsets, plus a copy.
pub fn memory_ops(b: &mut ProgramBuilder) {
    b.begin_block("memory_ops");
    b.guarantee("exercises memory at offsets near the top of the 32-bit range");
    b.guarantee("stack neutral; may run out of gas at runtime, which is expected");

    // Near u32::MAX - 31, the largest word-aligned offset that still fits the
    // copy-cost narrowing the runtime performs.
    let high_offset: u64 = 0xffff_ffe0;

    // MSTORE a random word high up, then read it back and drop it.
    let word = random_word(b);
    b.push_word(&word);
    b.push_u64(high_offset);
    b.op(opcodes::MSTORE);
    b.push_u64(high_offset);
    b.op(opcodes::MLOAD);
    b.op(opcodes::POP);

    // A single-byte write a little further along.
    let byte = b.rng().random::<u8>();
    b.push_bytes(&[byte]);
    b.push_u64(high_offset + 64);
    b.op(opcodes::MSTORE8);

    // Copy a small window from one high region to another. MCOPY pops the
    // destination, the source, then the length.
    b.push_u64(32); // length
    b.push_u64(high_offset); // source
    b.push_u64(high_offset + 128); // destination
    b.op(opcodes::MCOPY);

    // Reading the return-data size is always safe and records the block touched
    // that surface.
    b.op(opcodes::RETURNDATASIZE);
    b.op(opcodes::POP);

    b.end_block();
}

/// Storage writes and reads: a hot slot, several cold slots, and transient
/// storage.
pub fn storage_ops(b: &mut ProgramBuilder) {
    b.begin_block("storage_ops");
    b.guarantee("writes a hot slot repeatedly, several cold slots, and transient storage");
    b.guarantee("stack neutral");

    // Hot slot: same key written a few times, then read.
    let hot_slot: u64 = 0x10;
    let writes = b.rng().random_range(2..5);
    for _ in 0..writes {
        let value = random_word(b);
        b.push_word(&value); // value
        b.push_u64(hot_slot); // key on top
        b.op(opcodes::SSTORE);
    }
    b.push_u64(hot_slot);
    b.op(opcodes::SLOAD);
    b.op(opcodes::POP);

    // Cold slots: distinct fresh keys.
    let cold = b.rng().random_range(2..5);
    for _ in 0..cold {
        let key = b.rng().random::<u64>();
        let value = random_word(b);
        b.push_word(&value);
        b.push_u64(key);
        b.op(opcodes::SSTORE);
    }

    // Transient storage: write then read a slot.
    let t_slot: u64 = 0x20;
    let t_value = random_word(b);
    b.push_word(&t_value);
    b.push_u64(t_slot);
    b.op(opcodes::TSTORE);
    b.push_u64(t_slot);
    b.op(opcodes::TLOAD);
    b.op(opcodes::POP);

    b.end_block();
}

/// Reads of block and transaction context. Never reads remaining gas.
pub fn environment_reads(b: &mut ProgramBuilder) {
    b.begin_block("environment_reads");
    b.guarantee("reads block and transaction context only");
    b.guarantee("never reads remaining gas, so it is safe in gas-agnostic mode");

    let zero_input = [
        opcodes::ADDRESS,
        opcodes::ORIGIN,
        opcodes::CALLER,
        opcodes::CALLVALUE,
        opcodes::CALLDATASIZE,
        opcodes::CODESIZE,
        opcodes::GASPRICE,
        opcodes::COINBASE,
        opcodes::TIMESTAMP,
        opcodes::NUMBER,
        opcodes::DIFFICULTY,
        opcodes::GASLIMIT,
        opcodes::CHAINID,
        opcodes::SELFBALANCE,
        opcodes::BASEFEE,
        opcodes::BLOBBASEFEE,
        opcodes::PC,
        opcodes::MSIZE,
    ];
    let count = b.rng().random_range(4..zero_input.len());
    for _ in 0..count {
        let op = zero_input[b.rng().random_range(0..zero_input.len())];
        b.op(op);
        b.op(opcodes::POP);
    }

    // A couple of context reads that take one argument.
    let peer = pick_call_target(b);
    b.push_address(&peer);
    b.op(opcodes::BALANCE);
    b.op(opcodes::POP);

    let block_number = b.rng().random::<u32>() as u64;
    b.push_u64(block_number);
    b.op(opcodes::BLOCKHASH);
    b.op(opcodes::POP);

    let blob_index = b.rng().random_range(0..8);
    b.push_u64(blob_index);
    b.op(opcodes::BLOBHASH);
    b.op(opcodes::POP);

    b.end_block();
}

/// A LOG0 through LOG4 with random topics over a small memory payload.
pub fn log_ops(b: &mut ProgramBuilder) {
    b.begin_block("log_ops");
    b.guarantee("emits one log with a memory payload and up to four topics");
    b.guarantee("stack neutral");

    let payload_len = b.rng().random_range(0..65) as u64;
    let mut payload = vec![0u8; payload_len as usize];
    fill_random(b, &mut payload);
    b.store_bytes_in_memory(0, &payload);

    let topics = b.rng().random_range(0..5) as u8;
    // The opcode pops offset, then size, then the topics in order, so push the
    // topics first (deepest), then size, then offset on top.
    for _ in 0..topics {
        let topic = random_word(b);
        b.push_word(&topic);
    }
    b.push_u64(payload_len); // size
    b.push_u64(0); // offset on top
    b.op(opcodes::LOG0 + topics);

    b.end_block();
}

/// Static calls to a handful of this fork's precompiles with structured inputs.
pub fn precompile_calls(b: &mut ProgramBuilder) {
    b.begin_block("precompile_calls");
    b.guarantee("statically calls real precompiles on this fork with valid and edge inputs");
    b.guarantee("stack neutral; return data is discarded");

    let table = opcodes::precompiles(b.fork());
    let how_many = b.rng().random_range(1..=3).min(table.len());
    for _ in 0..how_many {
        let picked = table[b.rng().random_range(0..table.len())];
        let input = precompile_input(b, picked.name);
        b.store_bytes_in_memory(0, &input);
        let target = address_from_low(picked.address_low as u64);
        b.emit_call(CallKind::StaticCall, &target, 0, input.len() as u64, 0);
    }

    b.end_block();
}

/// Build a structured input for a named precompile. Lengths follow each
/// precompile's ABI; contents are a mix of plausible and edge values. Anything
/// longer than the cap is shortened, which still exercises the call.
fn precompile_input(b: &mut ProgramBuilder, name: &str) -> Vec<u8> {
    let mut input = match name {
        "identity" | "sha256" | "ripemd160" => {
            let len = b.rng().random_range(0..64);
            let mut v = vec![0u8; len];
            fill_random(b, &mut v);
            v
        }
        "ecrecover" => {
            // hash (32) | v (32) | r (32) | s (32)
            let mut v = vec![0u8; 128];
            fill_random(b, &mut v);
            v[63] = if b.rng().random::<bool>() { 27 } else { 28 };
            v
        }
        "modexp" => {
            // Header of three 32-byte sizes, then base, exponent, modulus.
            let mut v = Vec::new();
            let mut size_word = [0u8; 32];
            size_word[31] = 32;
            v.extend_from_slice(&size_word); // base size
            v.extend_from_slice(&size_word); // exponent size
            v.extend_from_slice(&size_word); // modulus size
            let mut body = [0u8; 96];
            fill_random(b, &mut body);
            body[95] = 1; // keep the modulus non-zero
            v.extend_from_slice(&body);
            v
        }
        "ecadd" => vec![0u8; 128],
        "ecmul" => vec![0u8; 96],
        "ecpairing" => Vec::new(), // empty input is valid and returns success
        "blake2f" => {
            // rounds (4) | h (64) | m (128) | t (16) | f (1) = 213 bytes.
            let mut v = vec![0u8; 213];
            fill_random(b, &mut v);
            let rounds = b.rng().random_range(0..13u32);
            v[0..4].copy_from_slice(&rounds.to_be_bytes());
            v[212] = if b.rng().random::<bool>() { 1 } else { 0 };
            v
        }
        "point_evaluation" => vec![0u8; 192],
        "p256_verify" => {
            let mut v = vec![0u8; 160];
            fill_random(b, &mut v);
            v
        }
        // BLS precompiles: zero-filled inputs, which are the point-at-infinity or
        // empty cases, capped to keep the block small.
        "bls12_g1add" => vec![0u8; 256],
        "bls12_g1msm" => vec![0u8; 160],
        "bls12_g2add" => vec![0u8; 256],
        "bls12_g2msm" => vec![0u8; 288],
        "bls12_pairing_check" => Vec::new(),
        "bls12_map_fp_to_g1" => vec![0u8; 64],
        "bls12_map_fp2_to_g2" => vec![0u8; 128],
        _ => Vec::new(),
    };
    input.truncate(MAX_PRECOMPILE_INPUT);
    input
}

/// Calls of every kind to other contracts, with the success flag discarded.
pub fn external_calls(b: &mut ProgramBuilder) {
    b.begin_block("external_calls");
    b.guarantee("issues CALL, DELEGATECALL and STATICCALL to other contracts");
    b.guarantee("stack neutral; hook-free mode never targets system addresses");

    let kinds = [
        CallKind::Call,
        CallKind::CallCode,
        CallKind::DelegateCall,
        CallKind::StaticCall,
    ];
    let how_many = b.rng().random_range(1..=2);
    for _ in 0..how_many {
        let kind = kinds[b.rng().random_range(0..kinds.len())];
        let target = pick_call_target(b);

        // A little calldata in memory for the call to read.
        let args_len = b.rng().random_range(0..33) as u64;
        let mut args = vec![0u8; args_len as usize];
        fill_random(b, &mut args);
        b.store_bytes_in_memory(0, &args);

        let value = if b.rng().random_range(0..4) == 0 {
            b.rng().random_range(0..1000)
        } else {
            0
        };
        b.emit_call(kind, &target, 0, args_len, value);
    }

    b.end_block();
}

/// CREATE and CREATE2 with a small, freshly generated child program as initcode.
pub fn create_ops(b: &mut ProgramBuilder) {
    b.begin_block("create_ops");
    b.guarantee("deploys a generated child program with CREATE and CREATE2");
    b.guarantee("stack neutral; the child initcode returns a small runtime");

    let child_runtime = generate_child_runtime(b);
    let child_initcode = wrap_runtime_as_initcode(&child_runtime);
    b.store_bytes_in_memory(0, &child_initcode);
    let len = child_initcode.len() as u64;

    // CREATE pops value, offset, size.
    b.push_u64(len); // size
    b.push_u64(0); // offset
    b.push_u64(0); // value
    b.op(opcodes::CREATE);
    b.op(opcodes::POP);

    // CREATE2 pops value, offset, size, salt.
    let salt = b.rng().random::<u64>();
    b.push_u64(salt); // salt
    b.push_u64(len); // size
    b.push_u64(0); // offset
    b.push_u64(0); // value
    b.op(opcodes::CREATE2);
    b.op(opcodes::POP);

    b.end_block();
}

/// A tiny valid runtime used as the child of a CREATE. It is generated from the
/// parent's rng so the whole program stays deterministic, and kept short so the
/// parent that embeds it stays small.
fn generate_child_runtime(b: &mut ProgramBuilder) -> Vec<u8> {
    let seed = b.rng().random::<u64>();
    let mut child = ProgramBuilder::new(b.fork(), b.mode(), ChaCha8Rng::seed_from_u64(seed));
    child.begin_block("child_body");
    let steps = child.rng().random_range(1..=3);
    for _ in 0..steps {
        let value = child.rng().random::<u32>() as u64;
        child.push_u64(value);
    }
    // Combine whatever is on the stack down toward one value, then drop it.
    while child.current_stack() > 1 {
        child.op(opcodes::ADD);
    }
    if child.current_stack() == 1 {
        child.op(opcodes::POP);
    }
    child.end_block();
    let (runtime, _, _) = child.finish();
    runtime
}

/// A loop that runs a bounded number of times, driven by a memory counter.
pub fn counter_loop(b: &mut ProgramBuilder) {
    b.begin_block("counter_loop");
    b.guarantee("runs its body a fixed number of times via a memory counter");
    b.guarantee("stack neutral; the body does not write memory, so the counter is safe");

    let iterations = b.rng().random_range(2..9);
    let handle = b.begin_loop(iterations);
    // An arithmetic-only body so the reserved counter word is never overwritten.
    let a = b.rng().random::<u32>() as u64;
    let c = b.rng().random::<u32>() as u64;
    b.push_u64(a);
    b.push_u64(c);
    b.op(opcodes::ADD);
    b.op(opcodes::POP);
    b.end_loop(handle);

    b.end_block();
}

/// Drives the stack close to the 1024 item ceiling and back down.
pub fn deep_stack(b: &mut ProgramBuilder) {
    b.begin_block("deep_stack");
    b.guarantee("pushes the stack near the 1024 item limit, then drains it");

    // Stay under the builder's working ceiling so the push loop does not fight
    // the overflow guard; the drain happens when the block closes.
    let target = b.rng().random_range(700..981);
    while b.current_stack() < target {
        b.op(opcodes::PUSH0);
    }

    b.end_block();
}

/// Reads remaining gas and branches on it. Only valid where gas reads are
/// allowed, so callers must not use it in gas-agnostic mode.
pub fn gas_branch(b: &mut ProgramBuilder) {
    b.begin_block("gas_branch");
    b.guarantee("reads remaining gas and takes a branch based on it");

    let threshold = b.rng().random::<u32>() as u64;
    b.op(opcodes::GAS);
    b.push_u64(threshold);
    b.op(opcodes::GT); // condition: remaining gas greater than a threshold
    let patch = b.branch_if_top();
    // A net-zero region taken when the condition is false.
    let w = random_word(b);
    b.push_word(&w);
    b.op(opcodes::POP);
    b.land_branch(patch);

    b.end_block();
}

/// Calls that re-enter our own code (ADDRESS) or our caller (CALLER), mixing
/// CALL, DELEGATECALL and STATICCALL to build a nested, reentrant call graph.
pub fn reentrancy_calls(b: &mut ProgramBuilder) {
    b.begin_block("reentrancy_calls");
    b.guarantee("calls back into ADDRESS and CALLER, forming a reentrant call graph");
    b.guarantee("stack neutral; mixes CALL, DELEGATECALL, STATICCALL; results discarded");

    // Every target is a runtime-computed address (self or caller), never a literal
    // system address, so this block is legal in any mode. Self-calls re-run this
    // same runtime, so the reentrancy nests until gas or the call-depth limit
    // stops it.
    let kinds = [CallKind::Call, CallKind::DelegateCall, CallKind::StaticCall];
    let levels = b.rng().random_range(2..=4);
    for _ in 0..levels {
        let kind = kinds[b.rng().random_range(0..kinds.len())];
        // Re-enter either our own code or whoever called us.
        let addr_op = if b.rng().random::<bool>() {
            opcodes::ADDRESS
        } else {
            opcodes::CALLER
        };

        // A little calldata for the nested call to read.
        let args_len = b.rng().random_range(0..33) as u64;
        let mut args = vec![0u8; args_len as usize];
        fill_random(b, &mut args);
        b.store_bytes_in_memory(0, &args);

        // Only plain CALL forwards value, and only occasionally.
        let value = if matches!(kind, CallKind::Call) && b.rng().random_range(0..4) == 0 {
            b.rng().random_range(0..1000)
        } else {
            0
        };
        emit_call_to_dynamic(
            b,
            kind,
            |b| b.op(addr_op),
            0,
            args_len,
            value,
            REENTRANCY_FORWARD_GAS,
        );
    }

    b.end_block();
}

/// Deploys a small contract whose runtime writes storage, then DELEGATECALLs it
/// so those writes land in this contract's own storage.
pub fn delegatecall_proxy(b: &mut ProgramBuilder) {
    b.begin_block("delegatecall_proxy");
    b.guarantee("DELEGATECALLs a freshly deployed contract whose code writes storage");
    b.guarantee("stack neutral; under DELEGATECALL the callee's SSTOREs hit our storage");

    // A freshly created address is never a system address, so this is legal in
    // every mode: we deploy the proxy ourselves rather than picking a target.
    let proxy_runtime = generate_storage_writer_runtime(b);
    let proxy_initcode = wrap_runtime_as_initcode(&proxy_runtime);
    b.store_bytes_in_memory(0, &proxy_initcode);
    let len = proxy_initcode.len() as u64;

    // CREATE pops value, offset, size and leaves the deployed address on top.
    b.push_u64(len); // size
    b.push_u64(0); // offset
    b.push_u64(0); // value
    b.op(opcodes::CREATE);
    // Park the address in a scratch word: MSTORE pops the offset then the value,
    // so push the offset on top of the address the CREATE just left.
    b.push_u64(PROXY_ADDR_SCRATCH);
    b.op(opcodes::MSTORE);

    // DELEGATECALL into the stored address; its runtime writes our storage. The
    // callee is a freshly deployed leaf (no reentry), so a full gas budget is safe.
    emit_call_to_dynamic(
        b,
        CallKind::DelegateCall,
        |b| {
            b.push_u64(PROXY_ADDR_SCRATCH);
            b.op(opcodes::MLOAD);
        },
        0,
        0,
        0,
        BLOCK_CALL_FORWARD_GAS,
    );

    b.end_block();
}

/// CALL and STATICCALL the chain's system and hook addresses with varied
/// calldata, to hunt for crashes on that surface. Unrestricted mode only.
pub fn system_contract_calls(b: &mut ProgramBuilder) {
    debug_assert!(
        b.mode().allows_system_addresses(),
        "system_contract_calls must only be used where system addresses are allowed"
    );
    b.begin_block("system_contract_calls");
    b.guarantee("calls the chain's system and hook addresses to probe that surface");
    b.guarantee("stack neutral; only legal in unrestricted mode; results discarded");

    let targets = system_and_hook_addresses();
    let how_many = b.rng().random_range(2..=5);
    for _ in 0..how_many {
        let target = targets[b.rng().random_range(0..targets.len())];

        // Varied calldata so different system entry points see different shapes.
        let args_len = b.rng().random_range(0..65) as u64;
        let mut args = vec![0u8; args_len as usize];
        fill_random(b, &mut args);
        b.store_bytes_in_memory(0, &args);

        if b.rng().random::<bool>() {
            // A plain CALL, occasionally carrying a small value.
            let value = if b.rng().random_range(0..3) == 0 {
                b.rng().random_range(1..1000)
            } else {
                0
            };
            b.emit_call(CallKind::Call, &target, 0, args_len, value);
        } else {
            b.emit_call(CallKind::StaticCall, &target, 0, args_len, 0);
        }
    }

    b.end_block();
}

/// Code-account queries (EXTCODESIZE / EXTCODEHASH / BALANCE / EXTCODECOPY) over
/// a mix of targets: self, a low precompile, an almost-certainly-empty address,
/// and a system address when the mode allows one.
pub fn code_account_edge(b: &mut ProgramBuilder) {
    b.begin_block("code_account_edge");
    b.guarantee("reads code size, code hash, balance and code bytes of assorted targets");
    b.guarantee("stack neutral; system targets only when the mode allows them");

    // `None` means self, emitted as the ADDRESS opcode; `Some(addr)` is a literal
    // target. A fully random high address stands in for an empty account: forcing
    // a non-zero high byte keeps it clear of the system/hook ranges in every mode.
    let mut empty = [0u8; 20];
    fill_random(b, &mut empty);
    empty[0] = 0xff;
    let precompile_low = b.rng().random_range(0x01u64..=0x0a);
    let mut targets: Vec<Option<[u8; 20]>> = vec![
        None,                                   // self
        Some(address_from_low(precompile_low)), // a low precompile
        Some(empty),                            // almost certainly empty
    ];
    if b.mode().allows_system_addresses() {
        targets.push(Some(address_from_low(0x8003)));
    }

    // The one-in, one-out queries: push the target, run the query, drop the word.
    let simple = [opcodes::EXTCODESIZE, opcodes::EXTCODEHASH, opcodes::BALANCE];
    let rounds = b.rng().random_range(3..7);
    for _ in 0..rounds {
        let target = targets[b.rng().random_range(0..targets.len())];
        let op = simple[b.rng().random_range(0..simple.len())];
        push_code_target(b, target);
        b.op(op);
        b.op(opcodes::POP);
    }

    // EXTCODECOPY pops address, memory dest, code offset, length (address on top),
    // and writes to memory, so it is stack neutral without a trailing POP.
    let copies = b.rng().random_range(1..3);
    for _ in 0..copies {
        let target = targets[b.rng().random_range(0..targets.len())];
        let length = b.rng().random_range(0..48) as u64;
        let code_offset = b.rng().random_range(0..64) as u64;
        b.push_u64(length); // length (deepest)
        b.push_u64(code_offset); // code offset
        b.push_u64(0); // memory destination
        push_code_target(b, target); // address on top
        b.op(opcodes::EXTCODECOPY);
    }

    b.end_block();
}

/// Value-bearing CALLs to a fresh address and a precompile, plus a SELFBALANCE
/// read. Exercises the value-transfer edges (funding an empty account, sending
/// to a precompile).
pub fn value_transfer_edge(b: &mut ProgramBuilder) {
    b.begin_block("value_transfer_edge");
    b.guarantee("sends small values via CALL to a fresh address and a precompile");
    b.guarantee("stack neutral; reads SELFBALANCE; system targets only when allowed");

    // A fresh, almost-certainly-empty address: forcing a non-zero high byte keeps
    // it out of the system/hook ranges, so the value transfer is legal in any
    // mode.
    let mut fresh = [0u8; 20];
    fill_random(b, &mut fresh);
    fresh[0] = 0xff;
    let args_len = b.rng().random_range(0..17) as u64;
    let mut args = vec![0u8; args_len as usize];
    fill_random(b, &mut args);
    b.store_bytes_in_memory(0, &args);
    let fresh_value = b.rng().random_range(1..1000);
    b.emit_call(CallKind::Call, &fresh, 0, args_len, fresh_value);

    // A precompile, with a small value attached. Precompile addresses are ordinary
    // EVM addresses, not system/hook ones, so this too is legal in any mode.
    let table = opcodes::precompiles(b.fork());
    let picked = table[b.rng().random_range(0..table.len())];
    let precompile = address_from_low(picked.address_low as u64);
    let precompile_value = b.rng().random_range(1..1000);
    b.emit_call(CallKind::Call, &precompile, 0, 0, precompile_value);

    // Read our own balance and drop it.
    b.op(opcodes::SELFBALANCE);
    b.op(opcodes::POP);

    // Occasionally send a small value to a system address when the mode allows it.
    if b.mode().allows_system_addresses() && b.rng().random::<bool>() {
        let system = address_from_low(0x8003);
        let system_value = b.rng().random_range(1..1000);
        b.emit_call(CallKind::Call, &system, 0, 0, system_value);
    }

    b.end_block();
}

/// CREATE2 the same initcode and salt twice so the second deploy targets an
/// address that already holds code (a CREATE2 collision), optionally repeating
/// the pattern with a second child.
pub fn create2_collision(b: &mut ProgramBuilder) {
    b.begin_block("create2_collision");
    b.guarantee("CREATE2s identical initcode and salt twice, so the second address collides");
    b.guarantee("stack neutral; both resulting addresses are discarded");

    let child_runtime = generate_child_runtime(b);
    let child_initcode = wrap_runtime_as_initcode(&child_runtime);
    b.store_bytes_in_memory(0, &child_initcode);
    let len = child_initcode.len() as u64;
    let salt = b.rng().random::<u64>();

    // First deploy succeeds and places code at the salted address; the second
    // reuses the same salt and initcode, so it targets that same occupied
    // address. Whatever each CREATE2 returns is discarded by emit_create2.
    emit_create2(b, 0, len, salt, 0);
    emit_create2(b, 0, len, salt, 0);

    // Optionally exercise a collision on a second, distinct child too.
    if b.rng().random::<bool>() {
        let other_runtime = generate_child_runtime(b);
        let other_initcode = wrap_runtime_as_initcode(&other_runtime);
        b.store_bytes_in_memory(SECOND_INITCODE_OFFSET, &other_initcode);
        let other_len = other_initcode.len() as u64;
        let other_salt = b.rng().random::<u64>();
        emit_create2(b, SECOND_INITCODE_OFFSET, other_len, other_salt, 0);
        emit_create2(b, SECOND_INITCODE_OFFSET, other_len, other_salt, 0);
    }

    b.end_block();
}

/// A tiny valid runtime whose body writes a few storage slots. Used as the
/// callee of [`delegatecall_proxy`]: under DELEGATECALL these SSTOREs execute
/// against the caller's storage. Generated from the parent's rng so the whole
/// program stays deterministic, and kept short so the embedding parent stays
/// small.
fn generate_storage_writer_runtime(b: &mut ProgramBuilder) -> Vec<u8> {
    let seed = b.rng().random::<u64>();
    let mut child = ProgramBuilder::new(b.fork(), b.mode(), ChaCha8Rng::seed_from_u64(seed));
    child.begin_block("storage_writer_body");
    let writes = child.rng().random_range(1..=3);
    for _ in 0..writes {
        let value = child.rng().random::<u32>() as u64;
        let slot = child.rng().random_range(0..8u64);
        child.push_u64(value); // value (deeper)
        child.push_u64(slot); // key on top
        child.op(opcodes::SSTORE);
    }
    child.end_block();
    let (runtime, _, _) = child.finish();
    runtime
}

/// The chain's system and hook addresses, mirrored from `modes.rs` (which owns
/// the canonical predicate but does not export the list). A unit test asserts
/// every entry here is still classified as system/hook, so the two stay in sync.
fn system_and_hook_addresses() -> Vec<[u8; 20]> {
    let mut addresses = Vec::new();
    for low in [0x7001u64, 0x7002, 0x7004, 0x7100, 0x8003] {
        addresses.push(address_from_low(low));
    }
    for low in 0x8006u64..=0x800f {
        addresses.push(address_from_low(low));
    }
    for low in 0x0001_0001u64..=0x0001_0015 {
        addresses.push(address_from_low(low));
    }
    addresses.push(CREATE2_FACTORY);
    addresses
}

/// Push a code-query target: `None` is self, emitted as the ADDRESS opcode so it
/// resolves at runtime; `Some(addr)` is a literal address.
fn push_code_target(b: &mut ProgramBuilder, target: Option<[u8; 20]>) {
    match target {
        None => b.op(opcodes::ADDRESS),
        Some(addr) => b.push_address(&addr),
    }
}

// --- terminal blocks -------------------------------------------------------

/// RETURN a payload of random size.
pub fn return_data(b: &mut ProgramBuilder) {
    b.begin_block("return_data");
    b.guarantee("returns a payload of a random size");

    let len = b.rng().random_range(0..97) as u64;
    let mut payload = vec![0u8; len as usize];
    fill_random(b, &mut payload);
    b.store_bytes_in_memory(0, &payload);
    b.push_u64(len); // size
    b.push_u64(0); // offset on top
    b.op(opcodes::RETURN);

    b.end_terminal_block();
}

/// REVERT with a payload of random size.
pub fn revert_data(b: &mut ProgramBuilder) {
    b.begin_block("revert_data");
    b.guarantee("reverts with a payload of a random size");

    let len = b.rng().random_range(0..97) as u64;
    let mut payload = vec![0u8; len as usize];
    fill_random(b, &mut payload);
    b.store_bytes_in_memory(0, &payload);
    b.push_u64(len); // size
    b.push_u64(0); // offset on top
    b.op(opcodes::REVERT);

    b.end_terminal_block();
}

/// Execute the INVALID opcode, which consumes all gas and reverts.
pub fn invalid_op(b: &mut ProgramBuilder) {
    b.begin_block("invalid");
    b.guarantee("executes INVALID, which consumes all remaining gas");
    b.op(opcodes::INVALID);
    b.end_terminal_block();
}

/// SELFDESTRUCT to a beneficiary address.
pub fn selfdestruct_op(b: &mut ProgramBuilder) {
    b.begin_block("selfdestruct");
    b.guarantee("selfdestructs to a beneficiary; hook-free mode keeps it ordinary");
    let beneficiary = pick_call_target(b);
    b.push_address(&beneficiary);
    b.op(opcodes::SELFDESTRUCT);
    b.end_terminal_block();
}

// --- selection -------------------------------------------------------------

/// Append one randomly chosen body block that is legal in the builder's mode.
pub fn append_body_block(b: &mut ProgramBuilder) {
    // The eligible set depends only on the mode: the gas-reading block is
    // dropped where gas reads are forbidden.
    let mut choices: Vec<fn(&mut ProgramBuilder)> = vec![
        arithmetic_soup,
        memory_ops,
        storage_ops,
        environment_reads,
        log_ops,
        precompile_calls,
        external_calls,
        create_ops,
        counter_loop,
        deep_stack,
        // These are legal in every mode: their call targets are runtime-computed
        // (self/caller), freshly created, precompile or forced non-system
        // addresses, never a literal system address.
        reentrancy_calls,
        delegatecall_proxy,
        code_account_edge,
        value_transfer_edge,
        create2_collision,
    ];
    if b.mode().allows_gas_reads() {
        choices.push(gas_branch);
    }
    if b.mode().allows_system_addresses() {
        // Deliberately targets the chain's system and hook addresses.
        choices.push(system_contract_calls);
    }
    if b.mode() == Mode::Delegation7702 {
        // In the 7702 delegation mode the runtime runs in the delegated EOA's
        // context, so weight the create-bearing blocks: deploying from a
        // delegated account is a distinctive edge (the new contract's address
        // derives from the EOA's own nonce).
        choices.push(create_ops);
        choices.push(create2_collision);
    }
    let pick = b.rng().random_range(0..choices.len());
    choices[pick](b);
}

/// Append one randomly chosen terminal block, which ends execution.
pub fn append_terminal_block(b: &mut ProgramBuilder) {
    // In the 7702 delegation mode the runtime halts in the delegated EOA's
    // context; bias toward SELFDESTRUCT there to exercise a delegated account
    // self-destructing and its EIP-6780 interaction with the designator.
    if b.mode() == Mode::Delegation7702 && b.rng().random_range(0..100) < 40 {
        selfdestruct_op(b);
        return;
    }
    let choices: [fn(&mut ProgramBuilder); 4] =
        [return_data, revert_data, invalid_op, selfdestruct_op];
    let pick = b.rng().random_range(0..choices.len());
    choices[pick](b);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::{trace_stack, verify_jump_targets};
    use crate::opcodes::Fork;

    fn run_block(block: fn(&mut ProgramBuilder), fork: Fork, mode: Mode) -> Vec<u8> {
        let mut b = ProgramBuilder::new(fork, mode, ChaCha8Rng::seed_from_u64(42));
        block(&mut b);
        let (code, _, _) = b.finish();
        code
    }

    #[test]
    fn restricted_modes_never_pick_system_call_targets() {
        for mode in [Mode::GasAgnostic, Mode::HookFree, Mode::Delegation7702] {
            let mut builder = ProgramBuilder::new(Fork::Osaka, mode, ChaCha8Rng::seed_from_u64(42));
            for _ in 0..1000 {
                assert!(
                    !is_system_or_hook_address(&pick_call_target(&mut builder)),
                    "{mode:?}"
                );
            }
        }
    }

    #[test]
    fn every_body_block_is_valid_on_both_forks() {
        let body: [fn(&mut ProgramBuilder); 10] = [
            arithmetic_soup,
            memory_ops,
            storage_ops,
            environment_reads,
            log_ops,
            precompile_calls,
            external_calls,
            create_ops,
            counter_loop,
            deep_stack,
        ];
        for fork in [Fork::Osaka, Fork::CancunWith7702] {
            for block in body {
                let code = run_block(block, fork, Mode::Unrestricted);
                assert!(
                    trace_stack(&code, fork).is_ok(),
                    "stack invalid for a body block"
                );
                assert!(
                    verify_jump_targets(&code).is_ok(),
                    "jump invalid for a body block"
                );
            }
        }
    }

    #[test]
    fn terminal_blocks_are_valid() {
        let terminal: [fn(&mut ProgramBuilder); 4] =
            [return_data, revert_data, invalid_op, selfdestruct_op];
        for block in terminal {
            let code = run_block(block, Fork::Osaka, Mode::Unrestricted);
            assert!(trace_stack(&code, Fork::Osaka).is_ok());
            assert!(verify_jump_targets(&code).is_ok());
        }
    }

    /// The blocks added for the lab lane that are legal in every mode. Each is
    /// checked to leave the stack exactly as it found it (final depth zero) and
    /// to keep every jump aimed at a real JUMPDEST, on both forks and in all
    /// three modes.
    #[test]
    fn new_body_blocks_are_stack_neutral_and_jump_valid() {
        let blocks: [fn(&mut ProgramBuilder); 5] = [
            reentrancy_calls,
            delegatecall_proxy,
            code_account_edge,
            value_transfer_edge,
            create2_collision,
        ];
        for fork in [Fork::Osaka, Fork::CancunWith7702] {
            for mode in [Mode::GasAgnostic, Mode::HookFree, Mode::Unrestricted] {
                for block in blocks {
                    let code = run_block(block, fork, mode);
                    let report = trace_stack(&code, fork).unwrap_or_else(|e| {
                        panic!("stack invalid for a new body block ({fork:?}, {mode:?}): {e:?}")
                    });
                    assert_eq!(
                        report.final_depth, 0,
                        "a new body block was not stack neutral ({fork:?}, {mode:?})"
                    );
                    assert!(
                        verify_jump_targets(&code).is_ok(),
                        "jump invalid for a new body block ({fork:?}, {mode:?})"
                    );
                }
            }
        }
    }

    /// `system_contract_calls` is only ever run in unrestricted mode; check it is
    /// stack neutral and jump valid there on both forks.
    #[test]
    fn system_contract_calls_is_valid_in_unrestricted_mode() {
        for fork in [Fork::Osaka, Fork::CancunWith7702] {
            let code = run_block(system_contract_calls, fork, Mode::Unrestricted);
            let report = trace_stack(&code, fork).unwrap_or_else(|e| {
                panic!("stack invalid for system_contract_calls ({fork:?}): {e:?}")
            });
            assert_eq!(
                report.final_depth, 0,
                "system_contract_calls was not stack neutral ({fork:?})"
            );
            assert!(
                verify_jump_targets(&code).is_ok(),
                "jump invalid for system_contract_calls ({fork:?})"
            );
        }
    }

    /// The addresses `system_contract_calls` targets must all be ones a hook-free
    /// program would avoid; this pins the local list to the predicate in
    /// `modes.rs` so the two cannot drift apart silently.
    #[test]
    fn enumerated_system_addresses_are_all_system_or_hook() {
        let addresses = system_and_hook_addresses();
        assert!(!addresses.is_empty());
        for addr in addresses {
            assert!(
                is_system_or_hook_address(&addr),
                "address is not classified as system/hook: {addr:02x?}"
            );
        }
    }
}
