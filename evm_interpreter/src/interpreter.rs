use super::*;
use core::fmt::Write;
use core::ops::Range;
use errors::EvmSubsystemError;
use native_resource_constants::STEP_NATIVE_COST;
use ruint::aliases::B160;
use zk_ee::common_structs::system_hooks::HooksStorage;
use zk_ee::memory::ArrayBuilder;
use zk_ee::system::tracer::evm_tracer::EvmTracer;
use zk_ee::system::tracer::Tracer;
use zk_ee::system::{
    logger::Logger, CallModifier, CompletedExecution, EthereumLikeTypes,
    ExecutionEnvironmentPreemptionPoint, ExternalCallRequest, ReturnValues,
};
use zk_ee::system::{CallResult, IOSubsystemExt, SystemFunctions};
use zk_ee::system_log;
use zk_ee::types_config::SystemIOTypesConfig;
use zk_ee::utils::cheap_clone::CheapCloneRiscV;

impl<'ee, S: EthereumLikeTypes> Interpreter<'ee, S> {
    /// Keeps executing instructions (steps) from the system, until it hits a yield point -
    /// either due to some error, or return, or when trying to call a different contract
    /// or create one.
    pub fn execute_till_yield_point<'a>(
        &'a mut self,
        system: &mut System<S>,
        hooks: &mut HooksStorage<S, S::Allocator>,
        tracer: &mut impl Tracer<S>,
    ) -> Result<ExecutionEnvironmentPreemptionPoint<'a, S>, EvmSubsystemError>
    where
        S::IO: IOSubsystemExt,
    {
        let mut external_call = None;
        let exit_code = self.run(system, hooks, &mut external_call, tracer)?;

        match exit_code {
            ExitCode::FatalError(e) => return Err(e),
            ExitCode::FatalRuntime(f) => return Err(RuntimeError::FatalRuntimeError(f).into()),
            _ => {}
        }

        if let Some(call) = external_call {
            assert!(exit_code == ExitCode::ExternalCall);
            let (current_heap, next_heap) = self.heap.freeze();

            let external_call_request = {
                let EVMCallRequest {
                    ergs_to_pass,
                    call_value,
                    destination_address,
                    input_data,
                    modifier,
                    full_caller_resources,
                } = call;
                ExternalCallRequest {
                    available_resources: full_caller_resources,
                    ergs_to_pass,
                    caller: self.address,
                    callee: destination_address,
                    callers_caller: self.caller,
                    modifier,
                    input: &current_heap[input_data],
                    nominal_token_value: call_value,
                    call_scratch_space: None,
                }
            };

            return Ok(ExecutionEnvironmentPreemptionPoint::CallRequest {
                heap: next_heap,
                request: external_call_request,
            });
        }

        self.create_immediate_return_state(system, exit_code, tracer)
    }
}

/// Returned by a step of the loop form instead of the next opcode when the run is over
const HALT: u32 = 0x100;

/// Everything an instruction may need besides the interpreter itself
pub(crate) struct Env<'a, S: EthereumLikeTypes, T: Tracer<S>> {
    system: &'a mut System<S>,
    hooks: &'a mut HooksStorage<S, S::Allocator>,
    external_call_dest: &'a mut Option<EVMCallRequest<S>>,
    tracer: &'a mut T,
    cycles: usize,
}

/// The dispatch loop: one frame for the whole run, a jump table per instruction, and every
/// instruction charged together with its step
pub(crate) struct Dispatch<'ee, S: EthereumLikeTypes, T: Tracer<S>>(
    core::marker::PhantomData<(&'ee (), S, T)>,
);

macro_rules! handlers {
    ($($op:ident => |$this:ident, $env:ident| $body:expr,)*) => {
        /// One instruction. Returns the next opcode, fetched
        /// while the updated instruction pointer is still at hand, or `HALT` after storing the
        /// exit code: a word in a register instead of a `Result` in memory.
        #[inline(always)]
        fn step(this: &mut Interpreter<'ee, S>, env: &mut Env<'_, S, T>, opcode: u8) -> u32 {
            let result: InstructionResult = match opcode {
                $(
                    opcodes::$op => {
                        let $this = &mut *this;
                        let $env = &mut *env;
                        let _ = &$env;
                        $body
                    }
                )*
                _ => this
                    .gas
                    .spend_native(STEP_NATIVE_COST)
                    .and(Err(EvmError::InvalidOpcode(opcode).into())),
            };
            match result {
                Ok(()) => this.get_bytecode_unchecked(this.instruction_pointer) as u32,
                Err(exit_code) => {
                    this.exit_code = Some(exit_code);
                    HALT
                }
            }
        }
    };
}

