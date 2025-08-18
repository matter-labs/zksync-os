use crate::gas::gas_utils;
use crate::interpreter::*;
use core::hint::unreachable_unchecked;
use gas_constants::{CALL_STIPEND, INITCODE_WORD_COST, SHA3WORD};

use native_resource_constants::*;
use zk_ee::kv_markers::MAX_EVENT_TOPICS;
use zk_ee::system::tracer::Tracer;
use zk_ee::{system::*, wrap_error};

use super::*;

impl<'ee, S: EthereumLikeTypes> Interpreter<'ee, S> {
    pub fn balance(&mut self, system: &mut System<S>) -> InstructionResult {
        self.gas.spend_gas_and_native(0, BALANCE_NATIVE_COST)?;
        let stack_top = self.stack.top_mut()?;
        let address = u256_to_b160(stack_top);
        let value = system.io.get_nominal_token_balance(
            THIS_EE_TYPE,
            self.gas.resources_mut(),
            &address,
        )?;
        *stack_top = value;
        Ok(())
    }

    pub fn selfbalance(&mut self, system: &mut System<S>) -> InstructionResult {
        self.gas.spend_gas_and_native(0, SELFBALANCE_NATIVE_COST)?;
        let value =
            system
                .io
                .get_selfbalance(THIS_EE_TYPE, self.gas.resources_mut(), &self.address)?;
        self.stack.push(&value)
    }

    pub fn extcodesize(&mut self, system: &mut System<S>) -> InstructionResult {
        self.gas.spend_gas_and_native(0, EXTCODESIZE_NATIVE_COST)?;
        let stack_top = self.stack.top_mut()?;
        let address = u256_to_b160(stack_top);
        let value = system.io.get_observable_bytecode_size(
            THIS_EE_TYPE,
            self.gas.resources_mut(),
            &address,
        )?;
        *stack_top = U256::from(value);
        Ok(())
    }

    pub fn extcodehash(&mut self, system: &mut System<S>) -> InstructionResult {
        self.gas.spend_gas_and_native(0, EXTCODEHASH_NATIVE_COST)?;
        let stack_top = self.stack.top_mut()?;
        let address = u256_to_b160(stack_top);
        let value = system.io.get_observable_bytecode_hash(
            THIS_EE_TYPE,
            self.gas.resources_mut(),
            &address,
        )?;

        *stack_top = value.into_u256_be();
        Ok(())
    }

    pub fn extcodecopy(&mut self, system: &mut System<S>) -> InstructionResult {
        let (address, memory_offset, source_offset, len) = self.stack.pop_4()?;
        let address = u256_to_b160(address);
        // first deal with locals memory
        let (memory_offset, len) =
            Self::cast_offset_and_len(&memory_offset, &len, EvmError::InvalidOperandOOG.into())?;

        // resize memory to account for the destination memory required
        Self::resize_heap_implementation(&mut self.heap, &mut self.gas, memory_offset, len)?;

        let bytecode =
            system
                .io
                .get_observable_bytecode(THIS_EE_TYPE, self.gas.resources_mut(), &address)?;

        // now follow logic of calldatacopy
        let source = u256_try_to_usize(&source_offset)
            .and_then(|offset| bytecode.get(offset..))
            .unwrap_or(&[]);

        // Charge for copy cost
        let (gas_cost, native_cost) = gas_utils::copy_cost(len as u64)?;
        self.gas
            .spend_gas_and_native(gas_cost, native_cost + EXTCODECOPY_NATIVE_COST)?;

        copy_and_zeropad_nonoverlapping(source, &mut self.heap[memory_offset..memory_offset + len]);

        if Self::PRINT_OPCODES {
            use core::fmt::Write;
            let _ = system.get_logger().write_fmt(format_args!(
                " len {len}, source offset: {source_offset:?}, dest offset {memory_offset}"
            ));
        }

        Ok(())
    }

