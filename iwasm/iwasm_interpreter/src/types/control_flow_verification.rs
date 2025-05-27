use super::*;

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControlFlowType {
    Func {
        func_idx: u32,
        end: u32,
        frame_start: u32,
        is_unreachable: bool,
    },
    Block {
        ip: u32,
        end: Option<u32>,
        block_type: BlockType,
        frame_start: u32,
        next_stp_at_end: Option<u32>,
        is_unreachable: bool,
    },
    Loop {
        ip: u32,
        end: Option<u32>,
        block_type: BlockType,
        frame_start: u32,
        next_stp_at_start: u32,
        is_unreachable: bool,
    },
    IfBlock {
        ip: u32,
        else_ip: Option<u32>,
        end: Option<u32>,
        block_type: BlockType,
        frame_start: u32,
        next_stp_at_else: Option<u32>,
        next_stp_at_end: Option<u32>,
        if_branch_is_unreachable: bool,
        else_branch_is_unreachable: bool,
    },
}

impl ControlFlowType {
    #[allow(clippy::result_unit_err)]
    pub fn as_block_get_type(&self) -> Result<BlockType, ()> {
        match self {
            Self::Func { .. } => Err(()),
            Self::Block { block_type, .. } => Ok(*block_type),
            Self::Loop { block_type, .. } => Ok(*block_type),
            Self::IfBlock { block_type, .. } => Ok(*block_type),
        }
    }

    pub fn get_frame_start(&self) -> usize {
        match self {
            Self::Func { frame_start, .. } => *frame_start as usize,
            Self::Block { frame_start, .. } => *frame_start as usize,
            Self::Loop { frame_start, .. } => *frame_start as usize,
            Self::IfBlock { frame_start, .. } => *frame_start as usize,
        }
    }

    #[allow(clippy::result_unit_err)]
    pub fn get_next_sidetable_at_end(&self) -> Result<u32, ()> {
        match self {
            Self::Func { .. } => Err(()),
            Self::Block {
                next_stp_at_end, ..
            } => (*next_stp_at_end).ok_or(()),
            Self::Loop { .. } => Err(()),
            Self::IfBlock {
                next_stp_at_end, ..
            } => (*next_stp_at_end).ok_or(()),
        }
    }

    #[allow(clippy::result_unit_err)]
    pub fn get_next_sidetable_at_start(&self) -> Result<u32, ()> {
        match self {
            Self::Func { .. } => Err(()),
            Self::Block { .. } => Err(()),
            Self::Loop {
                next_stp_at_start, ..
            } => Ok(*next_stp_at_start),
            Self::IfBlock { .. } => Err(()),
        }
    }

    #[allow(clippy::result_unit_err)]
    pub fn get_next_sidetable_at_else(&self) -> Result<u32, ()> {
        match self {
            Self::Func { .. } => Err(()),
            Self::Block { .. } => Err(()),
            Self::Loop { .. } => Err(()),
            Self::IfBlock {
                next_stp_at_else, ..
            } => (*next_stp_at_else).ok_or(()),
        }
    }

    #[allow(clippy::result_unit_err)]
    pub fn end_ip(&self) -> Result<u32, ()> {
        match self {
            Self::Func { end, .. } => Ok(*end),
            Self::Block { end, .. } => end.ok_or(()),
            Self::Loop { end, .. } => end.ok_or(()),
            Self::IfBlock { end, .. } => end.ok_or(()),
        }
    }

    #[allow(clippy::result_unit_err)]
    pub fn start_ip(&self) -> Result<u32, ()> {
        match self {
            Self::Func { .. } => Err(()),
            Self::Block { ip, .. } => Ok(*ip),
            Self::Loop { ip, .. } => Ok(*ip),
            Self::IfBlock { ip, .. } => Ok(*ip),
        }
    }

    #[allow(clippy::result_unit_err)]
    pub fn else_ip(&self) -> Result<u32, ()> {
        match self {
            Self::Func { .. } => Err(()),
            Self::Block { .. } => Err(()),
            Self::Loop { .. } => Err(()),
            Self::IfBlock { else_ip, .. } => else_ip.ok_or(()),
        }
    }

    pub fn set_unreachable(&mut self) {
        match self {
            Self::Func { is_unreachable, .. } => {
                *is_unreachable = true;
            }
            Self::Block { is_unreachable, .. } => {
                *is_unreachable = true;
            }
            Self::Loop { is_unreachable, .. } => {
                *is_unreachable = true;
            }
            Self::IfBlock {
                else_ip,
                if_branch_is_unreachable,
                else_branch_is_unreachable,
                ..
            } => {
                if else_ip.is_none() {
                    *if_branch_is_unreachable = true;
                } else {
                    *else_branch_is_unreachable = true;
                }
            }
        }
    }

    pub fn is_unreachable(&self) -> bool {
        match self {
            Self::Func { is_unreachable, .. } => *is_unreachable,
            Self::Block { is_unreachable, .. } => *is_unreachable,
            Self::Loop { is_unreachable, .. } => *is_unreachable,
            Self::IfBlock {
                else_ip,
                if_branch_is_unreachable,
                else_branch_is_unreachable,
                ..
            } => {
                if else_ip.is_none() {
                    *if_branch_is_unreachable
                } else {
                    *else_branch_is_unreachable
                }
            }
        }
    }
}
