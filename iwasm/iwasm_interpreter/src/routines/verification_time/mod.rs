use crate::constants::*;
mod function_body_validation;
use self::function_body_validation::*;
use crate::parsers::*;
use crate::routines::memory::*;
use crate::types::*;
use alloc::collections::BTreeSet;
use alloc::vec::Vec;

const PRINT_DEBUG_INFO: bool = false;

#[derive(Debug)]
pub struct Validator<
    'a,
    B: IWasmBaseSourceParser<Error = ()> + 'a,
    P: IWasmParser<B> + 'a,
    F: Fn(&str),
    M: SystemMemoryManager = (),
> {
    pub src: P,
    pub absolute_start: B::StartMark,

    pub decoded_section_mask: u32,

    pub function_types: M::ScratchSpace<FunctionType<ValueTypeVecRef<'a>>>,
    pub import_defs: M::ScratchSpace<ImportRecord<'a>>,
    pub function_defs: M::ScratchSpace<FunctionDef>,
    pub export_defs: M::ScratchSpace<ExportRecord<'a>>,
    pub memory: Option<MemoryLimits>,
    pub globals: M::ScratchSpace<GlobalType>,
    pub tables: M::ScratchSpace<(ValueType, Limits)>,
    pub data_counts: Option<u32>,
    pub data_sections: M::ScratchSpace<DataSection>,
    pub function_bodies: M::ScratchSpace<FunctionBody<M::Allocator>>,
    pub start_function: Option<u32>,

    // num imported functions
    pub num_imported_functions: u16,
    // num imported tables
    pub num_imported_tables: u16,
    // num imported globals
    pub num_imported_globals: u16,

    // scratch space
    pub abstract_value_stack: M::ScratchSpace<ValueType>,
    pub all_control_structures: M::ScratchSpace<ControlFlowType>,
    pub control_stack: M::ScratchSpace<usize>,
    pub frame_pointers: M::ScratchSpace<usize>,
    pub sidetable_scratch: M::ScratchSpace<SideTableEntry>,
    // finally constructed sidetable
    pub constructed_sidetable: M::OutputBuffer<RawSideTableEntry>,
    // this one we can consider removing or changing an implementation
    pub export_names_set: BTreeSet<&'a str, M::Allocator>,

    pub print_fn: F,

    pub system_memory_manager: &'a mut M,
    pub _marker: core::marker::PhantomData<B>,
}

