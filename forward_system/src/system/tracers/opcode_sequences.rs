//! Counts statically adjacent EVM opcode sequences (pairs and triples) as they
//! execute, to find candidates for instruction fusing in bytecode preprocessing.
//!
//! A sequence is only counted when each opcode starts exactly where the previous
//! one ended in the bytecode (fall-through, including a not-taken `JUMPI`), so a
//! fused handler could replace it statically. Taken jumps, frame boundaries and
//! transaction boundaries reset the history.

use std::collections::HashMap;
use std::io::Write;
use std::marker::PhantomData;
use std::path::Path;

use evm_interpreter::opcodes::{self, OPCODE_JUMPMAP};
use zk_ee::{
    execution_environment_type::ExecutionEnvironmentType,
    system::{
        evm::{EvmError, EvmFrameInterface, EvmStackInterface},
        tracer::{evm_tracer::EvmTracer, Tracer},
        CallResult, EthereumLikeTypes, ExecutionEnvironmentLaunchParams, SystemTypes,
    },
    types_config::SystemIOTypesConfig,
};

#[derive(Clone, Copy, Default)]
struct FrameHistory {
    /// The last two opcodes, most recent last.
    prev: [u8; 2],
    /// How many entries of `prev` are valid (0..=2).
    len: u8,
    /// Where the next opcode must start for the sequence to be static.
    next_ip: usize,
}

pub struct EvmOpcodeSequenceTracer<S: SystemTypes> {
    pub total_steps: u64,
    /// Executions per opcode.
    pub unigrams: HashMap<u8, u64>,
    pub bigrams: HashMap<[u8; 2], u64>,
    pub trigrams: HashMap<[u8; 3], u64>,
    /// `KECCAK256` input lengths, bucketed by 32-byte words (`len.div_ceil(32)`),
    /// capped at 64 words.
    pub sha3_words: HashMap<u32, u64>,
    /// `MULMOD` executions whose modulus is `2^256 - 1` (the interpreter's fast path).
    pub mulmod_max_modulus: u64,
    /// `DIV`/`SDIV`/`MOD`/`SMOD` executions with a zero divisor (no division is done).
    pub div_by_zero: HashMap<u8, u64>,
    /// Per executing contract: steps and executions of a few costly opcodes
    /// (`SLOAD`, `SSTORE`, `SHA3`, `MSTORE`, `MULMOD`), keyed by the 20-byte address.
    pub by_address: HashMap<[u8; 20], [u64; 6]>,
    frames: Vec<FrameHistory>,
    _marker: PhantomData<S>,
}

impl<S: SystemTypes> Default for EvmOpcodeSequenceTracer<S> {
    fn default() -> Self {
        Self {
            total_steps: 0,
            unigrams: HashMap::new(),
            bigrams: HashMap::new(),
            trigrams: HashMap::new(),
            sha3_words: HashMap::new(),
            mulmod_max_modulus: 0,
            div_by_zero: HashMap::new(),
            by_address: HashMap::new(),
            frames: Vec::new(),
            _marker: PhantomData,
        }
    }
}

fn opcode_len(opcode: u8) -> usize {
    if (opcodes::PUSH1..=opcodes::PUSH32).contains(&opcode) {
        2 + (opcode - opcodes::PUSH1) as usize
    } else {
        1
    }
}

pub fn opcode_name(opcode: u8) -> String {
    OPCODE_JUMPMAP[opcode as usize]
        .map(str::to_string)
        .unwrap_or_else(|| format!("0x{opcode:02x}"))
}

fn sequence_name(seq: &[u8]) -> String {
    seq.iter()
        .map(|op| opcode_name(*op))
        .collect::<Vec<_>>()
        .join(" ")
}

impl<S: SystemTypes> EvmOpcodeSequenceTracer<S> {
    /// Adds the counts of `other` into `self`.
    pub fn merge(&mut self, other: &Self) {
        self.total_steps += other.total_steps;
        for (k, v) in other.unigrams.iter() {
            *self.unigrams.entry(*k).or_default() += v;
        }
        self.mulmod_max_modulus += other.mulmod_max_modulus;
        for (k, v) in other.div_by_zero.iter() {
            *self.div_by_zero.entry(*k).or_default() += v;
        }
        for (k, v) in other.by_address.iter() {
            let e = self.by_address.entry(*k).or_default();
            for (a, b) in e.iter_mut().zip(v.iter()) {
                *a += b;
            }
        }
        for (k, v) in other.sha3_words.iter() {
            *self.sha3_words.entry(*k).or_default() += v;
        }
        for (k, v) in other.bigrams.iter() {
            *self.bigrams.entry(*k).or_default() += v;
        }
        for (k, v) in other.trigrams.iter() {
            *self.trigrams.entry(*k).or_default() += v;
        }
    }

