//! Reference Merkle-Patricia trie builder for tests: builds the trie of a set
//! of entries and keeps every node encoding, so a witness can be served from
//! it (the stack trie unfolds the nodes by hash).

use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use crypto::sha3::Keccak256;
use crypto::MiniDigest;
use zk_ee::utils::Bytes32;

use super::EMPTY_ROOT_HASH;

/// RLP of a byte string
pub fn rlp_string(slice: &[u8]) -> Vec<u8> {
    if slice.len() == 1 && slice[0] < 0x80 {
        return slice.to_vec();
    }
    let mut out = Vec::new();
    if slice.len() <= 55 {
        out.push(0x80 + slice.len() as u8);
    } else if slice.len() < 256 {
        out.extend_from_slice(&[0xb8, slice.len() as u8]);
    } else {
        out.extend_from_slice(&[0xb9, (slice.len() >> 8) as u8, slice.len() as u8]);
    }
    out.extend_from_slice(slice);
    out
}

/// RLP of a minimal big-endian integer given as big-endian bytes
pub fn rlp_integer(be: &[u8]) -> Vec<u8> {
    let first = be.iter().position(|b| *b != 0).unwrap_or(be.len());
    rlp_string(&be[first..])
}

/// RLP of a list of already encoded items
pub fn rlp_list(items: &[Vec<u8>]) -> Vec<u8> {
    let payload: Vec<u8> = items.concat();
    let mut out = Vec::new();
    if payload.len() <= 55 {
        out.push(0xc0 + payload.len() as u8);
    } else if payload.len() < 256 {
        out.extend_from_slice(&[0xf8, payload.len() as u8]);
    } else {
        out.extend_from_slice(&[0xf9, (payload.len() >> 8) as u8, payload.len() as u8]);
    }
    out.extend_from_slice(&payload);
    out
}

/// The 64 nibbles of a trie key
pub fn key_nibbles(key: &[u8; 32]) -> Vec<u8> {
    key.iter().flat_map(|b| [b >> 4, b & 0xf]).collect()
}

/// Hex-prefix encoding of a path
pub fn compact(nibbles: &[u8], is_leaf: bool) -> Vec<u8> {
    let mut out = Vec::new();
    let odd = nibbles.len() % 2 == 1;
    let mut first = if is_leaf { 0x20 } else { 0x00 };
    let rest = if odd {
        first |= 0x10 | nibbles[0];
        &nibbles[1..]
    } else {
        nibbles
    };
    out.push(first);
    for pair in rest.chunks(2) {
        out.push((pair[0] << 4) | pair[1]);
    }
    out
}

/// The reference to a node from its parent: the node itself when shorter than
/// 32 bytes, its hash (stored in `store`) otherwise
pub fn node_ref(encoding: Vec<u8>, store: &mut BTreeMap<Bytes32, Vec<u8>>) -> Vec<u8> {
    if encoding.len() >= 32 {
        let hash = *Keccak256::digest(&encoding);
        store.insert(Bytes32::from_array(hash), encoding);
        let mut out = alloc::vec![0x80 + 32];
        out.extend_from_slice(&hash);
        out
    } else {
        encoding
    }
}

/// The node of `entries` (64 nibbles and the full RLP item of the value,
/// sorted by nibbles, non-empty) below `depth` nibbles
pub fn build_node(
    entries: &[(Vec<u8>, Vec<u8>)],
    depth: usize,
    store: &mut BTreeMap<Bytes32, Vec<u8>>,
) -> Vec<u8> {
    assert!(entries.is_empty() == false);
    if entries.len() == 1 {
        let (nibbles, value) = &entries[0];
        return rlp_list(&[rlp_string(&compact(&nibbles[depth..], true)), value.clone()]);
    }
    let first = &entries[0].0;
    let mut common = 0;
    while depth + common < 64
        && entries
            .iter()
            .all(|(n, _)| n[depth + common] == first[depth + common])
    {
        common += 1;
    }
    if common > 0 {
        let child = build_node(entries, depth + common, store);
        let child_ref = node_ref(child, store);
        return rlp_list(&[
            rlp_string(&compact(&first[depth..depth + common], false)),
            child_ref,
        ]);
    }
    let mut items = Vec::with_capacity(17);
    let mut start = 0;
    for nibble in 0..16u8 {
        let end = start
            + entries[start..]
                .iter()
                .take_while(|(n, _)| n[depth] == nibble)
                .count();
        if end == start {
            items.push(alloc::vec![0x80]);
        } else {
            let child = build_node(&entries[start..end], depth + 1, store);
            items.push(node_ref(child, store));
        }
        start = end;
    }
    assert_eq!(start, entries.len());
    items.push(alloc::vec![0x80]);
    rlp_list(&items)
}

/// Builds the trie of `entries` (trie key and the full RLP item of the leaf
/// value, in any order), stores every node by hash and returns the root
pub fn build_trie(
    entries: &[([u8; 32], Vec<u8>)],
    store: &mut BTreeMap<Bytes32, Vec<u8>>,
) -> [u8; 32] {
    let mut entries: Vec<(Vec<u8>, Vec<u8>)> = entries
        .iter()
        .map(|(key, value)| (key_nibbles(key), value.clone()))
        .collect();
    entries.sort();
    if entries.is_empty() {
        return EMPTY_ROOT_HASH.as_u8_array();
    }
    let root = build_node(&entries, 0, store);
    let hash = *Keccak256::digest(&root);
    store.insert(Bytes32::from_array(hash), root);
    hash
}
