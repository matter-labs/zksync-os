//! Read-only lookups in the transactions and receipts tries, which are keyed
//! by `rlp(index)`. Every node on the path is fetched by hash through the
//! preimage oracle and verified against that hash, so a value found is the
//! committed one and an index not found is proven absent by the verified
//! structure (see `docs/ethereum_index_tries.md` for the shape of these tries).

use super::super::GlamsterdamError;
use crate::bootloader::transaction::rlp_encoded::rlp::minimal_rlp_parser::Rlp;
use basic_system::system_implementation::ethereum_storage_model::{
    Interner, PreimagesOracle, EMPTY_ROOT_HASH,
};
use crypto::sha3::Keccak256;
use crypto::MiniDigest;
use zk_ee::utils::Bytes32;

type Result<T> = core::result::Result<T, GlamsterdamError>;

/// The trie key of an item index: `rlp(index)`, as bytes
pub fn index_key(index: u32) -> ([u8; 5], usize) {
    let mut key = [0u8; 5];
    if index == 0 {
        key[0] = 0x80;
        (key, 1)
    } else if index < 0x80 {
        key[0] = index as u8;
        (key, 1)
    } else {
        let be = index.to_be_bytes();
        let first = be.iter().position(|b| *b != 0).unwrap_or(4);
        let len = 4 - first;
        key[0] = 0x80 + len as u8;
        key[1..1 + len].copy_from_slice(&be[first..]);
        (key, 1 + len)
    }
}

/// The nibbles of a key
fn nibbles(key: &[u8], out: &mut [u8; 10]) -> usize {
    for (i, byte) in key.iter().enumerate() {
        out[2 * i] = byte >> 4;
        out[2 * i + 1] = byte & 0xf;
    }
    2 * key.len()
}

/// The path of a hex-prefix encoded segment and whether it is a leaf's
fn decode_hex_prefix(encoding: &[u8], out: &mut [u8; 10]) -> Result<(usize, bool)> {
    let (first, rest) = encoding
        .split_first()
        .ok_or(GlamsterdamError::IndexTrie("empty hex prefix"))?;
    let flag = first >> 4;
    if flag > 3 {
        return Err(GlamsterdamError::IndexTrie("bad hex prefix flag"));
    }
    let is_leaf = flag >= 2;
    let odd = flag & 1 == 1;
    let mut len = 0;
    if odd {
        out[0] = first & 0xf;
        len = 1;
    } else if first & 0xf != 0 {
        return Err(GlamsterdamError::IndexTrie("bad hex prefix padding"));
    }
    if len + 2 * rest.len() > out.len() {
        return Err(GlamsterdamError::IndexTrie("hex prefix too long"));
    }
    for byte in rest {
        out[len] = byte >> 4;
        out[len + 1] = byte & 0xf;
        len += 2;
    }
    Ok((len, is_leaf))
}

/// Fetches the node behind `reference` (a hash, or the node itself when it is
/// shorter than 32 bytes), verifying a hashed one
fn resolve<'a>(
    reference: &mut Rlp<'a>,
    preimages: &mut impl PreimagesOracle,
    interner: &mut (impl Interner<'a> + 'a),
    hasher: &mut Keccak256,
) -> Result<&'a [u8]> {
    let mark = reference.mark();
    if reference.remaining().first().is_some_and(|b| *b >= 0xc0) {
        // an embedded node
        reference
            .list()
            .map_err(|_| GlamsterdamError::IndexTrie("bad embedded node"))?;
        return Ok(reference.consumed_since(mark));
    }
    let hash = reference
        .bytes_exact::<32>()
        .map_err(|_| GlamsterdamError::IndexTrie("bad child reference"))?;
    let node = preimages
        .provide_preimage(hash, interner)
        .map_err(|_| GlamsterdamError::IndexTrie("missing node"))?;
    hasher.update(node);
    if *hasher.finalize_reset() != *hash {
        return Err(GlamsterdamError::IndexTrie("node does not match its hash"));
    }
    Ok(node)
}

