use self::host::EXPECTED_IMPORTED_FUNCTIONS;
use self::memory_manager::ZkOSIWasmMemoryManager;
use alloc::vec::Vec;
use iwasm_interpreter::types::*;
use zk_ee::system_trait::execution_environment::{
    EnvironmentParameters, MemoryRegion, MemoryRegionDescription, MemoryRegionType,
};

use super::*;

use iwasm_interpreter::routines::runtime::module_instance::*;
use iwasm_interpreter::routines::runtime::stack_value::StackValue;

#[allow(type_alias_bounds)]
pub type IWasmOwningFrame<S: System> = ModuleInstance<
    ZkOSIWasmMemoryManager<S>,
    ValueTypeArray,
    Vec<FunctionType<ValueTypeArray>, S::Allocator>,
    Vec<FunctionDef, S::Allocator>,
    Vec<FunctionBody<S::Allocator>, S::Allocator>,
    Vec<FunctionName, S::Allocator>,
>;

pub struct Context<S: System> {
    /// Call value
    pub call_value: U256,
    /// Is interpreter call static.
    pub is_static: bool,
    /// Is interpreter call executing construction code.
    pub is_constructor: bool,
    // TODO: wrap in a type with invariant `len % 256 == 0` and use that as soon in executions as
    // possible.
    /// Calldata region
    pub calldata: MemoryRegion,
    /// Returndata region
    pub last_returndata: MemoryRegion,
    /// Caller address
    pub caller: <S::IOTypes as SystemIOTypesConfig>::Address,
    /// Contract information and invoking data
    pub address: <S::IOTypes as SystemIOTypesConfig>::Address,
    /// Bytecode len for corresponding env calls
    pub bytecode_len: u32,
    /// Immutables bytes.
    pub immutables: &'static [u8],
    /// Saved execution point
    pub src_state: SourceRefsPos,
    /// Latest result of deployment
    pub last_deployed_address: <S::IOTypes as SystemIOTypesConfig>::Address,
}

impl<S: System> Context<S> {
    pub fn empty() -> Self {
        Self {
            call_value: U256::ZERO,
            is_static: false,
            is_constructor: false,
            calldata: MemoryRegion {
                region_type: MemoryRegionType::GlobalShared,
                description: MemoryRegionDescription::empty(),
            },
            last_returndata: MemoryRegion {
                region_type: MemoryRegionType::ReturnData,
                description: MemoryRegionDescription::empty(),
            },
            caller: <S::IOTypes as SystemIOTypesConfig>::Address::default(),
            address: <S::IOTypes as SystemIOTypesConfig>::Address::default(),
            src_state: SourceRefsPos {
                fn_source_offset: 0,
                fn_source_len: 0,
                src_offset: 0,
            },
            bytecode_len: 0,
            immutables: &[],
            last_deployed_address: <S::IOTypes as SystemIOTypesConfig>::Address::default(),
        }
    }
}

pub struct IWasmImportContext<S: EthereumLikeSystem> {
    pub globals: Vec<StackValue, S::Allocator>,
    pub tables: Vec<(Limits, Vec<StackValue, S::Allocator>), S::Allocator>,
    pub host_functions_idx_map: [u16; EXPECTED_IMPORTED_FUNCTIONS.len()],
    // pub host_functions: Vec<
    //     fn(&mut ZkOSHost<'_, S>, &mut [StackValue], usize) -> Result<ExecutionResult, ()>,
    //     S::Allocator,
    // >,
    // pub fn_abis: Vec<FunctionType<ValueTypeArray>, S::Allocator>,
    // pub host_function_names: Vec<(Vec<u8, S::Allocator>, Vec<u8, S::Allocator>), S::Allocator>,
}

impl<S: EthereumLikeSystem> IWasmImportContext<S> {
    pub fn empty(system: &mut S) -> Self {
        let allocator = system.get_allocator();
        Self {
            globals: Vec::new_in(allocator.clone()),
            tables: Vec::new_in(allocator.clone()),
            host_functions_idx_map: [0; EXPECTED_IMPORTED_FUNCTIONS.len()],
            // fn_abis: Vec::new_in(allocator.clone()),
        }
    }
}

pub struct IWasmInterpreter<S: EthereumLikeSystem> {
    pub instantiated_module: IWasmOwningFrame<S>,
    /// Generic resources
    pub resources: S::Resources,
    /// Context part
    pub context: Context<S>,
    pub iwasm_import_context: IWasmImportContext<S>,
    /// Preprocessed data
    pub sidetable: &'static [RawSideTableEntry],
    /// Bytecode related part
    /// Warning: can contain the owned bytecode bytes to which the sidetable and the context
    /// references. Dropped after.
    pub environment_params: EnvironmentParameters<S>,
}

pub fn create_placeholder_module<S: System>(system: &mut S) -> IWasmOwningFrame<S> {
    let allocator = system.get_allocator();

    IWasmOwningFrame {
        memory_definition: MemoryLimits::empty(),
        callstack: Vec::new_in(allocator.clone()),
        stack: Vec::new_in(allocator.clone()),

        datas: Vec::new_in(allocator.clone()),
        elements: Vec::new_in(allocator.clone()),
        globals: Vec::new_in(allocator.clone()),
        tables: Vec::new_in(allocator.clone()),

        num_imported_functions: 0,
        num_imported_tables: 0,
        num_imported_globals: 0,
        memory_is_imported: false,

        function_types: Vec::new_in(allocator.clone()),
        function_defs: Vec::new_in(allocator.clone()),
        function_bodies: Vec::new_in(allocator.clone()),
        function_names: Vec::new_in(allocator.clone()),

        _marker: core::marker::PhantomData,
    }
}
