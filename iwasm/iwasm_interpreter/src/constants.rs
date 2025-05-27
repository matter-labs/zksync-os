pub const MAX_NAME_LEN: usize = 32;
pub const MAGIC_NUMBER: &[u8] = b"\0asm";
pub const FUNC_TYPE: u8 = 0x60;
pub const END_BYTE: u8 = 0x0b;
pub const MAX_SECTIONS: u16 = 1 << 5;
pub const MAX_TYPES_IN_SECTION: u16 = 1 << 6;
pub const MAX_FUNCTIONS_IN_SECTION: u16 = 1 << 10; // An artificial limit. DDOS prevention.
pub const MAX_IMPORTS: u16 = 1 << 5;
pub const MAX_EXPORTS: u16 = 1 << 6;
pub const MAX_GLOBALS: u16 = 1 << 6;
pub const MAX_DATA_SECTIONS: u32 = 1 << 6;
pub const MAX_MEMORIES: u16 = 1 << 0;
pub const MAX_TABLES: u16 = 1 << 4;
pub const MAX_TABLE_SIZE: u16 = 1 << 5;
pub const MAX_ELEMENTS_SECTIONS: u16 = 1 << 4;
pub const MAX_FUNCTIONS_IN_CODE: u16 = 1 << 8;
pub const MAX_CONTROL_STACK_DEPTH: u16 = 1 << 10;
pub const MAX_SIDETABLE_SIZE_PER_FUNCTION: u16 = 1 << 8;
pub const MAX_TOTAL_SIDETABLE_SIZE: u16 = 1 << 15;
pub const VERIFICATION_TIME_ABSTRACT_VALUE_STACK_SIZE: u16 = u16::MAX;
pub const MAX_LOCALS: u16 = 1 << 8;
pub const MAX_LOCALS_VEC_LEN: u16 = 1 << 6;
pub const MAX_LOCALS_PER_TYPE: u16 = 1 << 7;
pub const PAGE_SIZE: usize = 1 << 16;
pub const MAX_PAGES: u16 = 1 << 10;
pub const MAX_CYCLES_PER_PROGRAM: u32 = 1 << 26;
pub const MAX_BREAK_BRANCHES: u32 = 1 << 8; // An artificial limit. DDOS prevention.
pub const MAX_STACK_SIZE_IN_BYTES: usize = 1 << 16; // 64kB
pub const MAX_STACK_SIZE: usize = const {
    MAX_STACK_SIZE_IN_BYTES
        / core::mem::size_of::<crate::routines::runtime::stack_value::StackValue>()
};
pub const MAX_CALL_FRAMES_STACK_SIZE_IN_BYTES: usize = 1 << 16; // 64kB
pub const MAX_CALL_FRAMES_STACK_DEPTH: usize = const {
    MAX_CALL_FRAMES_STACK_SIZE_IN_BYTES
        / core::mem::size_of::<crate::routines::runtime::instantiate::FunctionCallFrame>()
};

use crate::types::sections::SectionType;

pub const SECTIONS_ORDER: [SectionType; 12] = [
    SectionType::Type,
    SectionType::Import,
    SectionType::Function,
    SectionType::Table,
    SectionType::Memory,
    SectionType::Global,
    SectionType::Export,
    SectionType::Start,
    SectionType::Element,
    SectionType::DataCount,
    SectionType::Code,
    SectionType::Data,
];