impl<'ee, S: EthereumLikeTypes, T: Tracer<S>> Dispatch<'ee, S, T>
where
    S::IO: IOSubsystemExt,
{
    /// Run until an instruction stores an exit code
    pub(crate) fn run_loop(this: &mut Interpreter<'ee, S>, env: &mut Env<'_, S, T>) {
        let mut opcode = this.get_bytecode_unchecked(this.instruction_pointer);
        loop {
            env.tracer
                .evm_tracer()
                .before_evm_interpreter_execution_step(
                    opcode,
                    &InterpreterExternal::new_from(this, env.system),
                );

            this.instruction_pointer += 1;
            #[cfg(not(target_arch = "riscv32"))]
            {
                env.cycles += 1;
            }
            cycle_marker::opcode_start!();

            let next = Self::step(this, env, opcode);

            Self::after_step(this, env, opcode);
            if next == HALT {
                return;
            }
            opcode = next as u8;
        }
    }

    #[inline(always)]
    fn after_step(this: &mut Interpreter<'ee, S>, env: &mut Env<'_, S, T>, opcode: u8) {
        cycle_marker::opcode_end!(
            crate::opcodes::OPCODE_JUMPMAP[opcode as usize].unwrap_or("UNKNOWN")
        );

        env.tracer
            .evm_tracer()
            .after_evm_interpreter_execution_step(
                opcode,
                &InterpreterExternal::new_from(this, env.system),
            );

        if Interpreter::<'ee, S>::PRINT_OPCODES {
            let _ = env.system.get_logger().write_str("\n");
        }
    }

    handlers! {
        CREATE => |this, env| this.create::<false>(&mut *env.system, &mut *env.external_call_dest, &mut *env.tracer),
        CREATE2 => |this, env| this.create::<true>(&mut *env.system, &mut *env.external_call_dest, &mut *env.tracer),
        CALL => |this, env| this.call(&mut *env.external_call_dest),
        CALLCODE => |this, env| this.call_code(&mut *env.external_call_dest),
        DELEGATECALL => |this, env| this.delegate_call(&mut *env.external_call_dest),
        STATICCALL => |this, env| this.static_call(&mut *env.external_call_dest),
        STOP => |this, env| this.gas.spend_native(STEP_NATIVE_COST).and(Err(ExitCode::Stop)),
        ADD => |this, env| this.wrapped_add(),
        MUL => |this, env| this.wrapping_mul(),
        SUB => |this, env| this.wrapping_sub(),
        DIV => |this, env| this.div(&mut *env.system),
        SDIV => |this, env| this.sdiv(&mut *env.system),
        MOD => |this, env| this.rem(&mut *env.system),
        SMOD => |this, env| this.smod(&mut *env.system),
        ADDMOD => |this, env| this.addmod(&mut *env.system),
        MULMOD => |this, env| this.mulmod(&mut *env.system),
        EXP => |this, env| this.eval_exp(),
        SIGNEXTEND => |this, env| this.sign_extend(),
        LT => |this, env| this.lt(),
        GT => |this, env| this.gt(),
        SLT => |this, env| this.slt(),
        SGT => |this, env| this.sgt(),
        EQ => |this, env| this.eq(),
        ISZERO => |this, env| this.iszero(),
        AND => |this, env| this.bitand(),
        OR => |this, env| this.bitor(),
        XOR => |this, env| this.bitxor(),
        NOT => |this, env| this.not(),
        BYTE => |this, env| this.byte(),
        SHL => |this, env| this.shl(),
        SHR => |this, env| this.shr(),
        SAR => |this, env| this.sar(),
        CLZ => |this, env| this.clz(),
        SHA3 => |this, env| this.sha3(&mut *env.system),
        ADDRESS => |this, env| this.address(),
        BALANCE => |this, env| this.balance(&mut *env.system),
        SELFBALANCE => |this, env| this.selfbalance(&mut *env.system),
        CODESIZE => |this, env| this.codesize(),
        CODECOPY => |this, env| this.codecopy(&mut *env.system),
        CALLDATALOAD => |this, env| this.calldataload(&mut *env.system),
        CALLDATASIZE => |this, env| this.calldatasize(),
        CALLDATACOPY => |this, env| this.calldatacopy(&mut *env.system),
        POP => |this, env| this.pop(),
        MLOAD => |this, env| this.mload(&mut *env.system),
        MSTORE => |this, env| this.mstore(&mut *env.system),
        MSTORE8 => |this, env| this.mstore8(&mut *env.system),
        JUMP => |this, env| this.jump(),
        JUMPI => |this, env| this.jumpi(),
        PC => |this, env| this.pc(),
        MSIZE => |this, env| this.msize(),
        JUMPDEST => |this, env| this.jumpdest(),
        PUSH0 => |this, env| this.push0(),
        PUSH1 => |this, env| this.push1(),
        PUSH2 => |this, env| this.push2(),
        PUSH3 => |this, env| this.push_small::<3>(),
        PUSH4 => |this, env| this.push_small::<4>(),
        PUSH5 => |this, env| this.push_small::<5>(),
        PUSH6 => |this, env| this.push_small::<6>(),
        PUSH7 => |this, env| this.push_small::<7>(),
        PUSH8 => |this, env| this.push_small::<8>(),
        PUSH9 => |this, env| this.push::<9>(),
        PUSH10 => |this, env| this.push::<10>(),
        PUSH11 => |this, env| this.push::<11>(),
        PUSH12 => |this, env| this.push::<12>(),
        PUSH13 => |this, env| this.push::<13>(),
        PUSH14 => |this, env| this.push::<14>(),
        PUSH15 => |this, env| this.push::<15>(),
        PUSH16 => |this, env| this.push::<16>(),
        PUSH17 => |this, env| this.push::<17>(),
        PUSH18 => |this, env| this.push::<18>(),
        PUSH19 => |this, env| this.push::<19>(),
        PUSH20 => |this, env| this.push::<20>(),
        PUSH21 => |this, env| this.push::<21>(),
        PUSH22 => |this, env| this.push::<22>(),
        PUSH23 => |this, env| this.push::<23>(),
        PUSH24 => |this, env| this.push::<24>(),
        PUSH25 => |this, env| this.push::<25>(),
        PUSH26 => |this, env| this.push::<26>(),
        PUSH27 => |this, env| this.push::<27>(),
        PUSH28 => |this, env| this.push::<28>(),
        PUSH29 => |this, env| this.push::<29>(),
        PUSH30 => |this, env| this.push::<30>(),
        PUSH31 => |this, env| this.push::<31>(),
        PUSH32 => |this, env| this.push::<32>(),
        DUP1 => |this, env| this.dup::<1>(),
        DUP2 => |this, env| this.dup::<2>(),
        DUP3 => |this, env| this.dup::<3>(),
        DUP4 => |this, env| this.dup::<4>(),
        DUP5 => |this, env| this.dup::<5>(),
        DUP6 => |this, env| this.dup::<6>(),
        DUP7 => |this, env| this.dup::<7>(),
        DUP8 => |this, env| this.dup::<8>(),
        DUP9 => |this, env| this.dup::<9>(),
        DUP10 => |this, env| this.dup::<10>(),
        DUP11 => |this, env| this.dup::<11>(),
        DUP12 => |this, env| this.dup::<12>(),
        DUP13 => |this, env| this.dup::<13>(),
        DUP14 => |this, env| this.dup::<14>(),
        DUP15 => |this, env| this.dup::<15>(),
        DUP16 => |this, env| this.dup::<16>(),
        SWAP1 => |this, env| this.swap::<1>(),
        SWAP2 => |this, env| this.swap::<2>(),
        SWAP3 => |this, env| this.swap::<3>(),
        SWAP4 => |this, env| this.swap::<4>(),
        SWAP5 => |this, env| this.swap::<5>(),
        SWAP6 => |this, env| this.swap::<6>(),
        SWAP7 => |this, env| this.swap::<7>(),
        SWAP8 => |this, env| this.swap::<8>(),
        SWAP9 => |this, env| this.swap::<9>(),
        SWAP10 => |this, env| this.swap::<10>(),
        SWAP11 => |this, env| this.swap::<11>(),
        SWAP12 => |this, env| this.swap::<12>(),
        SWAP13 => |this, env| this.swap::<13>(),
        SWAP14 => |this, env| this.swap::<14>(),
        SWAP15 => |this, env| this.swap::<15>(),
        SWAP16 => |this, env| this.swap::<16>(),
        RETURN => |this, env| this.ret(),
        REVERT => |this, env| this.revert(),
        INVALID => |this, env| this.gas.spend_native(STEP_NATIVE_COST).and(Err(EvmError::InvalidOpcode(opcodes::INVALID).into())),
        BASEFEE => |this, env| this.basefee(&mut *env.system),
        ORIGIN => |this, env| this.origin(&mut *env.system),
        CALLER => |this, env| this.caller(),
        CALLVALUE => |this, env| this.callvalue(),
        GASPRICE => |this, env| this.gasprice(&mut *env.system),
        EXTCODESIZE => |this, env| this.extcodesize(&mut *env.system),
        EXTCODEHASH => |this, env| this.extcodehash(&mut *env.system),
        EXTCODECOPY => |this, env| this.extcodecopy(&mut *env.system),
        RETURNDATASIZE => |this, env| this.returndatasize(),
        RETURNDATACOPY => |this, env| this.returndatacopy(),
        BLOCKHASH => |this, env| this.blockhash(&mut *env.system),
        COINBASE => |this, env| this.coinbase(&mut *env.system),
        TIMESTAMP => |this, env| this.timestamp(&mut *env.system),
        NUMBER => |this, env| this.number(&mut *env.system),
        DIFFICULTY => |this, env| this.difficulty(&mut *env.system),
        GASLIMIT => |this, env| this.gaslimit(&mut *env.system),
        SLOAD => |this, env| this.sload(&mut *env.system, &mut *env.tracer),
        SSTORE => |this, env| this.sstore(&mut *env.system, &mut *env.tracer),
        TLOAD => |this, env| this.tload(&mut *env.system, &mut *env.tracer),
        TSTORE => |this, env| this.tstore(&mut *env.system, &mut *env.tracer),
        MCOPY => |this, env| this.mcopy(),
        GAS => |this, env| this.gas(),
        LOG0 => |this, env| this.log::<0>(&mut *env.system, &mut *env.hooks, &mut *env.tracer),
        LOG1 => |this, env| this.log::<1>(&mut *env.system, &mut *env.hooks, &mut *env.tracer),
        LOG2 => |this, env| this.log::<2>(&mut *env.system, &mut *env.hooks, &mut *env.tracer),
        LOG3 => |this, env| this.log::<3>(&mut *env.system, &mut *env.hooks, &mut *env.tracer),
        LOG4 => |this, env| this.log::<4>(&mut *env.system, &mut *env.hooks, &mut *env.tracer),
        SELFDESTRUCT => |this, env| this.selfdestruct(&mut *env.system, &mut *env.tracer),
        CHAINID => |this, env| this.chainid(&mut *env.system),
        BLOBHASH => |this, env| this.blobhash(&mut *env.system),
        BLOBBASEFEE => |this, env| this.blobbasefee(&mut *env.system),
    }
}

