use crate::interpreter::*;
use core::hint::unreachable_unchecked;
use gas_constants::{CALL_STIPEND, INITCODE_WORD_COST, SHA3WORD};

use native_resource_constants::*;
use zk_ee::kv_markers::MAX_EVENT_TOPICS;
use zk_ee::system::*;

use super::*;

impl<'ee, S: EthereumLikeTypes> Interpreter<'ee, S> {
    pub fn balance(&mut self, system: &mut System<S>) -> InstructionResult {
        self.spend_gas_and_native(0, BALANCE_NATIVE_COST)?;
        let [address] = self.stack.pop_addresses::<1>()?;
        let value =
            system
                .io
                .get_nominal_token_balance(THIS_EE_TYPE, &mut self.resources, &address)?;
        self.stack.push_one(value)
    }

    pub fn selfbalance(&mut self, system: &mut System<S>) -> InstructionResult {
        self.spend_gas_and_native(0, SELFBALANCE_NATIVE_COST)?;
        let value = system
            .io
            .get_selfbalance(THIS_EE_TYPE, &mut self.resources, &self.address)?;
        self.stack.push_one(value)
    }

    pub fn extcodesize(&mut self, system: &mut System<S>) -> InstructionResult {
        self.spend_gas_and_native(0, EXTCODESIZE_NATIVE_COST)?;
        let [address] = self.stack.pop_addresses::<1>()?;
        let value =
            system
                .io
                .get_observable_bytecode_size(THIS_EE_TYPE, &mut self.resources, &address)?;
        self.stack.push_one(U256::from(value))
    }

    pub fn extcodehash(&mut self, system: &mut System<S>) -> InstructionResult {
        self.spend_gas_and_native(0, EXTCODEHASH_NATIVE_COST)?;
        let [address] = self.stack.pop_addresses::<1>()?;
        let value =
            system
                .io
                .get_observable_bytecode_hash(THIS_EE_TYPE, &mut self.resources, &address)?;
        self.stack.push_one(value.into_u256_be())
    }

    pub fn extcodecopy(&mut self, system: &mut System<S>) -> InstructionResult {
        let [address] = self.stack.pop_addresses::<1>()?;
        let [memory_offset, source_offset, len] = self.stack.pop_values::<3>()?;

        // first deal with locals memory
        let (memory_offset, len) =
            self.cast_offset_and_len(&memory_offset, &len, ExitCode::InvalidOperandOOG)?;

        // resize memory to account for the destination memory required
        self.resize_heap(memory_offset, len)?;

        let bytecode =
            system
                .io
                .get_observable_bytecode(THIS_EE_TYPE, &mut self.resources, &address)?;

        // now follow logic of calldatacopy
        let source = u256_try_to_usize(&source_offset)
            .and_then(|offset| bytecode.get(offset..))
            .unwrap_or(&[]);

        // Charge for copy cost
        let (gas_cost, native_cost) = self.copy_cost(len as u64)?;
        self.spend_gas_and_native(gas_cost, native_cost + EXTCODECOPY_NATIVE_COST)?;

        copy_and_zeropad_nonoverlapping(source, &mut self.heap[memory_offset..memory_offset + len]);

        if Self::PRINT_OPCODES {
            use core::fmt::Write;
            let _ = system.get_logger().write_fmt(format_args!(
                " len {}, source offset: {:?}, dest offset {}",
                len, source_offset, memory_offset
            ));
        }

        Ok(())
    }

    pub fn sload(&mut self, system: &mut System<S>) -> InstructionResult {
        self.spend_gas_and_native(0, SLOAD_NATIVE_COST)?;
        let [index] = self.stack.pop_values::<1>()?.map(Bytes32::from_u256_be);
        let value = system.io.storage_read::<false>(
            THIS_EE_TYPE,
            &mut self.resources,
            &self.address,
            &index,
        )?;

        self.stack.push_one(value.into_u256_be())
    }

    pub fn tload(&mut self, system: &mut System<S>) -> InstructionResult {
        self.spend_gas_and_native(0, TLOAD_NATIVE_COST)?;
        let [index] = self.stack.pop_values::<1>()?.map(Bytes32::from_u256_be);
        let value = system.io.storage_read::<true>(
            THIS_EE_TYPE,
            &mut self.resources,
            &self.address,
            &index,
        )?;
        self.stack.push_one(value.into_u256_be())
    }

