use core::fmt::{Arguments, Write};

use crate::frame::IWasmImportContext;
use crate::host_ops::{long_host_op, short_host_op};
use crate::interpreter::MAX_IMMUTABLES_SIZE;
use alloc::vec::Vec;
use iwasm_interpreter::routines::runtime::stack_value::StackValue;
use iwasm_interpreter::routines::InterpreterError;
use iwasm_interpreter::{constants::PAGE_SIZE, routines::runtime::host::Host, types::*};
use zk_ee::system_trait::EthereumLikeSystem;

use super::frame::Context;
use iwasm_interpreter::routines::runtime::instantiate::ExecutionResult;
use zk_ee::system_trait::execution_environment::*;
use zk_ee::system_trait::Resources;

// WASM C binding: https://github.com/WebAssembly/tool-conventions/blob/main/BasicCABI.md#function-arguments-and-return-values
// Non-primitive return values are returned via a pointer prepended to the arg list.
pub const EXPECTED_IMPORTED_FUNCTIONS: [(&str, &str, FunctionType<ValueTypeVecRef<'static>>); 2] = [
    (
        "env",
        "short_host_op",
        FunctionType {
            inputs: ValueTypeVecRef {
                types: &[
                    ValueType::I32,
                    ValueType::I32,
                    ValueType::I64,
                    ValueType::I32,
                    ValueType::I32,
                ],
            },
            outputs: ValueTypeVecRef { types: &[] },
        },
    ),
    (
        "env",
        "long_host_op",
        FunctionType {
            inputs: ValueTypeVecRef {
                types: &[
                    ValueType::I32,
                    ValueType::I32,
                    ValueType::I64,
                    ValueType::I32,
                    ValueType::I32,
                    ValueType::I32,
                    ValueType::I32,
                ],
            },
            outputs: ValueTypeVecRef { types: &[] },
        },
    ),
];

pub struct ZkOSHost<'a, S: EthereumLikeSystem> {
    pub context: &'a Context<S>,
    pub import_context: &'a mut IWasmImportContext<S>,
    pub resources: &'a mut S::Resources,
    pub system: &'a mut S,
    pub returndata_region: MemoryRegion,
}

impl<'a, S: EthereumLikeSystem> ZkOSHost<'a, S> {
    pub fn compute_num_heap_pages(&self) -> u32 {
        let num_pages = self
            .system
            .get_memory_region(MemoryRegionType::Shared)
            .len()
            / PAGE_SIZE;

        num_pages as u32
    }

    fn get_host_functions(
        &self,
    ) -> [fn(
        &mut ZkOSHost<'_, S>,
        &mut [StackValue],
        usize,
    ) -> Result<ExecutionResult, InterpreterError>; 2] {
        [short_host_op::<S>, long_host_op::<S>]
    }

    pub fn create_immediate_return_state(
        self,
        returndata: MemoryRegion,
        reverted: bool,
    ) -> ExecutionEnvironmentExitState<S> {
        let resources = self.resources.clone();
        // TODO: ask alex for reason for zeroing this out.
        *self.resources = S::Resources::empty();

        // let mut return_values = ReturnValues::empty(self.system);
        // // if empty_returndata == false {
        // //     core::mem::swap(&mut return_values.returndata, &mut self.returndata_region);
        // // }
        //
        // let return_values = match empty_returndata {
        //     true => ReturnValues::empty(self.system),
        //     false => {
        //         let vref =
        //             self
        //
        //     }
        // }

        let return_values = ReturnValues::from_region(returndata, self.system).unwrap();

        if self.context.is_constructor {
            let deployment_result =
                if reverted || return_values.region().len() > MAX_IMMUTABLES_SIZE {
                    DeploymentResult::Failed {
                        return_values,
                        execution_reverted: reverted,
                    }
                } else {
                    DeploymentResult::Successful {
                        return_values,
                        deployed_at: self.context.address,
                    }
                };

            ExecutionEnvironmentExitState::CompletedDeployment(CompletedDeployment {
                resources_returned: resources,
                deployment_result,
            })
        } else {
            ExecutionEnvironmentExitState::CompletedExecution(CompletedExecution {
                return_values,
                resources_returned: resources,
                reverted,
            })
        }
    }
}

