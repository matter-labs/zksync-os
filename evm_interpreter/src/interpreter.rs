use super::*;
use core::fmt::Write;
use core::ops::Range;
use native_resource_constants::STEP_NATIVE_COST;
use zk_ee::system::{
    logger::Logger, CallModifier, CompletedDeployment, CompletedExecution,
    DeploymentPreparationParameters, DeploymentResult, EthereumLikeTypes,
    ExecutionEnvironmentPreemptionPoint, ExternalCallRequest, ReturnValues,
};
use zk_ee::system::{Ergs, ExecutionEnvironmentSpawnRequest, TransactionEndPoint};
use zk_ee::types_config::SystemIOTypesConfig;

impl<'ee, S: EthereumLikeTypes> Interpreter<'ee, S> {
    /// Keeps executing instructions (steps) from the system, until it hits a yield point -
    /// either due to some error, or return, or when trying to call a different contract
    /// or create one.
    pub fn execute_till_yield_point<'a>(
        &'a mut self,
        system: &mut System<S>,
    ) -> Result<ExecutionEnvironmentPreemptionPoint<'a, S>, FatalError> {
        let mut external_call = None;
        let exit_code = self.run(system, &mut external_call)?;

        if let ExitCode::FatalError(e) = exit_code {
            return Err(e);
        }

        if let Some(call) = external_call {
            assert!(exit_code == ExitCode::ExternalCall);
            let (current_heap, next_heap) = self.heap.freeze();

            return Ok(ExecutionEnvironmentPreemptionPoint::Spawn {
                heap: next_heap,
                request: match call {
                    ExternalCall::Call(EVMCallRequest {
                        gas_to_pass,
                        destination_address,
                        calldata,
                        modifier,
                        call_value,
                    }) => {
                        let ergs_to_pass = Ergs(gas_to_pass.saturating_mul(ERGS_PER_GAS));
                        let available_resources = self.gas.take_resources();
                        ExecutionEnvironmentSpawnRequest::RequestedExternalCall(
                            ExternalCallRequest {
                                calldata: &current_heap[calldata],
                                call_scratch_space: None,
                                nominal_token_value: call_value,
                                callers_caller: self.caller,
                                caller: self.address,
                                callee: destination_address,
                                modifier,
                                ergs_to_pass,
                                available_resources,
                            },
                        )
                    }

                    ExternalCall::Create(EVMDeploymentRequest {
                        deployment_code,
                        ee_specific_deployment_processing_data,
                        deployer_full_resources,
                        nominal_token_value,
                    }) => ExecutionEnvironmentSpawnRequest::RequestedDeployment(
                        DeploymentPreparationParameters {
                            address_of_deployer: self.address,
                            call_scratch_space: None,
                            deployment_code: &current_heap[deployment_code],
                            constructor_parameters: &[],
                            ee_specific_deployment_processing_data,
                            deployer_full_resources,
                            nominal_token_value,
                            deployer_nonce: None,
                        },
                    ),
                },
            });
        }

        let (empty_returndata, reverted) = match exit_code {
            ExitCode::Stop => (true, false),
            ExitCode::SelfDestruct => (true, false),
            ExitCode::Return => (false, false),
            ExitCode::Revert => (false, true),
            _ => (true, true),
        };

        self.create_immediate_return_state(empty_returndata, reverted, exit_code.is_error())
    }
}

pub enum ExternalCall<S: EthereumLikeTypes> {
    Call(EVMCallRequest<S>),
    Create(EVMDeploymentRequest<S>),
}

pub struct EVMCallRequest<S: EthereumLikeTypes> {
    pub(crate) gas_to_pass: u64,
    pub(crate) call_value: U256,
    pub(crate) destination_address: <S::IOTypes as SystemIOTypesConfig>::Address,
    pub(crate) calldata: Range<usize>,
    pub(crate) modifier: CallModifier,
}

