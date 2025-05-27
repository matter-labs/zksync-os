use super::*;

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SectionType {
    Custom = 0x0,
    Type = 0x1,
    Import = 0x2,
    Function = 0x3,
    Table = 0x4,
    Memory = 0x5,
    Global = 0x6,
    Export = 0x7,
    Start = 0x8,
    Element = 0x9,
    Code = 0xa,
    Data = 0xb,
    DataCount = 0xc,
    Unsupported = 0xff,
}

impl SectionType {
    pub const CUSTOM: u8 = Self::Custom as u8;
    pub const TYPE: u8 = Self::Type as u8;
    pub const IMPORT: u8 = Self::Import as u8;
    pub const FUNCTION: u8 = Self::Function as u8;
    pub const TABLE: u8 = Self::Table as u8;
    pub const MEMORY: u8 = Self::Memory as u8;
    pub const GLOBAL: u8 = Self::Global as u8;
    pub const EXPORT: u8 = Self::Export as u8;
    pub const START: u8 = Self::Start as u8;
    pub const ELEMENT: u8 = Self::Element as u8;
    pub const CODE: u8 = Self::Code as u8;
    pub const DATA: u8 = Self::Data as u8;
    pub const DATA_COUNT: u8 = Self::DataCount as u8;

    pub const fn from_byte(value: u8) -> Self {
        match value {
            Self::CUSTOM => SectionType::Custom,
            Self::TYPE => SectionType::Type,
            Self::IMPORT => SectionType::Import,
            Self::FUNCTION => SectionType::Function,
            Self::TABLE => SectionType::Table,
            Self::MEMORY => SectionType::Memory,
            Self::GLOBAL => SectionType::Global,
            Self::EXPORT => SectionType::Export,
            Self::START => SectionType::Start,
            Self::ELEMENT => SectionType::Element,
            Self::CODE => SectionType::Code,
            Self::DATA => SectionType::Data,
            Self::DATA_COUNT => SectionType::DataCount,
            _ => SectionType::Unsupported,
        }
    }
}

#[allow(clippy::len_without_is_empty)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DataSection {
    Active {
        memory_idx: u32,
        offset: ConstantExpression,
        source_offset: u32,
        len: u32,
    },
    Passive {
        source_offset: u32,
        len: u32,
    },
}

impl DataSection {
    pub fn len(&self) -> u32 {
        match self {
            Self::Active { len, .. } => *len,
            Self::Passive { len, .. } => *len,
        }
    }

    pub fn as_range(&self) -> (u32, u32) {
        match self {
            Self::Active {
                source_offset, len, ..
            } => (*source_offset, source_offset + len),
            Self::Passive {
                source_offset, len, ..
            } => (*source_offset, source_offset + len),
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn drop(&mut self) {
        match self {
            Self::Active { len, .. } => {
                *len = 0;
            }
            Self::Passive { len, .. } => {
                *len = 0;
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ElementSection {
    ActiveFuncRefExternval { start_idx: u32, end_idx: u32 },
}
