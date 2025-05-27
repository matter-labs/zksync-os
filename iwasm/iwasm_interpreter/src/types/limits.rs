use super::*;

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemoryLimits {
    MinOnly { min: u16 } = 0x00,
    MinMax { min: u16, max: u16 } = 0x01,
}

impl MemoryLimits {
    pub const MIN_ONLY_ENCODING: u8 = const {
        unsafe {
            core::ptr::read(&MemoryLimits::MinOnly { min: 0 } as *const MemoryLimits as *const u8)
        }
    };

    pub const MIN_MAX_ENCODING: u8 = const {
        unsafe {
            core::ptr::read(
                &MemoryLimits::MinMax { min: 0, max: 0 } as *const MemoryLimits as *const u8,
            )
        }
    };

    pub const fn empty() -> Self {
        MemoryLimits::MinMax { min: 0, max: 0 }
    }

    pub const fn min_pages(&self) -> u16 {
        match self {
            Self::MinOnly { min } => *min,
            Self::MinMax { min, .. } => *min,
        }
    }

    pub const fn max_pages_inclusive(&self) -> u16 {
        match self {
            Self::MinOnly { .. } => MAX_PAGES - 1,
            Self::MinMax { max, .. } => *max,
        }
    }
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Limits {
    MinOnly { min: u32 } = 0x00,
    MinMax { min: u32, max: u32 } = 0x01,
}

impl Limits {
    pub const MIN_ONLY_ENCODING: u8 = const {
        unsafe { core::ptr::read(&Limits::MinOnly { min: 0 } as *const Limits as *const u8) }
    };

    pub const MIN_MAX_ENCODING: u8 = const {
        unsafe { core::ptr::read(&Limits::MinMax { min: 0, max: 0 } as *const Limits as *const u8) }
    };

    pub const fn lower_bound(&self) -> u32 {
        match self {
            Self::MinOnly { min } => *min,
            Self::MinMax { min, .. } => *min,
        }
    }

    pub const fn upper_bound_inclusive(&self) -> u32 {
        match self {
            Self::MinOnly { .. } => u32::MAX,
            Self::MinMax { max, .. } => *max,
        }
    }
}
