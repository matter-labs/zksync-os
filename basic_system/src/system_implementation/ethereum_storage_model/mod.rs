// TODO the code should be cleaned up
#![allow(clippy::all)]

// Ethereum storage layout. There are multiple fundamental drawbacks of using it for zk:
// - inefficient for state diffs (no space to encode indexes)
// - inefficient for code analysis caching, or delegation caching (no space to put such data)
// - abusable by calls to EXTCODELENGTH as proving code length requires providing a preimage

pub mod caches;
pub(crate) mod cost_constants;
mod mpt;
mod persist_changes;
mod storage_model;

use crate::system_implementation::ethereum_storage_model::mpt::EMPTY_SLICE_ENCODING;

pub use self::persist_changes::{digits_from_key, MPTWithInterner};
pub use self::storage_model::EthereumStorageModel;

use zk_ee::utils::Bytes32;

pub const ETHEREUM_QUERIES_SUBSPACE_MASK: u32 = 0x00_00_e0_00;
pub const ETHEREUM_STORAGE_SUBSPACE_MASK: u32 = ETHEREUM_QUERIES_SUBSPACE_MASK;

pub use self::caches::account_properties::ETHEREUM_ACCOUNT_INITIAL_STATE_QUERY_ID;
pub use self::caches::preimage::{
    ETHEREUM_BYTECODE_LENGTH_FROM_PREIMAGE_QUERY_ID, ETHEREUM_BYTECODE_PREIMAGE_QUERY_ID,
};
pub use self::mpt::{
    BoxInterner, ByteBuffer, EthereumMPT, Interner, InterningBuffer, InterningWordBuffer,
    LazyEncodable, LazyLeafValue, LeafValue, MPTInternalCapacities, Path, PreimagesOracle,
    RLPSlice, EMPTY_ROOT_HASH,
};
pub use self::persist_changes::{
    ETHEREUM_MPT_PREIMAGE_BYTE_LEN_QUERY_ID, ETHEREUM_MPT_PREIMAGE_WORDS_QUERY_ID,
};

pub(crate) fn compare_bytes32_and_mpt_integer(a: &Bytes32, b: &[u8]) -> bool {
    // NOTE: `b` is RLP encoding of slice itself, so we will strip some prefix potentially
    debug_assert!(b.len() <= 33);
    let expected_b_len_from_a = a.num_trailing_nonzero_bytes();
    if expected_b_len_from_a == 0 {
        b.is_empty() || b == EMPTY_SLICE_ENCODING
    } else if expected_b_len_from_a == 1 {
        if b.is_empty() {
            return false;
        }
        let b0 = b[0];
        if b[0] < 0x80 {
            a.as_u8_array_ref()[31] == b0
        } else {
            if b.len() < 2 {
                return false;
            }
            a.as_u8_array_ref()[31] == b[1]
        }
    } else {
        if b.len() < expected_b_len_from_a + 1 {
            return false;
        }
        a.as_u8_array_ref()[(32 - expected_b_len_from_a)..] == b[1..]
    }
}
