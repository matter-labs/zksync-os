use core::fmt::Arguments;

use super::host::Host;
use super::instantiate::FunctionCallFrame;
use super::stack_value::*;
use crate::constants::*;
use crate::parsers::*;
use crate::routines::memory::*;
use crate::routines::InterpreterError;
use crate::types::*;
use crate::utils;

use super::instantiate::ExecutionResult;

const DEBUG_INSTRUCTIONS: bool = false;
const DEBUG_CALLS: bool = true;

pub(crate) struct OwnedModuleData<M: SystemMemoryManager> {
    pub(crate) datas: M::ScratchSpace<DataSection>,
    pub(crate) elements: M::ScratchSpace<ElementSection>,
    pub(crate) globals: M::ScratchSpace<StackValue>,
    pub(crate) tables: M::ScratchSpace<(Limits, M::ScratchSpace<StackValue>)>,
    pub(crate) memory_is_imported: bool,
}

pub struct SourceRefs<B: IWasmBaseSourceParser<Error = !>, P: IWasmParser<B, Error = !>> {
    pub full_bytecode: P,
    pub fn_full_source: P,
    pub src: P,
    _marker: core::marker::PhantomData<B>,
}

#[derive(Clone, Copy, Debug)]
pub struct SourceRefsPos {
    pub fn_source_offset: u32,
    pub fn_source_len: u32,
    pub src_offset: u32,
}

impl<B: IWasmBaseSourceParser<Error = !>, P: IWasmParser<B, Error = !>> SourceRefs<B, P> {
    pub fn new_rewinded(full_bytecode: P, offset: usize, len: usize) -> Self {
        let mut src = full_bytecode.clone();
        let Ok(_) = src.skip_bytes(offset);
        let Ok(src) = src.create_subparser(len);

        Self {
            full_bytecode,
            fn_full_source: src.clone(),
            src,
            _marker: core::marker::PhantomData,
        }
    }

    pub fn rewind_to_function(&mut self, start: usize, len: usize) {
        let mut src = self.full_bytecode.clone();
        let Ok(_) = src.skip_bytes(start);
        let Ok(src) = src.create_subparser(len);
        self.fn_full_source = src.clone();
        self.src = src;
    }

    pub fn rewind_to_pos_in_current_function(&mut self, ip: usize) {
        let mut src = self.fn_full_source.clone();
        let Ok(_) = src.skip_bytes(ip);
        self.src = src;
    }

    pub fn get_return_pc(&self) -> u32 {
        let start_mark = self.fn_full_source.inner_ref().get_start_mark();
        let return_pc = unsafe { self.src.inner_ref().absolute_offset(start_mark) };

        return_pc
    }

    pub fn save_pos(&self) -> SourceRefsPos {
        let absolute_start = self.full_bytecode.inner_ref().get_start_mark();
        let fn_source_offset = unsafe {
            self.fn_full_source
                .inner_ref()
                .absolute_offset(absolute_start)
        };
        let fn_source_len = self.fn_full_source.remaining_len() as u32;
        let src_offset =
            unsafe { self.src.inner_ref().absolute_offset(absolute_start) - fn_source_offset };

        SourceRefsPos {
            fn_source_offset,
            fn_source_len,
            src_offset,
        }
    }

    pub fn restore(full_bytecode: P, saved_pos: SourceRefsPos) -> Self {
        let mut src = full_bytecode.clone();
        let Ok(_) = src.skip_bytes(saved_pos.fn_source_offset as usize);
        let Ok(fn_full_source) = src.create_subparser(saved_pos.fn_source_len as usize);
        let mut src = fn_full_source.clone();
        let Ok(_) = src.skip_bytes(saved_pos.src_offset as usize);

        Self {
            full_bytecode,
            fn_full_source,
            src,
            _marker: core::marker::PhantomData,
        }
    }
}

pub struct ModuleInstance<
    M: SystemMemoryManager,
    VT: ValueTypeVec,
    C0: ContinuousIndexAccess<FunctionType<VT>>,
    C1: ContinuousIndexAccess<FunctionDef>,
    C2: ContinuousIndexAccess<FunctionBody<M::Allocator>>,
    C3: ContinuousIndexAccess<FunctionName>,
> {
    pub memory_definition: MemoryLimits,
    // NOTE on values below: we only vaguely split here,
    // roughly half of the values will come from pre-parsed module and are common,
    // and half - are specific for a function and will be owned

    // both stacks need to be "growable"
    pub callstack: M::ScratchSpace<FunctionCallFrame>,
    // stack is untyped as we expect prevalidated code
    pub stack: M::ScratchSpace<StackValue>,

    // even though elements below do NOT have to be owned/growable (they will be made on instantiation),
    // we still consider them so for simplicity

    // for a case if we will have passive segments
    pub datas: M::ScratchSpace<DataSection>,
    // for a case if we will have passive segments
    pub elements: M::ScratchSpace<ElementSection>,
    // locally declared globals to use
    pub globals: M::ScratchSpace<StackValue>,
    // locally declared tables
    pub tables: M::ScratchSpace<(Limits, M::ScratchSpace<StackValue>)>,
    // num imported functions
    pub num_imported_functions: u16,
    // num imported tables
    pub num_imported_tables: u16,
    // num imported globals
    pub num_imported_globals: u16,
    // if memory is imported
    pub memory_is_imported: bool,
    // all ABI types
    pub function_types: C0,
    // mapping of function index to ABI
    pub function_defs: C1,
    // function bodies metainformation if local function
    pub function_bodies: C2,
    pub function_names: C3,

    pub _marker: core::marker::PhantomData<VT>,
}

