use super::*;
use core::fmt::Write;
use core::ops::Range;
use errors::EvmSubsystemError;
use native_resource_constants::STEP_NATIVE_COST;
use ruint::aliases::B160;
use zk_ee::memory::ArrayBuilder;
use zk_ee::system::tracer::evm_tracer::EvmTracer;
use zk_ee::system::tracer::Tracer;
use zk_ee::system::Ergs;
use zk_ee::system::{
    logger::Logger, CallModifier, CompletedExecution, EthereumLikeTypes,
    ExecutionEnvironmentPreemptionPoint, ExternalCallRequest, ReturnValues,
};
use zk_ee::system::{CallResult, IOSubsystemExt, SystemFunctions};
use zk_ee::types_config::SystemIOTypesConfig;
use zk_ee::utils::cheap_clone::CheapCloneRiscV;

impl<'ee, S: EthereumLikeTypes> Interpreter<'ee, S> {
    /// Keeps executing instructions (steps) from the system, until it hits a yield point -
    /// either due to some error, or return, or when trying to call a different contract
    /// or create one.
    pub fn execute_till_yield_point<'a>(
        &'a mut self,
        system: &mut System<S>,
        tracer: &mut impl Tracer<S>,
    ) -> Result<ExecutionEnvironmentPreemptionPoint<'a, S>, EvmSubsystemError>
    where
        S::IO: IOSubsystemExt,
    {
        let mut external_call = None;
        let exit_code = self.run(system, &mut external_call, tracer)?;

        if let ExitCode::FatalError(e) = exit_code {
            return Err(e);
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

pub struct EVMCallRequest<S: EthereumLikeTypes> {
    pub ergs_to_pass: Ergs,
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

/// Create scheme.
#[repr(usize)]
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub enum CreateScheme {
    /// Legacy create scheme of `CREATE`.
    Create = 1,
    /// Create scheme of `CREATE2`.
    Create2 {
        /// Salt.
        salt: U256,
    },
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
            .as_ref()
            .get(offset)
            .copied()
            .unwrap_or(crate::opcodes::STOP)
    }

    pub fn run(
        &mut self,
        system: &mut System<S>,
        external_call_dest: &mut Option<EVMCallRequest<S>>,
        tracer: &mut impl Tracer<S>,
    ) -> Result<ExitCode, EvmSubsystemError> {
        let mut cycles = 0;
        let result = loop {
            let opcode = self.get_bytecode_unchecked(self.instruction_pointer);

            match crate::opcodes::OpCode::try_from_u8(opcode) {
                Some(op) => {
                    if Self::PRINT_OPCODES {
                        let _ = system
                            .get_logger()
                            .write_fmt(format_args!("Executing {op}"));
                    }
                }
                None => {
                    let _ = system
                        .get_logger()
                        .write_fmt(format_args!("Unknown opcode = 0x{opcode:02x}\n"));
                }
            }

            tracer.evm_tracer().before_evm_interpreter_execution_step(
                opcode,
                &InterpreterExternal::new_from(&self, system),
            );

            self.instruction_pointer += 1;
            let result = self
                .gas
                .spend_gas_and_native(0, STEP_NATIVE_COST)
                .and_then(|_| match opcode {
                    opcodes::CREATE => self.create::<false>(system, external_call_dest),
                    opcodes::CREATE2 => self.create::<true>(system, external_call_dest),
                    opcodes::CALL => self.call(external_call_dest),
                    opcodes::CALLCODE => self.call_code(external_call_dest),
                    opcodes::DELEGATECALL => self.delegate_call(external_call_dest),
                    opcodes::STATICCALL => self.static_call(external_call_dest),
                    opcodes::STOP => Err(ExitCode::Stop),
                    opcodes::ADD => self.wrapped_add(),
                    opcodes::MUL => self.wrapping_mul(),
                    opcodes::SUB => self.wrapping_sub(),
                    opcodes::DIV => self.div(),
                    opcodes::SDIV => self.sdiv(),
                    opcodes::MOD => self.rem(),
                    opcodes::SMOD => self.smod(),
                    opcodes::ADDMOD => self.addmod(),
                    opcodes::MULMOD => self.mulmod(),
                    opcodes::EXP => self.eval_exp(),
                    opcodes::SIGNEXTEND => self.sign_extend(),
                    opcodes::LT => self.lt(),
                    opcodes::GT => self.gt(),
                    opcodes::SLT => self.slt(),
                    opcodes::SGT => self.sgt(),
                    opcodes::EQ => self.eq(),
                    opcodes::ISZERO => self.iszero(),
                    opcodes::AND => self.bitand(),
                    opcodes::OR => self.bitor(),
                    opcodes::XOR => self.bitxor(),
                    opcodes::NOT => self.not(),
                    opcodes::BYTE => self.byte(),
                    opcodes::SHL => self.shl(),
                    opcodes::SHR => self.shr(),
                    opcodes::SAR => self.sar(),
                    opcodes::SHA3 => self.sha3(system),
                    opcodes::ADDRESS => self.address(),
                    opcodes::BALANCE => self.balance(system),
                    opcodes::SELFBALANCE => self.selfbalance(system),
                    opcodes::CODESIZE => self.codesize(),
                    opcodes::CODECOPY => self.codecopy(system),
                    opcodes::CALLDATALOAD => self.calldataload(system),
                    opcodes::CALLDATASIZE => self.calldatasize(),
                    opcodes::CALLDATACOPY => self.calldatacopy(system),
                    opcodes::POP => self.pop(),
                    opcodes::MLOAD => self.mload(system),
                    opcodes::MSTORE => self.mstore(system),
                    opcodes::MSTORE8 => self.mstore8(system),
                    opcodes::JUMP => self.jump(),
                    opcodes::JUMPI => self.jumpi(),
                    opcodes::PC => self.pc(),
                    opcodes::MSIZE => self.msize(),
                    opcodes::JUMPDEST => self.jumpdest(),
                    opcodes::PUSH0 => self.push0(),
                    opcodes::PUSH1 => self.push::<1>(),
                    opcodes::PUSH2 => self.push::<2>(),
                    opcodes::PUSH3 => self.push::<3>(),
                    opcodes::PUSH4 => self.push::<4>(),
                    opcodes::PUSH5 => self.push::<5>(),
                    opcodes::PUSH6 => self.push::<6>(),
                    opcodes::PUSH7 => self.push::<7>(),
                    opcodes::PUSH8 => self.push::<8>(),
                    opcodes::PUSH9 => self.push::<9>(),
                    opcodes::PUSH10 => self.push::<10>(),
                    opcodes::PUSH11 => self.push::<11>(),
                    opcodes::PUSH12 => self.push::<12>(),
                    opcodes::PUSH13 => self.push::<13>(),
                    opcodes::PUSH14 => self.push::<14>(),
                    opcodes::PUSH15 => self.push::<15>(),
                    opcodes::PUSH16 => self.push::<16>(),
                    opcodes::PUSH17 => self.push::<17>(),
                    opcodes::PUSH18 => self.push::<18>(),
                    opcodes::PUSH19 => self.push::<19>(),
                    opcodes::PUSH20 => self.push::<20>(),
                    opcodes::PUSH21 => self.push::<21>(),
                    opcodes::PUSH22 => self.push::<22>(),
                    opcodes::PUSH23 => self.push::<23>(),
                    opcodes::PUSH24 => self.push::<24>(),
                    opcodes::PUSH25 => self.push::<25>(),
                    opcodes::PUSH26 => self.push::<26>(),
                    opcodes::PUSH27 => self.push::<27>(),
                    opcodes::PUSH28 => self.push::<28>(),
                    opcodes::PUSH29 => self.push::<29>(),
                    opcodes::PUSH30 => self.push::<30>(),
                    opcodes::PUSH31 => self.push::<31>(),
                    opcodes::PUSH32 => self.push::<32>(),
                    opcodes::DUP1 => self.dup::<1>(),
                    opcodes::DUP2 => self.dup::<2>(),
                    opcodes::DUP3 => self.dup::<3>(),
                    opcodes::DUP4 => self.dup::<4>(),
                    opcodes::DUP5 => self.dup::<5>(),
                    opcodes::DUP6 => self.dup::<6>(),
                    opcodes::DUP7 => self.dup::<7>(),
                    opcodes::DUP8 => self.dup::<8>(),
                    opcodes::DUP9 => self.dup::<9>(),
                    opcodes::DUP10 => self.dup::<10>(),
                    opcodes::DUP11 => self.dup::<11>(),
                    opcodes::DUP12 => self.dup::<12>(),
                    opcodes::DUP13 => self.dup::<13>(),
                    opcodes::DUP14 => self.dup::<14>(),
                    opcodes::DUP15 => self.dup::<15>(),
                    opcodes::DUP16 => self.dup::<16>(),

                    opcodes::SWAP1 => self.swap::<1>(),
                    opcodes::SWAP2 => self.swap::<2>(),
                    opcodes::SWAP3 => self.swap::<3>(),
                    opcodes::SWAP4 => self.swap::<4>(),
                    opcodes::SWAP5 => self.swap::<5>(),
                    opcodes::SWAP6 => self.swap::<6>(),
                    opcodes::SWAP7 => self.swap::<7>(),
                    opcodes::SWAP8 => self.swap::<8>(),
                    opcodes::SWAP9 => self.swap::<9>(),
                    opcodes::SWAP10 => self.swap::<10>(),
                    opcodes::SWAP11 => self.swap::<11>(),
                    opcodes::SWAP12 => self.swap::<12>(),
                    opcodes::SWAP13 => self.swap::<13>(),
                    opcodes::SWAP14 => self.swap::<14>(),
                    opcodes::SWAP15 => self.swap::<15>(),
                    opcodes::SWAP16 => self.swap::<16>(),

                    opcodes::RETURN => self.ret(),
                    opcodes::REVERT => self.revert(),
                    opcodes::INVALID => Err(EvmError::InvalidOpcode(opcodes::INVALID).into()),
                    opcodes::BASEFEE => self.basefee(system),
                    opcodes::ORIGIN => self.origin(system),
                    opcodes::CALLER => self.caller(),
                    opcodes::CALLVALUE => self.callvalue(),
                    opcodes::GASPRICE => self.gasprice(system),
                    opcodes::EXTCODESIZE => self.extcodesize(system),
                    opcodes::EXTCODEHASH => self.extcodehash(system),
                    opcodes::EXTCODECOPY => self.extcodecopy(system),
                    opcodes::RETURNDATASIZE => self.returndatasize(),
                    opcodes::RETURNDATACOPY => self.returndatacopy(),
                    opcodes::BLOCKHASH => self.blockhash(system),
                    opcodes::COINBASE => self.coinbase(system),
                    opcodes::TIMESTAMP => self.timestamp(system),
                    opcodes::NUMBER => self.number(system),
                    opcodes::DIFFICULTY => self.difficulty(system),
                    opcodes::GASLIMIT => self.gaslimit(system),
                    opcodes::SLOAD => self.sload(system, tracer),
                    opcodes::SSTORE => self.sstore(system, tracer),
                    opcodes::TLOAD => self.tload(system, tracer),
                    opcodes::TSTORE => self.tstore(system, tracer),
                    opcodes::MCOPY => self.mcopy(),
                    opcodes::GAS => self.gas(),
                    opcodes::LOG0 => self.log::<0>(system, tracer),
                    opcodes::LOG1 => self.log::<1>(system, tracer),
                    opcodes::LOG2 => self.log::<2>(system, tracer),
                    opcodes::LOG3 => self.log::<3>(system, tracer),
                    opcodes::LOG4 => self.log::<4>(system, tracer),
                    opcodes::SELFDESTRUCT => self.selfdestruct(system, tracer),
                    opcodes::CHAINID => self.chainid(system),
                    opcodes::BLOBHASH => self.blobhash(system),
                    opcodes::BLOBBASEFEE => self.blobbasefee(system),
                    x => Err(EvmError::InvalidOpcode(x).into()),
                });

            tracer.evm_tracer().after_evm_interpreter_execution_step(
                opcode,
                &InterpreterExternal::new_from(&self, system),
            );

            if Self::PRINT_OPCODES {
                let _ = system.get_logger().write_str("\n");
            }

            cycles += 1;

            if let Err(r) = result {
                break r;
            }
        };

        let _ = system.get_logger().write_fmt(format_args!(
            "Instructions executed = {}\nFinal instruction result = {:?}\n",
            cycles, &result
        ));

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
            ExitCode::ExternalCall | ExitCode::FatalError(_) => {
                return Err(internal_error!("Invalid exit code passed").into())
            }
        };

        if let ExitCode::EvmError(evm_error) = exit_code {
            if evm_error != EvmError::Revert {
                // Spend all remaining resources on EVM error
                self.gas.consume_all_gas();
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
                // EIP-158: reject code of length > 24576.
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
                    Ok(_) => {
                        // TODO: debug implementation for Bits uses global alloc, which panics in ZKsync OS
                        #[cfg(not(target_arch = "riscv32"))]
                        let _ = system.get_logger().write_fmt(format_args!(
                            "Successfully deployed contract at {:?} \n",
                            self.address
                        ));
                    }
                    Err(SystemError::LeafRuntime(RuntimeError::OutOfErgs(_))) => {
                        error_after_constructor = Some(EvmError::CodeStoreOutOfGas);
                    }
                    Err(SystemError::LeafRuntime(RuntimeError::OutOfNativeResources(loc))) => {
                        return Err(RuntimeError::OutOfNativeResources(loc).into())
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

    pub(crate) fn copy_returndata_to_heap(&mut self, returndata_region: &'ee [u8]) {
        // NOTE: it's not "returndatacopy", but if there was a "call" that did set up non-empty buffer for returndata,
        // it'll be automatically copied there
        if !self.returndata_location.is_empty() {
            unsafe {
                let to_copy =
                    core::cmp::min(returndata_region.len(), self.returndata_location.len());
                let src = returndata_region.as_ptr();
                let dst = self.heap.as_mut_ptr().add(self.returndata_location.start);
                core::ptr::copy_nonoverlapping(src, dst, to_copy);
            }
        }

        self.returndata = returndata_region;
    }

    pub fn derive_address_for_deployment(
        system: &mut System<S>,
        resources: &mut <S as SystemTypes>::Resources,
        scheme: CreateScheme,
        deployer_address: &<S::IOTypes as SystemIOTypesConfig>::Address,
        deployer_nonce: u64,
        deployment_code: &[u8],
    ) -> Result<<S::IOTypes as SystemIOTypesConfig>::Address, EvmSubsystemError> {
        use crypto::sha3::{Digest, Keccak256};
        let deployed_address = match &scheme {
            CreateScheme::Create => {
                let mut buffer = [0u8; crate::utils::MAX_CREATE_RLP_ENCODING_LEN];
                let encoding_it = crate::utils::create_quasi_rlp(deployer_address, deployer_nonce);
                let encoding_len = ExactSizeIterator::len(&encoding_it);
                for (dst, src) in buffer.iter_mut().zip(encoding_it) {
                    *dst = src;
                }
                let new_address = Keccak256::digest(&buffer[..encoding_len]);
                let new_address = B160::try_from_be_slice(&new_address.as_slice()[12..])
                    .expect("must create address");
                new_address
            }
            CreateScheme::Create2 { salt } => {
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
                            RootCause::Runtime(e @ RuntimeError::OutOfNativeResources(_)) => {
                                e.clone_or_copy().into()
                            }
                            _ => internal_error!("Keccak in create2 cannot fail").into(),
                        }
                    })?;
                let initcode_hash = Bytes32::from_array(initcode_hash.build());

                let mut create2_buffer = [0xffu8; 1 + 20 + 32 + 32];
                create2_buffer[1..(1 + 20)]
                    .copy_from_slice(&deployer_address.to_be_bytes::<{ B160::BYTES }>());
                create2_buffer[(1 + 20)..(1 + 20 + 32)]
                    .copy_from_slice(&salt.to_be_bytes::<{ U256::BYTES }>());
                create2_buffer[(1 + 20 + 32)..(1 + 20 + 32 + 32)]
                    .copy_from_slice(initcode_hash.as_u8_array_ref());

                let new_address = Keccak256::digest(&create2_buffer);
                let new_address = B160::try_from_be_slice(&new_address.as_slice()[12..])
                    .expect("must create address");
                new_address
            }
        };

        Ok(deployed_address)
    }
}
