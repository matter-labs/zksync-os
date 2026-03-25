use std::io::Write;
use std::marker::PhantomData;
use std::path::Path;

use evm_interpreter::{opcodes, opcodes::OPCODE_JUMPMAP, ERGS_PER_GAS};
use zk_ee::{
    execution_environment_type::ExecutionEnvironmentType,
    system::{
        evm::{EvmError, EvmFrameInterface},
        tracer::{evm_tracer::EvmTracer, Tracer},
        CallResult, Computational, EthereumLikeTypes, ExecutionEnvironmentLaunchParams, Resources,
        SystemTypes,
    },
    types_config::SystemIOTypesConfig,
};

#[derive(Clone, Default)]
pub struct OpcodeStats {
    pub count: u64,
    pub total_gas: u64,
    pub total_native: u64,
    /// Per-execution gas values for min/max/median computation.
    /// Empty for CALL-like opcodes where gas delta is unreliable.
    pub gas_samples: Vec<u64>,
    /// Per-execution native values for min/max/median computation.
    pub native_samples: Vec<u64>,
}

impl OpcodeStats {
    fn median(samples: &[u64]) -> u64 {
        if samples.is_empty() {
            return 0;
        }
        let mut sorted = samples.to_vec();
        sorted.sort_unstable();
        let mid = sorted.len() / 2;
        if sorted.len().is_multiple_of(2) {
            ((sorted[mid - 1] as u128 + sorted[mid] as u128) / 2) as u64
        } else {
            sorted[mid]
        }
    }

    pub fn gas_median(&self) -> u64 {
        Self::median(&self.gas_samples)
    }

    pub fn native_median(&self) -> u64 {
        Self::median(&self.native_samples)
    }

    pub fn gas_min(&self) -> u64 {
        self.gas_samples.iter().copied().min().unwrap_or(0)
    }

    pub fn gas_max(&self) -> u64 {
        self.gas_samples.iter().copied().max().unwrap_or(0)
    }

    pub fn native_min(&self) -> u64 {
        self.native_samples.iter().copied().min().unwrap_or(0)
    }

    pub fn native_max(&self) -> u64 {
        self.native_samples.iter().copied().max().unwrap_or(0)
    }
}

impl OpcodeStats {
    /// Dump per-execution samples to a file: one line per execution with "gas,native".
    /// Samples are in execution order — the Kth line is the Kth execution.
    pub fn dump_samples(&self, writer: &mut impl Write) -> std::io::Result<()> {
        for (g, n) in self.gas_samples.iter().zip(self.native_samples.iter()) {
            writeln!(writer, "{},{}", g, n)?;
        }
        Ok(())
    }
}

fn is_call_like(opcode: u8) -> bool {
    matches!(
        opcode,
        opcodes::CALL
            | opcodes::STATICCALL
            | opcodes::DELEGATECALL
            | opcodes::CALLCODE
            | opcodes::CREATE
            | opcodes::CREATE2
    )
}

pub struct EvmOpcodeStatsTracer<S: SystemTypes> {
    pub stats: Vec<OpcodeStats>,
    gas_before: u64,
    native_before: u64,
    _marker: PhantomData<S>,
}

impl<S: SystemTypes> Default for EvmOpcodeStatsTracer<S> {
    fn default() -> Self {
        let mut stats = Vec::with_capacity(256);
        stats.resize_with(256, OpcodeStats::default);
        Self {
            stats,
            gas_before: 0,
            native_before: 0,
            _marker: PhantomData,
        }
    }
}

impl<S: SystemTypes> EvmOpcodeStatsTracer<S> {
    pub fn print_stats(&self) {
        println!("=== EVM Opcode Stats:");
        println!(
            "{:<16} {:>10} {:>10} {:>10} {:>10} {:>10} {:>10} {:>10} {:>10} {:>10}",
            "opcode",
            "count",
            "avg_gas",
            "med_gas",
            "min_gas",
            "max_gas",
            "avg_native",
            "med_native",
            "min_native",
            "max_native",
        );
        for (i, stat) in self.stats.iter().enumerate() {
            if stat.count == 0 {
                continue;
            }
            let name = OPCODE_JUMPMAP[i].unwrap_or("UNKNOWN");
            if is_call_like(i as u8) {
                println!(
                    "{:<16} {:>10} {:>10} {:>10} {:>10} {:>10} {:>10} {:>10} {:>10} {:>10}",
                    name, stat.count, "-", "-", "-", "-", "-", "-", "-", "-",
                );
                continue;
            }
            let avg_gas = stat.total_gas as f64 / stat.count as f64;
            let avg_native = stat.total_native as f64 / stat.count as f64;
            let gas_med = stat.gas_median();
            let native_med = stat.native_median();
            println!(
                "{:<16} {:>10} {:>10.1} {:>10} {:>10} {:>10} {:>10.1} {:>10} {:>10} {:>10}",
                name,
                stat.count,
                avg_gas,
                gas_med,
                stat.gas_min(),
                stat.gas_max(),
                avg_native,
                native_med,
                stat.native_min(),
                stat.native_max(),
            );
        }
        println!("==================");
    }

