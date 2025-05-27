use super::super::memory::*;
use super::instantiate::ExecutionResult;
use super::stack_value::*;
use crate::routines::InterpreterError;
use crate::{constants::PAGE_SIZE, types::*};
use alloc::alloc::Global;
use alloc::string::String;
use alloc::vec::Vec;
use core::alloc::Allocator;
use core::fmt::Arguments;

pub trait Host {
    type Allocator: Allocator + Clone;
    type Heap: ?Sized;

    fn heap_ref(&self) -> &Self::Heap;
    fn heap_ref_mut(&mut self) -> &mut Self::Heap;

    fn num_heap_pages(&self) -> u32;
    #[allow(clippy::result_unit_err)]
    fn grow_heap(&mut self, new_num_pages: u32) -> Result<(), ()>;
    #[allow(clippy::result_unit_err)]
    fn copy_into_memory(&mut self, src: &[u8], offset: u32) -> Result<(), ()>;
    #[allow(clippy::result_unit_err)]
    fn mem_read_into_buffer<const N: usize>(
        &self,
        dst: &mut [u8; N],
        offset: u32,
        num_bytes: u32,
    ) -> Result<(), ()> {
        Self::mem_read_into_slice(self, &mut dst[..(num_bytes as usize)], offset)
    }
    #[allow(clippy::result_unit_err)]
    fn mem_read_into_slice(&self, dst: &mut [u8], offset: u32) -> Result<(), ()>;
    #[allow(clippy::result_unit_err)]
    fn fill_memory(&mut self, byte: u8, offset: u32, len: u32) -> Result<(), ()>;
    #[allow(clippy::result_unit_err)]
    fn copy_memory(&mut self, src_offset: u32, dst_offset: u32, len: u32) -> Result<(), ()>;

    fn num_imported_globals(&self) -> usize;
    fn num_imported_tables(&self) -> usize;
    fn num_imported_functions(&self) -> usize;

    #[allow(clippy::result_unit_err)]
    fn add_imporable_global(
        &mut self,
        module: &str,
        global_name: &str,
        global_type: GlobalType,
    ) -> Result<(), ()>;
    #[allow(clippy::result_unit_err)]
    fn add_imporable_table(
        &mut self,
        module: &str,
        table_name: &str,
        table_type: ValueType,
        limits: Limits,
    ) -> Result<(), ()>;
    #[allow(clippy::result_unit_err)]
    fn add_host_function<T: ValueTypeVec>(
        &mut self,
        module: &str,
        func_name: &str,
        abi: FunctionType<T>,
        fn_ptr: fn(
            &mut Self,
            &mut [StackValue],
            usize,
        ) -> Result<ExecutionResult, InterpreterError>,
    ) -> Result<(), ()>;

    #[allow(clippy::result_unit_err)]
    fn verify_importable_global(
        &mut self,
        at_index: u16,
        module: &str,
        global_name: &str,
        global_type: &GlobalType,
    ) -> Result<(), ()>;
    #[allow(clippy::result_unit_err)]
    fn verify_importable_table(
        &mut self,
        at_index: u16,
        module: &str,
        table_name: &str,
        table_type: &ValueType,
        limits: &Limits,
    ) -> Result<(), ()>;
    fn link_importable_function<T: ValueTypeVec>(
        &mut self,
        at_index: u16,
        abi: &FunctionType<T>,
        module: &str,
        func_name: &str,
    ) -> Result<(), InterpreterError>;

    fn get_global(&self, index: u16) -> StackValue;
    fn set_global(&mut self, index: u16, value: StackValue);

    #[allow(clippy::result_unit_err)]
    fn get_table_value(&self, table_index: u32, index: u32) -> Result<StackValue, ()>;
    #[allow(clippy::result_unit_err)]
    fn set_table_value(
        &mut self,
        table_index: u32,
        index: u32,
        value: StackValue,
    ) -> Result<(), ()>;

    fn call_host_function(
        &mut self,
        func_idx: u16,
        stack_top_for_input_output: &mut [StackValue],
        num_inputs: usize,
    ) -> Result<ExecutionResult, InterpreterError>;

    fn print(&self, args: Arguments);
}

#[allow(clippy::type_complexity)]
pub struct TrivialHost<T = ()> {
    pub heap: Vec<Vec<u8>>,
    pub globals: Vec<StackValue>,
    pub tables: Vec<(Limits, Vec<StackValue>)>,
    pub host_functions:
        Vec<fn(&mut Self, &mut [StackValue], usize) -> Result<ExecutionResult, InterpreterError>>,
    pub fn_abis: Vec<FunctionType<ValueTypeArray>>,
    pub host_function_names: Vec<(String, String)>,
    pub context: T,
}

impl TrivialHost<()> {
    pub fn empty() -> Self {
        Self {
            heap: Vec::new(),
            globals: Vec::new(),
            tables: Vec::new(),
            host_functions: Vec::new(),
            fn_abis: Vec::new(),
            host_function_names: Vec::new(),
            context: (),
        }
    }
}