pub struct EVMCallRequest<S: EthereumLikeTypes> {
    pub ergs_to_pass: <S::Resources as Resources>::Ergs,
    pub call_value: <S::IOTypes as SystemIOTypesConfig>::NominalTokenValue,
    pub destination_address: <S::IOTypes as SystemIOTypesConfig>::Address,
    pub input_data: Range<usize>,
    pub modifier: CallModifier,
    pub full_caller_resources: S::Resources,
}

#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub enum CallScheme {
    /// `CALL`
    Call,
    /// `CALLCODE`
    CallCode,
    /// `DELEGATECALL`
    DelegateCall,
    /// `STATICCALL`
    StaticCall,
}

impl<'ee, S: EthereumLikeTypes> Interpreter<'ee, S> {
    pub(crate) const PRINT_OPCODES: bool = false;

    #[allow(dead_code)]
    pub(crate) fn stack_debug_print(&self, logger: &mut impl Logger) {
        self.stack.print_stack_content(logger);
    }

    #[inline]
    pub(crate) fn get_bytecode_unchecked(&self, offset: usize) -> u8 {
        self.bytecode
            .get(offset)
            .copied()
            .unwrap_or(crate::opcodes::STOP)
    }

    pub fn run(
        &mut self,
        system: &mut System<S>,
        hooks: &mut HooksStorage<S, S::Allocator>,
        external_call_dest: &mut Option<EVMCallRequest<S>>,
        tracer: &mut impl Tracer<S>,
    ) -> Result<ExitCode, EvmSubsystemError>
    where
        S::IO: IOSubsystemExt,
    {
        let mut env = Env {
            system,
            hooks,
            external_call_dest,
            tracer,
            cycles: 0,
        };
        self.exit_code = None;
        Dispatch::<S, _>::run_loop(self, &mut env);
        let result = self
            .exit_code
            .take()
            .ok_or(internal_error!("interpreter stopped without an exit code"))?;

        system_log!(
            env.system,
            "Instructions executed = {}\nFinal instruction result = {:?}\n",
            env.cycles,
            &result
        );

        Ok(result)
    }

