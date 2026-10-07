//! Applies the access list to the state with the stack MPT.
//!
//! Accounts are visited in trie key order, and the slots of an account in
//! their trie key order, so every trie is walked in a single pass: the
//! accounts trie stays open at the account while its storage trie is walked,
//! and the account leaf is rewritten with the new storage root afterwards.
//!
//! Nothing is decoded or re-encoded on the way: the pre-block leaves are kept
//! as they come out of the trie (the RLP item of the leaf value), appended to
//! the compact streams of [`StateConsistencyOutput`](super::StateConsistencyOutput)
//! as they are visited; the post-block slot values go into the trie as slices
//! of the access list encoding (the RLP integer item is the leaf's payload,
//! under a one-byte string header); and the post-block account leaf is
//! assembled from the nonce and balance items of the witness, the new storage
//! root and the code hash.

use super::super::GlamsterdamError;
use super::{
    SlotAccess, Tables, ACCOUNT_BALANCE_CHANGED, ACCOUNT_CODE_CHANGED, ACCOUNT_NONCE_CHANGED,
    SLOT_WRITTEN,
};
use crate::bootloader::transaction::rlp_encoded::rlp::minimal_rlp_parser::Rlp;
use alloc::vec::Vec;
use basic_system::system_implementation::ethereum_storage_model::caches::EMPTY_STRING_KECCAK_HASH;
use basic_system::system_implementation::ethereum_storage_model::{
    BoxInternerCtor, MptPreimagesOracle, SortedMPTWithInterner, TrieKey, EMPTY_ROOT_HASH,
};
use core::alloc::Allocator;
use core::mem::MaybeUninit;
use crypto::sha3::Keccak256;
use zk_ee::oracle::IOOracle;
use zk_ee::utils::Bytes32;

/// Capacity of the node interner of the accounts trie
const ACCOUNTS_INTERNER_CAPACITY: usize = 1 << 24;
/// Capacity of the node interner of the storage tries (one account at a time)
const STORAGE_INTERNER_CAPACITY: usize = 1 << 22;

/// The longest RLP item of an account leaf: the list header and four strings
/// (a nonce of up to 8 bytes, a balance of up to 32 bytes, two hashes)
pub const MAX_ACCOUNT_LEAF_ITEM_LEN: usize = 2 + (1 + 8) + (1 + 32) + 33 + 33;
/// The longest RLP item of a slot leaf: a 32-byte integer
pub const MAX_SLOT_LEAF_ITEM_LEN: usize = 33;
/// The longest entry of the account stream (length byte + item)
pub const MAX_ACCOUNT_ENTRY_LEN: usize = 1 + MAX_ACCOUNT_LEAF_ITEM_LEN;
/// The longest entry of the slot stream (length byte + item)
pub const MAX_SLOT_ENTRY_LEN: usize = 1 + MAX_SLOT_LEAF_ITEM_LEN;
/// Length of the account leaf value with its string envelope, at most
const MAX_ACCOUNT_LEAF_ENCODING_LEN: usize = 2 + MAX_ACCOUNT_LEAF_ITEM_LEN;

const _: () = assert!(MAX_ACCOUNT_LEAF_ITEM_LEN < 256 && MAX_SLOT_LEAF_ITEM_LEN < 256);

/// `rlp(0)`: the item of a zero integer
const ZERO_ITEM: &[u8] = &[0x80];

/// The fields of an account leaf, as the RLP items of its list
struct AccountLeaf<'a> {
    nonce: &'a [u8],
    balance: &'a [u8],
    storage_root: &'a [u8; 32],
    code_hash: &'a [u8; 32],
}