    fn sorted<const N: usize>(map: &HashMap<[u8; N], u64>) -> Vec<([u8; N], u64)> {
        let mut v: Vec<_> = map.iter().map(|(k, v)| (*k, *v)).collect();
        v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        v
    }

    pub fn top_bigrams(&self) -> Vec<([u8; 2], u64)> {
        Self::sorted(&self.bigrams)
    }

    pub fn top_trigrams(&self) -> Vec<([u8; 3], u64)> {
        Self::sorted(&self.trigrams)
    }

    pub fn print_top(&self, top: usize) {
        let total = self.total_steps.max(1) as f64;
        println!("=== EVM opcode sequences: {} steps", self.total_steps);
        println!("--- top {top} statically adjacent pairs (count, % of steps, cumulative %)");
        let mut cumulative = 0u64;
        for (seq, count) in self.top_bigrams().into_iter().take(top) {
            cumulative += count;
            println!(
                "{:<32} {:>12} {:>7.2}% {:>7.2}%",
                sequence_name(&seq),
                count,
                100.0 * count as f64 / total,
                100.0 * cumulative as f64 / total
            );
        }
        println!("--- top {top} statically adjacent triples (count, % of steps, cumulative %)");
        let mut cumulative = 0u64;
        for (seq, count) in self.top_trigrams().into_iter().take(top) {
            cumulative += count;
            println!(
                "{:<40} {:>12} {:>7.2}% {:>7.2}%",
                sequence_name(&seq),
                count,
                100.0 * count as f64 / total,
                100.0 * cumulative as f64 / total
            );
        }
    }

    /// Writes all sequences as `length,sequence,count` rows.
    pub fn write_csv(&self, path: &Path) -> std::io::Result<()> {
        let mut f = std::fs::File::create(path)?;
        writeln!(f, "length,sequence,count")?;
        let mut unigrams: Vec<_> = self.unigrams.iter().collect();
        unigrams.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
        for (op, count) in unigrams {
            writeln!(f, "1,{},{}", opcode_name(*op), count)?;
        }
        for (seq, count) in self.top_bigrams() {
            writeln!(f, "2,{},{}", sequence_name(&seq), count)?;
        }
        for (seq, count) in self.top_trigrams() {
            writeln!(f, "3,{},{}", sequence_name(&seq), count)?;
        }
        let mut sha3: Vec<_> = self.sha3_words.iter().collect();
        sha3.sort();
        for (words, count) in sha3 {
            writeln!(f, "sha3_words,{},{}", words, count)?;
        }
        writeln!(f, "mulmod_max_modulus,,{}", self.mulmod_max_modulus)?;
        let mut by_zero: Vec<_> = self.div_by_zero.iter().collect();
        by_zero.sort();
        for (op, count) in by_zero {
            writeln!(f, "div_by_zero,{},{}", opcode_name(*op), count)?;
        }
        let mut by_address: Vec<_> = self.by_address.iter().collect();
        by_address.sort_by(|a, b| b.1[0].cmp(&a.1[0]));
        for (address, counts) in by_address {
            writeln!(
                f,
                "by_address,0x{},{}",
                address
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<String>(),
                counts
                    .iter()
                    .map(u64::to_string)
                    .collect::<Vec<_>>()
                    .join(";")
            )?;
        }
        Ok(())
    }

    fn current_frame(&mut self) -> &mut FrameHistory {
        if self.frames.is_empty() {
            self.frames.push(FrameHistory::default());
        }
        self.frames.last_mut().expect("non-empty")
    }
}