impl<'a, S: EthereumLikeSystem> Host for ZkOSHost<'a, S> {
    type Allocator = S::Allocator;
    type Heap = [u8];

    fn heap_ref(&self) -> &Self::Heap {
        self.system.get_memory_region(MemoryRegionType::Shared)
    }
    fn heap_ref_mut(&mut self) -> &mut Self::Heap {
        self.system.get_memory_region_mut(MemoryRegionType::Shared)
    }
    fn num_heap_pages(&self) -> u32 {
        self.compute_num_heap_pages()
    }

    fn grow_heap(&mut self, new_num_pages: u32) -> Result<(), ()> {
        let new_size = new_num_pages as usize * PAGE_SIZE;
        // TODO: refine errors in iwasm
        self.system
            .grow_memory_region(MemoryRegionType::Shared, new_size)
            .map_err(|_| ())
    }

    fn copy_into_memory(&mut self, src: &[u8], offset: u32) -> Result<(), ()> {
        let shared_memory = self.heap_ref_mut();
        if shared_memory.len() < src.len() + offset as usize {
            return Err(());
        }
        shared_memory[(offset as usize)..(offset as usize + src.len())].copy_from_slice(src);

        Ok(())
    }

    fn mem_read_into_slice(&self, dst: &mut [u8], offset: u32) -> Result<(), ()> {
        let shared_memory = self.heap_ref();
        if shared_memory.len() < dst.len() + offset as usize {
            return Err(());
        }
        let len = dst.len();
        dst.copy_from_slice(&shared_memory[(offset as usize)..(offset as usize + len)]);

        Ok(())
    }

    fn fill_memory(&mut self, byte: u8, offset: u32, len: u32) -> Result<(), ()> {
        let shared_memory = self.heap_ref_mut();
        if shared_memory.len() < len as usize + offset as usize {
            return Err(());
        }
        shared_memory[(offset as usize)..(offset as usize + len as usize)].fill(byte);

        Ok(())
    }

    fn copy_memory(&mut self, src_offset: u32, dst_offset: u32, len: u32) -> Result<(), ()> {
        let shared_memory = self.heap_ref_mut();
        if shared_memory.len() < src_offset as usize + len as usize {
            return Err(());
        }
        if shared_memory.len() < dst_offset as usize + len as usize {
            return Err(());
        }

        unsafe {
            core::ptr::copy(
                shared_memory.get_unchecked_mut(src_offset as usize) as *mut u8,
                shared_memory.get_unchecked_mut(dst_offset as usize) as *mut u8,
                len as usize,
            )
        }

        Ok(())
    }

    fn num_imported_globals(&self) -> usize {
        self.import_context.globals.len()
    }
    fn num_imported_tables(&self) -> usize {
        self.import_context.tables.len()
    }
    fn num_imported_functions(&self) -> usize {
        EXPECTED_IMPORTED_FUNCTIONS.len()
    }

    fn add_imporable_global(
        &mut self,
        _module: &str,
        _global_name: &str,
        _global_type: GlobalType,
    ) -> Result<(), ()> {
        unreachable!();
    }

    fn add_imporable_table(
        &mut self,
        _module: &str,
        _table_name: &str,
        _table_type: ValueType,
        _limits: Limits,
    ) -> Result<(), ()> {
        unreachable!();
    }