impl<T> Default for TrivialHost<T>
where
    T: Default,
{
    fn default() -> Self {
        Self {
            heap: Vec::new(),
            globals: Vec::new(),
            tables: Vec::new(),
            host_functions: Vec::new(),
            fn_abis: Vec::new(),
            host_function_names: Vec::new(),
            context: T::default(),
        }
    }
}

impl<T> TrivialHost<T> {
    pub fn new_with_context(context: T) -> Self {
        Self {
            heap: Vec::new(),
            globals: Vec::new(),
            tables: Vec::new(),
            host_functions: Vec::new(),
            fn_abis: Vec::new(),
            host_function_names: Vec::new(),
            context,
        }
    }

    pub fn set_context(&mut self, context: T) {
        self.context = context;
    }
}

impl<T> Host for TrivialHost<T> {
    type Allocator = Global;
    type Heap = Vec<Vec<u8>>;

    fn heap_ref(&self) -> &Self::Heap {
        &self.heap
    }

    fn heap_ref_mut(&mut self) -> &mut Self::Heap {
        &mut self.heap
    }

    fn num_heap_pages(&self) -> u32 {
        self.heap.len() as u32
    }

    fn grow_heap(&mut self, new_num_pages: u32) -> Result<(), ()> {
        let mut page = Vec::with_capacity_in(PAGE_SIZE, Global);
        page.resize(PAGE_SIZE, 0u8);
        self.heap.resize(new_num_pages as usize, page);
        Ok(())
    }

    fn copy_into_memory(&mut self, src: &[u8], offset: u32) -> Result<(), ()> {
        let mem_len = PAGE_SIZE * self.heap.len();
        let src_len = u32::try_from(src.len()).map_err(|_| ())?;
        let end = src_len.checked_add(offset).ok_or(())?;
        if end > mem_len as u32 {
            return Err(());
        }

        if src_len == 0 {
            return Ok(());
        }

        let mut src_offset = 0_usize;
        let mut dst_offset = offset as usize;
        let mut len = src_len as usize;

        // we will proceed by identifying per-page subranges that can be copied with just "copy_from_slice"
        loop {
            let dst_page = dst_offset / PAGE_SIZE;
            let dst_in_page_offset = dst_offset % PAGE_SIZE;
            let dst_in_page_len = PAGE_SIZE - dst_offset % PAGE_SIZE;

            let len_to_copy = core::cmp::min(dst_in_page_len, len);

            // we have to do ptr::copy due to borrow checker
            unsafe {
                core::ptr::copy(
                    src[src_offset..].as_ptr(),
                    self.heap[dst_page][dst_in_page_offset..].as_mut_ptr(),
                    len_to_copy,
                );
            }

            src_offset += len_to_copy;
            dst_offset += len_to_copy;
            len -= len_to_copy;
            if len == 0 {
                break;
            }
        }

        Ok(())
    }

    fn mem_read_into_slice(&self, dst: &mut [u8], offset: u32) -> Result<(), ()> {
        let num_bytes = dst.len() as u32;
        let mem_len = PAGE_SIZE * self.heap.len();
        let end = offset.checked_add(num_bytes).ok_or(())?;
        if end > mem_len as u32 {
            return Err(());
        }

        if num_bytes == 0 {
            return Ok(());
        }

        let mut src_offset = offset as usize;
        let mut dst_offset = 0_usize;
        let mut len = num_bytes as usize;

        // we will proceed by identifying per-page subranges that can be copied with just "copy_from_slice"
        loop {
            let src_in_page_len = PAGE_SIZE - src_offset % PAGE_SIZE;
            let len_to_copy = core::cmp::min(src_in_page_len, len);

            let src_page = src_offset / PAGE_SIZE;
            let src_in_page_offset = src_offset % PAGE_SIZE;

            // we have to do ptr::copy due to borrow checker
            unsafe {
                core::ptr::copy(
                    self.heap[src_page][src_in_page_offset..].as_ptr(),
                    dst[dst_offset..].as_mut_ptr(),
                    len_to_copy,
                );
            }

            src_offset += len_to_copy;
            dst_offset += len_to_copy;
            len -= len_to_copy;
            if len == 0 {
                break;
            }
        }

        Ok(())
    }