impl<'a> AccountLeaf<'a> {
    const EMPTY: AccountLeaf<'static> = AccountLeaf {
        nonce: ZERO_ITEM,
        balance: ZERO_ITEM,
        storage_root: EMPTY_ROOT_HASH.as_u8_array_ref(),
        code_hash: EMPTY_STRING_KECCAK_HASH.as_u8_array_ref(),
    };

    /// Splits the RLP item of a leaf value, `[nonce, balance, storage_root, code_hash]`
    fn parse(item: &'a [u8]) -> Result<Self, GlamsterdamError> {
        let error = GlamsterdamError::AccountEncoding;
        let mut outer = Rlp::new(item);
        let mut r = outer.list().map_err(|_| error)?;
        if !outer.is_empty() {
            return Err(error);
        }
        let mut integer_item = |max_len: usize| {
            let mark = r.mark();
            let digits = r.bytes().map_err(|_| error)?;
            if digits.len() > max_len || digits.first() == Some(&0) {
                return Err(error);
            }
            Ok(r.consumed_since(mark))
        };
        let nonce = integer_item(8)?;
        let balance = integer_item(32)?;
        let storage_root = r.bytes_exact::<32>().map_err(|_| error)?;
        let code_hash = r.bytes_exact::<32>().map_err(|_| error)?;
        if !r.is_empty() {
            return Err(error);
        }
        Ok(Self {
            nonce,
            balance,
            storage_root,
            code_hash,
        })
    }

    fn is_empty(&self) -> bool {
        self.nonce == ZERO_ITEM
            && self.balance == ZERO_ITEM
            && self.code_hash == EMPTY_STRING_KECCAK_HASH.as_u8_array_ref()
            && self.storage_root == EMPTY_ROOT_HASH.as_u8_array_ref()
    }

    /// The leaf value: the RLP list of the fields inside its string envelope
    /// (what the trie stores), returned with the envelope
    fn encode<'b>(
        &self,
        buffer: &'b mut [MaybeUninit<u8>; MAX_ACCOUNT_LEAF_ENCODING_LEN],
    ) -> &'b [u8] {
        let mut len = 0usize;
        let mut put = |bytes: &[u8]| {
            buffer[len..][..bytes.len()].write_copy_of_slice(bytes);
            len += bytes.len();
        };
        let payload = self.nonce.len() + self.balance.len() + 33 + 33;
        // the two hashes alone make the list longer than 55 bytes
        put(&[0xb8, (2 + payload) as u8, 0xf8, payload as u8]);
        put(self.nonce);
        put(self.balance);
        put(&[0x80 + 32]);
        put(self.storage_root);
        put(&[0x80 + 32]);
        put(self.code_hash);
        // SAFETY: the first `len` bytes were written above
        unsafe { core::slice::from_raw_parts(buffer.as_ptr().cast::<u8>(), len) }
    }
}

/// Appends `length byte || item` to a stream that has room for it. A missing
/// leaf is an entry of length zero.
#[inline(always)]
fn push_entry<A: Allocator>(
    stream: &mut Vec<u8, A>,
    item: Option<&[u8]>,
) -> Result<u32, GlamsterdamError> {
    let offset = stream.len() as u32;
    let item = item.unwrap_or(&[]);
    if item.len() >= 256 || stream.capacity() - stream.len() < 1 + item.len() {
        return Err(GlamsterdamError::Capacity("initial value stream"));
    }
    stream.push(item.len() as u8);
    stream.extend_from_slice(item);
    Ok(offset)
}

/// The initial value streams and their offset tables, written in trie order
pub(super) struct InitialValues<'t, A: Allocator> {
    pub accounts: &'t mut Vec<u8, A>,
    pub account_offsets: &'t mut Vec<u32, A>,
    pub fresh_accounts: &'t mut [u8],
    pub slots: &'t mut Vec<u8, A>,
    pub slot_offsets: &'t mut Vec<u32, A>,
}

