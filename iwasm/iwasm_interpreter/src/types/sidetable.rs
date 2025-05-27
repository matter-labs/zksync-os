#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SideTableEntry {
    IfBranch {
        jump_to_end_of_block: u32,
        sidetable_entries_delta_to_set: u32,
        num_copied: u16,
        num_popped: u16,
    },
    ElseBranch {
        jump_to_else_of: u32,
        sidetable_entries_delta_to_set: u32,
        num_copied: u16,
        num_popped: u16,
    },
    BreakJumpToStartOf {
        jump_ip: u32,
        next_sidetable_index: u32,
        num_copied: u16,
        num_popped: u16,
    },
    BreakJumpToEndOf {
        jump_to_end_of_block: u32,
        sidetable_entries_delta_to_set: u32,
        num_copied: u16,
        num_popped: u16,
    },
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RawSideTableEntry {
    pub next_ip: u32,
    pub next_sidetable_entry_delta: i32,
    pub num_copied: u16,
    pub num_popped: u16,
}