/// The value stored at `index`: the encoded transaction or receipt (a typed
/// envelope or a legacy RLP list), which the trie holds as the string payload
/// of the leaf value; `None` when the index has no leaf.
pub fn lookup<'a>(
    root: &Bytes32,
    index: u32,
    preimages: &mut impl PreimagesOracle,
    interner: &mut (impl Interner<'a> + 'a),
    hasher: &mut Keccak256,
) -> Result<Option<&'a [u8]>> {
    if *root == EMPTY_ROOT_HASH {
        return Ok(None);
    }
    let (key, key_len) = index_key(index);
    let mut path = [0u8; 10];
    let path_len = nibbles(&key[..key_len], &mut path);
    let mut consumed = 0usize;

    let mut node = preimages
        .provide_preimage(root.as_u8_array_ref(), interner)
        .map_err(|_| GlamsterdamError::IndexTrie("missing root"))?;
    hasher.update(node);
    if *hasher.finalize_reset() != *root.as_u8_array_ref() {
        return Err(GlamsterdamError::IndexTrie("root does not match its hash"));
    }

    loop {
        let mut outer = Rlp::new(node);
        let mut items = outer
            .list()
            .map_err(|_| GlamsterdamError::IndexTrie("node is not a list"))?;
        if !outer.is_empty() {
            return Err(GlamsterdamError::IndexTrie("trailing node bytes"));
        }
        // count the items to tell a branch from a short node
        let count = {
            let mut probe = items;
            let mut count = 0;
            while !probe.is_empty() {
                let mark = probe.mark();
                if probe.remaining()[0] >= 0xc0 {
                    probe
                        .list()
                        .map_err(|_| GlamsterdamError::IndexTrie("bad item"))?;
                } else {
                    probe
                        .bytes()
                        .map_err(|_| GlamsterdamError::IndexTrie("bad item"))?;
                }
                let _ = probe.consumed_since(mark);
                count += 1;
            }
            count
        };
        match count {
            17 => {
                if consumed == path_len {
                    // the value slot of a branch: never used by index tries
                    return Ok(None);
                }
                let nibble = path[consumed] as usize;
                for _ in 0..nibble {
                    skip_item(&mut items)?;
                }
                if items.remaining().first() == Some(&0x80) {
                    return Ok(None);
                }
                node = resolve(&mut items, preimages, interner, hasher)?;
                consumed += 1;
            }
            2 => {
                let encoded_path = items
                    .bytes()
                    .map_err(|_| GlamsterdamError::IndexTrie("bad path"))?;
                let mut segment = [0u8; 10];
                let (segment_len, is_leaf) = decode_hex_prefix(encoded_path, &mut segment)?;
                let matches = path_len - consumed >= segment_len
                    && path[consumed..consumed + segment_len] == segment[..segment_len];
                if is_leaf {
                    if !matches || consumed + segment_len != path_len {
                        return Ok(None);
                    }
                    let value = items
                        .bytes()
                        .map_err(|_| GlamsterdamError::IndexTrie("bad leaf value"))?;
                    if !items.is_empty() {
                        return Err(GlamsterdamError::IndexTrie("trailing leaf bytes"));
                    }
                    return Ok(Some(value));
                }
                if !matches {
                    return Ok(None);
                }
                node = resolve(&mut items, preimages, interner, hasher)?;
                consumed += segment_len;
            }
            _ => return Err(GlamsterdamError::IndexTrie("node has a wrong item count")),
        }
    }
}

fn skip_item(r: &mut Rlp<'_>) -> Result<()> {
    if r.remaining().first().is_some_and(|b| *b >= 0xc0) {
        r.list()
            .map(|_| ())
            .map_err(|_| GlamsterdamError::IndexTrie("bad item"))
    } else {
        r.bytes()
            .map(|_| ())
            .map_err(|_| GlamsterdamError::IndexTrie("bad item"))
    }
}

/// The number of items in an index trie, proven by the presence of the leaf
/// at `count - 1` and the absence of a leaf at `count`, with `count` the
/// claimed number
pub fn verify_count<'a>(
    root: &Bytes32,
    count: u32,
    preimages: &mut impl PreimagesOracle,
    interner: &mut (impl Interner<'a> + 'a),
    hasher: &mut Keccak256,
) -> Result<()> {
    if count == 0 {
        if *root != EMPTY_ROOT_HASH {
            return Err(GlamsterdamError::IndexTrie(
                "empty count for a non-empty trie",
            ));
        }
        return Ok(());
    }
    if lookup(root, count - 1, preimages, interner, hasher)?.is_none() {
        return Err(GlamsterdamError::IndexTrie("last item missing"));
    }
    if lookup(root, count, preimages, interner, hasher)?.is_some() {
        return Err(GlamsterdamError::IndexTrie(
            "an item past the claimed count",
        ));
    }
    Ok(())
}
