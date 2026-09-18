use crate::system_implementation::caches::storage_access_policy::StorageAccessPolicy;
use crate::system_implementation::ethereum_storage_model::caches::account_cache::EthereumAccountCache;
use crate::system_implementation::ethereum_storage_model::caches::account_properties::EthereumAccountProperties;
use crate::system_implementation::ethereum_storage_model::caches::full_storage_cache::EthereumStorageCache;
use crate::system_implementation::ethereum_storage_model::caches::EMPTY_STRING_KECCAK_HASH;
use crate::system_implementation::ethereum_storage_model::compare_bytes32_and_mpt_integer;
use crate::system_implementation::ethereum_storage_model::mpt::{
    BoxInternerCtor, InternerCtor, MPTInternalCapacities, Path, StackMPT, TrieKey,
};
use crate::system_implementation::ethereum_storage_model::LeafValue;
use crate::system_implementation::ethereum_storage_model::{
    EthereumMPT, InterningWordBuffer, PreimagesOracle,
};
use alloc::collections::btree_map::Entry;
use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use core::alloc::Allocator;
use core::mem::MaybeUninit;
use crypto::sha3::Keccak256;
use crypto::MiniDigest;
use zk_ee::internal_error;
use zk_ee::memory::stack_trait::StackFactory;
use zk_ee::oracle::query_ids::STATE_AND_MERKLE_PATHS_SUBSPACE_MASK;
use zk_ee::oracle::IOOracle;
use zk_ee::system::errors::internal::InternalError;
use zk_ee::system::logger::Logger;
use zk_ee::system::{IOResultKeeper, Resources};
use zk_ee::types_config::EthereumIOTypesConfig;
use zk_ee::utils::{Bytes32, USIZE_SIZE};

use super::vec_trait::VecLikeCtor;

struct OracleProxy<'o, O: IOOracle>(&'o mut O);

pub const ETHEREUM_MPT_PREIMAGE_BYTE_LEN_QUERY_ID: u32 =
    STATE_AND_MERKLE_PATHS_SUBSPACE_MASK | 0x81;
pub const ETHEREUM_MPT_PREIMAGE_WORDS_QUERY_ID: u32 = STATE_AND_MERKLE_PATHS_SUBSPACE_MASK | 0x82;

const LEAF_VALUE_PRE_ENCODING_MAX_LEN: usize = 34;

impl<'o, O: IOOracle> PreimagesOracle for OracleProxy<'o, O> {
    fn provide_preimage<'a, I: super::Interner<'a> + 'a>(
        &mut self,
        key: &[u8; 32],
        interner: &'_ mut I,
    ) -> Result<&'a [u8], ()> {
        let key = Bytes32::from_array(*key);
        // first length
        let expected_bytes: u32 = self
            .0
            .query_serializable(ETHEREUM_MPT_PREIMAGE_BYTE_LEN_QUERY_ID, &key)
            .map_err(|_| ())?;
        let words_buffer_size = (expected_bytes as usize).next_multiple_of(USIZE_SIZE) / USIZE_SIZE;
        assert!(I::SUPPORTS_WORD_LEVEL_INTERNING);
        // NOTE: we leave some slack for 64/32 bit arch mismatches
        let mut buffer = interner.get_word_buffer(words_buffer_size.next_multiple_of(2))?;
        let capacity = buffer.spare_capacity_mut();
        let num_written = self
            .0
            .expose_preimage(ETHEREUM_MPT_PREIMAGE_WORDS_QUERY_ID, &key, capacity)
            .map_err(|_| ())?;
        unsafe {
            buffer.set_word_len(num_written);
        }

        Ok(buffer.flush_as_bytes(expected_bytes as usize))
    }
}

#[derive(Default)]
pub struct EthereumStoragePersister;

pub fn digits_from_key(key: &[u8; 32]) -> [u8; 64] {
    let mut result = [0u8; 64];
    for (src, dst) in key.iter().zip(result.as_chunks_mut::<2>().0.iter_mut()) {
        let low = *src & 0x0f;
        let high = *src >> 4;
        dst[0] = high;
        dst[1] = low;
    }

    result
}

