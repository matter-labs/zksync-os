use std::io::Write;
use std::marker::PhantomData;
use std::path::Path;

use evm_interpreter::{opcodes::OPCODE_JUMPMAP, ERGS_PER_GAS};
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

#[derive(Clone, Copy, Default)]
pub struct OpcodeStats {
    pub count: u64,
    pub total_gas: u64,
    pub total_native: u64,
}

pub struct EvmOpcodeStatsTracer<S: SystemTypes> {
    pub stats: [OpcodeStats; 256],
    gas_before: u64,
    native_before: u64,
    _marker: PhantomData<S>,
}

impl<S: SystemTypes> Default for EvmOpcodeStatsTracer<S> {
    fn default() -> Self {
        Self {
            stats: [OpcodeStats::default(); 256],
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
            "{:<20} {:>12} {:>14} {:>14} {:>10} {:>10}",
            "opcode", "count", "total_gas", "total_native", "avg_gas", "avg_native"
        );
        for (i, stat) in self.stats.iter().enumerate() {
            if stat.count == 0 {
                continue;
            }
            let name = OPCODE_JUMPMAP[i].unwrap_or("UNKNOWN");
            let avg_gas = stat.total_gas as f64 / stat.count as f64;
            let avg_native = stat.total_native as f64 / stat.count as f64;
            println!(
                "{:<20} {:>12} {:>14} {:>14} {:>10.1} {:>10.1}",
                name, stat.count, stat.total_gas, stat.total_native, avg_gas, avg_native
            );
        }
        println!("==================");
    }

    pub fn write_csv(&self, path: &Path) -> std::io::Result<()> {
        let mut f = std::fs::File::create(path)?;
        writeln!(
            f,
            "opcode,opcode_hex,count,total_gas,total_native,avg_gas,avg_native,native_per_gas"
        )?;
        for (i, stat) in self.stats.iter().enumerate() {
            if stat.count == 0 {
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
            writeln!(
                f,
                "{},{:#04x},{},{},{},{:.2},{:.2},{:.2}",
                name,
                i,
                stat.count,
                stat.total_gas,
                stat.total_native,
                avg_gas,
                avg_native,
                native_per_gas,
            )?;
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
        let gas_after = frame_state.resources().ergs().0 / ERGS_PER_GAS;
        let native_after = frame_state.resources().native().as_u64();

        let gas_used = self.gas_before.saturating_sub(gas_after);
        let native_used = self.native_before.saturating_sub(native_after);

        let stat = &mut self.stats[opcode as usize];
        stat.count += 1;
        stat.total_gas += gas_used;
        stat.total_native += native_used;
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
