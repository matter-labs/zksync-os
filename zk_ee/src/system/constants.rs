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

///
/// Maximum amount of computational native resources a full program
/// execution (be it block or batch) can spend.
/// Actual limit is the largest multiple of 2^22 - 1 (segment limit)
/// that is less than 2^36. To be safe, in case our native model
/// is not upper-bounding for some corner cases, we set this limit
/// conservatively to 2^35.
///
pub const MAX_NATIVE_COMPUTATIONAL: u64 = 1 << 35;

pub const EIP7702_DELEGATION_MARKER: [u8; 3] = [0xef, 0x01, 0x00];

// Max number of interop roots supported for a batch
pub const MAX_NUMBER_INTEROP_ROOTS: usize = 100;
