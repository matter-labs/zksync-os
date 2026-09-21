use ruint::aliases::U256;

/// The code of the account a frame runs on, as known to the bootloader
#[non_exhaustive]
#[derive(Clone, Copy, Debug)]
pub enum BytecodeData<'a> {
    /// The code is loaded, in the layout that `code_version` describes
    Available {
        bytecode: &'a [u8],
        unpadded_code_len: u32,
        artifacts_len: u32,
    },
    /// The account has code but it was not loaded. A deployment target is read this way:
    /// only its code hash decides the collision, and the code itself is never needed
    /// (the real execution does not read it either).
    UnknownButNotEmpty,
    /// Nothing is known about the code
    Unknown,
}

impl<'a> BytecodeData<'a> {
    /// No code, which is known
    pub const EMPTY: Self = Self::Available {
        bytecode: &[],
        unpadded_code_len: 0,
        artifacts_len: 0,
    };

    /// Whether the account has code, if that is known
    pub fn has_code(&self) -> Option<bool> {
        match self {
            Self::Available {
                unpadded_code_len, ..
            } => Some(*unpadded_code_len != 0),
            Self::UnknownButNotEmpty => Some(true),
            Self::Unknown => None,
        }
    }
}

pub struct CalleeAccountProperties<'a> {
    pub nominal_token_balance: U256,
    pub nonce: u64,
    pub bytecode: BytecodeData<'a>,
    pub ee_type: u8,
    pub code_version: u8,
}