impl<
        'a,
        B: IWasmBaseSourceParser<Error = ()> + 'a,
        P: IWasmParser<B, Error = ()> + 'a,
        F: Fn(&str),
        M: SystemMemoryManager,
    > Validator<'a, B, P, F, M>
{
    #[allow(clippy::type_complexity)]
    #[allow(clippy::result_unit_err)]
    pub fn parse(
        mut src: P,
        system_memory_manager: &'a mut M,
        print_fn: F,
    ) -> Result<(M::OutputBuffer<RawSideTableEntry>, M::OutputBuffer<u32>), ()> {
        let absolute_start = src.inner().get_start_mark();
        let mut new = Self {
            decoded_section_mask: 0u32,
            absolute_start,
            abstract_value_stack: system_memory_manager
                .allocate_scratch_space(VERIFICATION_TIME_ABSTRACT_VALUE_STACK_SIZE as usize)?,
            all_control_structures: system_memory_manager
                .allocate_scratch_space(MAX_CONTROL_STACK_DEPTH as usize)?,
            control_stack: system_memory_manager
                .allocate_scratch_space(MAX_CONTROL_STACK_DEPTH as usize)?,
            frame_pointers: system_memory_manager
                .allocate_scratch_space(MAX_CONTROL_STACK_DEPTH as usize)?,
            function_types: system_memory_manager.empty_scratch_space(),
            function_defs: system_memory_manager.empty_scratch_space(),
            import_defs: system_memory_manager.empty_scratch_space(),
            memory: None,
            export_defs: system_memory_manager.empty_scratch_space(),
            tables: system_memory_manager.empty_scratch_space(),
            globals: system_memory_manager.empty_scratch_space(),
            data_counts: None,
            data_sections: system_memory_manager.empty_scratch_space(),
            function_bodies: system_memory_manager.empty_scratch_space(),
            sidetable_scratch: system_memory_manager
                .allocate_scratch_space(MAX_SIDETABLE_SIZE_PER_FUNCTION as usize)?,
            constructed_sidetable: system_memory_manager
                .allocate_output_buffer(MAX_TOTAL_SIDETABLE_SIZE as usize)?,
            src,
            export_names_set: BTreeSet::new_in(system_memory_manager.get_allocator()),
            start_function: None,
            num_imported_functions: 0,
            num_imported_tables: 0,
            num_imported_globals: 0,
            print_fn,
            system_memory_manager,
            _marker: core::marker::PhantomData,
        };

        new.parse_inner()?;

        let mut initial_sidetable_entries = new
            .system_memory_manager
            .allocate_output_buffer(new.function_bodies.len())?;
        for el in new.function_bodies.iter() {
            initial_sidetable_entries.push(el.initial_sidetable_idx)?;
        }
        let sidetable = new.constructed_sidetable;

        Ok((sidetable, initial_sidetable_entries))
    }

    fn parse_inner(&mut self) -> Result<(), ()> {
        let magic = self.src.inner().read_slice(4)?;
        if magic != MAGIC_NUMBER {
            return Err(());
        }

        let version = self.src.inner().parse_u32_fixed()?;
        if version != 1 {
            return Err(());
        }

        let mut current_section_type_idx = -1i32;

        // we should resize function defs, tables and globals to the max capacity
        self.function_defs = self
            .system_memory_manager
            .allocate_scratch_space(MAX_IMPORTS as usize + MAX_FUNCTIONS_IN_SECTION as usize)?;
        self.tables = self
            .system_memory_manager
            .allocate_scratch_space(MAX_IMPORTS as usize + MAX_TABLES as usize)?;
        self.globals = self
            .system_memory_manager
            .allocate_scratch_space(MAX_IMPORTS as usize + MAX_GLOBALS as usize)?;

        for _ in 0..MAX_SECTIONS {
            if self.src.remaining_len() == 0 {
                break;
            }

            let (section_type, section_len) = self.src.parse_section_data()?;
            if section_type != SectionType::Custom {
                let section_idx = SECTIONS_ORDER
                    .iter()
                    .position(|el| el == &section_type)
                    .ok_or(())? as i32;

                // the rest of the sections is ordered
                if section_idx < current_section_type_idx {
                    // by spec sections must be ordered
                    return Err(());
                }
                current_section_type_idx = section_idx;
            }

            if section_len == 0 {
                continue;
            }

            let section_slice = self.src.create_subparser(section_len)?;
            match section_type {
                SectionType::Custom => {
                    self.parse_custom_section(section_slice)?;
                }
                SectionType::Type => {
                    self.parse_type_section(section_slice)?;
                }
                SectionType::Import => {
                    self.parse_imports_section(section_slice)?;
                }
                SectionType::Function => {
                    self.parse_function_section(section_slice)?;
                }
                SectionType::Table => {
                    self.parse_table_section(section_slice)?;
                }
                SectionType::Memory => {
                    self.parse_memory_section(section_slice)?;
                }
                SectionType::Global => {
                    self.parse_globals_section(section_slice)?;
                }
                SectionType::Export => {
                    self.parse_exports_section(section_slice)?;
                }
                SectionType::Start => {
                    let start_function_idx = self.parse_start_section(section_slice)?;
                    self.start_function = Some(start_function_idx);
                }
                SectionType::Element => {
                    self.parse_elements_section(section_slice)?;
                }
                SectionType::Code => {
                    self.parse_code_section(section_slice)?;
                }
                SectionType::Data => {
                    self.parse_data_section(section_slice)?;
                }
                SectionType::DataCount => {
                    self.parse_data_count_section(section_slice)?;
                }
                SectionType::Unsupported => {
                    return Err(());
                }
            }
        }

        Ok(())
    }

    fn parse_type_section(&mut self, mut src: P) -> Result<(), ()> {
        if self.decoded_section_mask & (1u32 << SectionType::Type as u8) != 0 {
            return Err(());
        }
        let num_types = src.inner().parse_leb_u32()?;
        if num_types > MAX_TYPES_IN_SECTION as u32 {
            return Err(());
        }
        self.function_types = self
            .system_memory_manager
            .allocate_scratch_space(num_types as usize)?;

        for _ in 0..num_types {
            let element = src.parse_type_section_element()?;
            self.function_types.push(element)?;
        }

        if !src.is_empty() {
            return Err(());
        }

        self.decoded_section_mask |= 1u32 << SectionType::Type as u8;
        Ok(())
    }

    fn parse_imports_section(&mut self, mut src: P) -> Result<(), ()> {
        if self.decoded_section_mask & (1u32 << SectionType::Import as u8) != 0 {
            return Err(());
        }

        let count = src.inner().parse_leb_u32()?;
        if count > MAX_IMPORTS as u32 {
            return Err(());
        }
        self.import_defs = self
            .system_memory_manager
            .allocate_scratch_space(count as usize)?;

        for _ in 0..count {
            let PartialImportRecord {
                module,
                name,
                import_type,
            } = src.parse_import_type()?;
            let import_type_descr = import_type.as_import_description();
            let record = match import_type {
                ImportType::Function { def } => {
                    let type_idx = def.abi_index;
                    if type_idx as usize >= self.function_types.len() {
                        return Err(());
                    }
                    if self.function_defs.len() >= MAX_FUNCTIONS_IN_SECTION as usize {
                        return Err(());
                    }
                    self.function_defs.push(def)?;
                    self.num_imported_functions += 1;

                    ImportRecord {
                        module,
                        name,
                        import_type: import_type_descr,
                        abstract_index: type_idx,
                    }
                }
                ImportType::Table { table_type, limits } => {
                    let index = self.tables.len();
                    if index >= u16::MAX as usize {
                        return Err(());
                    }
                    if index >= MAX_TABLES as usize {
                        return Err(());
                    }

                    self.tables.push((table_type, limits))?;
                    self.num_imported_tables += 1;

                    ImportRecord {
                        module,
                        name,
                        import_type: import_type_descr,
                        abstract_index: index as u16,
                    }
                }
                ImportType::Memory { limits } => {
                    if self.memory.is_some() {
                        return Err(());
                    }
                    self.memory = Some(limits);

                    ImportRecord {
                        module,
                        name,
                        import_type: import_type_descr,
                        abstract_index: 0,
                    }
                }
                ImportType::Global { global_type } => {
                    let index = self.globals.len();
                    if index >= u16::MAX as usize {
                        return Err(());
                    }
                    if index >= MAX_GLOBALS as usize {
                        return Err(());
                    }

                    self.globals.push(global_type)?;
                    self.num_imported_globals += 1;

                    ImportRecord {
                        module,
                        name,
                        import_type: import_type_descr,
                        abstract_index: index as u16,
                    }
                }
            };

            self.import_defs.push(record)?;
        }

        if !src.is_empty() {
            return Err(());
        }

        self.decoded_section_mask |= 1u32 << SectionType::Import as u8;
        Ok(())
    }

    fn parse_function_section(&mut self, mut src: P) -> Result<(), ()> {
        if self.decoded_section_mask & (1u32 << SectionType::Function as u8) != 0 {
            return Err(());
        }

        let count = src.inner().parse_leb_u32()?;
        if count > MAX_FUNCTIONS_IN_SECTION as u32 {
            return Err(());
        }

        let max_idx = self.function_types.len();

        for _ in 0..count {
            let index = src.inner().parse_leb_u32()?;
            if index as usize >= max_idx {
                return Err(());
            }
            if index > u16::MAX as u32 {
                return Err(());
            }
            let func_def = FunctionDef {
                abi_index: index as u16,
            };
            self.function_defs.push(func_def)?;
        }

        if !src.is_empty() {
            return Err(());
        }

        self.decoded_section_mask |= 1u32 << SectionType::Function as u8;
        Ok(())
    }

    fn parse_table_section(&mut self, mut src: P) -> Result<(), ()> {
        if self.decoded_section_mask & (1u32 << SectionType::Table as u8) != 0 {
            return Err(());
        }

        let count = src.inner().parse_leb_u32()?;
        if count > MAX_TABLES as u32 {
            return Err(());
        }

        for _ in 0..count {
            let value_type = src.inner().parse_value_type()?;
            if value_type != ValueType::FuncRef {
                return Err(());
            }
            let limit = src.parse_limit()?;

            self.tables.push((value_type, limit))?;
        }

        if !src.is_empty() {
            return Err(());
        }

        self.decoded_section_mask |= 1u32 << SectionType::Table as u8;
        Ok(())
    }

    fn parse_memory_section(&mut self, mut src: P) -> Result<(), ()> {
        if self.decoded_section_mask & (1u32 << SectionType::Memory as u8) != 0 {
            return Err(());
        }

        let count = src.inner().parse_leb_u32()?;
        if count > MAX_MEMORIES as u32 {
            return Err(());
        }

        for _ in 0..count {
            if self.memory.is_some() {
                return Err(());
            }
            let limits = src.parse_memory_limit()?;
            self.memory = Some(limits);
        }

        if !src.is_empty() {
            return Err(());
        }

        self.decoded_section_mask |= 1u32 << SectionType::Memory as u8;
        Ok(())
    }

    fn parse_exports_section(&mut self, mut src: P) -> Result<(), ()> {
        if self.decoded_section_mask & (1u32 << SectionType::Export as u8) != 0 {
            return Err(());
        }

        let count = src.inner().parse_leb_u32()?;
        if count > MAX_EXPORTS as u32 {
            return Err(());
        }
        self.export_defs = self
            .system_memory_manager
            .allocate_scratch_space(count as usize)?;

        for _ in 0..count {
            self.parse_export_type(&mut src)?;
        }

        if !src.is_empty() {
            return Err(());
        }

        self.decoded_section_mask |= 1u32 << SectionType::Export as u8;
        Ok(())
    }

    fn parse_export_type(&mut self, src: &mut P) -> Result<(), ()> {
        let export_record = src.parse_export_type()?;
        let is_unique = self.export_names_set.insert(export_record.name);
        if !is_unique {
            return Err(());
        }
        match export_record.export_type {
            ExportDescriptionType::Memory => {
                if export_record.abstract_index > 0
                    || export_record.abstract_index >= MAX_MEMORIES
                    || self.memory.is_none()
                {
                    return Err(());
                }
            }
            ExportDescriptionType::Function => {
                if export_record.abstract_index as usize >= self.function_defs.len() {
                    return Err(());
                }
            }
            ExportDescriptionType::Global => {
                if export_record.abstract_index as usize >= self.globals.len() {
                    return Err(());
                }
            }
            ExportDescriptionType::Table => {
                if export_record.abstract_index as usize >= self.tables.len() {
                    return Err(());
                }
            }
            ExportDescriptionType::Unsupported => return Err(()),
        }
        self.export_defs.push(export_record)?;

        Ok(())
    }

    fn parse_data_section(&mut self, mut src: P) -> Result<(), ()> {
        if self.decoded_section_mask & (1u32 << SectionType::Data as u8) != 0 {
            return Err(());
        }

        // we have no memory to have data at all
        if self.memory.is_none() {
            return Err(());
        }

        let count = src.inner().parse_leb_u32()?;
        if count > MAX_DATA_SECTIONS {
            return Err(());
        }

        if let Some(expected_count) = self.data_counts {
            if count != expected_count {
                return Err(());
            }
        }
        self.data_sections = self
            .system_memory_manager
            .allocate_scratch_space(count as usize)?;

        for _ in 0..count {
            let section_type = src.inner().parse_leb_u32()?;
            match section_type {
                0 => {
                    let offset_expr = src.parse_constant_expression(
                        self.function_defs.len() as u16,
                        self.num_imported_globals as u32,
                    )?;
                    if !offset_expr.can_be_u32_expr() {
                        return Err(());
                    }
                    if let ConstantExpression::Global(global_idx) = &offset_expr {
                        if *global_idx >= self.num_imported_globals as u32 {
                            return Err(());
                        }
                        let global = self.globals.get(*global_idx as usize).ok_or(())?;
                        if global.value_type != ValueType::I32 {
                            return Err(());
                        }
                        if global.is_mutable {
                            return Err(());
                        }
                    }
                    let len = src.inner().parse_leb_u32()?;
                    let source_offset = unsafe { src.inner().absolute_offset(self.absolute_start) };
                    let _end = source_offset.checked_add(len).ok_or(())?;
                    let _ = src.inner().read_slice(len)?;
                    let section = DataSection::Active {
                        memory_idx: 0,
                        offset: offset_expr,
                        source_offset,
                        len,
                    };
                    self.data_sections.push(section)?;
                }
                1 => {
                    let len = src.inner().parse_leb_u32()?;
                    let source_offset = unsafe { src.inner().absolute_offset(self.absolute_start) };
                    let _end = source_offset.checked_add(len).ok_or(())?;
                    let _ = src.inner().read_slice(len)?;
                    let section = DataSection::Passive { source_offset, len };
                    self.data_sections.push(section)?;
                }
                2 => {
                    let memory_idx = src.inner().parse_leb_u32()?;
                    if memory_idx != 0 {
                        return Err(());
                    }
                    let offset_expr = src.parse_constant_expression(
                        self.function_defs.len() as u16,
                        self.globals.len() as u32,
                    )?;
                    if !offset_expr.can_be_u32_expr() {
                        return Err(());
                    }
                    if let ConstantExpression::Global(global_idx) = &offset_expr {
                        if *global_idx >= self.num_imported_globals as u32 {
                            return Err(());
                        }
                        let global = self.globals.get(*global_idx as usize).ok_or(())?;
                        if global.value_type != ValueType::I32 {
                            return Err(());
                        }
                        if global.is_mutable {
                            return Err(());
                        }
                    }
                    let len = src.inner().parse_leb_u32()?;
                    let source_offset = unsafe { src.inner().absolute_offset(self.absolute_start) };
                    let _end = source_offset.checked_add(len).ok_or(())?;
                    let _ = src.inner().read_slice(len)?;
                    let section = DataSection::Active {
                        memory_idx,
                        offset: offset_expr,
                        source_offset,
                        len,
                    };
                    self.data_sections.push(section)?;
                }
                _ => {
                    return Err(());
                }
            }
        }

        if !src.is_empty() {
            return Err(());
        }

        self.decoded_section_mask |= 1u32 << SectionType::Data as u8;
        Ok(())
    }

    fn parse_data_count_section(&mut self, mut src: P) -> Result<(), ()> {
        if self.decoded_section_mask & (1u32 << SectionType::DataCount as u8) != 0 {
            return Err(());
        }

        // we have no memory to have data at all
        if self.memory.is_none() {
            return Err(());
        }

        let count = src.inner().parse_leb_u32()?;
        if count > MAX_DATA_SECTIONS {
            return Err(());
        }

        self.data_counts = Some(count);
        self.data_sections = self
            .system_memory_manager
            .allocate_scratch_space(count as usize)?;

        if !src.is_empty() {
            return Err(());
        }

        self.decoded_section_mask |= 1u32 << SectionType::DataCount as u8;
        Ok(())
    }

    fn parse_start_section(&mut self, mut src: P) -> Result<u32, ()> {
        if self.decoded_section_mask & (1u32 << SectionType::Start as u8) != 0 {
            return Err(());
        }

        let function_idx = src.inner().parse_leb_u32()?;
        if function_idx as usize >= self.function_defs.len() {
            return Err(());
        }

        if !src.is_empty() {
            return Err(());
        }

        self.decoded_section_mask |= 1u32 << SectionType::Start as u8;
        Ok(function_idx)
    }

    fn parse_elements_section(&mut self, mut src: P) -> Result<(), ()> {
        if self.decoded_section_mask & (1u32 << SectionType::Element as u8) != 0 {
            return Err(());
        }

        let count = src.inner().parse_leb_u32()?;
        if count > MAX_ELEMENTS_SECTIONS as u32 {
            return Err(());
        }

        for _ in 0..count {
            let segment_flags = src.inner().parse_leb_u32()?;
            match segment_flags {
                0 => {
                    // we expect constant expression and then list of function indexes
                    if self.tables.is_empty() {
                        return Err(());
                    }

                    let table_idx = 0usize;
                    let limits = self.tables.get(table_idx).ok_or(())?.1;

                    let offset = src.parse_i32_constant_expression()?;
                    let num_indexes = src.inner().parse_leb_u32()?;
                    let end_idx = offset + num_indexes;
                    if end_idx >= MAX_TABLE_SIZE as u32 {
                        return Err(());
                    }

                    // active segment, so by default we should have enough created
                    if end_idx > limits.lower_bound() {
                        return Err(());
                    }

                    if limits.upper_bound_inclusive() != u32::MAX
                        && end_idx > limits.upper_bound_inclusive() + 1
                    {
                        return Err(());
                    }

                    for _ in 0..num_indexes {
                        let func_idx = src.inner().parse_leb_u32()?;
                        if func_idx as usize >= self.function_defs.len() {
                            return Err(());
                        }
                    }
                }
                _ => return Err(()),
            }
        }

        if !src.is_empty() {
            return Err(());
        }

        self.decoded_section_mask |= 1u32 << SectionType::Element as u8;
        Ok(())
    }

    fn parse_globals_section(&mut self, mut src: P) -> Result<(), ()> {
        if self.decoded_section_mask & (1u32 << SectionType::Global as u8) != 0 {
            return Err(());
        }

        let count = src.inner().parse_leb_u32()?;
        if count > MAX_GLOBALS as u32 {
            return Err(());
        }

        for _ in 0..count {
            let global = src.parse_global_decl(
                self.num_imported_globals as u32,
                unsafe {
                    self.function_defs
                        .get_slice_unchecked(0..self.function_defs.len())
                },
                unsafe { self.globals.get_slice_unchecked(0..self.globals.len()) },
            )?;
            self.globals.push(global.global_type)?;
        }

        if !src.is_empty() {
            return Err(());
        }

        self.decoded_section_mask |= 1u32 << SectionType::Global as u8;
        Ok(())
    }

    fn parse_custom_section(&mut self, mut src: P) -> Result<(), ()> {
        let str_enc_len = src.inner().parse_leb_u32()?;
        if str_enc_len > MAX_NAME_LEN as u32 {
            return Err(());
        }
        let may_be_str = src.inner().read_slice(str_enc_len)?;
        let _name = core::str::from_utf8(may_be_str).map_err(|_| ())?;

        Ok(())
    }

    fn parse_code_section(&mut self, mut src: P) -> Result<(), ()> {
        if self.decoded_section_mask & (1u32 << SectionType::Code as u8) != 0 {
            return Err(());
        }

        let count = src.inner().parse_leb_u32()?;
        if count > MAX_FUNCTIONS_IN_CODE as u32 {
            return Err(());
        }
        self.function_bodies = self
            .system_memory_manager
            .allocate_scratch_space(count as usize)?;

        for idx in 0..count {
            let size = src.inner().parse_leb_u32()?;
            let function_body = src.create_subparser(size as usize)?;
            let function_idx = self.num_imported_functions as u32 + idx;
            let function_body = self.decode_function_body(function_body, function_idx)?;
            self.function_bodies.push(function_body)?;
        }

        if !src.is_empty() {
            return Err(());
        }

        self.decoded_section_mask |= 1u32 << SectionType::Code as u8;
        Ok(())
    }

    fn decode_function_body(
        &mut self,
        mut src: P,
        function_def_idx: u32,
    ) -> Result<FunctionBody<M::Allocator>, ()> {
        let locals_vec_len = src.inner().parse_leb_u32()?;
        if locals_vec_len >= MAX_LOCALS_VEC_LEN as u32 {
            return Err(());
        }

        let mut new_decl = FunctionBody {
            function_def_idx,
            instruction_pointer: 0,
            end_instruction_pointer: 0,
            total_locals: 0,
            locals: Vec::with_capacity_in(
                locals_vec_len as usize,
                self.system_memory_manager.get_allocator(),
            ),
            initial_sidetable_idx: 0,
        };

        for _ in 0..locals_vec_len {
            let mut locals = src.parse_local_decl()?;
            let total_locals = new_decl.total_locals;
            new_decl.total_locals = new_decl
                .total_locals
                .checked_add(locals.elements as u32)
                .ok_or(())?;
            if new_decl.total_locals > MAX_LOCALS as u32 {
                return Err(());
            }

            locals.elements += total_locals as u16; // can not overflow
            new_decl.locals.push(locals);
        }

        self.compute_sidetable(src, &mut new_decl)?;

        Ok(new_decl)
    }

    fn compute_sidetable(
        &mut self,
        mut src: P,
        function_body_decl: &mut FunctionBody<M::Allocator>,
    ) -> Result<(), ()> {
        // now it's a main body, so we need IP
        let start_offset = unsafe { src.inner().absolute_offset(self.absolute_start) };
        let len = src.inner().remaining_len();
        // do not consume
        match src.inner().peek_end() {
            Some(last) => {
                if last == END_BYTE {
                    function_body_decl.instruction_pointer = start_offset;
                    function_body_decl.end_instruction_pointer =
                        function_body_decl.instruction_pointer + len as u32;
                } else {
                    return Err(());
                }
            }
            None => return Err(()),
        }

        self.abstract_value_stack.clear();
        self.all_control_structures.clear();
        self.control_stack.clear();
        self.sidetable_scratch.clear();

        let memory_limit = self
            .memory
            .unwrap_or(MemoryLimits::MinMax { min: 0, max: 0 });
        let mut body_validator = FunctionBodyValidator::<B, P, M>::new(
            &mut src,
            function_body_decl,
            &mut self.abstract_value_stack,
            &mut self.all_control_structures,
            &mut self.control_stack,
            &self.globals,
            &self.function_defs,
            &self.function_types,
            &self.tables,
            memory_limit,
            self.data_counts,
            &mut self.sidetable_scratch,
            &mut self.constructed_sidetable,
        )?;

        let fn_len = body_validator.src.remaining_len();
        let st_len = body_validator.formed_sidetable.len();

        body_validator.validate_function_body()?;

        if cfg!(feature = "testing") && PRINT_DEBUG_INFO {
            let st_len = body_validator.formed_sidetable.len() - st_len;
            let ratio = st_len as f32 / fn_len as f32;
            let p = alloc::format!(
                "Fn ix {}\n - side table len: {}\n - code len: {}\n - ratio: {}",
                body_validator.function_body_decl.function_def_idx,
                st_len,
                fn_len,
                ratio,
            );
            (self.print_fn)(p.as_str());
        }

        #[allow(clippy::drop_non_drop)]
        drop(body_validator);

        if !src.is_empty() {
            return Err(());
        }

        Ok(())
    }
}