pub struct MPTWithInterner<'a, A: Allocator + Clone + 'a, VC: VecLikeCtor, IC: InternerCtor<A>> {
    interner: IC::Interner<'a>,
    mpt: EthereumMPT<'a, A, VC>,
    allocator: A,
}

impl<'a, A: Allocator + Clone + 'a, VC: VecLikeCtor, IC: InternerCtor<A>>
    MPTWithInterner<'a, A, VC, IC>
{
    const INTERNER_DEFAULT_CAPACITY: usize = 1 << 25; // 32 Mb

    pub fn new_in(allocator: A) -> Self {
        let interner =
            IC::make_interner_with_capacity_in(Self::INTERNER_DEFAULT_CAPACITY, allocator.clone());
        let capacities = MPTInternalCapacities::new_in(allocator.clone());
        let mpt = EthereumMPT::empty_with_preallocated_capacities(capacities, allocator.clone());

        Self {
            interner,
            mpt,
            allocator,
        }
    }

    pub fn reinit_with_root<'b>(self, root_hash: [u8; 32]) -> MPTWithInterner<'b, A, VC, IC>
    where
        A: 'a + 'b,
    {
        let Self {
            interner,
            mpt,
            allocator,
        } = self;

        let capacities = mpt.deconstruct_to_reuse_capacity();
        let interner = IC::purge(interner);

        let mpt = EthereumMPT::empty_with_preallocated_capacities(capacities, allocator.clone());
        let mut new = MPTWithInterner {
            mpt,
            interner,
            allocator,
        };

        new.mpt
            .set_root(root_hash, &mut new.interner)
            .expect("must set initial root");

        new
    }

    pub fn get(
        &mut self,
        path: Path<'_>,
        preimages_oracle: &mut impl PreimagesOracle,
        hasher: &mut crypto::sha3::Keccak256,
    ) -> Result<&'a [u8], ()> {
        self.mpt
            .get(path, preimages_oracle, &mut self.interner, hasher)
    }

    pub fn root(
        &self,
        hasher: &mut impl MiniDigest<HashOutput: core::ops::Deref<Target = [u8; 32]>>,
    ) -> [u8; 32] {
        self.mpt.root(hasher)
    }

    pub fn recompute(
        &mut self,
        preimages_oracle: &mut impl PreimagesOracle,
        hasher: &mut impl MiniDigest<HashOutput: core::ops::Deref<Target = [u8; 32]>>,
    ) -> Result<(), ()> {
        self.mpt
            .recompute(preimages_oracle, &mut self.interner, hasher)
    }

    pub fn update(&mut self, path: Path<'_>, pre_encoded_value: &[u8]) -> Result<(), ()> {
        self.mpt.update(path, pre_encoded_value, &mut self.interner)
    }

    pub fn delete(&mut self, path: Path<'_>) -> Result<(), ()> {
        self.mpt.delete(path)
    }

    pub fn insert(
        &mut self,
        path: Path<'_>,
        pre_encoded_value: &[u8],
        preimages_oracle: &mut impl PreimagesOracle,
        hasher: &mut impl MiniDigest<HashOutput: core::ops::Deref<Target = [u8; 32]>>,
    ) -> Result<(), ()> {
        self.mpt.insert(
            path,
            pre_encoded_value,
            preimages_oracle,
            &mut self.interner,
            hasher,
        )
    }

    pub fn insert_lazy_value(
        &mut self,
        path: Path<'_>,
        value: LeafValue<'a>,
        preimages_oracle: &mut impl PreimagesOracle,
        hasher: &mut impl MiniDigest<HashOutput: core::ops::Deref<Target = [u8; 32]>>,
    ) -> Result<(), ()> {
        self.mpt
            .insert_lazy_value(path, value, preimages_oracle, &mut self.interner, hasher)
    }
}

struct SlotUpdate {
    key: TrieKey,
    initial: Bytes32,
    current: Bytes32,
}