/// Walks the tries and returns the new state root, filling the initial values
/// along the way. `encoding` is the access list the tables' slices point into.
/// The offset tables must be empty, with the capacity of the account and slot
/// counts: they are written by index and their lengths set at the end.
#[allow(clippy::too_many_arguments)]
pub(super) fn apply_access_list<'e, O: IOOracle, A: Allocator + Clone>(
    oracle: &mut O,
    parent_state_root: &Bytes32,
    encoding: &'e [u8],
    tables: &mut Tables<A>,
    initial: InitialValues<'_, A>,
    max_slots_of_account: usize,
    hasher: &mut Keccak256,
    allocator: A,
) -> Result<Bytes32, GlamsterdamError> {
    use GlamsterdamError::Mpt;

    let num_accounts = tables.accounts.len();
    let num_slots = tables.slots.len();
    debug_assert!(initial.account_offsets.is_empty() && initial.slot_offsets.is_empty());
    if initial.account_offsets.capacity() < num_accounts
        || initial.slot_offsets.capacity() < num_slots
        || initial.fresh_accounts.len() < num_accounts
    {
        return Err(GlamsterdamError::Capacity("initial value tables"));
    }
    let account_offsets = &mut initial.account_offsets.spare_capacity_mut()[..num_accounts];
    let slot_offsets = &mut initial.slot_offsets.spare_capacity_mut()[..num_slots];

    let mut preimages = MptPreimagesOracle(oracle);
    let mut account_encoding_buffer = [MaybeUninit::uninit(); MAX_ACCOUNT_LEAF_ENCODING_LEN];

    // the accounts trie is walked by trie key
    tables
        .address_hashes
        .sort_unstable_by(|a, b| a.0.as_u8_array_ref().cmp(b.0.as_u8_array_ref()));

    let mut accounts_mpt = SortedMPTWithInterner::<A, BoxInternerCtor>::new_with_capacity_in(
        ACCOUNTS_INTERNER_CAPACITY,
        allocator.clone(),
    )
    .reinit_with_root(parent_state_root.as_u8_array());
    let mut storage_mpt = SortedMPTWithInterner::<A, BoxInternerCtor>::new_with_capacity_in(
        STORAGE_INTERNER_CAPACITY,
        allocator.clone(),
    );
    // the slots of one account, as positions in `tables.slots`, sorted by trie key
    let mut slot_order = Vec::<u32, A>::with_capacity_in(max_slots_of_account, allocator);

    for (address_hash, address_index) in tables.address_hashes.iter() {
        let address_index = *address_index as usize;
        let access = tables.accounts[address_index];

        let existing = accounts_mpt
            .seek(
                TrieKey::from_hash(address_hash.as_u8_array_ref()),
                &mut preimages,
                hasher,
            )
            .map_err(|_| Mpt("seek account"))?;
        let initial_leaf = match existing {
            Some(item) => AccountLeaf::parse(item)?,
            None => AccountLeaf::EMPTY,
        };
        account_offsets[address_index].write(push_entry(initial.accounts, existing)?);
        initial.fresh_accounts[address_index] = existing.is_none() as u8;

        // the storage trie
        let slots = access.slots_start as usize..access.slots_end as usize;
        let mut storage_root = *initial_leaf.storage_root;
        if !slots.is_empty() {
            storage_mpt = storage_mpt.reinit_with_root(storage_root);
            slot_order.clear();
            slot_order.extend(slots.clone().map(|position| position as u32));
            let slot_entries: &[SlotAccess] = &tables.slots;
            let key_hashes = &tables.slot_key_hashes;
            slot_order.sort_unstable_by(|a, b| {
                let a = &key_hashes[slot_entries[*a as usize].key_index as usize].0;
                let b = &key_hashes[slot_entries[*b as usize].key_index as usize].0;
                a.as_u8_array_ref().cmp(b.as_u8_array_ref())
            });

            let mut any_write = false;
            for position in slot_order.iter() {
                let position = *position as usize;
                let slot = slot_entries[position];
                let key =
                    TrieKey::from_hash(key_hashes[slot.key_index as usize].0.as_u8_array_ref());
                // the integer item of the existing value, absent for zero
                let existing = storage_mpt
                    .seek(key, &mut preimages, hasher)
                    .map_err(|_| Mpt("seek slot"))?;
                if let Some(item) = existing {
                    if item.len() > MAX_SLOT_LEAF_ITEM_LEN {
                        return Err(GlamsterdamError::SlotEncoding);
                    }
                }
                slot_offsets[position].write(push_entry(initial.slots, existing)?);
                if slot.flags & SLOT_WRITTEN == 0 {
                    continue;
                }
                let post = slot.post_value.get(encoding);
                if post == ZERO_ITEM {
                    if existing.is_some() {
                        any_write = true;
                        storage_mpt.delete().map_err(|_| Mpt("delete slot"))?;
                    }
                } else if existing != Some(post) {
                    any_write = true;
                    // the leaf value is the integer item as a string: a single byte below
                    // 0x80 is its own string, anything else gets a one-byte header
                    let header = [0x80 + post.len() as u8];
                    let header: &[u8] = if post.len() == 1 && post[0] < 0x80 {
                        &[]
                    } else {
                        &header
                    };
                    storage_mpt
                        .set_borrowed(header, post, hasher)
                        .map_err(|_| Mpt("set slot"))?;
                }
            }
            if any_write {
                storage_root = storage_mpt
                    .finalize(&mut preimages, hasher)
                    .map_err(|_| Mpt("finalize storage"))?;
            }
        }

        // the post-block account
        let post = AccountLeaf {
            nonce: if access.flags & ACCOUNT_NONCE_CHANGED != 0 {
                access.post_nonce.get(encoding)
            } else {
                initial_leaf.nonce
            },
            balance: if access.flags & ACCOUNT_BALANCE_CHANGED != 0 {
                access.post_balance.get(encoding)
            } else {
                initial_leaf.balance
            },
            storage_root: &storage_root,
            code_hash: if access.flags & ACCOUNT_CODE_CHANGED != 0 {
                access.post_code_hash.as_u8_array_ref()
            } else {
                initial_leaf.code_hash
            },
        };

        if post.is_empty() {
            // EIP-161: an empty account has no leaf
            if existing.is_some() {
                accounts_mpt.delete().map_err(|_| Mpt("delete account"))?;
            }
        } else {
            let leaf = post.encode(&mut account_encoding_buffer);
            // the leaf changes unless the item (after the two envelope bytes) is the existing one
            if existing != Some(&leaf[2..]) {
                accounts_mpt
                    .set(leaf, hasher)
                    .map_err(|_| Mpt("set account"))?;
            }
        }
    }

    // SAFETY: `address_hashes` holds every address index exactly once (the
    // interner assigns 0..num_accounts in order, and `collect` rejects a
    // repeated address), so every account offset was written above; the slot
    // ranges of the accounts partition 0..num_slots, and every position of
    // every range was visited, so every slot offset was written too.
    unsafe {
        initial.account_offsets.set_len(num_accounts);
        initial.slot_offsets.set_len(num_slots);
    }

    let root = accounts_mpt
        .finalize(&mut preimages, hasher)
        .map_err(|_| Mpt("finalize accounts"))?;
    Ok(Bytes32::from_array(root))
}