    pub fn sload(
        &mut self,
        system: &mut System<S>,
        tracer: &mut impl Tracer<S>,
    ) -> InstructionResult {
        self.gas.spend_gas_and_native(0, SLOAD_NATIVE_COST)?;
        let stack_head = self.stack.top_mut()?;
        let key = Bytes32::from_u256_be(stack_head);
        let value = system.io.storage_read::<false>(
            THIS_EE_TYPE,
            self.gas.resources_mut(),
            &self.address,
            &key,
        )?;

        *stack_head = value.into_u256_be();

        tracer.on_storage_write(THIS_EE_TYPE, false, self.address, key, value);

        Ok(())
    }

    pub fn tload(
        &mut self,
        system: &mut System<S>,
        tracer: &mut impl Tracer<S>,
    ) -> InstructionResult {
        self.gas.spend_gas_and_native(0, TLOAD_NATIVE_COST)?;
        let stack_head = self.stack.top_mut()?;
        let key = Bytes32::from_u256_be(stack_head);
        let value = system.io.storage_read::<true>(
            THIS_EE_TYPE,
            self.gas.resources_mut(),
            &self.address,
            &key,
        )?;

        *stack_head = value.into_u256_be();

        tracer.on_storage_write(THIS_EE_TYPE, true, self.address, key, value);

        Ok(())
    }

    pub fn sstore(
        &mut self,
        system: &mut System<S>,
        tracer: &mut impl Tracer<S>,
    ) -> InstructionResult {
        self.gas.spend_gas_and_native(0, SSTORE_NATIVE_COST)?;
        if self.is_static_frame() {
            return Err(EvmError::StateChangeDuringStaticCall.into());
        }
        if self.gas.gas_left() <= CALL_STIPEND {
            return Err(EvmError::InvalidOperandOOG.into());
        }
        let (index, value) = self.stack.pop_2()?;
        let index = Bytes32::from_u256_be(index);
        let value = Bytes32::from_u256_be(value);

        system.io.storage_write::<false>(
            THIS_EE_TYPE,
            self.gas.resources_mut(),
            &self.address,
            &index,
            &value,
        )?;

        tracer.on_storage_write(THIS_EE_TYPE, false, self.address, index, value);

        // This is an example of what would need to be done with tracing
        if Self::PRINT_OPCODES {
            use core::fmt::Write;
            let _ = system.get_logger().write_fmt(format_args!(
                " address {:?}, key {:?}, value {:?}",
                &self.address, &index, &value
            ));
        }

        Ok(())
    }

    pub fn tstore(
        &mut self,
        system: &mut System<S>,
        tracer: &mut impl Tracer<S>,
    ) -> InstructionResult {
        self.gas.spend_gas_and_native(0, TSTORE_NATIVE_COST)?;
        if self.is_static_frame() {
            return Err(EvmError::StateChangeDuringStaticCall.into());
        }
        let (index, value) = self.stack.pop_2()?;
        let index = Bytes32::from_u256_be(index);
        let value = Bytes32::from_u256_be(value);
        system.io.storage_write::<true>(
            THIS_EE_TYPE,
            self.gas.resources_mut(),
            &self.address,
            &index,
            &value,
        )?;

        tracer.on_storage_write(THIS_EE_TYPE, true, self.address, index, value);

        Ok(())
    }

    pub fn log<const N: usize>(
        &mut self,
        system: &mut System<S>,
        tracer: &mut impl Tracer<S>,
    ) -> InstructionResult {
        assert!(N <= MAX_EVENT_TOPICS);
        self.gas.spend_gas_and_native(0, LOG_NATIVE_COST)?;

        if self.is_static_frame() {
            return Err(EvmError::StateChangeDuringStaticCall.into());
        }

        let (mem_offset, len) = self.stack.pop_2()?;
        let (mem_offset, len) =
            Self::cast_offset_and_len(&mem_offset, &len, EvmError::InvalidOperandOOG.into())?;
        let mut topics: arrayvec::ArrayVec<Bytes32, 4> = arrayvec::ArrayVec::new();
        for _ in 0..N {
            topics.push(Bytes32::from_u256_be(self.stack.pop_1()?));
        }

        // resize memory
        self.resize_heap(mem_offset, len)?;
        let data = &self.heap[mem_offset..mem_offset + len];

        tracer.on_event(THIS_EE_TYPE, &self.address, &topics, data);

        system.io.emit_event(
            ExecutionEnvironmentType::EVM,
            self.gas.resources_mut(),
            &self.address,
            &topics,
            data,
        )?;

        Ok(())
    }

