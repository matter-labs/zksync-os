use super::*;

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImportDescriptionType {
    Function = 0x00,
    Table = 0x01,
    Memory = 0x02,
    Global = 0x03,
    Unsupported,
}

impl ImportDescriptionType {
    pub const FUNCTION: u8 = Self::Function as u8;
    pub const TABLE: u8 = Self::Table as u8;
    pub const MEMORY: u8 = Self::Memory as u8;
    pub const GLOBAL: u8 = Self::Global as u8;

    pub const fn from_byte(value: u8) -> Self {
        match value {
            Self::FUNCTION => ImportDescriptionType::Function,
            Self::TABLE => ImportDescriptionType::Table,
            Self::MEMORY => ImportDescriptionType::Memory,
            Self::GLOBAL => ImportDescriptionType::Global,
            _ => ImportDescriptionType::Unsupported,
        }
    }
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImportType {
    Function {
        def: FunctionDef,
    } = 0x00,
    Table {
        table_type: ValueType,
        limits: Limits,
    } = 0x01,
    Memory {
        limits: MemoryLimits,
    } = 0x02,
    Global {
        global_type: GlobalType,
    } = 0x03,
}

impl ImportType {
    pub const fn as_import_description(&self) -> ImportDescriptionType {
        match self {
            Self::Function { .. } => ImportDescriptionType::Function,
            Self::Table { .. } => ImportDescriptionType::Table,
            Self::Memory { .. } => ImportDescriptionType::Memory,
            Self::Global { .. } => ImportDescriptionType::Global,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ImportRecord<'a> {
    pub module: &'a str,
    pub name: &'a str,
    pub import_type: ImportDescriptionType,
    pub abstract_index: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PartialImportRecord<'a> {
    pub module: &'a str,
    pub name: &'a str,
    pub import_type: ImportType,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ImportRecordRuntime<'a> {
    pub partial_record: PartialImportRecord<'a>,
    pub abstract_index: u16,
}