pub struct EVMDeploymentRequest<S: SystemTypes> {
    pub deployment_code: Range<usize>,
    pub ee_specific_deployment_processing_data:
        Option<alloc::boxed::Box<dyn core::any::Any, S::Allocator>>,
    pub deployer_full_resources: S::Resources,
    pub nominal_token_value: <S::IOTypes as SystemIOTypesConfig>::NominalTokenValue,
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
        external_call_dest: &mut Option<ExternalCall<S>>,
    ) -> Result<ExitCode, FatalError> {
        let mut cycles = 0;
        let result = loop {
            let opcode = self.get_bytecode_unchecked(self.instruction_pointer);

            match crate::opcodes::OpCode::try_from_u8(opcode) {
                Some(op) => {
                    if Self::PRINT_OPCODES {
                        let _ = system
                            .get_logger()
                            .write_fmt(format_args!("Executing {}", op));
                    }
                }
                None => {
                    let _ = system
                        .get_logger()
                        .write_fmt(format_args!("Unknown opcode = 0x{:02x}\n", opcode));
                }
            }

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
                    opcodes::INVALID => Err(ExitCode::InvalidFEOpcode),
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
                    opcodes::SLOAD => self.sload(system),
                    opcodes::SSTORE => self.sstore(system),
                    opcodes::TLOAD => self.tload(system),
                    opcodes::TSTORE => self.tstore(system),
                    opcodes::MCOPY => self.mcopy(),
                    opcodes::GAS => self.gas(),
                    opcodes::LOG0 => self.log::<0>(system),
                    opcodes::LOG1 => self.log::<1>(system),
                    opcodes::LOG2 => self.log::<2>(system),
                    opcodes::LOG3 => self.log::<3>(system),
                    opcodes::LOG4 => self.log::<4>(system),
                    opcodes::SELFDESTRUCT => self.selfdestruct(system),
                    opcodes::CHAINID => self.chainid(system),
                    opcodes::BLOBHASH => self.blobhash(system),
                    opcodes::BLOBBASEFEE => self.blobbasefee(system),
                    _ => Err(ExitCode::OpcodeNotFound),
                });

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
        empty_returndata: bool,
        execution_reverted: bool,
        is_error: bool,
    ) -> Result<ExecutionEnvironmentPreemptionPoint<'a, S>, FatalError> {
        if is_error {
            // Spend all remaining resources on error
            self.gas.consume_all_gas();
        };
        let mut return_values = ReturnValues::empty();
        if empty_returndata == false {
            return_values.returndata = &self.heap[self.returndata_location.clone()];
        }

        if self.is_constructor {
            let deployment_result = if execution_reverted == false {
                let deployed_code_len = return_values.returndata.len() as u64;
                // EIP-3541: reject code starting with 0xEF.
                // EIP-158: reject code of length > 24576.
                let deployed = return_values.returndata;
                if deployed_code_len >= 1 && deployed[0] == 0xEF
                    || return_values.returndata.len() > MAX_CODE_SIZE
                {
                    // Spend all remaining resources
                    self.gas.consume_all_gas();
                    DeploymentResult::Failed {
                        return_values,
                        execution_reverted,
                    }
                } else {
                    // It's responsibility of the System/IO to properly charge,
                    // so we just construct the structure

                    let bytecode = return_values.returndata;
                    return_values.returndata = &[];
                    let bytecode_len = bytecode.len() as u32;
                    let artifacts_len = 0u32;
                    DeploymentResult::Successful {
                        bytecode,
                        bytecode_len,
                        artifacts_len,
                        return_values,
                        deployed_at: self.address,
                    }
                }
            } else {
                DeploymentResult::Failed {
                    return_values,
                    execution_reverted,
                }
            };

            Ok(ExecutionEnvironmentPreemptionPoint::End(
                TransactionEndPoint::CompletedDeployment(CompletedDeployment {
                    resources_returned: self.gas.take_resources(),
                    deployment_result,
                }),
            ))
        } else {
            Ok(ExecutionEnvironmentPreemptionPoint::End(
                TransactionEndPoint::CompletedExecution(CompletedExecution {
                    return_values,
                    resources_returned: self.gas.take_resources(),
                    reverted: execution_reverted,
                }),
            ))
        }
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
}
