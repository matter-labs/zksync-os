pub mod host;
pub mod instantiate;
pub mod module_instance;
pub mod stack_value;

use crate::constants::*;
use crate::types::*;

use self::host::Host;
use self::module_instance::{ModuleInstance, OwnedModuleData};
use self::stack_value::*;
use super::memory::*;
use super::InterpreterError;
use crate::parsers::*;

use alloc::vec::Vec;

pub struct Interpreter<
    'a,
    B: IWasmBaseSourceParser<Error = !> + 'a,
    P: IWasmParser<B> + 'a,
    VT: ValueTypeVec,
    F: Fn(&str),
    M: SystemMemoryManager = (),
> {
    pub full_bytecode: P,
    pub absolute_start: B::StartMark,

    // everything instance related
    // num imported functions
    pub num_imported_functions: u16,
    // num imported tables
    pub num_imported_tables: u16,
    // num imported globals
    pub num_imported_globals: u16,
    // all ABI types
    pub function_types: M::ScratchSpace<FunctionType<VT>>,
    // all imported things from environment
    pub import_defs: M::ScratchSpace<ImportRecordRuntime<'a>>,
    // mapping of function index to ABI
    pub function_defs: M::ScratchSpace<FunctionDef>,
    // function bodies metainformation if local function
    pub function_bodies: M::ScratchSpace<FunctionBody<M::Allocator>>,
    // function names stored as pointers to utf8 strings in the binary
    pub function_names: M::ScratchSpace<FunctionName>,
    // memory type
    pub memory: Option<MemoryLimits>,
    pub memory_is_imported: bool,
    pub globals: M::ScratchSpace<GlobalDecl>,
    pub tables: M::ScratchSpace<(Limits, M::ScratchSpace<StackValue>)>, // TODO: consider splitting into limits, uniform storage space, and indexes
    pub export_defs: M::ScratchSpace<ExportRecord<'a>>,
    pub data_sections: M::ScratchSpace<DataSection>,
    pub elements_sections: M::ScratchSpace<ElementSection>,
    pub system_memory_manager: M,

    pub print_fn: F,
}

