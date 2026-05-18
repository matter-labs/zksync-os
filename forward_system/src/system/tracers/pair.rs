//! Composite tracer that forwards every hook to two inner tracers.
//!
//! Use this to install multiple stats tracers on the same execution path
//! (e.g. EvmOpcodeStatsTracer + PrecompileStatsTracer in eth_runner).

use zk_ee::{
    execution_environment_type::ExecutionEnvironmentType,
    system::{
        evm::{EvmError, EvmFrameInterface},
        tracer::{evm_tracer::EvmTracer, Tracer},
        CallResult, EthereumLikeTypes, ExecutionEnvironmentLaunchParams, SystemTypes,
    },
    types_config::SystemIOTypesConfig,
};

pub struct Pair<A, B> {
    pub a: A,
    pub b: B,
}

impl<A, B> Pair<A, B> {
    pub fn new(a: A, b: B) -> Self {
        Self { a, b }
    }
}

impl<S, A, B> Tracer<S> for Pair<A, B>
where
    S: EthereumLikeTypes,
    A: Tracer<S>,
    B: Tracer<S>,
{
    #[inline(always)]
    fn evm_tracer(&mut self) -> &mut impl EvmTracer<S> {
        // Pair also implements EvmTracer<S> below; return self.
        self
    }

    fn on_new_execution_frame(&mut self, request: &ExecutionEnvironmentLaunchParams<S>) {
        self.a.on_new_execution_frame(request);
        self.b.on_new_execution_frame(request);
    }

    fn after_execution_frame_completed(&mut self, result: Option<(&S::Resources, &CallResult<S>)>) {
        self.a.after_execution_frame_completed(result);
        self.b.after_execution_frame_completed(result);
    }

    fn begin_tx(&mut self, calldata: &[u8]) {
        self.a.begin_tx(calldata);
        self.b.begin_tx(calldata);
    }

    fn finish_tx(&mut self) {
        self.a.finish_tx();
        self.b.finish_tx();
    }

    fn on_storage_read(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        is_transient: bool,
        address: <<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::Address,
        key: <<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::StorageKey,
        value: <<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::StorageValue,
    ) {
        self.a
            .on_storage_read(ee_type, is_transient, address, key, value);
        self.b
            .on_storage_read(ee_type, is_transient, address, key, value);
    }

    fn on_storage_write(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        is_transient: bool,
        address: <<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::Address,
        key: <<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::StorageKey,
        value: <<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::StorageValue,
    ) {
        self.a
            .on_storage_write(ee_type, is_transient, address, key, value);
        self.b
            .on_storage_write(ee_type, is_transient, address, key, value);
    }

    fn on_bytecode_change(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        address: <S::IOTypes as SystemIOTypesConfig>::Address,
        new_raw_bytecode: Option<&[u8]>,
        new_internal_bytecode_hash: <S::IOTypes as SystemIOTypesConfig>::BytecodeHashValue,
        new_observable_bytecode_length: u32,
    ) {
        self.a.on_bytecode_change(
            ee_type,
            address,
            new_raw_bytecode,
            new_internal_bytecode_hash,
            new_observable_bytecode_length,
        );
        self.b.on_bytecode_change(
            ee_type,
            address,
            new_raw_bytecode,
            new_internal_bytecode_hash,
            new_observable_bytecode_length,
        );
    }

    fn on_event(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        address: &<<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::Address,
        topics: &[<<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::EventKey],
        data: &[u8],
    ) {
        self.a.on_event(ee_type, address, topics, data);
        self.b.on_event(ee_type, address, topics, data);
    }
}

/// EVM-tracer events on a `Pair<A, B>` fan out via each child's `evm_tracer()`.
///
/// Implicit contract: each child's `Tracer::evm_tracer()` MUST return a
/// type implementing `EvmTracer<S>`. Both existing consumers
/// (`EvmOpcodeStatsTracer`, `PrecompileStatsTracer`) return `&mut self`,
/// satisfying this. A future tracer that delegates to a different inner
/// type would still work as long as that inner type is itself an
/// `EvmTracer<S>`. If you add a tracer whose `evm_tracer()` does
/// something non-trivial, make sure that's true.
impl<S, A, B> EvmTracer<S> for Pair<A, B>
where
    S: EthereumLikeTypes,
    A: Tracer<S>,
    B: Tracer<S>,
{
    #[inline(always)]
    fn before_evm_interpreter_execution_step(
        &mut self,
        opcode: u8,
        frame_state: &impl EvmFrameInterface<S>,
    ) {
        self.a
            .evm_tracer()
            .before_evm_interpreter_execution_step(opcode, frame_state);
        self.b
            .evm_tracer()
            .before_evm_interpreter_execution_step(opcode, frame_state);
    }

    #[inline(always)]
    fn after_evm_interpreter_execution_step(
        &mut self,
        opcode: u8,
        frame_state: &impl EvmFrameInterface<S>,
    ) {
        self.a
            .evm_tracer()
            .after_evm_interpreter_execution_step(opcode, frame_state);
        self.b
            .evm_tracer()
            .after_evm_interpreter_execution_step(opcode, frame_state);
    }

    #[inline(always)]
    fn on_opcode_error(&mut self, error: &EvmError, frame_state: &impl EvmFrameInterface<S>) {
        self.a.evm_tracer().on_opcode_error(error, frame_state);
        self.b.evm_tracer().on_opcode_error(error, frame_state);
    }

    #[inline(always)]
    fn on_call_error(&mut self, error: &EvmError) {
        self.a.evm_tracer().on_call_error(error);
        self.b.evm_tracer().on_call_error(error);
    }

    #[inline(always)]
    fn on_selfdestruct(
        &mut self,
        beneficiary: <<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::Address,
        token_value: <<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::NominalTokenValue,
        frame_state: &impl EvmFrameInterface<S>,
    ) {
        self.a
            .evm_tracer()
            .on_selfdestruct(beneficiary, token_value, frame_state);
        self.b
            .evm_tracer()
            .on_selfdestruct(beneficiary, token_value, frame_state);
    }

    #[inline(always)]
    fn on_create_request(&mut self, is_create2: bool) {
        self.a.evm_tracer().on_create_request(is_create2);
        self.b.evm_tracer().on_create_request(is_create2);
    }
}

// `Pair` is generic over `(A, B: Tracer<S>)`. A meaningful unit test would
// require a mock `Tracer<S>` implementation backed by a concrete `SystemTypes`,
// which is non-trivial to construct here. The actual integration test is via
// `eth_runner` block bench. Structural / compilation correctness is implicit
// in the `forward_system` build.
