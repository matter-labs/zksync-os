#[cfg(feature = "evaluate")]
pub mod compute;
#[cfg(feature = "evaluate")]
pub mod evaluate;

pub mod common;
pub mod verify;

pub use self::common::*;

use ruint::Uint;

pub type S1BigInt = Uint<64, 1>;
pub type S2BigInt = Uint<128, 2>;
pub type S3BigInt = Uint<256, 4>;
pub type S4BigInt = Uint<384, 6>;

pub const HASH_TO_PRIME_ORACLE_ID: u32 = 0x100;

pub const ENTROPY_BITS: u32 = 256;
pub const BIGINT_BITS: u32 = 322;
pub const GENERATION_STEPS: [(u32, u32); 5] = [(21, 11), (20, 11), (49, 12), (108, 13), (63, 14)];

pub const MAX_ENTROPY_BYTES: u32 = const {
    let mut max_bytes = 0;
    let mut i = 0;
    while i < GENERATION_STEPS.len() {
        max_bytes += GENERATION_STEPS[i].0.next_multiple_of(8) / 8;
        i += 1;
    }

    max_bytes
};
