#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportDescriptionType {
    Function = 0x00,
    Table = 0x01,
    Memory = 0x02,
    Global = 0x03,
    Unsupported,
}

impl ExportDescriptionType {
    pub const FUNCTION: u8 = Self::Function as u8;
    pub const TABLE: u8 = Self::Table as u8;
    pub const MEMORY: u8 = Self::Memory as u8;
    pub const GLOBAL: u8 = Self::Global as u8;

    pub const fn from_byte(value: u8) -> Self {
        match value {
            Self::FUNCTION => ExportDescriptionType::Function,
            Self::TABLE => ExportDescriptionType::Table,
            Self::MEMORY => ExportDescriptionType::Memory,
            Self::GLOBAL => ExportDescriptionType::Global,
            _ => ExportDescriptionType::Unsupported,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExportRecord<'a> {
    pub name: &'a str,
    pub export_type: ExportDescriptionType,
    pub abstract_index: u16,
}