    pub(crate) fn create_immediate_return_state<'a>(
        &'a mut self,
        system: &mut System<S>,
        exit_code: ExitCode,
        tracer: &mut impl Tracer<S>,
    ) -> Result<ExecutionEnvironmentPreemptionPoint<'a, S>, EvmSubsystemError>
    where
        S::IO: IOSubsystemExt,
    {
        let mut return_values = ReturnValues::empty();
        // Set returndata if exit code is Return or Revert
        match exit_code {
            ExitCode::Return | ExitCode::EvmError(EvmError::Revert) => {
                return_values.returndata = &self.heap[self.returndata_location.clone()];
            }
            ExitCode::Stop | ExitCode::SelfDestruct | ExitCode::EvmError(_) => (),
            ExitCode::ExternalCall | ExitCode::FatalError(_) | ExitCode::FatalRuntime(_) => {
                return Err(internal_error!("Invalid exit code passed").into())
            }
        };

        if let ExitCode::EvmError(evm_error) = exit_code {
            if evm_error != EvmError::Revert {
                // Spend all remaining resources on EVM error
                self.gas.consume_all_gas();
                // Clear returndata
                return_values.returndata = &[];
            }
            tracer
                .evm_tracer()
                .on_opcode_error(&evm_error, &InterpreterExternal::new_from(&self, system));
            return Ok(ExecutionEnvironmentPreemptionPoint::End(
                CompletedExecution {
                    resources_returned: self.gas.take_resources(),
                    result: CallResult::Failed { return_values },
                },
            ));
        };

        let result = if self.is_constructor {
            let deployed_code = return_values.returndata;
            let mut error_after_constructor = None;
            if deployed_code.len() > MAX_CODE_SIZE {
                // EIP-170: reject code of length > 24576.
                error_after_constructor = Some(EvmError::CreateContractSizeLimit)
            } else if !deployed_code.is_empty() && deployed_code[0] == 0xEF {
                // EIP-3541: reject code starting with 0xEF.
                error_after_constructor = Some(EvmError::CreateContractStartingWithEF);
            } else {
                match system.deploy_bytecode(
                    THIS_EE_TYPE,
                    self.gas.resources_mut(),
                    &self.address,
                    deployed_code,
                ) {
                    Ok((
                        actual_deployed_bytecode,
                        internal_bytecode_hash,
                        observable_bytecode_len,
                    )) => {
                        system_log!(
                            system,
                            "Successfully deployed contract at {:?} \n",
                            self.address
                        );

                        tracer.on_bytecode_change(
                            THIS_EE_TYPE,
                            self.address,
                            Some(actual_deployed_bytecode),
                            internal_bytecode_hash,
                            observable_bytecode_len,
                        );
                    }
                    Err(SystemError::LeafRuntime(RuntimeError::OutOfErgs(_))) => {
                        error_after_constructor = Some(EvmError::CodeStoreOutOfGas);
                    }
                    Err(SystemError::LeafRuntime(RuntimeError::FatalRuntimeError(e))) => {
                        return Err(RuntimeError::FatalRuntimeError(e).into())
                    }
                    Err(SystemError::LeafDefect(e)) => return Err(e.into()),
                }
            }

            if let Some(error) = error_after_constructor {
                // Spend all remaining resources
                self.gas.consume_all_gas();

                tracer
                    .evm_tracer()
                    .on_opcode_error(&error, &InterpreterExternal::new_from(&self, system));

                CallResult::Failed {
                    return_values: ReturnValues::empty(),
                }
            } else {
                CallResult::Successful {
                    return_values: ReturnValues::empty(),
                }
            }
        } else {
            CallResult::Successful { return_values }
        };

        Ok(ExecutionEnvironmentPreemptionPoint::End(
            CompletedExecution {
                resources_returned: self.gas.take_resources(),
                result,
            },
        ))
    }

    pub(crate) fn copy_returndata_to_heap(
        &mut self,
        returndata_region: &'ee [u8],
    ) -> Result<(), ExitCode> {
        if !self.returndata_location.is_empty() {
            let to_copy = core::cmp::min(returndata_region.len(), self.returndata_location.len());
            if to_copy > 0 {
                let (_, native_cost) = gas::gas_utils::copy_cost(to_copy as u64)?;
                self.gas.spend_gas_and_native(0, native_cost)?;
                unsafe {
                    let src = returndata_region.as_ptr();
                    let dst = self.heap.as_mut_ptr().add(self.returndata_location.start);
                    core::ptr::copy_nonoverlapping(src, dst, to_copy);
                }
            }
        }

        self.returndata = returndata_region;
        Ok(())
    }

    pub fn derive_address_for_deployment_create(
        _resources: &mut <S as SystemTypes>::Resources,
        deployer_address: &<S::IOTypes as SystemIOTypesConfig>::Address,
        deployer_nonce: u64,
    ) -> Result<<S::IOTypes as SystemIOTypesConfig>::Address, EvmSubsystemError> {
        use crypto::sha3::Keccak256;
        use crypto::MiniDigest;
        let mut buffer = [0u8; crate::utils::MAX_CREATE_RLP_ENCODING_LEN];
        let encoding_it = crate::utils::create_quasi_rlp(deployer_address, deployer_nonce);
        let encoding_len = ExactSizeIterator::len(&encoding_it);
        for (dst, src) in buffer.iter_mut().zip(encoding_it) {
            *dst = src;
        }
        let new_address = Keccak256::digest(&buffer[..encoding_len]);
        let new_address =
            B160::try_from_be_slice(&new_address.as_slice()[12..]).expect("must create address");

        Ok(new_address)
    }

    pub fn derive_address_for_deployment_create2(
        system: &mut System<S>,
        resources: &mut <S as SystemTypes>::Resources,
        salt: &U256,
        deployer_address: &<S::IOTypes as SystemIOTypesConfig>::Address,
        deployment_code: &[u8],
    ) -> Result<<S::IOTypes as SystemIOTypesConfig>::Address, EvmSubsystemError> {
        use crypto::sha3::Keccak256;
        use crypto::MiniDigest;
        // we need to compute address based on the hash of the code and salt
        let mut initcode_hash = ArrayBuilder::default();
        resources
            .with_infinite_ergs(|inf_resources| {
                S::SystemFunctions::keccak256(
                    deployment_code,
                    &mut initcode_hash,
                    inf_resources,
                    system.get_allocator(),
                )
            })
            .map_err(|e| -> EvmSubsystemError {
                match e.root_cause() {
                    RootCause::Runtime(e @ RuntimeError::FatalRuntimeError(_)) => {
                        e.clone_or_copy().into()
                    }
                    _ => internal_error!("Keccak in create2 cannot fail").into(),
                }
            })?;
        let initcode_hash = Bytes32::from_array(initcode_hash.build());

        let mut create2_buffer = [0xffu8; 1 + 20 + 32 + 32];
        create2_buffer[1..(1 + 20)]
            .copy_from_slice(&deployer_address.to_be_bytes::<{ B160::BYTES }>());
        create2_buffer[(1 + 20)..(1 + 20 + 32)].copy_from_slice(&salt.to_be_bytes());
        create2_buffer[(1 + 20 + 32)..(1 + 20 + 32 + 32)]
            .copy_from_slice(initcode_hash.as_u8_array_ref());

        let new_address = Keccak256::digest(&create2_buffer);
        let new_address =
            B160::try_from_be_slice(&new_address.as_slice()[12..]).expect("must create address");

        Ok(new_address)
    }
}