impl<
        'a,
        B: IWasmBaseSourceParser<Error = !> + 'a,
        P: IWasmParser<B, Error = !> + 'a,
        VT: ValueTypeVec,
        M: SystemMemoryManager,
        F: Fn(&str),
    > Interpreter<'a, B, P, VT, F, M>
{
    #[allow(clippy::result_unit_err)]
    pub fn new_from_validated_code<MFN: Fn(u16) -> u32>(
        mut full_bytecode: P,
        func_to_sidetable_mapping_fn: &MFN,
        system_memory_manager: M,
        print_fn: F,
    ) -> Result<Self, ()> {
        let absolute_start = full_bytecode.inner().get_start_mark();
        let mut new = Self {
            full_bytecode,
            absolute_start,
            num_imported_functions: 0,
            num_imported_tables: 0,
            num_imported_globals: 0,
            function_types: system_memory_manager.empty_scratch_space(),
            import_defs: system_memory_manager.empty_scratch_space(),
            function_defs: system_memory_manager.empty_scratch_space(),
            function_bodies: system_memory_manager.empty_scratch_space(),
            function_names: system_memory_manager.empty_scratch_space(),
            memory: None,
            memory_is_imported: false,
            globals: system_memory_manager.empty_scratch_space(),
            tables: system_memory_manager.empty_scratch_space(),
            export_defs: system_memory_manager.empty_scratch_space(),
            data_sections: system_memory_manager.empty_scratch_space(),
            elements_sections: system_memory_manager.empty_scratch_space(),
            system_memory_manager,
            print_fn,
        };

        new.parse_initial(func_to_sidetable_mapping_fn)?;

        Ok(new)
    }

    fn parse_initial<MFN: Fn(u16) -> u32>(
        &mut self,
        func_to_sidetable_mapping_fn: &MFN,
    ) -> Result<(), ()> {
        let mut src = self.full_bytecode.clone();

        let Ok(_magic) = src.inner().read_slice(4);
        let Ok(_version) = src.inner().parse_u32_fixed();

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
            if src.is_empty() {
                break;
            }

            let Ok((section_type, section_len)) = src.parse_section_data();
            if section_len == 0 {
                continue;
            }

            let Ok(section_slice) = src.create_subparser(section_len);
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
                    let Ok(_) = self.parse_memory_section(section_slice);
                }
                SectionType::Global => {
                    self.parse_globals_section(section_slice)?;
                }
                SectionType::Export => {
                    self.parse_exports_section(section_slice)?;
                }
                SectionType::Start => {
                    let Ok(_start_function_idx) = self.parse_start_section(section_slice);
                }
                SectionType::Element => {
                    self.parse_elements_section(section_slice)?;
                }
                SectionType::Code => {
                    self.parse_code_section(section_slice, func_to_sidetable_mapping_fn)?;
                }
                SectionType::Data => {
                    self.parse_data_section(section_slice)?;
                }
                SectionType::DataCount => {
                    let Ok(_) = self.parse_data_count_section(section_slice);
                }
                SectionType::Unsupported => unsafe { core::hint::unreachable_unchecked() },
            }
        }

        Ok(())
    }

    #[allow(clippy::result_unit_err)]
    pub fn find_function_idx_by_name(&self, name: &str) -> Result<u16, ()> {
        for exported in self.export_defs.iter() {
            if exported.export_type == ExportDescriptionType::Function && exported.name == name {
                return Ok(exported.abstract_index);
            }
        }

        Err(())
    }

    fn prepare_to_instantiate_module<H: Host>(
        &self,
        host: &mut H,
        system_memory_manager: &mut M,
    ) -> Result<OwnedModuleData<M>, InterpreterError> {
        let mut module_element_sections =
            system_memory_manager.allocate_scratch_space(self.elements_sections.len())?;
        let mut module_data_sections =
            system_memory_manager.allocate_scratch_space(self.data_sections.len())?;
        let mut module_globals =
            system_memory_manager.allocate_scratch_space(self.globals.len())?;

        // now we should fill all the scratch spaces

        // a little convoluted for tables
        let mut module_tables = system_memory_manager.allocate_scratch_space(self.tables.len())?;
        for (limits, values) in self.tables.iter() {
            module_tables.push((*limits, system_memory_manager.clone_scratch_space(values)?))?;
        }

        // easier for the rest
        // verify/init imports

        let mut memory_is_imported = false;

        for import in self.import_defs.iter() {
            let ImportRecordRuntime {
                partial_record,
                abstract_index,
            } = import;
            let PartialImportRecord {
                module,
                name,
                import_type,
            } = partial_record;
            match import_type {
                ImportType::Function { def } => {
                    let abi = unsafe { self.function_types.get_unchecked(def.abi_index as usize) };
                    host.link_importable_function(*abstract_index, abi, module, name)?;
                }
                ImportType::Global { global_type } => {
                    host.verify_importable_global(*abstract_index, module, name, global_type)?;
                }
                ImportType::Memory { limits: _ } => {
                    memory_is_imported = true;
                }
                ImportType::Table { table_type, limits } => {
                    host.verify_importable_table(
                        *abstract_index,
                        module,
                        name,
                        table_type,
                        limits,
                    )?;
                }
            }
        }

        // after imports are linked we can init globals
        // we need to allocate/initialize globals, and for those we only support constant expressions. We do not need to
        // implement mutability checks as those are done for us already

        for global in self.globals.iter() {
            let value = match global.value {
                ConstantExpression::I32(value) => StackValue::new_i32(value),
                ConstantExpression::I64(value) => StackValue::new_i64(value),
                ConstantExpression::FuncRef(value) => StackValue::new_funcref(value),
                ConstantExpression::RefNull => StackValue::new_nullref(),
                ConstantExpression::Global(imported_global) => {
                    host.get_global(imported_global as u16)
                }
            };
            module_globals.push(value)?;
        }

        // initialize active element sections
        for element_section in self.elements_sections.iter().copied() {
            let mut element_section = element_section;

            match &mut element_section {
                ElementSection::ActiveFuncRefExternval { start_idx, end_idx } => {
                    let mut src = self.full_bytecode.clone();
                    let Ok(_) = src.skip_bytes(*start_idx as usize);
                    let Ok(mut src) = src.create_subparser(*end_idx as usize - *start_idx as usize);
                    let table_idx = 0usize;
                    let offset = src.parse_i32_constant_expression().unwrap();
                    let Ok(num_indexes) = src.inner().parse_leb_u32();
                    let iter_end = offset + num_indexes;

                    if table_idx < self.num_imported_tables as usize {
                        for idx in offset..iter_end {
                            let Ok(func_idx) = src.inner().parse_leb_u32();
                            host.set_table_value(
                                table_idx as u32,
                                idx,
                                StackValue::new_funcref(func_idx as u16),
                            )?;
                        }
                    } else {
                        // initialize in instance
                        let table_content =
                            unsafe { &mut module_tables.get_unchecked_mut(table_idx).1 };
                        for idx in offset..iter_end {
                            let Ok(func_idx) = src.inner().parse_leb_u32();
                            unsafe {
                                *table_content.get_unchecked_mut(idx as usize) =
                                    StackValue::new_funcref(func_idx as u16);
                            }
                        }
                    }

                    // drop element
                    *end_idx = *start_idx;
                }
            }

            module_element_sections.push(element_section)?;
        }

        // initialize active data sections

        let get_global_fn = |host: &'_ mut H, index: usize| -> StackValue {
            if index < self.num_imported_globals as usize {
                host.get_global(index as u16)
            } else {
                let local_idx = index - self.num_imported_globals as usize;
                unsafe { *module_globals.get_unchecked(local_idx) }
            }
        };

        for data_section in self.data_sections.iter().copied() {
            let mut data_section = data_section;

            let DataSection::Active {
                offset,
                source_offset,
                len,
                ..
            } = data_section
            else {
                module_data_sections.push(data_section)?;
                continue;
            };

            use crate::routines::runtime::instantiate::evaluate_const_expr_as_u32;
            let offset =
                evaluate_const_expr_as_u32(host, offset, self.num_imported_globals, get_global_fn)?;

            // copy memory
            let mut slice_source = self.full_bytecode.clone();
            let Ok(_) = slice_source.skip_bytes(source_offset as usize);
            let Ok(src) = slice_source.inner().read_slice(len);
            Host::copy_into_memory(host, src, offset)?;
            // drop it
            data_section.drop();
            module_data_sections.push(data_section)?;
        }

        let data = OwnedModuleData {
            elements: module_element_sections,
            datas: module_data_sections,
            globals: module_globals,
            tables: module_tables,
            memory_is_imported,
        };

        Ok(data)
    }

    #[allow(clippy::type_complexity)]
    pub fn instantiate_module_owned<H: Host>(
        self,
        host: &mut H,
        system_memory_manager: &mut M,
    ) -> Result<
        ModuleInstance<
            M,
            VT,
            M::ScratchSpace<FunctionType<VT>>,
            M::ScratchSpace<FunctionDef>,
            M::ScratchSpace<FunctionBody<M::Allocator>>,
            M::ScratchSpace<FunctionName>,
        >,
        InterpreterError,
    > {
        let memory_definition = self
            .memory
            .unwrap_or(MemoryLimits::MinMax { min: 0, max: 0 });
        let num_pages = memory_definition.min_pages();

        host.grow_heap(num_pages as u32)?;

        let module_data = self.prepare_to_instantiate_module(host, system_memory_manager)?;
        let OwnedModuleData {
            elements,
            datas,
            globals,
            tables,
            memory_is_imported,
        } = module_data;

        let module_stack = system_memory_manager.allocate_scratch_space(MAX_STACK_SIZE)?;
        let module_callstack =
            system_memory_manager.allocate_scratch_space(MAX_CALL_FRAMES_STACK_DEPTH)?;

        let instance = ModuleInstance {
            memory_definition,
            elements,
            datas,
            stack: module_stack,
            callstack: module_callstack,
            globals,
            num_imported_functions: self.num_imported_functions,
            num_imported_tables: self.num_imported_tables,
            num_imported_globals: self.num_imported_globals,
            memory_is_imported,
            function_types: self.function_types,
            function_defs: self.function_defs,
            function_bodies: self.function_bodies,
            function_names: self.function_names,
            tables,
            _marker: core::marker::PhantomData,
        };

        Ok(instance)
    }

    #[allow(clippy::type_complexity)]
    pub fn instantiate_module<'b, H: Host>(
        &'b self,
        host: &'_ mut H,
        system_memory_manager: &mut M,
    ) -> Result<
        ModuleInstance<
            M,
            VT,
            <M::ScratchSpace<FunctionType<VT>> as ScratchSpace<FunctionType<VT>>>::Ref<'b>,
            <M::ScratchSpace<FunctionDef> as ScratchSpace<FunctionDef>>::Ref<'b>,
            <M::ScratchSpace<FunctionBody<M::Allocator>> as ScratchSpace<
                FunctionBody<M::Allocator>,
            >>::Ref<'b>,
            <M::ScratchSpace<FunctionName> as ScratchSpace<FunctionName>>::Ref<'b>,
        >,
        InterpreterError,
    > {
        // so we parsed, and can not initialize memory and use data section
        let memory_definition = self
            .memory
            .unwrap_or(MemoryLimits::MinMax { min: 0, max: 0 });
        let num_pages = memory_definition.min_pages();

        host.grow_heap(num_pages as u32)?;

        let module_data = self.prepare_to_instantiate_module(host, system_memory_manager)?;
        let OwnedModuleData {
            elements,
            datas,
            globals,
            tables,
            memory_is_imported,
        } = module_data;

        let module_stack = system_memory_manager.allocate_scratch_space(MAX_STACK_SIZE)?;
        let module_callstack =
            system_memory_manager.allocate_scratch_space(MAX_CALL_FRAMES_STACK_DEPTH)?;

        let instance = ModuleInstance {
            memory_definition,
            elements,
            datas,
            stack: module_stack,
            callstack: module_callstack,
            globals,
            num_imported_functions: self.num_imported_functions,
            num_imported_tables: self.num_imported_tables,
            num_imported_globals: self.num_imported_globals,
            memory_is_imported,
            function_types: self.function_types.by_ref(),
            function_defs: self.function_defs.by_ref(),
            function_bodies: self.function_bodies.by_ref(),
            function_names: self.function_names.by_ref(),
            tables,
            _marker: core::marker::PhantomData,
        };

        Ok(instance)
    }

    fn parse_custom_section(&mut self, mut src: P) -> Result<(), ()> {
        if cfg!(not(feature = "testing")) {
            return Ok(());
        }

        let Ok(name_len) = src.inner().parse_leb_u32();
        if name_len > MAX_NAME_LEN as u32 {
            return Err(());
        }
        let Ok(bytes) = src.inner().read_slice(name_len);
        let name = core::str::from_utf8(bytes).map_err(|_| ())?;

        if name == "name" {
            loop {
                if src.remaining_len() == 0 {
                    break;
                }

                let Ok(id) = src.inner().read_byte();
                let Ok(len) = src.inner().parse_leb_u32();

                let Ok(mut sec_src) = src.create_subparser(len as usize);

                if id == 1 {
                    let Ok(len) = sec_src.inner().parse_leb_u32();

                    self.function_names = self
                        .system_memory_manager
                        .allocate_scratch_space(len as usize)?;

                    for _i in 0..len {
                        let Ok(ix) = sec_src.inner().parse_leb_u32();

                        let Ok(str_len) = sec_src.inner().parse_leb_u32();

                        let Ok(str_bytes) = sec_src.inner().read_slice(str_len);

                        // gets rid of lifetime err
                        let str_bytes = unsafe { core::mem::transmute::<&[u8], &[u8]>(str_bytes) };

                        let str = unsafe { core::str::from_utf8_unchecked(str_bytes) };

                        let record = FunctionName { name: str };

                        assert!(ix as usize == self.function_names.len());

                        // (&self.print_fn)(alloc::format!("fn ix {} -> {:?}", self.function_names.len(), record).as_str());

                        #[allow(unused_must_use)]
                        self.function_names.push(record);
                    }

                    assert!(sec_src.remaining_len() == 0);
                }
            }
        }

        Ok(())
    }

    fn parse_type_section(&mut self, mut src: P) -> Result<(), ()> {
        let Ok(num_types) = src.inner().parse_leb_u32();
        self.function_types = self
            .system_memory_manager
            .allocate_scratch_space(num_types as usize)?;

        for _ in 0..num_types {
            let Ok(_type_type) = src.inner().read_byte();
            let Ok(function_type) = src.parse_function_type();
            self.function_types.push(function_type)?;
        }
        Ok(())
    }

    // here we only parse definitions, but we will need to "connect" to host later on
    fn parse_imports_section(&mut self, mut src: P) -> Result<(), ()> {
        let Ok(count) = src.inner().parse_leb_u32();
        self.import_defs = self
            .system_memory_manager
            .allocate_scratch_space(count as usize)?;

        for _ in 0..count {
            let Ok(partial_record) = src.parse_import_type();
            let abstract_index = match &partial_record.import_type {
                ImportType::Function { def } => {
                    let index = self.num_imported_functions;
                    self.function_defs.push(*def)?;
                    self.num_imported_functions += 1;

                    index
                }
                ImportType::Table { .. } => {
                    let index = self.tables.len();
                    self.num_imported_tables += 1;

                    index as u16
                }
                ImportType::Memory { limits } => {
                    self.memory = Some(*limits);
                    self.memory_is_imported = true;

                    0
                }
                ImportType::Global { .. } => {
                    let index = self.globals.len();
                    self.num_imported_globals += 1;

                    index as u16
                }
            };

            let record = ImportRecordRuntime {
                partial_record,
                abstract_index,
            };

            self.import_defs.push(record)?;
        }

        Ok(())
    }

    fn parse_function_section(&mut self, mut src: P) -> Result<(), ()> {
        let Ok(count) = src.inner().parse_leb_u32();

        for _ in 0..count {
            let Ok(index) = src.inner().parse_leb_u32();
            let func_def = FunctionDef {
                abi_index: index as u16,
            };
            self.function_defs.push(func_def)?;
        }

        Ok(())
    }

    fn parse_table_section(&mut self, mut src: P) -> Result<(), ()> {
        let Ok(count) = src.inner().parse_leb_u32();

        for _ in 0..count {
            let Ok(_value_type) = src.inner().parse_value_type();
            let Ok(limit) = src.parse_limit();
            let mut elements = self
                .system_memory_manager
                .allocate_scratch_space(limit.lower_bound() as usize)?;
            elements.put_many(StackValue::new_nullref(), limit.lower_bound() as usize)?;

            self.tables.push((limit, elements))?;
        }

        Ok(())
    }

    fn parse_memory_section(&mut self, mut src: P) -> Result<(), !> {
        let count = src.inner().parse_leb_u32()?;
        for _ in 0..count {
            let limit = src.parse_memory_limit()?;
            assert!(self.memory.is_none());
            self.memory = Some(limit);
            self.memory_is_imported = false;
        }

        Ok(())
    }

    fn parse_exports_section(&mut self, mut src: P) -> Result<(), ()> {
        let Ok(count) = src.inner().parse_leb_u32();
        self.export_defs = self
            .system_memory_manager
            .allocate_scratch_space(count as usize)?;

        for _ in 0..count {
            let Ok(export_record) = src.parse_export_type();
            self.export_defs.push(export_record)?;
        }

        Ok(())
    }

    fn parse_data_section(&mut self, mut src: P) -> Result<(), ()> {
        assert!(self.memory.is_some());
        let Ok(count) = src.inner().parse_leb_u32();
        self.data_sections = self
            .system_memory_manager
            .allocate_scratch_space(count as usize)?;

        for _ in 0..count {
            let Ok(section_type) = src.inner().parse_leb_u32();
            match section_type {
                0 => {
                    let Ok(offset_expr) = src.parse_constant_expression(
                        self.function_defs.len() as u16,
                        self.num_imported_globals as u32,
                    );
                    let Ok(len) = src.inner().parse_leb_u32();
                    let source_offset = unsafe { src.inner().absolute_offset(self.absolute_start) };
                    let Ok(_) = src.inner().read_slice(len);
                    let section = DataSection::Active {
                        memory_idx: 0,
                        offset: offset_expr,
                        source_offset,
                        len,
                    };
                    self.data_sections.push(section)?;
                }
                1 => {
                    let Ok(len) = src.inner().parse_leb_u32();
                    let source_offset = unsafe { src.inner().absolute_offset(self.absolute_start) };
                    let Ok(_) = src.inner().read_slice(len);
                    let section = DataSection::Passive { source_offset, len };
                    self.data_sections.push(section)?;
                }
                2 => {
                    let Ok(memory_idx) = src.inner().parse_leb_u32();
                    let Ok(offset_expr) = src.parse_constant_expression(
                        self.function_defs.len() as u16,
                        self.globals.len() as u32,
                    );
                    let Ok(len) = src.inner().parse_leb_u32();
                    let source_offset = unsafe { src.inner().absolute_offset(self.absolute_start) };
                    let Ok(_) = src.inner().read_slice(len);
                    let section = DataSection::Active {
                        memory_idx,
                        offset: offset_expr,
                        source_offset,
                        len,
                    };
                    self.data_sections.push(section)?;
                }
                _ => unsafe { core::hint::unreachable_unchecked() },
            }
        }

        Ok(())
    }

    fn parse_data_count_section(&mut self, mut src: P) -> Result<(), !> {
        assert!(self.memory.is_some());
        let _count = src.inner().parse_leb_u32()?;
        Ok(())
    }

    fn parse_start_section(&mut self, mut src: P) -> Result<u32, !> {
        let function_idx = src.inner().parse_leb_u32()?;
        Ok(function_idx)
    }

    fn parse_elements_section(&mut self, mut src: P) -> Result<(), ()> {
        let Ok(count) = src.inner().parse_leb_u32();
        self.elements_sections = self
            .system_memory_manager
            .allocate_scratch_space(count as usize)?;

        for _ in 0..count {
            let Ok(segment_flags) = src.inner().parse_leb_u32();
            let element = match segment_flags {
                0 => {
                    let start_idx = unsafe { src.inner().absolute_offset(self.absolute_start) };
                    // we expect constant expression and then list of function indexes
                    let _table_idx = 0usize;
                    let Ok(_offset) = src.parse_i32_constant_expression();
                    let Ok(num_indexes) = src.inner().parse_leb_u32();
                    for _ in 0..num_indexes {
                        let Ok(_func_idx) = src.inner().parse_leb_u32();
                    }
                    let end_idx = unsafe { src.inner().absolute_offset(self.absolute_start) };

                    ElementSection::ActiveFuncRefExternval { start_idx, end_idx }
                }
                _ => unsafe { core::hint::unreachable_unchecked() },
            };

            self.elements_sections.push(element)?;
        }

        Ok(())
    }

    fn parse_globals_section(&mut self, mut src: P) -> Result<(), ()> {
        let Ok(count) = src.inner().parse_leb_u32();

        for _ in 0..count {
            let Ok(global) = self.parse_global_decl(&mut src);
            self.globals.push(global)?;
        }

        Ok(())
    }

    fn parse_global_decl(&mut self, src: &mut P) -> Result<GlobalDecl, !> {
        let global_type = src.parse_global_type()?;
        let const_expr = src.parse_constant_expression(
            self.function_defs.len() as u16,
            self.num_imported_globals as u32,
        )?;
        // we know that types match, so just continue

        let result = GlobalDecl {
            global_type,
            value: const_expr,
        };

        Ok(result)
    }

    fn parse_code_section<MFN: Fn(u16) -> u32>(
        &mut self,
        mut src: P,
        func_to_sidetable_mapping_fn: &MFN,
    ) -> Result<(), ()> {
        let Ok(count) = src.inner().parse_leb_u32();
        self.function_bodies = self
            .system_memory_manager
            .allocate_scratch_space(count as usize)?;

        let num_imported_functions = self.num_imported_functions;

        for idx in 0..count {
            let Ok(size) = src.inner().parse_leb_u32();
            let Ok(function_body) = src.create_subparser(size as usize);
            let function_idx = num_imported_functions + idx as u16;
            let Ok(function_body) = self.decode_function_body(
                function_body,
                func_to_sidetable_mapping_fn,
                function_idx,
            );
            self.function_bodies.push(function_body)?;
        }

        Ok(())
    }

    fn decode_function_body<MFN: Fn(u16) -> u32>(
        &mut self,
        mut src: P,
        func_to_sidetable_mapping_fn: &MFN,
        function_def_idx: u16,
    ) -> Result<FunctionBody<M::Allocator>, !> {
        let locals_vec_len = src.inner().parse_leb_u32()?;
        // we need to skip locals declaration

        let mut new_decl = FunctionBody {
            function_def_idx: function_def_idx as u32,
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
            let mut locals = src.parse_local_decl().unwrap();
            let total_locals = new_decl.total_locals;
            new_decl.total_locals += locals.elements as u32;
            locals.elements += total_locals as u16; // can not overflow
            new_decl.locals.push(locals);
        }

        let start_instruction_pointer = unsafe { src.inner().absolute_offset(self.absolute_start) };
        let end_instruction_pointer = start_instruction_pointer + src.remaining_len() as u32;
        let len = src.remaining_len();
        src.inner().skip_bytes(len)?;
        let local_func_idx = function_def_idx - self.num_imported_functions;

        new_decl.instruction_pointer = start_instruction_pointer;
        new_decl.end_instruction_pointer = end_instruction_pointer;
        new_decl.initial_sidetable_idx = (func_to_sidetable_mapping_fn)(local_func_idx);

        Ok(new_decl)
    }
}
