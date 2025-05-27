use crate::constants::*;
use crate::parsers::*;
use crate::routines::memory::*;
use crate::routines::InterpreterError;
use crate::types::*;
use core::hint::unreachable_unchecked;

pub struct FunctionBodyValidator<
    'b,
    'a: 'b,
    B: IWasmBaseSourceParser<Error = ()> + 'a,
    P: IWasmParser<B> + 'a,
    M: SystemMemoryManager = (),
> {
    pub src: &'b mut P,
    pub function_body_decl: &'b mut FunctionBody<M::Allocator>,
    pub abstract_value_stack: &'b mut M::ScratchSpace<ValueType>,
    pub all_control_stack_entries: &'b mut M::ScratchSpace<ControlFlowType>,
    pub control_stack: &'b mut M::ScratchSpace<usize>,
    pub globals_decl: &'b M::ScratchSpace<GlobalType>,
    pub function_defs: &'b M::ScratchSpace<FunctionDef>,
    pub function_abis: &'b M::ScratchSpace<FunctionType<ValueTypeVecRef<'a>>>,
    pub tables: &'b M::ScratchSpace<(ValueType, Limits)>,
    pub memory: MemoryLimits,
    pub data_count: Option<u32>, // data sections follow after code section, so we can only use count
    pub max_local_from_inputs: usize,
    pub max_local: usize,
    pub abi: &'b FunctionType<ValueTypeVecRef<'a>>,
    pub sidetable_scratch: &'b mut M::ScratchSpace<SideTableEntry>,
    pub formed_sidetable: &'b mut M::OutputBuffer<RawSideTableEntry>,
    pub _marker: core::marker::PhantomData<B>,
}