    pub fn sstore(&mut self, system: &mut System<S>) -> InstructionResult {
        self.spend_gas_and_native(0, SSTORE_NATIVE_COST)?;
        if self.is_static_frame() {
            return Err(ExitCode::StateChangeDuringStaticCall);
        }
        if self.gas_left() <= CALL_STIPEND {
            return Err(ExitCode::InvalidOperandOOG);
        }
        let [index, value] = self.stack.pop_values::<2>()?.map(Bytes32::from_u256_be);

        system.io.storage_write::<false>(
            THIS_EE_TYPE,
            &mut self.resources,
            &self.address,
            &index,
            &value,
        )?;

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

    pub fn tstore(&mut self, system: &mut System<S>) -> InstructionResult {
        self.spend_gas_and_native(0, TSTORE_NATIVE_COST)?;
        if self.is_static_frame() {
            return Err(ExitCode::StateChangeDuringStaticCall);
        }
        let [index, value] = self.stack.pop_values::<2>()?.map(Bytes32::from_u256_be);
        system.io.storage_write::<true>(
            THIS_EE_TYPE,
            &mut self.resources,
            &self.address,
            &index,
            &value,
        )?;

        Ok(())
    }

    pub fn log<const N: usize>(&mut self, system: &mut System<S>) -> InstructionResult {
        assert!(N <= MAX_EVENT_TOPICS);
        self.spend_gas_and_native(0, LOG_NATIVE_COST)?;

        if self.is_static_frame() {
            return Err(ExitCode::StateChangeDuringStaticCall);
        }

        let [mem_offset, len] = self.stack.pop_values::<2>()?;
        let topics: arrayvec::ArrayVec<Bytes32, 4> =
            arrayvec::ArrayVec::from_iter(self.stack.pop_values::<N>()?.map(Bytes32::from_u256_be));

        // resize memory
        let (mem_offset, len) =
            self.cast_offset_and_len(&mem_offset, &len, ExitCode::InvalidOperandOOG)?;

        self.resize_heap(mem_offset, len)?;
        let data = &self.heap[mem_offset..mem_offset + len];

        system.io.emit_event(
            ExecutionEnvironmentType::EVM,
            &mut self.resources,
            &self.address,
            &topics,
            data,
        )?;

        Ok(())
    }

    pub fn selfdestruct(&mut self, system: &mut System<S>) -> InstructionResult {
        self.spend_gas_and_native(gas_constants::SELFDESTRUCT, SELFDESTRUCT_NATIVE_COST)?;

        if self.is_static_frame() {
            return Err(ExitCode::StateChangeDuringStaticCall);
        }

        let [beneficiary] = self.stack.pop_addresses::<1>()?;

        system.io.mark_for_deconstruction(
            THIS_EE_TYPE,
            &mut self.resources,
            &self.address,
            &beneficiary,
            self.is_constructor,
        )?;

        Err(ExitCode::SelfDestruct)
    }

    pub fn create<const IS_CREATE2: bool>(
        &mut self,
        system: &mut System<S>,
        external_call_dest: &mut Option<ExternalCall<S>>,
    ) -> InstructionResult {
        self.spend_gas_and_native(
            gas_constants::CREATE,
            if IS_CREATE2 {
                native_resource_constants::CREATE2_NATIVE_COST
            } else {
                native_resource_constants::CREATE_NATIVE_COST
            },
        )?;

        if self.is_static_frame() {
            return Err(ExitCode::StateChangeDuringStaticCall);
        }
        self.clear_last_returndata();

        let [value, code_offset, len] = self.stack.pop_values::<3>()?;

        let (code_offset, len) =
            self.cast_offset_and_len(&code_offset, &len, ExitCode::InvalidOperandOOG)?;

        self.resize_heap(code_offset, len)?;

        // Create code size is limited
        if len > MAX_INITCODE_SIZE {
            return Err(ExitCode::CreateInitcodeSizeLimit);
        }

        // Charge for dynamic gas
        let cost_per_word = if IS_CREATE2 {
            INITCODE_WORD_COST + SHA3WORD
        } else {
            INITCODE_WORD_COST
        };
        let initcode_cost = cost_per_word * ((len as u64).next_multiple_of(32) / 32);
        self.spend_gas(initcode_cost)?;
        let end = code_offset + len; // can not overflow as we resized heap above using same values

        // we will charge for everything in the "should_continue..." function
        let scheme = if IS_CREATE2 {
            let [salt] = self.stack.pop_values::<1>()?;
            CreateScheme::Create2 { salt }
        } else {
            CreateScheme::Create
        };

        // TODO: not necessary once heaps get the same treatment as calldata
        let deployment_code = code_offset..end;

        let ee_specific_data = alloc::boxed::Box::try_new_in(scheme, system.get_allocator())
            .expect("system allocator must be capable to allocate for EE deployment parameters");
        // at this preemption point we give all resources for preparation
        let all_resources = self.resources.take();

        let deployment_parameters = EVMDeploymentRequest {
            deployment_code,
            ee_specific_deployment_processing_data: Some(
                ee_specific_data as alloc::boxed::Box<dyn core::any::Any, S::Allocator>,
            ),
            nominal_token_value: value,
            deployer_full_resources: all_resources,
        };

        *external_call_dest = Some(ExternalCall::Create(deployment_parameters));

        Err(ExitCode::ExternalCall)
    }

    pub fn call(&mut self, external_call_dest: &mut Option<ExternalCall<S>>) -> InstructionResult {
        self.call_impl(CallScheme::Call, external_call_dest)
    }

    pub fn call_code(
        &mut self,
        external_call_dest: &mut Option<ExternalCall<S>>,
    ) -> InstructionResult {
        self.call_impl(CallScheme::CallCode, external_call_dest)
    }

    pub fn delegate_call(
        &mut self,
        external_call_dest: &mut Option<ExternalCall<S>>,
    ) -> InstructionResult {
        self.call_impl(CallScheme::DelegateCall, external_call_dest)
    }

    pub fn static_call(
        &mut self,
        external_call_dest: &mut Option<ExternalCall<S>>,
    ) -> InstructionResult {
        self.call_impl(CallScheme::StaticCall, external_call_dest)
    }

    fn call_impl(
        &mut self,
        scheme: CallScheme,
        external_call_dest: &mut Option<ExternalCall<S>>,
    ) -> InstructionResult {
        self.spend_gas_and_native(0, native_resource_constants::CALL_NATIVE_COST)?;
        self.clear_last_returndata();

        let [local_gas_limit] = self.stack.pop_values::<1>()?;
        let [to] = self.stack.pop_addresses::<1>()?;

        let local_gas_limit = u256_to_u64_saturated(&local_gas_limit);

        let value = match scheme {
            CallScheme::CallCode => {
                let [value] = self.stack.pop_values::<1>()?;
                value
            }
            CallScheme::Call => {
                let [value] = self.stack.pop_values::<1>()?;
                if self.is_static && value != U256::ZERO {
                    return Err(ExitCode::CallNotAllowedInsideStatic);
                }
                value
            }
            CallScheme::DelegateCall => self.call_value,
            CallScheme::StaticCall => U256::ZERO,
        };

        let [in_offset, in_len, out_offset, out_len] = self.stack.pop_values::<4>()?;

        let (in_offset, in_len) =
            self.cast_offset_and_len(&in_offset, &in_len, ExitCode::InvalidOperandOOG)?;

        let (out_offset, out_len) =
            self.cast_offset_and_len(&out_offset, &out_len, ExitCode::InvalidOperandOOG)?;

        self.resize_heap(in_offset, in_len)?;
        self.resize_heap(out_offset, out_len)?;

        // TODO: not necessary once heaps get the calldata treatment
        let calldata = in_offset..(in_offset + in_len);

        // NOTE: we give to the system both what we have NOW, and what we WANT to pass,
        // and depending on warm/cold behavior it may charge more from the current frame,
        // and pass less.

        let ergs_to_pass = Ergs(local_gas_limit.saturating_mul(ERGS_PER_GAS));

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

        let call_request = EVMCallRequest {
            destination_address: to,
            calldata,
            modifier: call_modifier,
            ergs_to_pass,
            call_value: value,
        };

        *external_call_dest = Some(ExternalCall::Call(call_request));
        Err(ExitCode::ExternalCall)
    }
}
