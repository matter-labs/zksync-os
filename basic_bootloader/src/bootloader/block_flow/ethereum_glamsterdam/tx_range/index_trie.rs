//! Read-only lookups in the transactions and receipts tries, which are keyed
//! by `rlp(index)`. Every node on the path is fetched by hash through the
//! preimage oracle and verified against that hash, so a value found is the
//! committed one and an index not found is proven absent by the verified
//! structure (see `docs/ethereum_index_tries.md` for the shape of these tries).
//!
//! # Proving that a trie holds exactly the items `0..n`
//!
//! A block is valid only if its `transactionsRoot` and `receiptsRoot` are the
//! tries of exactly its `n` transactions and `n` receipts: keys `rlp(0)` to
//! `rlp(n - 1)`, nothing else. The programs of a block establish this
//! together; no program sees the whole trie, and no program reads a leaf it
//! does not need (a leaf is a whole transaction or receipt).
//!
//! What a single lookup proves. Walking the path of `rlp(i)` from the root
//! and checking every node against the hash its parent holds proves that the
//! leaf found is the one the root commits to at that key, and that a key not
//! found has no leaf. It proves nothing about the other keys: a trie can hold
//! leaves at keys no lookup ever visits, at an index past `n` or at a key
//! that is not `rlp(i)` of any `i` (a non-canonical integer encoding such as
//! `0x8105`, or a path of the wrong length).
//!
//! ```text
//!  n = 20: the keys are 0x80 (index 0) and 0x01..0x13 (indices 1..19)
//!
//!                            root (branch)
//!      slot:   0        1        2..7     8          9..f
//!              |        |        empty    |          empty   <- no key below 20
//!           branch   branch             leaf                   starts with 2..7
//!            [0]      [1]            path "0"                  or 9..f
//!                                   (key 0x80,
//!  slot:  0  1 .. f   0 1 2 3 4..f   index 0)
//!         ?  |    |   | | | | empty  <- indices 20..31 would be here: past n
//!         ^  1   15  16 ..  19
//!         key 0x00 is not rlp(i) of any i: must be empty as well
//! ```
//!
//! The rule. At a branch node whose path (the nibbles consumed from the root)
//! is `p`, the only child slots that may be non-empty are the nibbles `c`
//! such that some `rlp(i)` with `i < n` starts with `p ++ c`
//! ([`expected_children`]), and the value slot must be empty (an index trie
//! stores no value at a prefix). Every lookup applies the rule to every branch
//! it passes, for the count it runs with: [`lookup`] fails instead of
//! continuing when a branch on its path has a child outside the rule.
//!
//! Why that suffices, once every item `0..n` has been looked up. Suppose the
//! trie holds a leaf `L` at a key outside `{rlp(i) : i < n}`. Take the deepest
//! node on the path to `L` that some lookup of an item `0..n` also passed (the
//! root is one such node, so it exists). It is not a leaf: a leaf has no
//! descendant, and `L` is not an item. It is not an extension: a lookup that
//! passed an extension followed its only child, which is then also shared,
//! deeper. So it is a branch, and `L` hangs below a child slot (or sits in
//! the value slot) that no item's key goes through: the rule applied there by
//! the lookup that passed it rejects exactly that. Hence, when the ranges of
//! a block have looked up every item `0..n` against the same `n`, the trie
//! holds exactly the items `0..n`: no gap (each looked up), nothing else.
//!
//! What the pre-execution check proves on its own ([`verify_count`]): with
//! the rule, the leaf at `n - 1` is present and (because every branch on its
//! path admits no key at or past `n`) the keys that branch off that path are
//! all below `n`. Keys past `n` hanging off branches that path does not pass
//! (for instance, with `n = 128`, anything under `0x81..`, which shares no
//! branch with the path of `rlp(127) = 0x7f` below the root's child `8`) are
//! left to the ranges' lookups. That is why the ranges apply the rule with
//! the count the pre-execution check published, and why the aggregation
//! requires every range to have run with that count: a range running with a
//! larger count would admit the keys between.

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

/// The item index classes of the key `rlp(i)`: the header nibbles of the
/// encoding (`0x80` for index 0, none for a one-byte index, the length
/// prefix `0x81..0x84` otherwise), the number of nibbles of the index that
/// follow, and the index range of the class
const KEY_CLASSES: [(&[u8], usize, u32, u32); 6] = [
    (&[8, 0], 0, 0, 0),
    (&[], 2, 1, 0x7f),
    (&[8, 1], 2, 0x80, 0xff),
    (&[8, 2], 4, 0x100, 0xffff),
    (&[8, 3], 6, 0x1_0000, 0xff_ffff),
    (&[8, 4], 8, 0x100_0000, u32::MAX),
];