pub struct SortedMPTWithInterner<'a, A: Allocator + Clone + 'a, IC: InternerCtor<A>> {
    interner: IC::Interner<'a>,
    mpt: StackMPT<'a, A>,
}

impl<'a, A: Allocator + Clone + 'a, IC: InternerCtor<A>> SortedMPTWithInterner<'a, A, IC> {
    const INTERNER_DEFAULT_CAPACITY: usize = 1 << 25; // 32 Mb

    pub fn new_in(allocator: A) -> Self {
        let interner =
            IC::make_interner_with_capacity_in(Self::INTERNER_DEFAULT_CAPACITY, allocator.clone());
        let mpt = StackMPT::new_in(allocator);

        Self { interner, mpt }
    }

    pub fn reinit_with_root<'b>(self, root_hash: [u8; 32]) -> SortedMPTWithInterner<'b, A, IC>
    where
        A: 'a + 'b,
    {
        let Self { interner, mpt } = self;
        let mut interner = IC::purge(interner);
        let mut mpt = mpt.purge_reborrow();
        mpt.set_root(&root_hash, &mut interner)
            .expect("must set initial root");

        SortedMPTWithInterner { interner, mpt }
    }

    pub fn seek(
        &mut self,
        key: TrieKey,
        preimages_oracle: &mut impl PreimagesOracle,
        hasher: &mut impl MiniDigest<HashOutput: core::ops::Deref<Target = [u8; 32]>>,
    ) -> Result<Option<&'a [u8]>, ()> {
        self.mpt
            .seek(key, preimages_oracle, &mut self.interner, hasher)
    }

    pub fn set(
        &mut self,
        value: &[u8],
        hasher: &mut impl MiniDigest<HashOutput: core::ops::Deref<Target = [u8; 32]>>,
    ) -> Result<(), ()> {
        self.mpt.set(value, &mut self.interner, hasher)
    }

    pub fn delete(&mut self) -> Result<(), ()> {
        self.mpt.delete()
    }

    pub fn finalize(
        &mut self,
        preimages_oracle: &mut impl PreimagesOracle,
        hasher: &mut impl MiniDigest<HashOutput: core::ops::Deref<Target = [u8; 32]>>,
    ) -> Result<[u8; 32], ()> {
        self.mpt
            .finalize(preimages_oracle, &mut self.interner, hasher)
    }
}

impl EthereumStoragePersister {
    fn cache_slot_trie_key<A: Allocator + Clone>(
        slot: &Bytes32,
        cache: &mut BTreeMap<Bytes32, TrieKey, A>,
        hasher: &mut Keccak256,
    ) -> TrieKey {
        match cache.entry(*slot) {
            Entry::Occupied(e) => *e.get(),
            Entry::Vacant(e) => {
                hasher.update(slot.as_u8_array_ref());
                let key = TrieKey::from_hash(&hasher.finalize_reset());
                *e.insert(key)
            }
        }
    }

