mod interner;
mod lazy_leaf_value;
mod nodes;
mod parse_node;
mod preimages;
mod rlp;
mod trie;
mod updates;

use core::alloc::Allocator;
use crypto::MiniDigest;
use zk_ee::utils::Bytes32;

pub(crate) use self::nodes::*;
pub(crate) use self::parse_node::*;
pub(crate) use self::rlp::*;
pub(crate) use self::trie::*;

pub use self::interner::*;
pub use self::lazy_leaf_value::{LazyEncodable, LazyLeafValue, LeafValue};
pub use self::nodes::Path;
pub use self::parse_node::RLPSlice;
pub use self::preimages::*;
pub use self::trie::{EthereumMPT, MPTInternalCapacities};

pub(crate) const EMPTY_SLICE_ENCODING: &[u8] = &[0x80];

// Hash of RLP encoded empty slice
pub const EMPTY_ROOT_HASH: Bytes32 =
    Bytes32::from_hex("56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421");

#[cfg(test)]
mod tests;

#[inline]
pub(crate) fn consume<'a>(src: &mut &'a [u8], bytes: usize) -> Result<&'a [u8], ()> {
    let (data, rest) = src.split_at_checked(bytes).ok_or(())?;
    *src = rest;

    Ok(data)
}
