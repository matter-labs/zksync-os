use crate::system::Ergs;
use crate::system::{MemorySubsystem, OSManagedRegion, SystemTypes};

pub struct CalleeParameters<S: SystemTypes> {
    pub next_ee_version: u8,
    pub bytecode:
        <<S::Memory as MemorySubsystem>::ManagedRegion as OSManagedRegion>::OSManagedImmutableSlice,
    pub bytecode_len: u32,
    pub artifacts_len: u32,
    pub stipend: Option<Ergs>,
}