    fn add_host_function<TT: ValueTypeVec>(
        &mut self,
        _module: &str,
        _func_name: &str,
        _abi: FunctionType<TT>,
        _fn_ptr: fn(
            &mut ZkOSHost<'a, S>,
            &mut [StackValue],
            usize,
        ) -> Result<ExecutionResult, InterpreterError>,
    ) -> Result<(), ()> {
        unreachable!("ZkOS host should only have pre-set functions");

        // let fn_ptr = unsafe { core::mem::transmute(fn_ptr) };
        // self.import_context.host_functions.push(fn_ptr);
        // self.import_context
        //     .fn_abis
        //     .push(FunctionType::from_other_type(&abi));
        // let module_as_vec = module.as_bytes().to_vec_in(self.system.get_allocator());
        // let func_name = func_name.as_bytes().to_vec_in(self.system.get_allocator());
        // self.import_context
        //     .host_function_names
        //     .push((module_as_vec, func_name));

        // Ok(())
    }

    fn verify_importable_global(
        &mut self,
        _at_index: u16,
        _module: &str,
        _global_name: &str,
        _global_type: &GlobalType,
    ) -> Result<(), ()> {
        self.import_context.globals.push(StackValue::empty());

        Ok(())
    }

    fn verify_importable_table(
        &mut self,
        _at_index: u16,
        _module: &str,
        _table_name: &str,
        _table_type: &ValueType,
        limits: &Limits,
    ) -> Result<(), ()> {
        let lower_bound = limits.lower_bound();
        self.import_context.tables.push((
            *limits,
            Vec::with_capacity_in(lower_bound as usize, self.system.get_allocator()),
        ));

        Ok(())
    }
    fn link_importable_function<TT: ValueTypeVec>(
        &mut self,
        at_index: u16,
        abi: &FunctionType<TT>,
        module: &str,
        func_name: &str,
    ) -> Result<(), InterpreterError> {
        let (i, (declared_module, _, expected_abi)) = EXPECTED_IMPORTED_FUNCTIONS
            .iter()
            .enumerate()
            .find(|(_, (_, n, _))| func_name == *n)
            .ok_or(())?;

        if expected_abi.inputs.as_ref() != abi.inputs.as_ref() {
            return Err(().into());
        }
        if expected_abi.outputs.as_ref() != abi.outputs.as_ref() {
            return Err(().into());
        }

        if module != *declared_module {
            return Err(().into());
        }

        self.import_context.host_functions_idx_map[at_index as usize] = i as u16;

        Ok(())
    }

    fn get_global(&self, index: u16) -> StackValue {
        self.import_context.globals[index as usize]
    }
    fn set_global(&mut self, index: u16, value: StackValue) {
        self.import_context.globals[index as usize] = value;
    }

    fn get_table_value(&self, table_index: u32, index: u32) -> Result<StackValue, ()> {
        let (limits, table) = &self.import_context.tables[table_index as usize];
        if index > limits.upper_bound_inclusive() {
            return Err(());
        }

        if index as usize >= table.len() {
            return Err(());
        }

        let value = table[index as usize];

        Ok(value)
    }
    fn set_table_value(
        &mut self,
        table_index: u32,
        index: u32,
        value: StackValue,
    ) -> Result<(), ()> {
        let (limits, table) = &mut self.import_context.tables[table_index as usize];
        if index > limits.upper_bound_inclusive() {
            return Err(());
        }

        if index as usize >= table.len() {
            table.resize(index as usize + 1, StackValue::new_nullref());
        }
        table[index as usize] = value;

        Ok(())
    }

    fn call_host_function(
        &mut self,
        func_idx: u16,
        stack_top_for_input_output: &mut [StackValue],
        num_inputs: usize,
    ) -> Result<ExecutionResult, InterpreterError> {
        let host_ix = self.import_context.host_functions_idx_map[func_idx as usize];
        let fn_ptr = self.get_host_functions()[host_ix as usize];
        (fn_ptr)(self, stack_top_for_input_output, num_inputs)
    }

    fn print(&self, args: Arguments) {
        let _ = self.system.get_logger().write_fmt(args);
    }
}
