//! RLP encoding helpers used for block-header and receipt hashing.
//!
//! The encoders write directly into a sink (`WriteBytes`) without allocating, so
//! the same code can target a hasher (block/receipt roots) or any other byte
//! buffer. Lists are length-prefixed, so encoding is a two-step dance: estimate
//! each element's encoded length (`length`), then write the list length prefix
//! followed by the elements.
//!
//! Modules:
//! - `primitives`: low-level RLP — encoding-length estimation (pure `usize`
//!   math) and the `WriteBytes` writers (numbers, byte strings, length
//!   prefixes). Shared by the receipt encoder and the streaming header /
//!   EIP-7702 authorization hashes.
//! - `encodable`: the `RLPEncodable` / `CachingRLPEncodable` framework and list
//!   envelopes, plus impls for the primitive types used in receipts.
//! - `receipt`: the transaction `ReceiptEncoder`.
//!
//! The MPT-leaf bridge (`CellEnvelope` / `LazyEncodable`) lives in
//! `block_flow::ethereum`, since only the Ethereum-mode trie needs to plug these
//! encoders into MPT leaves.

mod encodable;
mod primitives;
mod receipt;

pub use self::encodable::*;
pub use self::receipt::*;
// Only the primitives needed outside this module are re-exported; the
// length-prefix / `u64` helpers stay internal to `encodable`/`receipt`.
pub use self::primitives::{
    apply_bytes_encoding, apply_list_length_encoding, apply_number_encoding,
    estimate_bytes_encoding_len, estimate_list_length_encoding_len, estimate_number_encoding_len,
};

/// Addresses are encoded as a 20-byte string: one length byte + 20 bytes.
pub const ADDRESS_ENCODING_LEN: usize = 21;