impl<
        M: SystemMemoryManager,
        VT: ValueTypeVec,
        C0: ContinuousIndexAccess<FunctionType<VT>>,
        C1: ContinuousIndexAccess<FunctionDef>,
        C2: ContinuousIndexAccess<FunctionBody<M::Allocator>>,
        C3: ContinuousIndexAccess<FunctionName>,
    > ModuleInstance<M, VT, C0, C1, C2, C3>
{
    pub fn reset(&mut self) {
        self.stack.clear();
        self.callstack.clear();
    }

    #[allow(clippy::result_unit_err)]
    pub fn run_function_by_index<
        B: IWasmBaseSourceParser<Error = !>,
        P: IWasmParser<B, Error = !>,
        BB: ContinuousIndexAccess<RawSideTableEntry> + Copy,
        H: Host,
    >(
        &'_ mut self,
        sidetable: BB,
        index: u16,
        input: &[StackValue],
        full_bytecode: P,
        host: &'_ mut H,
    ) -> Result<ExecutionResult, ()> {
        assert!(self.stack.is_empty());
        assert!(self.callstack.is_empty());

        if index < self.num_imported_functions {
            return Err(());
        }
        let function_def = unsafe { *self.function_defs.get_unchecked(index as usize) };
        let func_idx = index - self.num_imported_functions;
        let body = self.function_bodies.get(func_idx as usize).ok_or(())?;
        let total_locals = body.total_locals;
        let abi = unsafe {
            self.function_types
                .get_unchecked(function_def.abi_index as usize)
        };
        if abi.inputs.as_ref().len() != input.len() {
            return Err(());
        }
        let initial_sidetable = body.initial_sidetable_idx;

        let function_call_frame = FunctionCallFrame {
            return_pc: 0,
            function_idx: index,
            frame_start: input.len() as u32,
            inputs_locals_start: 0,
            sidetable_idx: initial_sidetable as i32,
            num_locals: total_locals,
        };

        let offset = body.instruction_pointer as usize;
        let len = body.end_instruction_pointer - body.instruction_pointer;

        let mut src_ref = SourceRefs::new_rewinded(full_bytecode, offset, len as usize);

        // copy initial values
        for el in input.iter() {
            self.stack.push_1(*el)?;
        }
        // then put locals in the same storage
        self.stack
            .put_many(StackValue::empty(), total_locals as usize)?;

        // active data sections should be processed already
        self.push_callstack_value(function_call_frame)?;

        self.interpreter_loop(&mut src_ref, sidetable, host)
            .map_err(|x| x.into())
    }

    pub fn prepare_to_run_function_by_index<
        B: IWasmBaseSourceParser<Error = !>,
        P: IWasmParser<B, Error = !>,
        F: Fn(Arguments),
    >(
        &'_ mut self,
        index: u16,
        input: &[StackValue],
        full_bytecode: P,
        print_fn: F,
    ) -> Result<SourceRefs<B, P>, InterpreterError> {
        assert!(self.stack.is_empty());
        assert!(self.callstack.is_empty());

        if index < self.num_imported_functions {
            return Err(().into());
        }
        let function_def = unsafe { *self.function_defs.get_unchecked(index as usize) };
        let func_idx = index - self.num_imported_functions;
        let body = self.function_bodies.get(func_idx as usize).ok_or(())?;
        let total_locals = body.total_locals;
        let abi = unsafe {
            self.function_types
                .get_unchecked(function_def.abi_index as usize)
        };
        if abi.inputs.as_ref().len() != input.len() {
            return Err(().into());
        }
        let initial_sidetable = body.initial_sidetable_idx;

        let function_call_frame = FunctionCallFrame {
            return_pc: 0,
            function_idx: index,
            frame_start: input.len() as u32,
            inputs_locals_start: 0,
            sidetable_idx: initial_sidetable as i32,
            num_locals: total_locals,
        };

        let offset = body.instruction_pointer as usize;
        let len = body.end_instruction_pointer - body.instruction_pointer;

        let src_ref = SourceRefs::new_rewinded(full_bytecode, offset, len as usize);

        // copy initial values
        for el in input.iter() {
            self.stack.push_1(*el)?;
        }
        // then put locals in the same storage
        self.stack
            .put_many(StackValue::empty(), total_locals as usize)?;

        // active data sections should be processed already
        self.push_callstack_value(function_call_frame)?;

        if cfg!(feature = "testing") {
            print_fn(format_args!(
                "IWASM: Prepared to run: {} ({})\n",
                // Safety: Binary is validated.
                unsafe { self.function_names.get_unchecked(index as usize).name },
                index,
            ));
        }

        Ok(src_ref)
    }

    pub fn dump_returnvalues(&self) -> &[StackValue] {
        unsafe { self.stack.get_slice_unchecked(0..self.stack.len()) }
    }

    fn get_abi_by_function_idx(&self, index: u16) -> &FunctionType<VT> {
        unsafe {
            let function_def = self.function_defs.get_unchecked(index as usize);
            self.function_types
                .get_unchecked(function_def.abi_index as usize)
        }
    }

    fn get_abi(&self) -> &FunctionType<VT> {
        let index = self.get_current_frame().function_idx;
        self.get_abi_by_function_idx(index)
    }

    fn push_callstack_value(&mut self, function_call_frame: FunctionCallFrame) -> Result<(), ()> {
        if self.callstack.len() == MAX_CALL_FRAMES_STACK_DEPTH {
            return Err(());
        }
        self.callstack.push(function_call_frame)?;

        Ok(())
    }

    fn get_current_frame(&self) -> &FunctionCallFrame {
        unsafe { self.callstack.last_unchecked() }
    }

    fn get_current_frame_mut(&mut self) -> &mut FunctionCallFrame {
        unsafe { self.callstack.last_unchecked_mut() }
    }

    fn pop_frame(&mut self) {
        self.callstack.pop().unwrap();
    }

    // #[allow(dead_code)]
    // fn unwind(&mut self) {
    //     let locals_start = self.get_current_frame().inputs_locals_start;
    //     self.stack.truncate(locals_start as usize);
    //     self.pop_frame();
    //     let new_current = self.get_current_frame();
    //     let body = unsafe {
    //         self.function_bodies
    //             .get_unchecked(new_current.function_idx as usize)
    //     };

    //     let body_start = body.instruction_pointer;
    //     let instructions_len = body.end_instruction_pointer - body.instruction_pointer;
    //     let new_pc = new_current.return_pc;

    //     let mut src = self.full_bytecode.clone();
    //     let Ok(_) = src.skip_bytes(body_start as usize);
    //     let Ok(mut src) = src.create_subparser(instructions_len as usize);
    //     self.fn_full_source = src.clone();
    //     let Ok(_) = src.skip_bytes(new_pc as usize);
    //     self.src = src;
    // }

    fn pop_value(&mut self) -> StackValue {
        debug_assert!(!self.stack.is_empty());
        unsafe { self.stack.pop().unwrap_unchecked() }
    }

    fn push_value(&mut self, value: StackValue) -> Result<(), ()> {
        if self.stack.len() == MAX_STACK_SIZE {
            return Err(());
        }
        self.stack.push_1(value)?;

        Ok(())
    }

    fn execute_control_flow_from_sidetable<
        B: IWasmBaseSourceParser<Error = !>,
        P: IWasmParser<B, Error = !>,
        BB: ContinuousIndexAccess<RawSideTableEntry> + Copy,
    >(
        &mut self,
        src_ref: &mut SourceRefs<B, P>,
        sidetable: BB,
    ) {
        let sidetable_idx = self.get_current_frame().sidetable_idx;
        let sidetable = unsafe { sidetable.get_unchecked(sidetable_idx as usize) };
        // we can copy top "num_copied" and should also pop extra "num popped"

        // first manipulate the stack
        if sidetable.num_popped != 0 {
            if sidetable.num_copied != 0 {
                // we need to copy backwards
                let src_start = self.stack.len() - sidetable.num_copied as usize;
                let dst_start = self.stack.len()
                    - sidetable.num_copied as usize
                    - sidetable.num_popped as usize;

                unsafe {
                    core::ptr::copy(
                        self.stack.get_unchecked(src_start) as *const StackValue,
                        self.stack.get_unchecked_mut(dst_start) as *mut StackValue,
                        sidetable.num_copied as usize,
                    )
                }
            }

            // then trim stack
            let new_len = self.stack.len() - sidetable.num_popped as usize;
            self.stack.truncate(new_len);
        }
        // then update IP and sidetable

        src_ref.rewind_to_pos_in_current_function(sidetable.next_ip as usize);

        let current_frame = self.get_current_frame_mut();
        current_frame.sidetable_idx += sidetable.next_sidetable_entry_delta;
    }

    pub fn interpreter_loop<
        B: IWasmBaseSourceParser<Error = !>,
        P: IWasmParser<B, Error = !>,
        BB: ContinuousIndexAccess<RawSideTableEntry> + Copy,
        H: Host,
    >(
        &mut self,
        src_ref: &mut SourceRefs<B, P>,
        sidetable: BB,
        host: &'_ mut H,
    ) -> Result<ExecutionResult, InterpreterError> {
        if cfg!(feature = "testing") {
            host.print(format_args!(
                "\n{}..:: Wasm interpretation start ::..{}\n",
                utils::colors::GREEN,
                utils::colors::RESET
            ));
            host.print(format_args!("NOTE: Stack pointer data assumes it's stored in global[0]. This assumption isn't checked by the interpreter, so make sure it's placed there."));
            host.print(format_args!("  SP: {:0x?}\n", unsafe {
                self.globals.get_unchecked(0)
            }));
        }

        let r = self.interpreter_loop_impl(src_ref, sidetable, host);

        if cfg!(feature = "testing") && r.is_err() {
            self.print_calltrace(host);
            self.print_counter_state(host, src_ref);
        }

        if cfg!(feature = "testing") {
            host.print(format_args!(
                "{}․.:: Wasm interpretation finished ::..{}\n\n",
                utils::colors::GREEN,
                utils::colors::RESET
            ));
        }

        r
    }

    pub fn interpreter_loop_impl<
        B: IWasmBaseSourceParser<Error = !>,
        P: IWasmParser<B, Error = !>,
        BB: ContinuousIndexAccess<RawSideTableEntry> + Copy,
        H: Host,
    >(
        &mut self,
        src_ref: &mut SourceRefs<B, P>,
        sidetable: BB,
        host: &'_ mut H,
    ) -> Result<ExecutionResult, InterpreterError> {
        let mut finished = false;

        // host.print(format_args!("WASM:\n"));
        let _sm = src_ref.src.inner().get_start_mark();

        for cycle in 0..MAX_CYCLES_PER_PROGRAM {
            if src_ref.src.is_empty() {
                if cfg!(feature = "testing") && DEBUG_CALLS {
                    host.print(format_args!(
                        "{}wasm [{}]:{} fn end ",
                        utils::colors::GREEN,
                        cycle,
                        utils::colors::RESET
                    ));
                }
                self.handle_return(src_ref, host);

                if self.callstack.is_empty() {
                    if cfg!(feature = "testing") {
                        host.print(format_args!(
                            "\nInterpreter: executed opcodes: {}.\n",
                            cycle
                        ));
                    }
                    finished = true;
                    break;
                }
            }

            let Ok(next_opcode) = src_ref.src.inner().read_byte();

            if cfg!(feature = "testing") && DEBUG_INSTRUCTIONS {
                let absolute_start = src_ref.full_bytecode.inner().get_start_mark();
                let absolute_offset =
                    unsafe { src_ref.src.inner().absolute_offset(absolute_start) - 1 };
                host.print(format_args!(
                    "{} - 0x{:08x} - 0x{:02x}: ",
                    cycle, absolute_offset, next_opcode
                ));
            }

            // if cycle == 4194303 {
            //     host.print(format_args!("breakpoint"));
            // }
            //

            // panic!("next opcode {}", next_opcode);

            match next_opcode {
                0x00 => {
                    // unreachable, so we trap
                    // self.unwind();
                    return Err(().into());
                }
                0x01 => {
                    // nop
                }
                0x02 => {
                    // block. Actually do nothing, but we need to read and advance source
                    src_ref.src.skip_blocktype();
                }
                0x03 => {
                    // loop. Actually do nothing, but we need to read and advance source
                    src_ref.src.skip_blocktype();
                }
                0x04 => {
                    // if block. Almost do nothing, but we need to read and advance source
                    src_ref.src.skip_blocktype();
                    let value = self.pop_value().as_i32();
                    if value != 0 {
                        // we are taking "IF" branch, so we go "close" and do not need to jump via sidetable
                        self.get_current_frame_mut().sidetable_idx += 1;
                    } else {
                        // we are taking "ELSE" branch and go "far"
                        self.execute_control_flow_from_sidetable(src_ref, sidetable);
                    }
                }
                0x05 => {
                    // `else` label. For all our purposes it's the END of "IF" block and nothing more
                    self.execute_control_flow_from_sidetable(src_ref, sidetable);
                }
                0x0b => {
                    // `end` of either block, loop, if-block or if-else-end construction
                }
                0x0c => {
                    // br l
                    let Ok(_depth) = src_ref.src.inner().parse_leb_u32();
                    self.execute_control_flow_from_sidetable(src_ref, sidetable);
                }
                0x0d => {
                    // br_if l
                    let Ok(_depth) = src_ref.src.inner().parse_leb_u32();
                    let value = self.pop_value().as_i32();
                    if value != 0 {
                        self.execute_control_flow_from_sidetable(src_ref, sidetable);
                    } else {
                        self.get_current_frame_mut().sidetable_idx += 1;
                    }
                }
                0x0e => {
                    // br_table l* lN
                    let branch = self.pop_value().as_i32() as u32;
                    let Ok(num_branches) = src_ref.src.inner().parse_leb_u32();
                    for _ in 0..num_branches {
                        let Ok(_depth) = src_ref.src.inner().parse_leb_u32();
                    }
                    let Ok(_l_n) = src_ref.src.inner().parse_leb_u32();
                    let branch = if branch >= num_branches {
                        num_branches
                    } else {
                        branch
                    };
                    self.get_current_frame_mut().sidetable_idx += branch as i32;
                    self.execute_control_flow_from_sidetable(src_ref, sidetable);
                }
                0x0f => {
                    // return
                    if cfg!(feature = "testing") && DEBUG_CALLS {
                        host.print(format_args!(
                            "{}wasm [{}]:{} return ",
                            utils::colors::GREEN,
                            cycle,
                            utils::colors::RESET
                        ));
                    }

                    self.handle_return(src_ref, host);
                    if self.callstack.is_empty() {
                        finished = true;
                        break;
                    }
                }
                0x10 => {
                    // call
                    let Ok(func_idx) = src_ref.src.inner().parse_leb_u32();
                    if func_idx > u16::MAX as u32 {
                        return Err(().into());
                    }

                    if cfg!(feature = "testing") && DEBUG_CALLS {
                        host.print(format_args!(
                            "{}wasm [{}]:{} call ",
                            utils::colors::GREEN,
                            cycle,
                            utils::colors::RESET
                        ));
                    }

                    let call_result = self.handle_call(src_ref, func_idx as u16, host)?;

                    if let Some(call_result) = call_result {
                        return Ok(call_result);
                    }
                }
                0x11 => {
                    // call_indirect

                    let value = self.pop_value().as_i32();

                    let Ok(_type_idx) = src_ref.src.inner().parse_leb_u32();
                    let Ok(table_idx) = src_ref.src.inner().parse_leb_u32();

                    if cfg!(feature = "testing") && DEBUG_CALLS {
                        host.print(format_args!(
                            "{}wasm [{}]:{} call.i ",
                            utils::colors::GREEN,
                            cycle,
                            utils::colors::RESET
                        ));
                    }

                    let may_be_func_ref = self.get_table_value(table_idx, value as u32, host)?;
                    let Ok(func_idx) = may_be_func_ref.get_func_from_ref() else {
                        // self.unwind();
                        return Err(().into());
                    };
                    let call_result = self.handle_call(src_ref, func_idx, host)?;
                    if let Some(call_result) = call_result {
                        return Ok(call_result);
                    }
                }
                0x1a => {
                    // drop
                    let _ = self.pop_value();
                }
                0x1b => {
                    // select
                    let condition = self.pop_value().as_i32();
                    let t2 = self.pop_value();
                    let t1 = self.pop_value();

                    if condition != 0 {
                        self.push_value(t1)?;
                    } else {
                        self.push_value(t2)?;
                    }
                }
                0x1c => {
                    // select *
                    // unsupported for now

                    // self.unwind();
                    return Err(().into());
                }
                0x20..=0x24 => {
                    self.process_globals_or_locals_instruction(next_opcode, src_ref, host)?;
                }
                0x25 => {
                    // table.get x
                    let Ok(table_idx) = src_ref.src.inner().parse_leb_u32();
                    let in_table_idx = self.pop_value().as_i32() as u32;
                    let value = if table_idx < self.num_imported_tables as u32 {
                        host.get_table_value(table_idx, in_table_idx)?
                    } else {
                        let table_idx = table_idx - self.num_imported_tables as u32;
                        let (_limits, entries) =
                            unsafe { self.tables.get_unchecked(table_idx as usize) };
                        if (in_table_idx as usize) < entries.len() {
                            return Err(().into());
                        }
                        unsafe { *entries.get_unchecked(in_table_idx as usize) }
                    };
                    self.push_value(value)?;
                }
                0x26 => {
                    // table.set x
                    let Ok(table_idx) = src_ref.src.inner().parse_leb_u32();
                    let ref_type_value = self.pop_value();
                    let in_table_idx = self.pop_value().as_i32() as u32;
                    if table_idx < self.num_imported_tables as u32 {
                        host.set_table_value(table_idx, in_table_idx, ref_type_value)?;
                    } else {
                        let table_idx = table_idx - self.num_imported_tables as u32;
                        let (_limits, entries) =
                            unsafe { self.tables.get_unchecked_mut(table_idx as usize) };
                        if (in_table_idx as usize) < entries.len() {
                            return Err(().into());
                        }
                        let el = unsafe { entries.get_unchecked_mut(in_table_idx as usize) };
                        *el = ref_type_value;
                    };
                }
                0x28..=0x3e => {
                    // memory
                    let Ok(_alignment) = src_ref.src.inner().parse_leb_u32();
                    let Ok(offset) = src_ref.src.inner().parse_leb_u32();
                    self.execute_mem_load_store(next_opcode, offset, host)?;
                }
                0x3f => {
                    // memory.size
                    let Ok(_next) = src_ref.src.inner().read_byte();

                    let size = host.num_heap_pages();
                    self.push_value(StackValue::new_i32(size as i32))?;
                }
                0x40 => {
                    // memory.grow
                    let Ok(_next) = src_ref.src.inner().read_byte();

                    let upper_memory_page_bound = self.memory_definition.max_pages_inclusive();
                    if upper_memory_page_bound == 0 {
                        // hard error, no memory
                        host.print(format_args!("No mem."));
                        return Err(().into());
                    }

                    let grow_by_num_pages = self.pop_value().as_i32() as u32;
                    if grow_by_num_pages > u16::MAX as u32 {
                        host.print(format_args!("\nmemory.grow u16 overflow.\n"));
                        self.push_value(StackValue::new_i32(-1))?;
                        continue;
                    }
                    let grow_by_num_pages = grow_by_num_pages as u16;
                    let current_size = host.num_heap_pages() as u16;
                    let Some(new_size) = current_size.checked_add(grow_by_num_pages) else {
                        host.print(format_args!("\nmemory.grow cur size overflow.\n"));
                        self.push_value(StackValue::new_i32(-1))?;
                        continue;
                    };
                    // can not overflow the memory limit
                    if new_size > upper_memory_page_bound {
                        host.print(format_args!("\nmemory.grow limit overflow.\n"));
                        self.push_value(StackValue::new_i32(-1))?;
                        continue;
                    }
                    host.print(format_args!(
                        "\ngrowing memory: {}, limit {}\n",
                        new_size, upper_memory_page_bound
                    ));
                    // now we can finally grow
                    host.grow_heap(new_size as u32)?;
                    self.push_value(StackValue::new_i32(current_size as u32 as i32))?;
                }
                0x41 => {
                    // i32 const
                    let Ok(value) = src_ref.src.inner().parse_leb_s32();
                    self.push_value(StackValue::new_i32(value))?;
                }
                0x42 => {
                    // i64 const
                    let Ok(value) = src_ref.src.inner().parse_leb_s64();
                    self.push_value(StackValue::new_i64(value))?;
                }
                0x45 => {
                    // i32 testop
                    let value = self.pop_value().as_i32();
                    self.push_value(StackValue::new_i32((value == 0) as i32))?;
                }
                0x46..=0x4f => {
                    // i32 relop
                    let op2 = self.pop_value().as_i32();
                    let op1 = self.pop_value().as_i32();
                    let result = match next_opcode {
                        0x46 => {
                            // eq
                            op1 == op2
                        }
                        0x47 => {
                            // ne
                            op1 != op2
                        }
                        0x48 => {
                            // lt.s
                            op1 < op2
                        }
                        0x49 => {
                            // lt.u
                            (op1 as u32) < op2 as u32
                        }
                        0x4a => {
                            // gt.s
                            op1 > op2
                        }
                        0x4b => {
                            // gt.u
                            (op1 as u32) > op2 as u32
                        }
                        0x4c => {
                            // le.s
                            op1 <= op2
                        }
                        0x4d => {
                            // le.u
                            (op1 as u32) <= op2 as u32
                        }
                        0x4e => {
                            // ge.s
                            op1 >= op2
                        }
                        0x4f => {
                            // ge.u
                            (op1 as u32) >= op2 as u32
                        }
                        _ => unsafe { core::hint::unreachable_unchecked() },
                    };

                    self.push_value(StackValue::new_i32(result as i32))?;
                }
                0x50 => {
                    // i64 testop
                    let value = self.pop_value().as_i64();
                    self.push_value(StackValue::new_i32((value == 0) as i32))?;
                }
                0x51..=0x5a => {
                    // i64 relop
                    let op2 = self.pop_value().as_i64();
                    let op1 = self.pop_value().as_i64();
                    let result = match next_opcode {
                        0x51 => {
                            // eq
                            op1 == op2
                        }
                        0x52 => {
                            // ne
                            op1 != op2
                        }
                        0x53 => {
                            // lt.s
                            op1 < op2
                        }
                        0x54 => {
                            // lt.u
                            (op1 as u64) < op2 as u64
                        }
                        0x55 => {
                            // gt.s
                            op1 > op2
                        }
                        0x56 => {
                            // gt.u
                            (op1 as u64) > op2 as u64
                        }
                        0x57 => {
                            // le.s
                            op1 <= op2
                        }
                        0x58 => {
                            // le.u
                            (op1 as u64) <= op2 as u64
                        }
                        0x59 => {
                            // ge.s
                            op1 >= op2
                        }
                        0x5a => {
                            // ge.u
                            (op1 as u64) >= op2 as u64
                        }
                        _ => unsafe { core::hint::unreachable_unchecked() },
                    };

                    self.push_value(StackValue::new_i32(result as i32))?;
                }
                0x67..=0x69 => {
                    // i32 unop
                    let value = self.pop_value().as_i32();
                    let result = match next_opcode {
                        0x67 => {
                            // clz
                            let result = value.leading_zeros();
                            result as i32
                        }
                        0x68 => {
                            // ctz
                            let result = value.trailing_zeros();
                            result as i32
                        }
                        0x69 => {
                            // popcnt
                            let result = value.count_ones();
                            result as i32
                        }
                        _ => unsafe { core::hint::unreachable_unchecked() },
                    };
                    self.push_value(StackValue::new_i32(result))?;
                }
                0x6a..=0x78 => {
                    // i32 binop
                    let op2 = self.pop_value().as_i32();
                    let op1 = self.pop_value().as_i32();
                    let result = match next_opcode {
                        0x6a => op1.wrapping_add(op2),
                        0x6b => op1.wrapping_sub(op2),
                        0x6c => op1.wrapping_mul(op2),
                        0x6d => {
                            if op2 == 0 {
                                return Err(().into());
                            }
                            let (result, of) = op1.overflowing_div(op2);
                            if of {
                                // undefined, so let's error
                                return Err(().into());
                            }

                            result
                        }
                        0x6e => {
                            if op2 == 0 {
                                return Err(().into());
                            }
                            ((op1 as u32) / (op2 as u32)) as i32
                        }
                        0x6f => {
                            if op2 == 0 {
                                return Err(().into());
                            }
                            if op1 == i32::MIN && op2 == -1i32 {
                                0
                            } else {
                                op1 % op2
                            }
                        }
                        0x70 => {
                            if op2 == 0 {
                                return Err(().into());
                            }
                            ((op1 as u32) % (op2 as u32)) as i32
                        }
                        0x71 => op1 & op2,
                        0x72 => op1 | op2,
                        0x73 => op1 ^ op2,
                        0x74 => {
                            let shift = op2 % 32;
                            op1.wrapping_shl(shift as u32)
                        }
                        0x75 => {
                            let shift = op2 % 32;
                            op1.wrapping_shr(shift as u32)
                        }
                        0x76 => {
                            let shift = op2 % 32;
                            (op1 as u32).wrapping_shr(shift as u32) as i32
                        }
                        0x77 => {
                            let rot = op2 % 32;
                            op1.rotate_left(rot as u32)
                        }
                        0x78 => {
                            let rot = op2 % 32;
                            op1.rotate_right(rot as u32)
                        }
                        _ => unsafe { core::hint::unreachable_unchecked() },
                    };
                    self.push_value(StackValue::new_i32(result))?;
                }
                0x79..=0x7b => {
                    // i64 unop
                    let value = self.pop_value().as_i64();
                    let result = match next_opcode {
                        0x79 => {
                            // clz
                            let result = value.leading_zeros();
                            result as i64
                        }
                        0x7a => {
                            // ctz
                            let result = value.trailing_zeros();
                            result as i64
                        }
                        0x7b => {
                            // popcnt
                            let result = value.count_ones();
                            result as i64
                        }
                        _ => unsafe { core::hint::unreachable_unchecked() },
                    };
                    self.push_value(StackValue::new_i64(result))?;
                }
                0x7c..=0x8a => {
                    // i64 binop
                    let op2 = self.pop_value().as_i64();
                    let op1 = self.pop_value().as_i64();
                    let result = match next_opcode {
                        0x7c => op1.wrapping_add(op2),
                        0x7d => op1.wrapping_sub(op2),
                        0x7e => op1.wrapping_mul(op2),
                        0x7f => {
                            if op2 == 0 {
                                return Err(().into());
                            }
                            let (result, of) = op1.overflowing_div(op2);
                            if of {
                                // undefined, so let's error
                                return Err(().into());
                            }

                            result
                        }
                        0x80 => {
                            if op2 == 0 {
                                return Err(().into());
                            }
                            ((op1 as u64) / (op2 as u64)) as i64
                        }
                        0x81 => {
                            if op2 == 0 {
                                return Err(().into());
                            }
                            if op1 == i64::MIN && op2 == -1i64 {
                                0
                            } else {
                                op1 % op2
                            }
                        }
                        0x82 => {
                            if op2 == 0 {
                                return Err(().into());
                            }
                            ((op1 as u64) % (op2 as u64)) as i64
                        }
                        0x83 => op1 & op2,
                        0x84 => op1 | op2,
                        0x85 => op1 ^ op2,
                        0x86 => {
                            let shift = op2 % 64;
                            op1.wrapping_shl(shift as u32)
                        }
                        0x87 => {
                            let shift = op2 % 64;
                            op1.wrapping_shr(shift as u32)
                        }
                        0x88 => {
                            let shift = op2 % 64;
                            (op1 as u64).wrapping_shr(shift as u32) as i64
                        }
                        0x89 => {
                            let rot = op2 % 64;
                            op1.rotate_left(rot as u32)
                        }
                        0x8a => {
                            let rot = op2 % 64;
                            op1.rotate_right(rot as u32)
                        }
                        _ => unsafe { core::hint::unreachable_unchecked() },
                    };
                    self.push_value(StackValue::new_i64(result))?;
                }
                0xa7 => {
                    // i32.wrap_i64
                    let value = self.pop_value().as_i64() as i32;
                    self.push_value(StackValue::new_i32(value))?;
                }
                0xac..=0xad => {
                    // i64.extend_i32
                    let op = self.pop_value().as_i32();
                    let result = match next_opcode {
                        0xac => {
                            // i64.extend_i32_s
                            op as i64
                        }
                        0xad => {
                            // i64.extend_i32_u
                            op as u32 as u64 as i64
                        }
                        _ => unsafe { core::hint::unreachable_unchecked() },
                    };
                    self.push_value(StackValue::new_i64(result))?;
                }
                0xc0..=0xc1 => {
                    // i32.extend
                    let op = self.pop_value().as_i32();
                    let result = match next_opcode {
                        0xc0 => {
                            // i32.extend_8s
                            op as u32 as u8 as i8 as i32
                        }
                        0xc1 => {
                            // i32.extend_16s
                            op as u32 as u16 as i16 as i32
                        }
                        _ => unsafe { core::hint::unreachable_unchecked() },
                    };
                    self.push_value(StackValue::new_i32(result))?;
                }
                0xc2..=0xc4 => {
                    // i64.extend
                    let op = self.pop_value().as_i64();
                    let result = match next_opcode {
                        0xc2 => {
                            // i64.extend_8s
                            op as u64 as u8 as i8 as i64
                        }
                        0xc3 => {
                            // i64.extend_16s
                            op as u64 as u16 as i16 as i64
                        }
                        0xc4 => {
                            // i64.extend_32s
                            op as u64 as u32 as i32 as i64
                        }
                        _ => unsafe { core::hint::unreachable_unchecked() },
                    };
                    self.push_value(StackValue::new_i64(result))?;
                }
                0xfc => {
                    // multibyte opcodes, we want to support only a limited set of them
                    let Ok(instr_idx) = src_ref.src.inner().parse_leb_u32();

                    match instr_idx {
                        8 => {
                            // memory.init
                            if self.memory_definition.max_pages_inclusive() == 0 {
                                return Err(().into());
                            }

                            let Ok(data_idx) = src_ref.src.inner().parse_leb_u32();
                            let Ok(_end_byte) = src_ref.src.inner().read_byte();

                            let len = self.pop_value().as_i32() as u32;
                            let src_offset = self.pop_value().as_i32() as u32;
                            let dst_offset = self.pop_value().as_i32() as u32;

                            let src_end = src_offset.checked_add(len).ok_or(())?;

                            let data_section =
                                unsafe { self.datas.get_unchecked(data_idx as usize) };
                            let section_len = data_section.len();
                            if section_len == 0 && (src_offset != 0 || len != 0) {
                                return Err(().into());
                            }
                            if src_end > section_len {
                                return Err(().into());
                            }

                            // below we will handle a check if our dst range doesn't fit

                            let (start, _) = data_section.as_range();
                            let start = start.checked_add(src_offset).ok_or(())? as usize;
                            let mut src_slice = src_ref.full_bytecode.clone();
                            let Ok(_) = src_slice.skip_bytes(start);
                            let Ok(src) = src_slice.inner().read_slice(len);
                            Host::copy_into_memory(host, src, dst_offset)?;
                        }
                        9 => {
                            // data.drop
                            let Ok(data_idx) = src_ref.src.inner().parse_leb_u32();
                            unsafe { self.datas.get_unchecked_mut(data_idx as usize).drop() };
                        }
                        10 => {
                            // memory.copy
                            if self.memory_definition.max_pages_inclusive() == 0 {
                                return Err(().into());
                            }
                            let Ok(_end_byte) = src_ref.src.inner().read_byte();
                            let Ok(_end_byte) = src_ref.src.inner().read_byte();

                            let len = self.pop_value().as_i32() as u32;
                            let src_offset = self.pop_value().as_i32() as u32;
                            let dst_offset = self.pop_value().as_i32() as u32;

                            Host::copy_memory(host, src_offset, dst_offset, len)?;
                        }
                        11 => {
                            // memory.fill
                            if self.memory_definition.max_pages_inclusive() == 0 {
                                return Err(().into());
                            }
                            let Ok(_end_byte) = src_ref.src.inner().read_byte();

                            let size = self.pop_value().as_i32() as u32;
                            let byte_value = self.pop_value().as_i32().to_le_bytes()[0];
                            let offset = self.pop_value().as_i32() as u32;

                            Host::fill_memory(host, byte_value, offset, size)?;
                        }
                        // 12 => {
                        //     // table.init
                        //     // extended table manipulation
                        //     let _elem_idx = self.src.read_u32()?;
                        //     let table_idx = self.src.read_u32()?;
                        //     if table_idx as usize >= self.tables.len() {
                        //         return Err(().into())
                        //     }
                        //     // we do not support passive element sections
                        //     return Err(().into())
                        // }
                        // 13 => {
                        //     // elem.drop
                        //     // extended table manipulation
                        //     let _elem_idx = self.src.read_u32()?;
                        //     // we do not support passive element sections
                        //     return Err(().into())
                        // }
                        // 14 => {
                        //     // table.copy
                        //     // extended table manipulation
                        //     let _elem_idx = self.src.read_u32()?;
                        //     let table_idx = self.src.read_u32()?;
                        //     if table_idx as usize >= self.tables.len() {
                        //         return Err(().into())
                        //     }
                        //     // we do not support passive element sections
                        //     return Err(().into())
                        // }
                        // 15 => {
                        //     // table.grow
                        //     // extended table manipulation
                        //     let table_idx = self.src.read_u32()?;
                        //     if table_idx as usize >= self.tables.len() {
                        //         return Err(().into())
                        //     }
                        //     self.pop_expected_value_type(ValueType::I32)?;
                        //     let (value_type, _limits) = &self.tables[table_idx as usize];
                        //     self.pop_expected_value_type(*value_type)?;
                        //     self.push_value_type(ValueType::I32)?;
                        // }
                        // 16 => {
                        //     // table.size
                        //     let table_idx = self.src.read_u32()?;
                        //     if table_idx as usize >= self.tables.len() {
                        //         return Err(().into())
                        //     }
                        //     self.push_value_type(ValueType::I32)?;
                        // }
                        // 17 => {
                        //     // table.fill
                        //     let table_idx = self.src.read_u32()?;
                        //     if table_idx as usize >= self.tables.len() {
                        //         return Err(().into())
                        //     }
                        //     let (value_type, _limits) = &self.tables[table_idx as usize];
                        //     let _n = self.pop_expected_value_type(ValueType::I32)?;
                        //     self.pop_expected_value_type(*value_type)?;
                        //     let _i = self.pop_expected_value_type(ValueType::I32)?;
                        //     // since it's a cycle internally, we only consider exiting condition,
                        //     // that pushes nothing after it (n == 0)
                        // }
                        _ => {
                            panic!("multibyte inst 0x{:08x}", instr_idx);
                        }
                    }
                }
                0xfd => {
                    // vector instructions, not supported
                    return Err(().into());
                }
                _ => {
                    // println!("Unknown opcode 0x{:02x}", next_opcode);
                    return Err(().into());
                }
            }

            if cfg!(feature = "testing") && DEBUG_INSTRUCTIONS {
                host.print(format_args!("\n"));
            }
        }

        // host.print(format_args!("WASM complete"));

        if finished {
            Ok(ExecutionResult::Return)
        } else {
            Ok(ExecutionResult::DidNotComplete)
        }
    }

    fn handle_return<B: IWasmBaseSourceParser<Error = !>, P: IWasmParser<B, Error = !>, H: Host>(
        &mut self,
        src_ref: &mut SourceRefs<B, P>,
        host: &H,
    ) {
        let num_values_to_copy = self.get_abi().outputs.as_ref().len();
        let locals_start = self.get_current_frame().inputs_locals_start;

        if cfg!(feature = "testing") && DEBUG_CALLS {
            unsafe {
                host.print(format_args!(
                    "{}\n",
                    self.function_names
                        .get_unchecked(self.callstack.last_unchecked().function_idx as usize)
                        .name
                ))
            };

            host.print(format_args!("  SP: {:08x?}\n", unsafe {
                self.globals.get_unchecked(0)
            }));

            for i in self.stack.len() - num_values_to_copy..self.stack.len() {
                host.print(format_args!("  ret: {:0x?}\n", self.stack.get(i).unwrap()));
            }
        }

        // we can not just truncate, but copy
        if num_values_to_copy != 0 {
            if locals_start as usize + num_values_to_copy != self.stack.len() {
                // we need to copy backwards

                // let mut src_idx = self.stack.len() - num_values_to_copy;
                // let mut dst_idx = locals_start as usize;
                // for _ in 0..num_values_to_copy {
                //     unsafe {
                //         let value = *self.stack.get_unchecked(src_idx);
                //         *self.stack.get_unchecked_mut(dst_idx) = value;
                //     }
                //     src_idx += 1;
                //     dst_idx += 1;
                // }

                let src_start = self.stack.len() - num_values_to_copy;
                let dst_start = locals_start as usize;
                // since we expect scratch spaces to decay into slices, we can do a continuous copy
                unsafe {
                    core::ptr::copy(
                        self.stack.get_unchecked(src_start) as *const StackValue,
                        self.stack.get_unchecked_mut(dst_start) as *mut StackValue,
                        num_values_to_copy,
                    )
                }
            } else {
                // we do not even need to unwind, and we will leave return values
                // on the stack
            }
        }

        self.stack
            .truncate(locals_start as usize + num_values_to_copy);
        self.pop_frame();
        if !self.callstack.is_empty() {
            let new_current = self.get_current_frame();
            let local_fn_idx = new_current.function_idx - self.num_imported_functions;
            let body = unsafe { self.function_bodies.get_unchecked(local_fn_idx as usize) };

            let body_start = body.instruction_pointer;
            let instructions_len = body.end_instruction_pointer - body.instruction_pointer;
            let new_pc = new_current.return_pc;

            src_ref.rewind_to_function(body_start as usize, instructions_len as usize);
            src_ref.rewind_to_pos_in_current_function(new_pc as usize);
        }
    }

    fn handle_call<B: IWasmBaseSourceParser<Error = !>, P: IWasmParser<B, Error = !>, H: Host>(
        &mut self,
        src_ref: &mut SourceRefs<B, P>,
        index: u16,
        host: &'_ mut H,
    ) -> Result<Option<ExecutionResult>, InterpreterError> {
        if index < self.num_imported_functions {
            // inputs are already on the stack, so grow for outputs if needed
            let function_def = unsafe { self.function_defs.get_unchecked(index as usize) };
            let abi = unsafe {
                self.function_types
                    .get_unchecked(function_def.abi_index as usize)
            };
            let num_inputs = abi.inputs.as_ref().len();
            let num_outputs = abi.outputs.as_ref().len();
            if num_outputs > num_inputs {
                let grow_by = num_outputs - num_inputs;
                self.stack.put_many(StackValue::empty(), grow_by)?;
            }

            if cfg!(feature = "testing") && DEBUG_CALLS {
                host.print(format_args!(
                    "{}{}{} (exported)({})\n",
                    utils::colors::BRIGHT_ORANGE,
                    // Safety: binary verified
                    unsafe { self.function_names.get_unchecked(index as usize).name },
                    utils::colors::RESET,
                    index,
                ));

                host.print(format_args!("  SP: {:08x?}\n", unsafe {
                    self.globals.get_unchecked(0)
                }));

                for i in 0..num_inputs {
                    host.print(format_args!("  arg: {:0x?}\n", unsafe {
                        self.stack.get_unchecked(i)
                    }));
                }
            }

            let start = self.stack.len() - core::cmp::max(num_inputs, num_outputs);
            let end = self.stack.len();
            let call_region = unsafe { self.stack.get_slice_unchecked_mut(start..end) };
            let host_function_result = host.call_host_function(index, call_region, num_inputs)?;
            if num_outputs < num_inputs {
                let new_len = self.stack.len() - num_inputs + num_outputs;
                self.stack.truncate(new_len);
            }
            if host_function_result != ExecutionResult::Continue {
                Ok(Some(host_function_result))
            } else {
                Ok(None)
            }
        } else {
            // we need to reserve space on the stack for locals of the next function,
            // and give control
            let function_def = unsafe { self.function_defs.get_unchecked(index as usize) };
            let func_idx = index - self.num_imported_functions;
            let body = self.function_bodies.get(func_idx as usize).ok_or(())?;
            let total_locals = body.total_locals;
            let initial_sidetable = body.initial_sidetable_idx as i32;
            let abi = unsafe {
                self.function_types
                    .get_unchecked(function_def.abi_index as usize)
            };
            // Make borrowcheck happy
            let num_inputs = abi.inputs.as_ref().len();
            let instruction_pointer = body.instruction_pointer;
            let end_instruction_pointer = body.end_instruction_pointer;

            let return_pc = src_ref.get_return_pc();
            self.get_current_frame_mut().return_pc = return_pc;
            let frame_start = self.stack.len();

            let inputs_locals_start = frame_start - num_inputs;

            let function_call_frame = FunctionCallFrame {
                return_pc: 0,
                function_idx: index,
                frame_start: frame_start as u32,
                inputs_locals_start: inputs_locals_start as u32,
                sidetable_idx: initial_sidetable,
                num_locals: total_locals,
            };

            if cfg!(feature = "testing") && DEBUG_CALLS {
                host.print(format_args!(
                    "{} ({})\n",
                    // Safety: binary verified
                    unsafe { self.function_names.get_unchecked(index as usize).name },
                    index,
                ));

                host.print(format_args!("  SP: {:08x?}\n", unsafe {
                    self.globals.get_unchecked(0)
                }));

                for i in 0..num_inputs {
                    host.print(format_args!("  arg: {:0x?}\n", unsafe {
                        self.stack.get_unchecked(i)
                    }));
                }
            }

            let instructions_len = end_instruction_pointer - instruction_pointer;

            src_ref.rewind_to_function(instruction_pointer as usize, instructions_len as usize);

            // then put locals in the same storage
            self.stack
                .put_many(StackValue::empty(), total_locals as usize)?;

            // active data sections should be processed already
            self.push_callstack_value(function_call_frame)?;

            Ok(None)
        }
    }

    fn get_local<H: Host>(&self, index: usize, _host: &H) -> StackValue {
        let current_frame = self.get_current_frame();
        unsafe {
            #[cfg(feature = "testing")]
            if current_frame.inputs_locals_start as usize + index >= self.stack.len() {
                use core::fmt::Write;

                let mut str = alloc::string::String::new();

                str.write_fmt(format_args!("IWASM: get_local out of bounds:\n"))
                    .expect("Write panic msg.");
                str.write_fmt(format_args!("Callstack:\n"))
                    .expect("Write panic msg.");
                for i in self.callstack.iter() {
                    str.write_fmt(format_args!(
                        "  {}\n",
                        self.function_names
                            .get(i.function_idx as usize)
                            .expect("Function index to have a name record.")
                            .name
                    ))
                    .expect("Write panic msg.");
                }
                panic!("{}", str.as_str());
            }

            *self
                .stack
                .get_unchecked(current_frame.inputs_locals_start as usize + index)
        }
    }

    fn set_local<H: Host>(&mut self, index: usize, value: StackValue, _host: &H) {
        let inputs_locals_start = self.get_current_frame().inputs_locals_start;

        unsafe {
            *self
                .stack
                .get_unchecked_mut(inputs_locals_start as usize + index) = value;
        }
    }

    fn get_global<H: Host>(&self, index: usize, host: &'_ mut H) -> StackValue {
        if index < self.num_imported_globals as usize {
            host.get_global(index as u16)
        } else {
            let local_idx = index - self.num_imported_globals as usize;
            unsafe { *self.globals.get_unchecked(local_idx) }
        }
    }

    fn set_global<H: Host>(&mut self, index: usize, value: StackValue, host: &'_ mut H) {
        if cfg!(feature = "testing") && index == 0 && value.as_i32() as u32 > 1 << 26 {
            host.print(format_args!(
                "\n!! Stack underflow. Current value 0x{:08x?}. Note: this is a (very reasonable) assumption, the actual check is `SP > 1 << 26. I mean this is 64Mb. Not only it's waaay beyond the heap base address, but also who in the right mind allocates 64Mb in crypto? Anyhow, either make the contract consume less stack space or increase it in the compilation options (./build.sh or cargo.toml).`\n",
                value.as_i32()
            ));
        }

        if index < self.num_imported_globals as usize {
            host.set_global(index as u16, value);
        } else {
            let local_idx = index - self.num_imported_globals as usize;
            unsafe {
                *self.globals.get_unchecked_mut(local_idx) = value;
            }
        }
    }

    fn get_table_value<H: Host>(
        &mut self,
        table_idx: u32,
        in_table_idx: u32,
        host: &'_ mut H,
    ) -> Result<StackValue, ()> {
        if table_idx < self.num_imported_tables as u32 {
            host.get_table_value(table_idx, in_table_idx)
        } else {
            let table_idx = table_idx - self.num_imported_tables as u32;
            let (_limits, entries) = unsafe { self.tables.get_unchecked(table_idx as usize) };
            if (in_table_idx as usize) >= entries.len() {
                return Err(());
            }
            unsafe { Ok(*entries.get_unchecked(in_table_idx as usize)) }
        }
    }

    fn process_globals_or_locals_instruction<
        B: IWasmBaseSourceParser<Error = !>,
        P: IWasmParser<B, Error = !>,
        H: Host,
    >(
        &mut self,
        instr: u8,
        src_ref: &mut SourceRefs<B, P>,
        host: &'_ mut H,
    ) -> Result<(), ()> {
        match instr {
            0x20 => {
                // local.get idx
                let Ok(index) = src_ref.src.inner().parse_leb_u32();
                let value = self.get_local(index as usize, host);
                self.push_value(value)?;
            }
            0x21 => {
                // local.set idx
                let Ok(index) = src_ref.src.inner().parse_leb_u32();
                let value = self.pop_value();
                self.set_local(index as usize, value, host);
            }
            0x22 => {
                // local.tee idx
                let Ok(index) = src_ref.src.inner().parse_leb_u32();
                let value = self.pop_value();
                self.push_value(value)?;
                self.set_local(index as usize, value, host);
            }
            0x23 => {
                // global.get idx
                let Ok(index) = src_ref.src.inner().parse_leb_u32();
                let value = self.get_global(index as usize, host);
                self.push_value(value)?;
            }
            0x24 => {
                // global.set idx
                let Ok(index) = src_ref.src.inner().parse_leb_u32();
                let value = self.pop_value();
                self.set_global(index as usize, value, host);
            }
            _ => unsafe { core::hint::unreachable_unchecked() },
        }

        Ok(())
    }

    fn execute_mem_load_store<H: Host>(
        &mut self,
        instr: u8,
        offset: u32,
        host: &'_ mut H,
    ) -> Result<(), InterpreterError> {
        let byte_size = match instr {
            0x28 | 0x34 | 0x35 | 0x36 | 0x3e => 4,
            0x29 | 0x37 => 8,
            0x2c | 0x2d | 0x30 | 0x31 | 0x3a | 0x3c => 1,
            0x2e | 0x2f | 0x32 | 0x33 | 0x3b | 0x3d => 2,
            _ => return Err(().into()),
        };

        match instr {
            0x28 | 0x2c | 0x2d | 0x2e | 0x2f => {
                // load i32
                let address = self.pop_value().as_i32() as u32;
                let offset = address.checked_add(offset).ok_or(())?;
                let mut buffer = [0u8; 4];
                Host::mem_read_into_buffer(host, &mut buffer, offset, byte_size)?;
                // either full load, or unsigned load, so by having top bytes to be 0
                // we effectively do unsigned zero-extend
                let mut value = i32::from_le_bytes(buffer);
                match instr {
                    0x2c => {
                        // signed single byte
                        if value > 0x7f {
                            // sign extend
                            value |= -1i32 << 24;
                        }
                    }
                    0x2e => {
                        // signed single byte
                        if value > 0x7fff {
                            // sign extend
                            value |= -1i32 << 16;
                        }
                    }
                    _ => {}
                };
                self.push_value(StackValue::new_i32(value))?;
            }
            0x29 | 0x30 | 0x31 | 0x32 | 0x33 | 0x34 | 0x35 => {
                // load i64
                let address = self.pop_value().as_i32() as u32;
                let offset = address.checked_add(offset).ok_or(())?;
                let mut buffer = [0u8; 8];
                Host::mem_read_into_buffer(host, &mut buffer, offset, byte_size)?;
                // either full load, or unsigned load, so by having top bytes to be 0
                // we effectively do unsigned zero-extend
                let mut value = i64::from_le_bytes(buffer);
                match instr {
                    0x30 => {
                        // signed single byte
                        if value > 0x7f {
                            // sign extend
                            value |= -1i64 << 24;
                        }
                    }
                    0x32 => {
                        // signed single byte
                        if value > 0x7fff {
                            // sign extend
                            value |= -1i64 << 16;
                        }
                    }
                    0x34 => {
                        // signed single byte
                        if value > 0x7fffffff {
                            // sign extend
                            value |= -1i64 << 32;
                        }
                    }
                    _ => {}
                }

                self.push_value(StackValue::new_i64(value))?;
            }
            0x36 | 0x3a | 0x3b => {
                // store i32
                let value = self.pop_value().as_i32().to_le_bytes();

                let address = self.pop_value().as_i32() as u32;
                let offset = address.checked_add(offset).unwrap();
                // .ok_or(InterpreterError::new(
                //     alloc::format!(
                //         "Ptr arithmetics overflow: addr 0x{:08x?}, offset 0x{:08x?}",
                //         address, offset
                // )))?;

                let src = match instr {
                    0x36 => &value[..],
                    0x3a => &value[0..1],
                    0x3b => &value[0..2],
                    _ => unsafe {
                        core::hint::unreachable_unchecked();
                    },
                };

                Host::copy_into_memory(host, src, offset)?;
            }
            0x37 | 0x3c | 0x3d | 0x3e => {
                // store i64
                let value = self.pop_value().as_i64().to_le_bytes();

                let address = self.pop_value().as_i32() as u32;
                let offset = address.checked_add(offset).ok_or(())?;

                let src = match instr {
                    0x37 => &value[..],
                    0x3c => &value[0..1],
                    0x3d => &value[0..2],
                    0x3e => &value[0..4],
                    _ => unsafe {
                        core::hint::unreachable_unchecked();
                    },
                };

                Host::copy_into_memory(host, src, offset)?;
            }
            _ => unsafe { core::hint::unreachable_unchecked() },
        }

        Ok(())
    }

    fn print_calltrace<H: Host>(&self, host: &H) {
        host.print(format_args!("Calltrace:\n"));

        for e in self.callstack.iter() {
            unsafe {
                host.print(format_args!(
                    "  {}\n",
                    self.function_names
                        .get_unchecked(e.function_idx as usize)
                        .name
                ))
            };
        }
    }

    fn print_counter_state<
        H: Host,
        B: IWasmBaseSourceParser<Error = !>,
        P: IWasmParser<B, Error = !>,
    >(
        &self,
        host: &H,
        src_ref: &mut SourceRefs<B, P>,
    ) {
        let absolute_start = src_ref.full_bytecode.inner().get_start_mark();
        let absolute_offset = unsafe { src_ref.src.inner().absolute_offset(absolute_start) - 1 };

        host.print(format_args!("PC: 0x{:08x}\n", absolute_offset));
    }
}