// #[cfg(test)]
// mod test {
//     use super::*;
//     use std::{ffi::OsStr, io::Read, path::PathBuf};

//     #[test]
//     fn test_parse() {
//         let filename = "loop_with_params";
//         let mut file = std::fs::File::open(&format!("./{}.wasm", filename)).unwrap();
//         let mut buffer = vec![];
//         file.read_to_end(&mut buffer).unwrap();
//         // println!("{}", hex::encode(&buffer));
//         let _ = Validator::parse(&buffer).unwrap();
//     }

//     const REFERENCE_TESTS_PATH: &str = "../testsuite/invalid/";

//     #[test]
//     fn run_reference_tests() {
//         let paths = std::fs::read_dir(REFERENCE_TESTS_PATH).unwrap();

//         let mut paths: Vec<_> = paths.map(|r| r.unwrap()).collect();
//         paths.sort_by_key(|dir| dir.path());

//         for path in paths {
//             if let Ok(file_type) = path.file_type() {
//                 if file_type.is_file() {
//                     run_reference_test_at_path(path.path());
//                 }
//             }
//         }
//     }

//     #[test]
//     fn run_reference_test_by_name() {
//         let name = "return.8.wasm";
//         let path = std::path::Path::new(REFERENCE_TESTS_PATH).join(name);
//         run_reference_test_at_path(path);
//     }

//     fn run_reference_test_at_path(path: PathBuf) {
//         let p = path.display().to_string();
//         println!("Running: {}", &p);

//         if let Some(ext) = path.extension() {
//             if let Some(ext) = OsStr::to_str(ext) {
//                 let res = if ext == "wat" {
//                     let mut buffer = vec![];
//                     let mut file = std::fs::File::open(path).unwrap();
//                     file.read_to_end(&mut buffer).unwrap();
//                     use wasmer::wat2wasm;
//                     if let Ok(bytecode) = wat2wasm(&buffer) {
//                         Validator::parse(&bytecode.to_owned())
//                     } else {
//                         println!("Invalid WAT at {}", &p);
//                         return;
//                     }
//                 } else if ext == "wasm" {
//                     let mut buffer = vec![];
//                     let mut file = std::fs::File::open(path).unwrap();
//                     file.read_to_end(&mut buffer).unwrap();
//                     Validator::parse(&buffer)
//                 } else {
//                     return;
//                 };
//                 if res.is_ok() {
//                     println!("Successfully parsed {}", &p);
//                 } else {
//                     println!("Failed to parse {}", &p);
//                 }
//             }
//         }
//     }
// }