    pub fn selfdestruct(&mut self, system: &mut System<S>) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::SELFDESTRUCT, SELFDESTRUCT_NATIVE_COST)?;

        if self.is_static_frame() {
            return Err(EvmError::StateChangeDuringStaticCall.into());
        }

        let beneficiary = u256_to_b160(self.stack.pop_1()?);

        system
            .io
            .mark_for_deconstruction(
                THIS_EE_TYPE,
                self.gas.resources_mut(),
                &self.address,
                &beneficiary,
                self.is_constructor,
            )
            .map_err(wrap_error!())?;

        Err(ExitCode::SelfDestruct)
    }

    pub fn create<const IS_CREATE2: bool>(
        &mut self,
        system: &mut System<S>,
        external_call_dest: &mut Option<EVMCallRequest<S>>,
    ) -> InstructionResult {
        self.gas.spend_gas_and_native(
            gas_constants::CREATE,
            if IS_CREATE2 {
                native_resource_constants::CREATE2_NATIVE_COST
            } else {
                native_resource_constants::CREATE_NATIVE_COST
            },
        )?;

        if self.is_static_frame() {
            return Err(EvmError::StateChangeDuringStaticCall.into());
        }
        self.clear_last_returndata();

        let (value, code_offset, len) = self.stack.pop_3()?;
        let value = *value;

        let (code_offset, len) =
            Self::cast_offset_and_len(code_offset, len, EvmError::InvalidOperandOOG.into())?;

        Self::resize_heap_implementation(&mut self.heap, &mut self.gas, code_offset, len)?;

        // Create code size is limited
        if len > MAX_INITCODE_SIZE {
            return Err(EvmError::CreateInitcodeSizeLimit.into());
        }

        // Charge for dynamic gas
        let cost_per_word = if IS_CREATE2 {
            INITCODE_WORD_COST + SHA3WORD
        } else {
            INITCODE_WORD_COST
        };
        let initcode_cost = cost_per_word * ((len as u64).next_multiple_of(32) / 32);
        self.gas.spend_gas(initcode_cost)?;
        let end = code_offset + len; // can not overflow as we resized heap above using same values

        // we will charge for everything in the "should_continue..." function
        let scheme = if IS_CREATE2 {
            let salt = self.stack.pop_1()?;
            CreateScheme::Create2 { salt: *salt }
        } else {
            CreateScheme::Create
        };

        // TODO: not necessary once heaps get the same treatment as calldata
        let deployment_code = code_offset..end;

        let deployer_nonce = self.gas.resources.with_infinite_ergs(|inf_resources| {
            system
                .io
                .read_nonce(THIS_EE_TYPE, inf_resources, &self.address)
        })?;

        let deployed_address = Self::derive_address_for_deployment(
            system,
            self.gas.resources_mut(),
            scheme,
            &self.address,
            deployer_nonce,
            &self.heap[deployment_code.clone()],
        )?;

        // at this preemption point we give all resources to the system
        let all_resources = self.gas.take_resources();

        self.pending_os_request = Some(PendingOsRequest::Create(deployed_address));

        *external_call_dest = Some(EVMCallRequest {
            ergs_to_pass: all_resources.ergs(),
            call_value: value,
            destination_address: deployed_address,
            input_data: deployment_code,
            modifier: CallModifier::Constructor,
            full_caller_resources: all_resources,
        });

        Err(ExitCode::ExternalCall)
    }

    pub fn call(
        &mut self,
        external_call_dest: &mut Option<EVMCallRequest<S>>,
    ) -> InstructionResult {
        self.call_impl(CallScheme::Call, external_call_dest)
    }

    pub fn call_code(
        &mut self,
        external_call_dest: &mut Option<EVMCallRequest<S>>,
    ) -> InstructionResult {
        self.call_impl(CallScheme::CallCode, external_call_dest)
    }

    pub fn delegate_call(
        &mut self,
        external_call_dest: &mut Option<EVMCallRequest<S>>,
    ) -> InstructionResult {
        self.call_impl(CallScheme::DelegateCall, external_call_dest)
    }

    pub fn static_call(
        &mut self,
        external_call_dest: &mut Option<EVMCallRequest<S>>,
    ) -> InstructionResult {
        self.call_impl(CallScheme::StaticCall, external_call_dest)
    }

    fn call_impl(
        &mut self,
        scheme: CallScheme,
        external_call_dest: &mut Option<EVMCallRequest<S>>,
    ) -> InstructionResult {
        self.gas
            .spend_gas_and_native(0, native_resource_constants::CALL_NATIVE_COST)?;
        self.clear_last_returndata();
        // TODO optimize stack operations
        let (gas_to_pass, to) = self.stack.pop_2()?;
        let to = u256_to_b160(to);
        let gas_to_pass = u256_to_u64_saturated(&gas_to_pass);

        let value = match scheme {
            CallScheme::CallCode => {
                let value = self.stack.pop_1()?;
                *value
            }
            CallScheme::Call => {
                let value = self.stack.pop_1()?;
                if self.is_static && *value != U256::ZERO {
                    return Err(EvmError::CallNotAllowedInsideStatic.into());
                }
                *value
            }
            CallScheme::DelegateCall => self.call_value,
            CallScheme::StaticCall => U256::ZERO,
        };

        let (in_offset, in_len, out_offset, out_len) = self.stack.pop_4()?;

        let (in_offset, in_len) =
            Self::cast_offset_and_len(in_offset, in_len, EvmError::InvalidOperandOOG.into())?;

        let (out_offset, out_len) =
            Self::cast_offset_and_len(out_offset, out_len, EvmError::InvalidOperandOOG.into())?;

        self.resize_heap(in_offset, in_len)?;
        self.resize_heap(out_offset, out_len)?;

        // TODO: not necessary once heaps get the calldata treatment
        let calldata = in_offset..(in_offset + in_len);

        // TODO clarify gas model here
        // NOTE: we give to the system both what we have NOW, and what we WANT to pass,
        // and depending on warm/cold behavior it may charge more from the current frame,
        // and pass less.

        let is_static = matches!(scheme, CallScheme::StaticCall) || self.is_static;
        let call_modifier = if is_static {
            match scheme {
                CallScheme::DelegateCall => CallModifier::DelegateStatic,
                CallScheme::CallCode => CallModifier::EVMCallcodeStatic,
                _ => CallModifier::Static,
            }
        } else {
            match scheme {
                CallScheme::Call => CallModifier::NoModifier,
                CallScheme::DelegateCall => CallModifier::Delegate,
                CallScheme::CallCode => CallModifier::EVMCallcode,
                _ => unsafe { unreachable_unchecked() },
            }
        };

        // we also set "last returndata" as a placeholder place for "to where to copy"
        self.returndata_location = out_offset..(out_offset + out_len);

        self.pending_os_request = Some(PendingOsRequest::Call);

        // at this preemption point we give all resources to the system
        let all_resources = self.gas.take_resources();

        *external_call_dest = Some(EVMCallRequest {
            ergs_to_pass: Ergs(gas_to_pass.saturating_mul(ERGS_PER_GAS)),
            call_value: value,
            destination_address: to,
            input_data: calldata,
            modifier: call_modifier,
            full_caller_resources: all_resources,
        });

        Err(ExitCode::ExternalCall)
    }
}