impl<S: EthereumLikeTypes> EvmTracer<S> for EvmOpcodeSequenceTracer<S> {
    fn before_evm_interpreter_execution_step(
        &mut self,
        opcode: u8,
        frame_state: &impl EvmFrameInterface<S>,
    ) {
        // The hook runs before the interpreter advances past the opcode, so the
        // instruction pointer is the opcode's own offset.
        let ip = frame_state.instruction_pointer();
        self.total_steps += 1;
        *self.unigrams.entry(opcode).or_default() += 1;
        {
            let address: [u8; 20] = frame_state.address().to_be_bytes();
            let counts = self.by_address.entry(address).or_default();
            counts[0] += 1;
            let slot = match opcode {
                opcodes::SLOAD => 1,
                opcodes::SSTORE => 2,
                opcodes::SHA3 => 3,
                opcodes::MSTORE => 4,
                opcodes::MULMOD => 5,
                _ => 0,
            };
            if slot != 0 {
                counts[slot] += 1;
            }
        }
        if matches!(
            opcode,
            opcodes::DIV | opcodes::SDIV | opcodes::MOD | opcodes::SMOD
        ) {
            // stack: dividend, divisor from the top
            if let Ok(divisor) = frame_state.stack().peek_n(1) {
                if divisor.is_zero() {
                    *self.div_by_zero.entry(opcode).or_default() += 1;
                }
            }
        }
        if opcode == opcodes::MULMOD {
            // stack: a, b, N from the top
            if let Ok(n) = frame_state.stack().peek_n(2) {
                if n.is_max() {
                    self.mulmod_max_modulus += 1;
                }
            }
        }
        if opcode == opcodes::SHA3 {
            // stack top is the offset, below it the length
            if let Ok(len) = frame_state.stack().peek_n(1) {
                let words = len
                    .try_to_u32()
                    .map_or(u32::MAX, |l| l.div_ceil(32))
                    .min(64);
                *self.sha3_words.entry(words).or_default() += 1;
            }
        }
        let frame = *self.current_frame();
        let mut history = frame;
        if history.len > 0 && ip != history.next_ip {
            // Control flow did not fall through: the sequence is not static.
            history.len = 0;
        }
        if history.len >= 1 {
            *self.bigrams.entry([history.prev[1], opcode]).or_default() += 1;
        }
        if history.len >= 2 {
            *self
                .trigrams
                .entry([history.prev[0], history.prev[1], opcode])
                .or_default() += 1;
        }
        history.prev[0] = history.prev[1];
        history.prev[1] = opcode;
        history.len = (history.len + 1).min(2);
        history.next_ip = ip + opcode_len(opcode);
        *self.current_frame() = history;
    }

    #[inline(always)]
    fn after_evm_interpreter_execution_step(
        &mut self,
        _opcode: u8,
        _frame_state: &impl EvmFrameInterface<S>,
    ) {
    }

    #[inline(always)]
    fn on_opcode_error(&mut self, _error: &EvmError, _frame_state: &impl EvmFrameInterface<S>) {}

    #[inline(always)]
    fn on_call_error(&mut self, _error: &EvmError) {}

    #[inline(always)]
    fn on_selfdestruct(
        &mut self,
        _beneficiary: <<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::Address,
        _token_value: <<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::NominalTokenValue,
        _frame_state: &impl EvmFrameInterface<S>,
    ) {
    }

    #[inline(always)]
    fn on_create_request(&mut self, _is_create2: bool) {}
}

impl<S: EthereumLikeTypes> Tracer<S> for EvmOpcodeSequenceTracer<S> {
    fn on_new_execution_frame(&mut self, _request: &ExecutionEnvironmentLaunchParams<S>) {
        self.frames.push(FrameHistory::default());
    }

    fn after_execution_frame_completed(
        &mut self,
        _result: Option<(&S::Resources, &CallResult<S>)>,
    ) {
        self.frames.pop();
    }

    fn begin_tx(&mut self, _calldata: &[u8]) {
        self.frames.clear();
    }

    #[inline(always)]
    fn finish_tx(&mut self) {}

    #[inline(always)]
    fn on_storage_read(
        &mut self,
        _ee_type: ExecutionEnvironmentType,
        _is_transient: bool,
        _address: <<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::Address,
        _key: <<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::StorageKey,
        _value: <<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::StorageValue,
    ) {
    }

    #[inline(always)]
    fn on_storage_write(
        &mut self,
        _ee_type: ExecutionEnvironmentType,
        _is_transient: bool,
        _address: <<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::Address,
        _key: <<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::StorageKey,
        _value: <<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::StorageValue,
    ) {
    }

    #[inline(always)]
    fn on_bytecode_change(
        &mut self,
        _ee_type: ExecutionEnvironmentType,
        _address: <S::IOTypes as SystemIOTypesConfig>::Address,
        _new_bytecode: Option<&[u8]>,
        _new_bytecode_hash: <S::IOTypes as SystemIOTypesConfig>::BytecodeHashValue,
        _new_observable_bytecode_length: u32,
    ) {
    }

    #[inline(always)]
    fn on_event(
        &mut self,
        _ee_type: ExecutionEnvironmentType,
        _address: &<<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::Address,
        _topics: &[<<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::EventKey],
        _data: &[u8],
    ) {
    }

    #[inline(always)]
    fn evm_tracer(&mut self) -> &mut impl EvmTracer<S> {
        self
    }
}