    fn fill_memory(&mut self, byte: u8, offset: u32, len: u32) -> Result<(), ()> {
        let mem_len = PAGE_SIZE * self.heap.len();
        let end = offset.checked_add(len).ok_or(())?;
        if end > mem_len as u32 {
            return Err(());
        }

        if len == 0 {
            return Ok(());
        }

        let mut dst_offset = offset as usize;
        let mut len = len as usize;

        // we will proceed by identifying per-page subranges that can be copied with just "copy_from_slice"
        loop {
            let dst_in_page_len = PAGE_SIZE - dst_offset % PAGE_SIZE;
            let len_to_copy = core::cmp::min(dst_in_page_len, len);

            let dst_page = dst_offset / PAGE_SIZE;
            let dst_in_page_offset = dst_offset % PAGE_SIZE;

            self.heap[dst_page][dst_in_page_offset..][..len_to_copy].fill(byte);

            dst_offset += len_to_copy;
            len -= len_to_copy;
            if len == 0 {
                break;
            }
        }

        Ok(())
    }

    fn copy_memory(&mut self, src_offset: u32, dst_offset: u32, len: u32) -> Result<(), ()> {
        let mem_len = PAGE_SIZE * self.heap.len();
        let src_end = src_offset.checked_add(len).ok_or(())?;
        if src_end > mem_len as u32 {
            return Err(());
        }

        let dst_end = dst_offset.checked_add(len).ok_or(())?;
        if dst_end > mem_len as u32 {
            return Err(());
        }

        if len == 0 {
            return Ok(());
        }

        let mut src_offset = src_offset as usize;
        let mut dst_offset = dst_offset as usize;
        let mut len = len as usize;

        // we will proceed by identifying per-page subranges that can be copied with just "copy_from_slice"
        loop {
            let src_in_page_len = PAGE_SIZE - src_offset % PAGE_SIZE;
            let dst_in_page_len = PAGE_SIZE - dst_offset % PAGE_SIZE;

            let len_to_copy = core::cmp::min(src_in_page_len, dst_in_page_len);
            let len_to_copy = core::cmp::min(len_to_copy, len);

            let src_page = src_offset / PAGE_SIZE;
            let src_in_page_offset = src_offset % PAGE_SIZE;
            let dst_page = dst_offset / PAGE_SIZE;
            let dst_in_page_offset = dst_offset % PAGE_SIZE;

            // we have to do ptr::copy due to borrow checker
            unsafe {
                core::ptr::copy(
                    self.heap[src_page][src_in_page_offset..].as_ptr(),
                    self.heap[dst_page][dst_in_page_offset..].as_mut_ptr(),
                    len_to_copy,
                );
            }

            src_offset += len_to_copy;
            dst_offset += len_to_copy;
            len -= len_to_copy;
            if len == 0 {
                break;
            }
        }

        Ok(())
    }

    fn num_imported_globals(&self) -> usize {
        self.globals.len()
    }
    fn num_imported_tables(&self) -> usize {
        self.tables.len()
    }
    fn num_imported_functions(&self) -> usize {
        self.host_functions.len()
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
        module: &str,
        func_name: &str,
        abi: FunctionType<TT>,
        fn_ptr: fn(
            &mut Self,
            &mut [StackValue],
            usize,
        ) -> Result<ExecutionResult, InterpreterError>,
    ) -> Result<(), ()> {
        use crate::alloc::string::ToString;

        self.host_functions.push(fn_ptr);
        self.fn_abis.push(FunctionType::from_other_type(&abi));
        self.host_function_names
            .push((module.to_string(), func_name.to_string()));

        Ok(())
    }

    fn verify_importable_global(
        &mut self,
        _at_index: u16,
        _module: &str,
        _global_name: &str,
        _global_type: &GlobalType,
    ) -> Result<(), ()> {
        self.globals.push(StackValue::empty());

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
        self.tables
            .push((*limits, Vec::with_capacity(lower_bound as usize)));

        Ok(())
    }
    fn link_importable_function<TT: ValueTypeVec>(
        &mut self,
        at_index: u16,
        abi: &FunctionType<TT>,
        module: &str,
        func_name: &str,
    ) -> Result<(), InterpreterError> {
        let expected_abi = self.fn_abis.get(at_index as usize).ok_or(())?;
        if expected_abi.inputs.as_ref() != abi.inputs.as_ref() {
            return Err(().into());
        }
        if expected_abi.outputs.as_ref() != abi.outputs.as_ref() {
            return Err(().into());
        }
        let (declared_module, declared_name) =
            self.host_function_names.get(at_index as usize).ok_or(())?;
        if module != *declared_module || func_name != *declared_name {
            return Err(().into());
        }

        Ok(())
    }

    fn get_global(&self, index: u16) -> StackValue {
        self.globals[index as usize]
    }
    fn set_global(&mut self, index: u16, value: StackValue) {
        self.globals[index as usize] = value;
    }

    fn get_table_value(&self, table_index: u32, index: u32) -> Result<StackValue, ()> {
        let (limits, table) = &self.tables[table_index as usize];
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
        let (limits, table) = &mut self.tables[table_index as usize];
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
        let fn_ptr = self.host_functions[func_idx as usize];
        (fn_ptr)(self, stack_top_for_input_output, num_inputs)
    }

    fn print(&self, _args: Arguments) {
        todo!()
    }
}