/// The child slots a branch at the path `prefix` may have in a trie of the
/// items `0..count`: the bit `c` is set when some `rlp(i)`, `i < count`,
/// starts with `prefix ++ [c]`. See the module documentation.
pub fn expected_children(prefix: &[u8], count: u32) -> u16 {
    let mut mask = 0u16;
    for (header, nibbles, lo, hi) in KEY_CLASSES {
        if count <= lo {
            continue;
        }
        let hi = hi.min(count - 1);
        if prefix.len() < header.len() {
            if header.starts_with(prefix) {
                mask |= 1 << header[prefix.len()];
            }
            continue;
        }
        if !prefix.starts_with(header) {
            continue;
        }
        let consumed = prefix.len() - header.len();
        if consumed >= nibbles {
            // a full key (or longer): nothing of this class hangs below it
            continue;
        }
        // the indices whose top `consumed` nibbles are the prefix's
        let mut top = 0u64;
        for nibble in &prefix[header.len()..] {
            top = (top << 4) | *nibble as u64;
        }
        let shift = 4 * (nibbles - consumed) as u64;
        let from = (top << shift).max(lo as u64);
        let to = (((top + 1) << shift) - 1).min(hi as u64);
        if from > to {
            continue;
        }
        // their next nibble runs from the one of `from` to the one of `to`
        let next = |i: u64| ((i >> (shift - 4)) & 0xf) as u16;
        for c in next(from)..=next(to) {
            mask |= 1 << c;
        }
    }
    mask
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
/// of the leaf value; `None` when the index has no leaf. Every branch on the
/// path is checked against the trie of the items `0..count` (the rule of the
/// module documentation): a child the rule does not admit is an error, not a
/// miss, so a lookup can only succeed or miss in a trie whose visited part is
/// that of a dense trie of `count` items.
pub fn lookup<'a>(
    root: &Bytes32,
    index: u32,
    count: u32,
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
        // count the items to tell a branch from a short node, noting the
        // non-empty ones (the slots of a branch)
        let (num_items, present) = {
            let mut probe = items;
            let mut num_items = 0usize;
            let mut present = 0u32;
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
                let item = probe.consumed_since(mark);
                if item != [0x80] && num_items < 32 {
                    present |= 1 << num_items;
                }
                num_items += 1;
            }
            (num_items, present)
        };
        match num_items {
            17 => {
                // the rule: only the slots of keys below `count` may hold a
                // child, and nothing may sit at a prefix
                let allowed = expected_children(&path[..consumed], count) as u32;
                if present & !allowed != 0 {
                    return Err(GlamsterdamError::IndexTrieNotDense);
                }
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

/// What the pre-execution check proves about the claimed item count of a
/// trie: the leaf at `count - 1` is present, and every branch on its path
/// admits no key at or past `count` (the rule of [`lookup`]); an empty trie
/// for a count of zero. Not a proof that the trie holds nothing else, which
/// the lookups of all the items by the ranges complete (see the module
/// documentation).
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
    if lookup(root, count - 1, count, preimages, interner, hasher)?.is_none() {
        return Err(GlamsterdamError::IndexTrie("last item missing"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The nibbles of `rlp(index)`
    fn key_nibbles(index: u32) -> Vec<u8> {
        let (key, len) = index_key(index);
        key[..len].iter().flat_map(|b| [b >> 4, b & 0xf]).collect()
    }

    /// The rule by enumeration: for every prefix of a key below `count`, the
    /// next nibbles of the keys below `count` that start with it
    fn expected_by_enumeration(count: u32) -> std::collections::HashMap<Vec<u8>, u16> {
        let mut expected = std::collections::HashMap::new();
        for index in 0..count {
            let nibbles = key_nibbles(index);
            for len in 0..nibbles.len() {
                *expected.entry(nibbles[..len].to_vec()).or_insert(0u16) |= 1 << nibbles[len];
            }
        }
        expected
    }

    #[test]
    fn expected_children_match_the_keys() {
        let counts = [
            1u32, 2, 3, 15, 16, 17, 100, 127, 128, 129, 130, 144, 145, 200, 255, 256, 257, 258,
            272, 273, 300, 512, 513, 4096, 4097, 5000, 65535, 65536, 65537, 70000,
        ];
        for count in counts {
            let expected = expected_by_enumeration(count);
            // every prefix of every key below the count (the enumeration), and
            // the prefixes of the keys just past it, which admit nothing new
            let mut prefixes: Vec<Vec<u8>> = expected.keys().cloned().collect();
            for index in count..count.saturating_add(300) {
                let nibbles = key_nibbles(index);
                for len in 1..=nibbles.len() {
                    prefixes.push(nibbles[..len].to_vec());
                }
            }
            prefixes.sort();
            prefixes.dedup();
            for prefix in prefixes {
                assert_eq!(
                    expected_children(&prefix, count),
                    expected.get(&prefix).copied().unwrap_or(0),
                    "count {count}, prefix {prefix:?}"
                );
            }
        }
        // the root of a one-item trie admits only the path of index 0
        assert_eq!(expected_children(&[], 1), 1 << 8);
        assert_eq!(expected_children(&[8], 1), 1 << 0);
        assert_eq!(expected_children(&[8, 0], 1), 0);
        // 0x00 is not a key
        assert_eq!(expected_children(&[0], 20), 0b1111_1111_1111_1110);
        assert_eq!(expected_children(&[1], 20), 0b1111);
        assert_eq!(expected_children(&[], 20), 0b1_0000_0011);
    }
}