impl<
        'b,
        'a: 'b,
        B: IWasmBaseSourceParser<Error = ()> + 'a,
        P: IWasmParser<B, Error = ()> + 'a,
        M: SystemMemoryManager,
    > FunctionBodyValidator<'b, 'a, B, P, M>
{
    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::result_unit_err)]
    pub fn new(
        src: &'b mut P,
        function_body_decl: &'b mut FunctionBody<M::Allocator>,
        abstract_value_stack: &'b mut M::ScratchSpace<ValueType>,
        all_control_stack_entries: &'b mut M::ScratchSpace<ControlFlowType>,
        control_stack: &'b mut M::ScratchSpace<usize>,
        globals_decl: &'b M::ScratchSpace<GlobalType>,
        function_defs: &'b M::ScratchSpace<FunctionDef>,
        function_abis: &'b M::ScratchSpace<FunctionType<ValueTypeVecRef<'a>>>,
        tables: &'b M::ScratchSpace<(ValueType, Limits)>,
        memory: MemoryLimits,
        data_count: Option<u32>,
        sidetable_scratch: &'b mut M::ScratchSpace<SideTableEntry>,
        formed_sidetable: &'b mut M::OutputBuffer<RawSideTableEntry>,
    ) -> Result<Self, ()> {
        assert!(abstract_value_stack.is_empty());
        assert!(all_control_stack_entries.is_empty());
        assert!(control_stack.is_empty());
        assert!(sidetable_scratch.is_empty());

        function_body_decl.initial_sidetable_idx = formed_sidetable.len() as u32;

        let function_def = function_defs
            .get(function_body_decl.function_def_idx as usize)
            .copied()
            .ok_or(())?;
        let abi = function_abis
            .get(function_def.abi_index as usize)
            .ok_or(())?;
        let max_local_from_inputs = abi.inputs.len();
        let max_local = max_local_from_inputs + function_body_decl.total_locals as usize;

        let initial_control_structure = ControlFlowType::Func {
            func_idx: function_body_decl.function_def_idx,
            end: function_body_decl.end_instruction_pointer
                - function_body_decl.instruction_pointer
                - 1,
            frame_start: 0,
            is_unreachable: false,
        };

        let mut new = Self {
            src,
            function_body_decl,
            abstract_value_stack,
            all_control_stack_entries,
            control_stack,
            globals_decl,
            function_defs,
            function_abis,
            tables,
            memory,
            data_count,
            max_local_from_inputs,
            max_local,
            abi,
            sidetable_scratch,
            formed_sidetable,
            _marker: core::marker::PhantomData,
        };
        let _ = new.push_control_structure(initial_control_structure)?;

        Ok(new)
    }

    fn get_abi_by_function_idx(
        &self,
        index: u32,
    ) -> Result<&FunctionType<ValueTypeVecRef<'a>>, ()> {
        let function_def = self.function_defs.get(index as usize).copied().ok_or(())?;
        self.function_abis
            .get(function_def.abi_index as usize)
            // .copied()
            .ok_or(())
    }

    fn get_abi_by_type_idx(&self, index: u32) -> Result<&FunctionType<ValueTypeVecRef<'a>>, ()> {
        self.function_abis
            .get(index as usize)
            // .copied()
            .ok_or(())
    }

    fn current_control_structure(&self) -> Result<(&ControlFlowType, usize), ()> {
        self.get_control_structure_at_depth(0)
    }

    fn get_control_structure_at_depth(&self, depth: u32) -> Result<(&ControlFlowType, usize), ()> {
        let len = self.control_stack.len();
        if len < depth as usize + 1 {
            return Err(());
        }
        unsafe {
            let idx = *self.control_stack.get_unchecked(len - depth as usize - 1);
            Ok((self.all_control_stack_entries.get_unchecked(idx), idx))
        }
    }

    fn current_control_structure_mut(&mut self) -> Result<&mut ControlFlowType, InterpreterError> {
        self.get_control_structure_at_depth_mut(0)
    }

    fn get_control_structure_at_depth_mut(
        &mut self,
        depth: u32,
    ) -> Result<&mut ControlFlowType, InterpreterError> {
        let len = self.control_stack.len();
        if len < depth as usize + 1 {
            return Err(InterpreterError::new(alloc::format!(
                "Could not get control structure: index out of bounds {} of {}",
                depth,
                len
            )));
        }
        unsafe {
            let idx = *self.control_stack.get_unchecked(len - depth as usize - 1);
            Ok(self.all_control_stack_entries.get_unchecked_mut(idx))
        }
    }

    fn next_sidetable_index(&self) -> usize {
        self.sidetable_scratch.len()
    }

    fn push_control_structure(&mut self, structure: ControlFlowType) -> Result<usize, ()> {
        let idx = self.all_control_stack_entries.len();
        self.all_control_stack_entries.push(structure)?;
        self.control_stack.push(idx)?;

        Ok(idx)
    }

    #[allow(dead_code)]
    fn pop_control_structure(&mut self) -> Result<ControlFlowType, ()> {
        let idx = self.control_stack.pop().ok_or(())?;
        unsafe {
            let entry = *self.all_control_stack_entries.get_unchecked(idx);
            Ok(entry)
        }
    }

    // that doesn't affect the value stack
    fn pop_and_discard_control_structure(&mut self) -> Result<(), ()> {
        let _idx = self.control_stack.pop().ok_or(())?;
        Ok(())
    }

    fn push_sidetable_entry(&mut self, entry: SideTableEntry) -> Result<usize, ()> {
        let idx = self.sidetable_scratch.len();
        self.sidetable_scratch.push(entry)?;

        Ok(idx)
    }

    // fn debug_print_control_stack(&self) {
    //     for el in self.control_stack.iter().rev() {
    //         dbg!(&self.all_control_stack_entries[*el]);
    //     }
    // }

    // Small note on what we try to do here:
    // - type check (obviously)
    // - compute side tables to efficiently execute the control flow when we encounter them

    // - let's use "if - else - end" example, and also explain how authors of the original articles got to stp deltas they use.
    // main idea is whenever we encounter a control flow instruction that potentially (or unconditionally) takes us "far away" - we create
    // a sidetable entry. In case of "if - else - end" when we read "if" opcode we can immediately place a sidetable entry that would mean
    // "in case of jump to `else` label we should change IP by some amount", and move in our sidetable somewhere (yet undetermined forward), but we
    // must also immediately create another sidetable entry that would either mean "some CFG walking happening during `if` branch", or in the worst
    // case - that we should to go the `end` label after we finish `if` block. There will be times when we will not know IP offset or STP offset, and
    // in this case we should use abstract label "end of block" or something like that, and after the whole function body is processed we can
    // walk over STP one more time and replace labels by actual values

    pub fn validate_function_body(&mut self) -> Result<(), InterpreterError> {
        let function_start = self.src.inner().get_start_mark();
        let mut end_found = false;
        loop {
            if self.src.is_empty() {
                break;
            }

            let ip = unsafe { self.src.inner().absolute_offset(function_start) };

            let next_opcode = self
                .src
                .inner()
                .read_byte()
                .map_err(|_| "Could not read opcode.")?;

            // println!("Next opcode = 0x{:02x}", next_opcode);

            match next_opcode {
                0x00 => {
                    // unreachable

                    // We should invalidate current block to skip return analysis
                    self.current_control_structure_mut()?.set_unreachable();
                }
                0x01 => {
                    // nop
                }
                0x02 => {
                    // block
                    let block_type = self.src.parse_blocktype()?;
                    let frame_start = self.pass_values_to_block_type(block_type)?;

                    // add control flow instruction
                    let control_flow_structure = ControlFlowType::Block {
                        ip,
                        end: None,
                        block_type,
                        frame_start: frame_start as u32,
                        next_stp_at_end: None,
                        is_unreachable: false,
                    };

                    let _ = self.push_control_structure(control_flow_structure)? as u32;
                }
                0x03 => {
                    // loop
                    let block_type = self.src.parse_blocktype()?;
                    let frame_start = self.pass_values_to_block_type(block_type)?;

                    // add control flow instruction
                    let next_stp = self.next_sidetable_index();
                    let control_flow_structure = ControlFlowType::Loop {
                        ip,
                        end: None,
                        block_type,
                        frame_start: frame_start as u32,
                        next_stp_at_start: next_stp as u32,
                        is_unreachable: false,
                    };

                    let _ = self.push_control_structure(control_flow_structure)? as u32;
                }
                0x04 => {
                    // if block

                    self.pop_expected_value_type(ValueType::I32)?;

                    // IF will create two sidetable entries:
                    // - ELSE entry that tells where to jump RIGHT NOW if we do not take IF
                    // - IF entry that should be a current one, and will tell where to jump when we encounter ELSE

                    let block_type = self.src.parse_blocktype()?;
                    let frame_start = self.pass_values_to_block_type(block_type)?;

                    // add control flow instruction
                    let control_flow_structure = ControlFlowType::IfBlock {
                        ip,
                        else_ip: None,
                        end: None,
                        block_type,
                        frame_start: frame_start as u32,
                        next_stp_at_else: None,
                        next_stp_at_end: None,
                        if_branch_is_unreachable: false,
                        else_branch_is_unreachable: false,
                    };
                    let control_flow_idx =
                        self.push_control_structure(control_flow_structure)? as u32;

                    let else_entry = SideTableEntry::ElseBranch {
                        jump_to_else_of: control_flow_idx,
                        sidetable_entries_delta_to_set: 0,
                        num_copied: 0,
                        num_popped: 0,
                    };
                    let _ = self.push_sidetable_entry(else_entry)?;
                    // and that's it, we can continue to parse body of the `if` branch
                }
                0x05 => {
                    // TODO: Pass over this branch with a debugger and comment/set error msgs.
                    // `else` label

                    // first we typecheck AND create a sidetable entry to jump to end of `if-else-end` construction
                    let (label, if_else_end_label_idx) = self.get_control_structure_at_depth(0)?;
                    let block_type = label.as_block_get_type()?;
                    let is_unreachable = label.is_unreachable();
                    let frame_start = label.get_frame_start();

                    let ControlFlowType::IfBlock { .. } = label else {
                        return Err(().into());
                    };

                    // this is an implicit end of branch, so we should check that we have right types in enough quantity
                    if !is_unreachable {
                        self.perform_end_analysis(block_type, false)?;
                    } else {
                        assert!(self.absolute_value_stack_depth() >= frame_start);
                        self.abstract_value_stack.truncate(frame_start);
                    }

                    // create a sidetable entry
                    let sidetable_entry = SideTableEntry::IfBranch {
                        jump_to_end_of_block: if_else_end_label_idx as u32,
                        sidetable_entries_delta_to_set: 0,
                        num_copied: 0,
                        num_popped: 0,
                    };
                    let _ = self.push_sidetable_entry(sidetable_entry)?;

                    let next_sidetable_index = self.next_sidetable_index() as u32;
                    // we mainly should check that we are indeed in the `if - else - end` branch, and typecheck the return
                    let control_flow_struct = self.current_control_structure_mut()?;

                    let ControlFlowType::IfBlock {
                        else_ip,
                        end,
                        next_stp_at_else,
                        next_stp_at_end,
                        ..
                    } = control_flow_struct
                    else {
                        return Err(().into());
                    };
                    if else_ip.is_some()
                        || next_stp_at_else.is_some()
                        || end.is_some()
                        || next_stp_at_end.is_some()
                    {
                        return Err(().into());
                    }
                    *else_ip = Some(ip);
                    *next_stp_at_else = Some(next_sidetable_index);

                    // and unroll frame start to the location that we had before `if`
                    if self.abstract_value_stack.len() < frame_start {
                        return Err(().into());
                    }
                    self.abstract_value_stack.truncate(frame_start);
                    // push back input values
                    match block_type {
                        BlockType::Empty => {}
                        BlockType::ValueType(..) => {}
                        BlockType::TypeIdx(type_idx) => {
                            let func_type = self.get_abi_by_type_idx(type_idx)?;
                            for value_type in func_type.inputs.types.iter().copied() {
                                self.push_value_type(value_type)?;
                            }
                        }
                    }
                }
                0x0b => {
                    // `end` of either block, loop, if-block or if-else-end construction

                    let (label, _end_of_label_idx) = self.get_control_structure_at_depth(0)?;
                    let is_unreachable = label.is_unreachable();
                    let frame_start = label.get_frame_start();
                    // we must do a shortcut here if it's end of the function
                    if let ControlFlowType::Func { .. } = label {
                        if !is_unreachable {
                            let frame_stack =
                                self.frame_value_stack_for_control_structure_at_depth(0)?;
                            let func_type = self.abi;
                            let num_to_return = func_type.outputs.len();
                            if frame_stack.len() != num_to_return {
                                return Err(().into());
                            }
                            for (expected_type, type_on_stack) in func_type
                                .outputs
                                .types
                                .iter()
                                .rev()
                                .zip(frame_stack.iter().rev())
                            {
                                if type_on_stack != expected_type {
                                    return Err(().into());
                                }
                            }
                        }

                        self.pop_and_discard_control_structure()?;
                        if !self.src.is_empty() {
                            // body ended, but we have some data
                            return Err(().into());
                        }

                        end_found = true;

                        break;
                    }

                    let block_type = label.as_block_get_type()?;
                    // special case of "if without else", but we expect to return from the block
                    let if_without_else = if let ControlFlowType::IfBlock { else_ip, .. } = label {
                        else_ip.is_none()
                    } else {
                        false
                    };

                    if !is_unreachable {
                        self.perform_end_analysis(block_type, if_without_else)?;
                    }

                    let next_sidetable_index = self.next_sidetable_index() as u32;
                    let next_ip = unsafe { self.src.inner().absolute_offset(function_start) - 1 };
                    // we mainly should check that we are indeed in the `if - else - end` branch, and typecheck the return
                    let control_flow_struct = self.current_control_structure_mut()?;
                    match control_flow_struct {
                        ControlFlowType::Func { .. } => {
                            // unreachable case, handled above
                            return Err(().into());
                        }
                        ControlFlowType::IfBlock {
                            end,
                            next_stp_at_end,
                            ..
                        } => {
                            if end.is_some() || next_stp_at_end.is_some() {
                                return Err(().into());
                            }
                            *end = Some(next_ip);
                            *next_stp_at_end = Some(next_sidetable_index);
                        }
                        ControlFlowType::Block {
                            end,
                            next_stp_at_end,
                            ..
                        } => {
                            *end = Some(next_ip);
                            *next_stp_at_end = Some(next_sidetable_index);
                        }
                        ControlFlowType::Loop { end, .. } => {
                            *end = Some(next_ip);
                        }
                    }

                    // that doesn't affect the value stack, and we have validated return values of the block above,
                    // so we just discard a control structure and stack is in a good shape
                    self.pop_and_discard_control_structure()?;

                    // if our block is in unreachalbe state it means it's stack is at start
                    if is_unreachable {
                        assert!(self.absolute_value_stack_depth() >= frame_start);
                        self.abstract_value_stack.truncate(frame_start);
                        // we need to return back return types
                        match block_type {
                            BlockType::Empty => {}
                            BlockType::ValueType(value_type) => {
                                self.push_value_type(value_type)?;
                            }
                            BlockType::TypeIdx(type_idx) => {
                                let func_type = self.get_abi_by_type_idx(type_idx)?;
                                for value_type in func_type.outputs.types.iter().copied() {
                                    self.push_value_type(value_type)?;
                                }
                            }
                        }
                    }
                }
                0x0c => {
                    // br l
                    let depth = self.src.inner().parse_leb_u32()?;
                    self.handle_break(depth, true)?;
                }
                0x0d => {
                    // br_if l
                    self.pop_expected_value_type(ValueType::I32)?;

                    let depth = self.src.inner().parse_leb_u32()?;
                    self.handle_break(depth, false)?;
                }
                0x0e => {
                    // br_table l* lN

                    self.pop_expected_value_type(ValueType::I32)?;

                    let num_branches = self.src.inner().parse_leb_u32()?;
                    if num_branches > MAX_BREAK_BRANCHES {
                        return Err(
                            alloc::format!(
                                "Maximum amount of branches for a switch exceeded: {} of {}, fn at 0x{:08x?}",
                                num_branches,
                                MAX_BREAK_BRANCHES,
                                self.function_body_decl.instruction_pointer)
                            .into());
                    }
                    let mut arity = None;
                    for _ in 0..num_branches {
                        let depth = self.src.inner().parse_leb_u32()?;
                        let control_structure = self.get_control_structure_at_depth(depth)?.0;
                        let label_types_arity = self.get_label_types_arity(control_structure)?;
                        if let Some(arity) = arity {
                            if arity != label_types_arity {
                                return Err("Inconsistent label arity in `br_table`.".into());
                            }
                        } else {
                            arity = Some(label_types_arity);
                        }
                        // basically it's an option for "normal branch"
                        self.handle_break(depth, false)?;
                    }
                    let l_n = self.src.inner().parse_leb_u32()?;
                    let control_structure = self.get_control_structure_at_depth(l_n)?.0;
                    let label_types_arity = self.get_label_types_arity(control_structure)?;
                    if let Some(arity) = arity {
                        if arity != label_types_arity {
                            return Err("Inconsistent label arity in `br_table`.".into());
                        }
                    }
                    self.handle_break(l_n, true)?;
                }
                0x0f => {
                    // return
                    if !self.current_control_structure()?.0.is_unreachable() {
                        let frame_stack =
                            self.frame_value_stack_for_control_structure_at_depth(0)?;
                        let func_type = self.abi;
                        let num_to_return = func_type.outputs.len();

                        if frame_stack.len() < num_to_return {
                            return Err("Not enough stack values to return.".into());
                        }

                        for (expected_type, type_on_stack) in func_type
                            .outputs
                            .types
                            .iter()
                            .rev()
                            .zip(frame_stack.iter().rev())
                        {
                            if type_on_stack != expected_type {
                                return Err("Wrong stack return value type.".into());
                            }
                        }
                    }
                    // We should invalidate current block to skip return analysis
                    self.current_control_structure_mut()?.set_unreachable();
                }
                0x10 => {
                    // call
                    let func_idx = self.src.inner().parse_leb_u32()?;
                    let func_type = self.get_abi_by_function_idx(func_idx)?;

                    self.handle_call(*func_type)?;
                }
                0x11 => {
                    // call_indirect

                    self.pop_expected_value_type(ValueType::I32)?;

                    let type_idx = self.src.inner().parse_leb_u32()?;
                    let table_idx = self.src.inner().parse_leb_u32()?;

                    if table_idx as usize >= self.tables.len() {
                        return Err("Call to an out of bound function index.".into());
                    }

                    let func_type = self.get_abi_by_type_idx(type_idx)?;
                    self.handle_call(*func_type)?;
                }
                0x1a => {
                    // drop
                    let _ = self.pop_value_type()?;
                }
                0x1b => {
                    // select
                    self.pop_expected_value_type(ValueType::I32)?;
                    let t1 = self.pop_value_type()?;
                    let t2 = self.pop_value_type()?;

                    if t1 != ValueType::FormalUnknown && t2 != ValueType::FormalUnknown && t1 != t2
                    {
                        return Err("Non equal types in select.".into());
                    }
                    if t1 == ValueType::FormalUnknown {
                        self.push_value_type(t2)?;
                    } else {
                        self.push_value_type(t1)?;
                    }
                }
                0x1c => {
                    // select *
                    // unsupported for now
                    return Err("Unsupported.".into());
                }
                0x20..=0x24 => {
                    self.validate_globals_or_locals_instruction(next_opcode)?;
                }
                0x25 => {
                    // table.get x
                    let table_idx = self.src.inner().parse_leb_u32()?;
                    if table_idx as usize >= self.tables.len() {
                        return Err("Out of bounds table index.".into());
                    }
                    self.pop_expected_value_type(ValueType::I32)?;
                    let (value_type, _limits) =
                        unsafe { self.tables.get_unchecked(table_idx as usize) };
                    self.push_value_type(*value_type)?;
                }
                0x26 => {
                    // table.set x
                    let table_idx = self.src.inner().parse_leb_u32()?;
                    if table_idx as usize >= self.tables.len() {
                        return Err("Out of bounds table index.".into());
                    }
                    let (value_type, _limits) =
                        unsafe { self.tables.get_unchecked(table_idx as usize) };
                    self.pop_expected_value_type(*value_type)?;
                    self.pop_expected_value_type(ValueType::I32)?;
                }
                0x28..=0x3e => {
                    // memory
                    if self.memory.max_pages_inclusive() == 0 {
                        return Err("No memory defined.".into());
                    }

                    let alignment = self.src.inner().parse_leb_u32()?;
                    let _offset = self.src.inner().parse_leb_u32()?;
                    self.validate_mem_load_store(next_opcode, alignment)?;
                }
                0x3f => {
                    // memory.size
                    let next = self.src.inner().read_byte()?;
                    if next != 0x00 {
                        return Err("Expected 0x0.".into());
                    }
                    if self.memory.max_pages_inclusive() == 0 {
                        return Err("No memory defined.".into());
                    }
                    self.push_value_type(ValueType::I32)?;
                }
                0x40 => {
                    // memory.grow
                    let next = self.src.inner().read_byte()?;
                    if next != 0x00 {
                        return Err("Expected 0x0.".into());
                    }
                    if self.memory.max_pages_inclusive() == 0 {
                        return Err("No memory defined.".into());
                    }
                    self.pop_expected_value_type(ValueType::I32)?;
                    self.push_value_type(ValueType::I32)?;
                }
                0x41 => {
                    // i32 const
                    let _ = self.src.inner().parse_leb_s32()?;
                    self.push_value_type(ValueType::I32)?;
                }
                0x42 => {
                    // i64 const
                    let _ = self.src.inner().parse_leb_s64()?;
                    self.push_value_type(ValueType::I64)?;
                }
                0x45 => {
                    // i32 testop
                    self.validate_itestop(ValueType::I32)?;
                }
                0x46..=0x4f => {
                    // i32 relop
                    self.validate_irelop(ValueType::I32)?;
                }
                0x50 => {
                    // i64 testop
                    self.validate_itestop(ValueType::I64)?;
                }
                0x51..=0x5a => {
                    // i64 relop
                    self.validate_irelop(ValueType::I64)?;
                }
                0x67..=0x69 => {
                    // i32 unop
                    self.validate_iunop(ValueType::I32)?;
                }
                0x6a..=0x78 => {
                    // i32 binop
                    self.validate_ibinop(ValueType::I32)?;
                }
                0x79..=0x7b => {
                    // i64 unop
                    self.validate_iunop(ValueType::I64)?;
                }
                0x7c..=0x8a => {
                    // i64 binop
                    self.validate_ibinop(ValueType::I64)?;
                }
                0xa7 => {
                    // i32.wrap_i64
                    self.validate_icvtop(ValueType::I64, ValueType::I32)?;
                }
                0xac..=0xad => {
                    // i64.extend_i32
                    self.validate_icvtop(ValueType::I32, ValueType::I64)?;
                }
                0xc0..=0xc1 => {
                    // i32.extend
                    self.validate_icvtop(ValueType::I32, ValueType::I32)?;
                }
                0xc2..=0xc4 => {
                    // i64.extend
                    self.validate_icvtop(ValueType::I64, ValueType::I64)?;
                }
                0xfc => {
                    // multibyte opcodes, we want to support only a limited set of them
                    let instr_idx = self.src.inner().parse_leb_u32()?;
                    match instr_idx {
                        8 => {
                            // memory.init
                            if self.memory.max_pages_inclusive() == 0 {
                                return Err("No memory defined.".into());
                            }

                            let data_idx = self.src.inner().parse_leb_u32()?;
                            let end_byte = self.src.inner().read_byte()?;
                            if end_byte != 0x00 {
                                return Err("Expected 0x0.".into());
                            }
                            let Some(data_count) = self.data_count else {
                                // it's required in case of such instruction
                                return Err("Data segments not defined.".into());
                            };
                            if data_idx >= data_count {
                                return Err("Out of bounds data segment referenced.".into());
                            }

                            for _ in 0..3 {
                                self.pop_expected_value_type(ValueType::I32)?;
                            }
                        }
                        9 => {
                            // data.drop

                            // it requires data section
                            let data_idx = self.src.inner().parse_leb_u32()?;
                            let Some(data_count) = self.data_count else {
                                // it's required in case of such instruction
                                return Err("Data segments not defined.".into());
                            };
                            if data_idx >= data_count {
                                return Err("Out of bounds data segment referenced.".into());
                            }
                        }
                        10 => {
                            // memory.copy
                            if self.memory.max_pages_inclusive() == 0 {
                                return Err("No memory defined.".into());
                            }

                            let end_byte = self.src.inner().read_byte()?;
                            if end_byte != 0x00 {
                                return Err("Expected 0x0.".into());
                            }
                            let end_byte = self.src.inner().read_byte()?;
                            if end_byte != 0x00 {
                                return Err("Expected 0x0.".into());
                            }
                            for _ in 0..3 {
                                self.pop_expected_value_type(ValueType::I32)?;
                            }
                        }
                        11 => {
                            // memory.fill
                            if self.memory.max_pages_inclusive() == 0 {
                                return Err("No memory defined.".into());
                            }

                            let end_byte = self.src.inner().read_byte()?;
                            if end_byte != 0x00 {
                                return Err("Expected 0x0.".into());
                            }
                            for _ in 0..3 {
                                self.pop_expected_value_type(ValueType::I32)?;
                            }
                        }
                        12 => {
                            // table.init
                            // extended table manipulation
                            let _elem_idx = self.src.inner().parse_leb_u32()?;
                            let table_idx = self.src.inner().parse_leb_u32()?;
                            if table_idx as usize >= self.tables.len() {
                                return Err("Table index out of bounds.".into());
                            }
                            // we do not support passive element sections
                            return Err("Unsupported.".into());
                        }
                        13 => {
                            // elem.drop
                            // extended table manipulation
                            let _elem_idx = self.src.inner().parse_leb_u32()?;
                            // we do not support passive element sections
                            return Err("Unsupported.".into());
                        }
                        14 => {
                            // table.copy
                            // extended table manipulation
                            let _elem_idx = self.src.inner().parse_leb_u32()?;
                            let table_idx = self.src.inner().parse_leb_u32()?;
                            if table_idx as usize >= self.tables.len() {
                                return Err("Table index out of bounds.".into());
                            }
                            // we do not support passive element sections
                            return Err("Unsupported.".into());
                        }
                        15 => {
                            // table.grow
                            // extended table manipulation
                            let table_idx = self.src.inner().parse_leb_u32()?;
                            if table_idx as usize >= self.tables.len() {
                                return Err("Table index out of bounds.".into());
                            }
                            self.pop_expected_value_type(ValueType::I32)?;
                            let (value_type, _limits) =
                                unsafe { self.tables.get_unchecked(table_idx as usize) };
                            self.pop_expected_value_type(*value_type)?;
                            self.push_value_type(ValueType::I32)?;
                        }
                        16 => {
                            // table.size
                            let table_idx = self.src.inner().parse_leb_u32()?;
                            if table_idx as usize >= self.tables.len() {
                                return Err("Table index out of bounds.".into());
                            }
                            self.push_value_type(ValueType::I32)?;
                        }
                        17 => {
                            // table.fill
                            let table_idx = self.src.inner().parse_leb_u32()?;
                            if table_idx as usize >= self.tables.len() {
                                return Err("Table index out of bounds.".into());
                            }
                            let (value_type, _limits) =
                                unsafe { self.tables.get_unchecked(table_idx as usize) };
                            self.pop_expected_value_type(ValueType::I32)?;
                            self.pop_expected_value_type(*value_type)?;
                            self.pop_expected_value_type(ValueType::I32)?;
                            // since it's a cycle internally, we only consider exiting condition,
                            // that pushes nothing after it (n == 0)
                        }
                        _ => return Err("Unsupported.".into()),
                    }
                }
                0xfd => {
                    // vector instructions, not supported
                    return Err("Unsupported.".into());
                }
                _ => {
                    // println!("Unknown opcode 0x{:02x}", next_opcode);
                    return Err("Unsupported.".into());
                }
            }
        }

        if !end_found {
            return Err("Unexpected end of stream.".into());
        }

        self.validation_postprocessing()?;

        Ok(())
    }

    fn validation_postprocessing(&mut self) -> Result<(), ()> {
        if !self.control_stack.is_empty() {
            // self.debug_print_control_stack();
            return Err(());
        }

        // now we need to walk over parsed blocks and assign missing value for sidetable entries
        let next_sidetable_for_func = self.sidetable_scratch.len() as u32;
        for (current_sidetable_idx, entry) in self.sidetable_scratch.drain_all().enumerate() {
            let final_sidetable = match entry {
                SideTableEntry::BreakJumpToEndOf {
                    jump_to_end_of_block,
                    sidetable_entries_delta_to_set: _,
                    num_copied,
                    num_popped,
                } => {
                    let control_structure = unsafe {
                        self.all_control_stack_entries
                            .get_unchecked(jump_to_end_of_block as usize)
                    };

                    let dst_ip = control_structure.end_ip()? + 1;
                    let dst_sidetable_idx = if let ControlFlowType::Func { .. } = control_structure
                    {
                        next_sidetable_for_func
                    } else {
                        control_structure.get_next_sidetable_at_end()?
                    };

                    RawSideTableEntry {
                        next_ip: dst_ip,
                        next_sidetable_entry_delta: dst_sidetable_idx as i32
                            - current_sidetable_idx as i32,
                        num_copied,
                        num_popped,
                    }
                }
                SideTableEntry::BreakJumpToStartOf {
                    jump_ip,
                    next_sidetable_index,
                    num_copied,
                    num_popped,
                } => RawSideTableEntry {
                    next_ip: jump_ip,
                    next_sidetable_entry_delta: next_sidetable_index as i32
                        - current_sidetable_idx as i32,
                    num_copied,
                    num_popped,
                },
                SideTableEntry::IfBranch {
                    jump_to_end_of_block,
                    sidetable_entries_delta_to_set: _,
                    num_copied,
                    num_popped,
                } => {
                    let control_structure = unsafe {
                        self.all_control_stack_entries
                            .get_unchecked(jump_to_end_of_block as usize)
                    };
                    let dst_ip = control_structure.end_ip()? + 1;
                    let dst_sidetable_idx = control_structure.get_next_sidetable_at_end()?;
                    RawSideTableEntry {
                        next_ip: dst_ip,
                        next_sidetable_entry_delta: dst_sidetable_idx as i32
                            - current_sidetable_idx as i32,
                        num_copied,
                        num_popped,
                    }
                }
                SideTableEntry::ElseBranch {
                    jump_to_else_of,
                    sidetable_entries_delta_to_set: _,
                    num_copied,
                    num_popped,
                } => {
                    let control_structure = unsafe {
                        self.all_control_stack_entries
                            .get_unchecked(jump_to_else_of as usize)
                    };
                    let ControlFlowType::IfBlock {
                        else_ip,
                        end,
                        next_stp_at_else,
                        next_stp_at_end,
                        ..
                    } = control_structure
                    else {
                        return Err(());
                    };
                    let dst_ip = if let Some(else_ip) = else_ip {
                        *else_ip + 1
                    } else {
                        end.unwrap() + 1
                    };
                    let dst_sidetable_idx = if let Some(next_stp_at_else) = next_stp_at_else {
                        *next_stp_at_else
                    } else {
                        next_stp_at_end.unwrap()
                    };
                    RawSideTableEntry {
                        next_ip: dst_ip,
                        next_sidetable_entry_delta: dst_sidetable_idx as i32
                            - current_sidetable_idx as i32,
                        num_copied,
                        num_popped,
                    }
                }
            };

            self.formed_sidetable.push(final_sidetable)?;
        }

        Ok(())
    }

    fn perform_end_analysis(
        &mut self,
        block_type: BlockType,
        if_without_else: bool,
    ) -> Result<(), ()> {
        let frame_stack = self.frame_value_stack_for_control_structure_at_depth(0)?;
        let num_to_return = match block_type {
            BlockType::Empty => {
                // do not need to touch the stack
                0
            }
            BlockType::ValueType(value_type) => {
                // also do not need to pop anything
                let stack_top = frame_stack.last().ok_or(())?;
                if stack_top != &value_type {
                    return Err(());
                }
                if if_without_else {
                    // block would consume 0 params from stack and return 1,
                    // and `else` branch is so divergent
                    return Err(());
                }
                1
            }
            BlockType::TypeIdx(type_idx) => {
                let func_type = self.get_abi_by_type_idx(type_idx)?;
                if if_without_else && func_type.inputs != func_type.outputs {
                    // block would consume some parameters and return other parameters,
                    // and so `else` branch is divergent
                    return Err(());
                }
                let num_to_return = func_type.outputs.len();
                for (expected_type, type_on_stack) in func_type
                    .outputs
                    .types
                    .iter()
                    .rev()
                    .zip(frame_stack.iter().rev())
                {
                    if type_on_stack != expected_type {
                        return Err(());
                    }
                }

                num_to_return
            }
        };

        if frame_stack.len() != num_to_return {
            return Err(());
        }

        Ok(())
    }

    // returns frame pointer for new block, but doesn't explicitly pop
    fn pass_values_to_block_type(&mut self, block_type: BlockType) -> Result<usize, ()> {
        let current_control_structure = self.current_control_structure()?.0;
        let is_unreachable = current_control_structure.is_unreachable();

        // check that stack contains enough inputs for block type
        let num_to_pop = if !is_unreachable {
            // check that stack contains enough inputs for block type
            match block_type {
                BlockType::Empty => {
                    // do not need to touch the stack
                    0
                }
                BlockType::ValueType(..) => {
                    // also do not need to pop anything
                    0
                }
                BlockType::TypeIdx(type_idx) => {
                    let func_type = self.get_abi_by_type_idx(type_idx)?;

                    let frame_stack = self.frame_value_stack_for_control_structure_at_depth(0)?;
                    let num_inputs = func_type.inputs.len();
                    if frame_stack.len() < num_inputs {
                        return Err(());
                    }
                    for (expected_type, type_on_stack) in func_type
                        .inputs
                        .types
                        .iter()
                        .rev()
                        .zip(frame_stack.iter().rev())
                    {
                        if type_on_stack != expected_type {
                            return Err(());
                        }
                    }

                    num_inputs
                }
            }
        } else {
            // we need to push the values
            let frame_start = current_control_structure.get_frame_start();
            assert!(self.absolute_value_stack_depth() >= frame_start);
            self.abstract_value_stack.truncate(frame_start);
            match block_type {
                BlockType::Empty => 0,
                BlockType::ValueType(..) => 0,
                BlockType::TypeIdx(type_idx) => {
                    let func_type = self.get_abi_by_type_idx(type_idx)?;
                    let num_inputs = func_type.inputs.len();
                    for value_type in func_type.inputs.types.iter().copied() {
                        self.push_value_type(value_type)?;
                    }

                    num_inputs
                }
            }
        };

        let frame_start = self.absolute_value_stack_depth() - num_to_pop;

        Ok(frame_start)
    }

    fn handle_break(&mut self, depth: u32, is_unconditional: bool) -> Result<(), ()> {
        // we expect that top N values on the abstract stack directly correspond to the signature of the label we are
        // breaking

        let (control_structure, label_to_break_from_idx) =
            self.get_control_structure_at_depth(depth)?;

        let block_type = if let ControlFlowType::Func { func_idx, .. } = control_structure {
            let function_def = self
                .function_defs
                .get(*func_idx as usize)
                .copied()
                .ok_or(())?;
            BlockType::TypeIdx(function_def.abi_index as u32)
        } else {
            control_structure.as_block_get_type()?
        };
        let original_frame_stack = self.frame_value_stack_for_control_structure_at_depth(depth)?;
        let full_depth = original_frame_stack.len();

        let is_loop = matches!(control_structure, ControlFlowType::Loop { .. });

        let current_frame_stack = self.frame_value_stack_for_control_structure_at_depth(0)?;
        let current_control_structure = self.current_control_structure()?.0;
        let current_frame_start = current_control_structure.get_frame_start();
        let is_unreachalbe = current_control_structure.is_unreachable();

        let num_to_return = if is_unreachalbe {
            0
        } else {
            match block_type {
                BlockType::Empty => {
                    // do not need to touch the stack
                    0
                }
                BlockType::ValueType(value_type) => {
                    // also do not need to pop anything
                    if !is_loop {
                        let stack_top = current_frame_stack.last().ok_or(())?;
                        if stack_top != &value_type {
                            return Err(());
                        }

                        1
                    } else {
                        0
                    }
                }
                BlockType::TypeIdx(type_idx) => {
                    let func_type = self.get_abi_by_type_idx(type_idx)?;
                    let label_type = if is_loop {
                        func_type.inputs.types
                    } else {
                        func_type.outputs.types
                    };
                    let num_to_return = label_type.len();
                    if num_to_return > current_frame_stack.len() {
                        return Err(());
                    }
                    for (expected_type, type_on_stack) in label_type
                        .iter()
                        .rev()
                        .zip(current_frame_stack.iter().rev())
                    {
                        if type_on_stack != expected_type {
                            return Err(());
                        }
                    }

                    num_to_return
                }
            }
        };

        let num_to_pop = full_depth - num_to_return;

        // we are either breaking "out" from most of the structures,
        // or breaking "in" in the case of `loop`
        let sidetable_entry = match control_structure {
            ControlFlowType::Func { .. } => SideTableEntry::BreakJumpToEndOf {
                jump_to_end_of_block: label_to_break_from_idx as u32,
                sidetable_entries_delta_to_set: 0,
                num_copied: num_to_return as u16,
                num_popped: num_to_pop as u16,
            },
            ControlFlowType::Block { end, .. } => {
                assert!(end.is_none());
                SideTableEntry::BreakJumpToEndOf {
                    jump_to_end_of_block: label_to_break_from_idx as u32,
                    sidetable_entries_delta_to_set: 0,
                    num_copied: num_to_return as u16,
                    num_popped: num_to_pop as u16,
                }
            }
            ControlFlowType::Loop {
                ip,
                end,
                next_stp_at_start,
                ..
            } => {
                assert!(end.is_none());
                SideTableEntry::BreakJumpToStartOf {
                    jump_ip: *ip,
                    next_sidetable_index: *next_stp_at_start,
                    num_copied: num_to_return as u16,
                    num_popped: num_to_pop as u16,
                }
            }
            ControlFlowType::IfBlock { .. } => {
                // in any case we break to the `end`
                SideTableEntry::BreakJumpToEndOf {
                    jump_to_end_of_block: label_to_break_from_idx as u32,
                    sidetable_entries_delta_to_set: 0,
                    num_copied: num_to_return as u16,
                    num_popped: num_to_pop as u16,
                }
            }
        };
        let _ = self.push_sidetable_entry(sidetable_entry)?;

        if is_unconditional {
            // cleanup abstract value stack of the current frame, and mark it unreachable
            assert!(self.absolute_value_stack_depth() >= current_frame_start);
            self.abstract_value_stack.truncate(current_frame_start);
            self.current_control_structure_mut()
                .map_err::<(), _>(|x| panic!("{:?}", x))?
                .set_unreachable();
        }
        // otherwise we did quasi pop - push

        Ok(())
    }

    fn handle_call(&mut self, func_type: FunctionType<ValueTypeVecRef<'a>>) -> Result<(), ()> {
        let current_control_structure = self.current_control_structure()?.0;
        let is_unreachable = current_control_structure.is_unreachable();
        if !is_unreachable {
            let frame_stack = self.frame_value_stack_for_control_structure_at_depth(0)?;

            let num_to_copy = func_type.inputs.len();
            if frame_stack.len() < num_to_copy {
                return Err(());
            }
            for (expected_type, type_on_stack) in func_type
                .inputs
                .types
                .iter()
                .rev()
                .zip(frame_stack.iter().rev())
            {
                if type_on_stack != expected_type {
                    return Err(());
                }
            }

            assert!(self.abstract_value_stack.len() >= frame_stack.len());
            assert!(self.abstract_value_stack.len() >= num_to_copy);
            let new_len = self.abstract_value_stack.len() - num_to_copy;
            self.abstract_value_stack.truncate(new_len);

            // and place returnvalues on the stack
            for return_type in func_type.outputs.types.iter().copied() {
                self.push_value_type(return_type)?;
            }
        } else {
            let frame_start = current_control_structure.get_frame_start();
            assert!(self.absolute_value_stack_depth() >= frame_start);
            self.abstract_value_stack.truncate(frame_start);
        }

        Ok(())
    }

    fn pop_value_type(&mut self) -> Result<ValueType, ()> {
        let (control_structure, _) = self.get_control_structure_at_depth(0)?;
        let frame_start = control_structure.get_frame_start();
        let current_frame_stack_depth = self.abstract_value_stack.len() - frame_start;
        if current_frame_stack_depth == 0 {
            if control_structure.is_unreachable() {
                return Ok(ValueType::FormalUnknown);
            } else {
                return Err(());
            }
        }
        self.abstract_value_stack.pop().ok_or(())
    }

    fn pop_expected_value_type(&mut self, expected: ValueType) -> Result<(), ()> {
        let popped = self.pop_value_type()?;
        if popped == ValueType::FormalUnknown || expected == ValueType::FormalUnknown {
            return Ok(());
        }
        if popped != expected {
            Err(())
        } else {
            Ok(())
        }
    }

    fn absolute_value_stack_depth(&self) -> usize {
        self.abstract_value_stack.len()
    }

    #[allow(dead_code)]
    fn frame_value_stack_depth_for_control_structure_at_depth(
        &self,
        depth: u32,
    ) -> Result<usize, ()> {
        let (current_control_struct, _) = self.get_control_structure_at_depth(depth)?;
        let frame_start = current_control_struct.get_frame_start();
        if self.abstract_value_stack.len() < frame_start {
            Err(())
        } else {
            Ok(self.abstract_value_stack.len() - frame_start)
        }
    }

    fn frame_value_stack_for_control_structure_at_depth(
        &self,
        depth: u32,
    ) -> Result<&[ValueType], ()> {
        let (current_control_struct, _) = self.get_control_structure_at_depth(depth)?;
        let frame_start = current_control_struct.get_frame_start();
        if self.abstract_value_stack.len() < frame_start {
            Err(())
        } else {
            let range_end = self.abstract_value_stack.len();
            unsafe {
                Ok(self
                    .abstract_value_stack
                    .get_slice_unchecked(frame_start..range_end))
            }
        }
    }

    fn get_label_types_arity(&self, control_flow_structure: &ControlFlowType) -> Result<usize, ()> {
        let is_loop = matches!(control_flow_structure, ControlFlowType::Loop { .. });
        let label = if let ControlFlowType::Func { func_idx, .. } = control_flow_structure {
            let function_def = self
                .function_defs
                .get(*func_idx as usize)
                .copied()
                .ok_or(())?;
            BlockType::TypeIdx(function_def.abi_index as u32)
        } else {
            control_flow_structure.as_block_get_type()?
        };

        match label {
            BlockType::Empty => Ok(0),
            BlockType::ValueType(_) => {
                if is_loop {
                    Ok(0)
                } else {
                    Ok(1)
                }
            }
            BlockType::TypeIdx(type_idx) => {
                let func_type = self.get_abi_by_type_idx(type_idx)?;
                if is_loop {
                    Ok(func_type.inputs.types.len())
                } else {
                    Ok(func_type.outputs.types.len())
                }
            }
        }
    }

    fn push_value_type(&mut self, value_type: ValueType) -> Result<(), ()> {
        if self.abstract_value_stack.len() == VERIFICATION_TIME_ABSTRACT_VALUE_STACK_SIZE as usize {
            return Err(());
        }
        self.abstract_value_stack.push(value_type)?;
        Ok(())
    }

    fn get_local_type(&self, index: usize) -> Result<ValueType, ()> {
        if index >= self.max_local {
            Err(())
        } else if index >= self.max_local_from_inputs {
            self.function_body_decl
                .get_input_for_inner_index(index - self.max_local_from_inputs)
        } else {
            unsafe { Ok(*self.abi.inputs.types.get_unchecked(index)) }
        }
    }

    fn get_global(&self, index: usize) -> Result<GlobalType, ()> {
        self.globals_decl.get(index).copied().ok_or(())
    }

    fn validate_globals_or_locals_instruction(&mut self, instr: u8) -> Result<(), ()> {
        match instr {
            0x20 => {
                // local.get idx
                let index = self.src.inner().parse_leb_u32()?;
                let local_type = self.get_local_type(index as usize)?;
                self.abstract_value_stack.push(local_type)?;
            }
            0x21 => {
                // local.set idx
                let index = self.src.inner().parse_leb_u32()?;
                let local_type = self.get_local_type(index as usize)?;
                self.pop_expected_value_type(local_type)?;
            }
            0x22 => {
                // local.tee idx
                let index = self.src.inner().parse_leb_u32()?;
                let local_type = self.get_local_type(index as usize)?;
                self.pop_expected_value_type(local_type)?;
                self.push_value_type(local_type)?;
            }
            0x23 => {
                // global.get idx
                let index = self.src.inner().parse_leb_u32()?;
                let global_type = self.get_global(index as usize)?;
                self.push_value_type(global_type.value_type)?;
            }
            0x24 => {
                // global.set idx
                let index = self.src.inner().parse_leb_u32()?;
                let global_type = self.get_global(index as usize)?;
                if !global_type.is_mutable {
                    return Err(());
                }
                self.pop_expected_value_type(global_type.value_type)?;
            }
            _ => unsafe { unreachable_unchecked() },
        }

        Ok(())
    }

    fn validate_mem_load_store(&mut self, instr: u8, alignment: u32) -> Result<(), ()> {
        // absolute largest alignment
        if alignment > 3 {
            return Err(());
        }
        let byte_size = match instr {
            0x28 | 0x34 | 0x35 | 0x36 | 0x3e => 4,
            0x29 | 0x37 => 8,
            0x2c | 0x2d | 0x30 | 0x31 | 0x3a | 0x3c => 1,
            0x2e | 0x2f | 0x32 | 0x33 | 0x3b | 0x3d => 2,
            _ => return Err(()),
        };
        // alignment can not be more than natural
        if 1 << alignment > byte_size {
            return Err(());
        }

        match instr {
            0x28 | 0x2c | 0x2d | 0x2e | 0x2f => {
                // if alignment
                // load i32
                self.pop_expected_value_type(ValueType::I32)?;
                self.push_value_type(ValueType::I32)?;
            }
            0x29 | 0x30 | 0x31 | 0x32 | 0x33 | 0x34 | 0x35 => {
                // load i64
                self.pop_expected_value_type(ValueType::I32)?;
                self.push_value_type(ValueType::I64)?;
            }
            0x36 | 0x3a | 0x3b => {
                // store i32
                self.pop_expected_value_type(ValueType::I32)?;
                self.pop_expected_value_type(ValueType::I32)?;
            }
            0x37 | 0x3c | 0x3d | 0x3e => {
                // store i64
                self.pop_expected_value_type(ValueType::I64)?;
                self.pop_expected_value_type(ValueType::I32)?;
            }
            _ => return Err(()),
        }

        Ok(())
    }

    fn validate_iunop(&mut self, value_type: ValueType) -> Result<(), ()> {
        self.pop_expected_value_type(value_type)?;
        self.push_value_type(value_type)?;
        Ok(())
    }

    fn validate_ibinop(&mut self, value_type: ValueType) -> Result<(), ()> {
        self.pop_expected_value_type(value_type)?;
        self.pop_expected_value_type(value_type)?;
        self.push_value_type(value_type)?;
        Ok(())
    }

    fn validate_itestop(&mut self, value_type: ValueType) -> Result<(), ()> {
        self.pop_expected_value_type(value_type)?;
        self.push_value_type(ValueType::I32)
    }

    fn validate_irelop(&mut self, value_type: ValueType) -> Result<(), ()> {
        self.pop_expected_value_type(value_type)?;
        self.pop_expected_value_type(value_type)?;
        self.push_value_type(ValueType::I32)
    }

    fn validate_icvtop(&mut self, from: ValueType, to: ValueType) -> Result<(), ()> {
        self.pop_expected_value_type(from)?;
        self.push_value_type(to)
    }
}