    pub fn write_csv(&self, path: &Path) -> std::io::Result<()> {
        let mut f = std::fs::File::create(path)?;
        writeln!(
            f,
            "opcode,opcode_hex,count,\
             avg_gas,median_gas,min_gas,max_gas,\
             avg_native,median_native,min_native,max_native,\
             native_per_gas"
        )?;
        for (i, stat) in self.stats.iter().enumerate() {
            if stat.count == 0 || is_call_like(i as u8) {
                continue;
            }
            let name = OPCODE_JUMPMAP[i].unwrap_or("UNKNOWN");
            let avg_gas = stat.total_gas as f64 / stat.count as f64;
            let avg_native = stat.total_native as f64 / stat.count as f64;
            let native_per_gas = if stat.total_gas > 0 {
                stat.total_native as f64 / stat.total_gas as f64
            } else {
                0.0
            };
            let gas_med = stat.gas_median();
            let gas_min = stat.gas_min();
            let gas_max = stat.gas_max();
            let native_med = stat.native_median();
            let native_min = stat.native_min();
            let native_max = stat.native_max();
            writeln!(
                f,
                "{},{:#04x},{},{:.2},{},{},{},{:.2},{},{},{},{:.2}",
                name,
                i,
                stat.count,
                avg_gas,
                gas_med,
                gas_min,
                gas_max,
                avg_native,
                native_med,
                native_min,
                native_max,
                native_per_gas,
            )?;
        }
        Ok(())
    }

    /// Dump per-execution samples to a directory.
    /// Creates one file per opcode: `<dir>/<OPCODE>.samples` with "gas,native" per line.
    /// Files are in execution order so line K = Kth execution.
    pub fn dump_samples(&self, dir: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(dir)?;
        for (i, stat) in self.stats.iter().enumerate() {
            if stat.gas_samples.is_empty() {
                continue;
            }
            let name = OPCODE_JUMPMAP[i].unwrap_or("UNKNOWN");
            let path = dir.join(format!("{}.samples", name));
            let mut f = std::fs::File::create(path)?;
            stat.dump_samples(&mut f)?;
        }
        Ok(())
    }
}

impl<S: EthereumLikeTypes> EvmTracer<S> for EvmOpcodeStatsTracer<S> {
    fn before_evm_interpreter_execution_step(
        &mut self,
        _opcode: u8,
        frame_state: &impl EvmFrameInterface<S>,
    ) {
        self.gas_before = frame_state.resources().ergs().0 / ERGS_PER_GAS;
        self.native_before = frame_state.resources().native().as_u64();
    }

    fn after_evm_interpreter_execution_step(
        &mut self,
        opcode: u8,
        frame_state: &impl EvmFrameInterface<S>,
    ) {
        let stat = &mut self.stats[opcode as usize];
        stat.count += 1;

        // CALL-like and CREATE opcodes move all resources to the call request
        // via take_resources(), so the after-step resources are 0 and the delta
        // would be meaningless. Only record count for these opcodes.
        if is_call_like(opcode) {
            return;
        }

        let gas_after = frame_state.resources().ergs().0 / ERGS_PER_GAS;
        let native_after = frame_state.resources().native().as_u64();

        let gas_used = self.gas_before.saturating_sub(gas_after);
        let native_used = self.native_before.saturating_sub(native_after);

        stat.total_gas += gas_used;
        stat.total_native += native_used;
        stat.gas_samples.push(gas_used);
        stat.native_samples.push(native_used);
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

impl<S: EthereumLikeTypes> Tracer<S> for EvmOpcodeStatsTracer<S> {
    #[inline(always)]
    fn on_new_execution_frame(&mut self, _request: &ExecutionEnvironmentLaunchParams<S>) {}

    #[inline(always)]
    fn after_execution_frame_completed(
        &mut self,
        _result: Option<(&S::Resources, &CallResult<S>)>,
    ) {
    }

    #[inline(always)]
    fn begin_tx(&mut self, _calldata: &[u8]) {}

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