    // TODO: consider to make it lazy
    fn encode_slot_value<'a>(
        value: &Bytes32,
        buffer: &'a mut [MaybeUninit<u8>; LEAF_VALUE_PRE_ENCODING_MAX_LEN],
    ) -> &'a [u8] {
        // NOTE: we need to actually do value -> shortest BE slice,
        // then encode such slice as RLP, and then again encode such slice as RLP to place it directly into leaf
        let byte_len = value.num_trailing_nonzero_bytes();
        let offset;
        if byte_len == 0 {
            // rlp(rlp([])) = rlp([0x80]) = 0x81, 0x80
            buffer[0].write(0x81);
            buffer[1].write(0x80);
            offset = 2;
        } else if byte_len == 1 {
            let b = value.as_u8_array_ref()[31];
            if b < 0x80 {
                // rlp(rlp([0x01])) = rlp([0x01]) = 0x01
                buffer[0].write(b);
                offset = 1;
            } else {
                // rlp(rlp([0xff])) = rlp([0x81, 0xff]) = 0x82, 0x81, 0xff
                buffer[0].write(0x80 + 2);
                buffer[1].write(0x80 + 1);
                buffer[2].write(b);
                offset = 3;
            }
        } else {
            // "inner" slice is at most 32 bytes, so outer is at most 33
            buffer[0].write(0x81 + (byte_len as u8));
            buffer[1].write(0x80 + (byte_len as u8));
            buffer[2..][..byte_len]
                .write_copy_of_slice(&value.as_u8_array_ref()[(32 - byte_len)..]);
            offset = 2 + byte_len;
        }
        assert!(offset <= LEAF_VALUE_PRE_ENCODING_MAX_LEN);

        unsafe { core::slice::from_raw_parts(buffer.as_ptr().cast::<u8>().cast(), offset) }
    }

    #[allow(dead_code)]
    fn encode_slot_value_inner<'a>(
        value: &Bytes32,
        buffer: &'a mut [MaybeUninit<u8>; 33],
    ) -> &'a [u8] {
        let byte_len = value.num_trailing_nonzero_bytes();
        let offset;
        if byte_len == 0 {
            buffer[0].write(0x80);
            offset = 1;
        } else if byte_len == 1 {
            let b = value.as_u8_array_ref()[31];
            if b < 0x80 {
                buffer[0].write(b);
                offset = 1;
            } else {
                buffer[0].write(0x80 + 1);
                buffer[1].write(b);
                offset = 2;
            }
        } else {
            buffer[0].write(0x80 + (byte_len as u8));
            buffer[1..][..byte_len]
                .write_copy_of_slice(&value.as_u8_array_ref()[(32 - byte_len)..]);
            offset = 1 + byte_len;
        }
        assert!(offset <= 33);

        unsafe { core::slice::from_raw_parts(buffer.as_ptr().cast::<u8>().cast(), offset) }
    }

    pub fn persist_changes<
        A: Allocator + Clone + Default,
        R: Resources,
        P: StorageAccessPolicy<R, Bytes32>,
        SF: StackFactory<N>,
        const N: usize,
    >(
        &mut self,
        account_cache: &mut EthereumAccountCache<A, R, SF, N>,
        storage_cache: &EthereumStorageCache<A, SF, N, R, P>,
        initial_state_root: &Bytes32,
        oracle: &mut impl IOOracle,
        logger: &mut impl Logger,
        result_keeper: &mut impl IOResultKeeper<EthereumIOTypesConfig>,
        allocator: A,
    ) -> Result<Bytes32, InternalError> {
        let _ = logger.write_fmt(format_args!("Beginning MTP updates\n"));

        let mut preimage_oracle = OracleProxy(oracle);
        let mut key_cache = BTreeMap::<Bytes32, TrieKey, A>::new_in(allocator.clone());
        let mut hasher = crypto::sha3::Keccak256::new();

        let mut reusable_mpt =
            SortedMPTWithInterner::<A, BoxInternerCtor>::new_in(allocator.clone());

        use crate::system_implementation::ethereum_storage_model::caches::account_properties::ACCOUNT_LEAF_VALUE_PRE_ENCODING_MAX_LEN;
        let mut account_data_encoding_buffer =
            [MaybeUninit::uninit(); ACCOUNT_LEAF_VALUE_PRE_ENCODING_MAX_LEN];
        let mut slot_value_encoding_buffer =
            [MaybeUninit::uninit(); LEAF_VALUE_PRE_ENCODING_MAX_LEN];

        // Storage tries: slots are grouped by address (the cache is ordered by address, then by slot),
        // and every group is sorted by the trie key to walk the trie in a single pass
        // NOTE: allocations can not grow in the proving environment, so we pre-allocate for the
        // worst case: all the accesses belong to a single account
        let mut slot_updates =
            Vec::<SlotUpdate, A>::with_capacity_in(storage_cache.num_accesses(), allocator.clone());
        let mut accesses = storage_cache.net_accesses_iter().peekable();

        while let Some((first_key, _)) = accesses.peek() {
            let active_address = first_key.address;
            slot_updates.clear();
            while let Some((addr, value)) = accesses.next_if(|(k, _)| k.address == active_address) {
                if value.initial_value_used {
                    let key = Self::cache_slot_trie_key(&addr.key, &mut key_cache, &mut hasher);
                    slot_updates.push(SlotUpdate {
                        key,
                        initial: value.initial_value,
                        current: value.current_value,
                    });
                } else {
                    let _ = logger.write_fmt(format_args!(
                        "Value for address 0x{:040x}, slot {:?} is unobservable\n",
                        addr.address.as_uint(),
                        addr.key,
                    ));
                }
            }
            if slot_updates.is_empty() {
                continue;
            }
            slot_updates.sort_unstable_by_key(|update| update.key);

            let _ = logger.write_fmt(format_args!(
                "Processing {} storage accesses for address 0x{:040x}\n",
                slot_updates.len(),
                active_address.as_uint()
            ));

            let mut entry = account_cache
                .cache
                .get_mut((&active_address).into())
                .expect("account with storage address must be cached");
            let initial_root = entry.current().value().storage_root;
            debug_assert!(
                initial_root.is_zero() == false,
                "storage root can not be zero"
            );

            reusable_mpt = reusable_mpt.reinit_with_root(initial_root.as_u8_array());

            let mut any_mutation = false;
            for update in slot_updates.iter() {
                let existing = reusable_mpt
                    .seek(update.key, &mut preimage_oracle, &mut hasher)
                    .map_err(|_| internal_error!("failed to get initial value in MPT"))?;
                let existing = existing.unwrap_or(&[]);
                assert!(
                    compare_bytes32_and_mpt_integer(&update.initial, existing),
                    "failed to compare expected storage slot value {:?} vs RLP encoded {:?}\n",
                    update.initial,
                    existing
                );

                if update.initial == update.current {
                    continue;
                }
                any_mutation = true;
                if update.current.is_zero() {
                    reusable_mpt
                        .delete()
                        .map_err(|_| internal_error!("failed to delete value from MPT"))?;
                } else {
                    let pre_encoded_value =
                        Self::encode_slot_value(&update.current, &mut slot_value_encoding_buffer);
                    reusable_mpt
                        .set(pre_encoded_value, &mut hasher)
                        .map_err(|_| internal_error!("failed to set value in MPT"))?;
                }
            }

            entry.element_properties_mut().mark_value_as_observed();

            if any_mutation {
                let new_root = reusable_mpt
                    .finalize(&mut preimage_oracle, &mut hasher)
                    .map_err(|_| internal_error!("failed to compute new root for MPT"))?;
                let new_root = Bytes32::from_array(new_root);

                let _ = logger.write_fmt(format_args!(
                    "New storage root for address 0x{:040x} is {:?}\n",
                    active_address.as_uint(),
                    new_root,
                ));

                assert_ne!(new_root, initial_root);
                entry.update(|v| {
                    v.update(|v, _m| {
                        v.storage_root = new_root;

                        Ok(())
                    })
                })?;
            } else {
                let _ = logger.write_fmt(format_args!(
                    "Storage root of 0x{:040x} will remain unchanged\n",
                    active_address.as_uint(),
                ));
            }
        }

        let _ = logger.write_fmt(format_args!("Will update accounts MTP now\n",));

        // Accounts trie: observed accounts sorted by the trie key
        let mut accounts_mpt = reusable_mpt.reinit_with_root(initial_state_root.as_u8_array());

        let mut account_updates =
            Vec::with_capacity_in(account_cache.cache.iter().len(), allocator.clone());
        for record in account_cache.cache.iter() {
            if record.key_properties().is_value_observed() == false {
                // whatever it was - it's unobservable, we can just skip it
                assert_eq!(record.initial().value(), record.current().value());
                continue;
            }
            hasher.update(record.key().0.to_be_bytes::<20>());
            let key = TrieKey::from_hash(&hasher.finalize_reset());
            account_updates.push((key, record));
        }
        account_updates.sort_unstable_by_key(|(key, _)| *key);

        for (key, record) in account_updates.iter() {
            let addr = record.key();
            let key_properties = record.key_properties();
            let initial = record.initial();
            let current = record.current();
            let current_metadata = current.metadata();
            assert!(
                current_metadata.is_marked_for_deconstruction == false,
                "Account 0x{:040x} was marked for deconstruction, but it was not completed",
                addr.0.as_uint()
            );

            let _ = logger.write_fmt(format_args!(
                "Updating the state of address 0x{:040x}\n",
                addr.0.as_uint()
            ));

            // we need to check that initial value is the one we claimed in cache
            let existing = accounts_mpt
                .seek(*key, &mut preimage_oracle, &mut hasher)
                .map_err(|_| internal_error!("failed to get initial account value in MPT"))?;

            if key_properties.is_new_element() {
                // check that it's empty
                assert!(existing.is_none());

                let current = current.value();
                if current == &EthereumAccountProperties::EMPTY_ACCOUNT
                    || current == &EthereumAccountProperties::EMPTY_BUT_EXISTING_ACCOUNT
                {
                    // empty -> observed -> empty
                    continue;
                }

                let _ = logger.write_fmt(format_args!(
                    "Will insert new account at address 0x{:040x}\n",
                    addr.0.as_uint()
                ));

                let mut current_value = *current;
                if current_value.bytecode_hash.is_zero() {
                    // if account was created, but bytecode was never touched, then we should
                    // put proper value instead of 0
                    current_value.bytecode_hash = EMPTY_STRING_KECCAK_HASH;
                }

                let pre_encoded_value =
                    current_value.rlp_encode_for_leaf(&mut account_data_encoding_buffer);
                result_keeper.account_state_opaque_encoding(&addr.0, pre_encoded_value);
                accounts_mpt
                    .set(pre_encoded_value, &mut hasher)
                    .map_err(|_| internal_error!("failed to insert account value into MPT"))?;
            } else {
                let existing = existing.expect("existing account must have a leaf");
                let parsed = EthereumAccountProperties::parse_from_rlp_bytes(existing)
                    .map_err(|_| internal_error!("failed to parse initial account value"))?;

                let initial = initial.value();
                let current = current.value();

                debug_assert!(
                    initial.bytecode_hash.is_zero() == false,
                    "bytecode hash must not be zero for retrieved account"
                );
                debug_assert!(
                    initial.storage_root.is_zero() == false,
                    "storage root hash must not be zero for retrieved account"
                );
                assert_eq!(initial, &parsed);

                debug_assert!(current.bytecode_hash.is_zero() == false);

                if initial == current {
                    result_keeper.account_state_opaque_encoding(&addr.0, existing);
                    continue;
                }

                if current == &EthereumAccountProperties::EMPTY_ACCOUNT
                    || current == &EthereumAccountProperties::EMPTY_BUT_EXISTING_ACCOUNT
                {
                    let _ = logger.write_fmt(format_args!(
                        "Will delete leaf for address 0x{:040x}\n",
                        addr.0.as_uint()
                    ));

                    accounts_mpt
                        .delete()
                        .map_err(|_| internal_error!("failed to delete account from MPT"))?;
                } else {
                    let _ = logger.write_fmt(format_args!(
                        "Will update account state at address 0x{:040x}\n",
                        addr.0.as_uint()
                    ));

                    let pre_encoded_value =
                        current.rlp_encode_for_leaf(&mut account_data_encoding_buffer);
                    result_keeper.account_state_opaque_encoding(&addr.0, pre_encoded_value);
                    accounts_mpt
                        .set(pre_encoded_value, &mut hasher)
                        .map_err(|_| internal_error!("failed to update account value in MPT"))?;
                }
            }
        }

        let _ = logger.write_fmt(format_args!("Will recompute state root\n",));

        let root = accounts_mpt
            .finalize(&mut preimage_oracle, &mut hasher)
            .map_err(|_| internal_error!("failed to compute new state root for MPT"))?;

        let _ = logger.write_fmt(format_args!("State MTP was updated\n",));

        Ok(Bytes32::from_array(root))
    }
}
