pub const MAX_SCRATCH_SPACE_USIZE_WORDS: usize = 128;

pub const BLAKE_DELEGATION_COEFFICIENT: u64 = 16;
pub const BIGINT_DELEGATION_COEFFICIENT: u64 = 4;

///
/// Compute native cost from
/// (raw cycles, bigint delegations, blake delegations)
///
#[macro_export]
macro_rules! native_with_delegations {
    ($raw:expr, $bigint:expr, $blake:expr) => {
        $raw + $bigint * zk_ee::system::constants::BIGINT_DELEGATION_COEFFICIENT
            + $blake * zk_ee::system::constants::BLAKE_DELEGATION_COEFFICIENT
    };
}

pub const MAX_NATIVE_COMPUTATIONAL: u64 = 1 << 36;

pub const EIP7702_DELEGATION_MARKER: [u8; 3] = [0xef, 0x01, 0x00];
