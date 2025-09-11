use crate::system_implementation::cache_structs::storage_values::StorageAccessPolicy;
use crate::system_implementation::cache_structs::BitsOrd160;
use crate::system_implementation::ethereum_storage_model::caches::account_cache::EthereumAccountCache;
use crate::system_implementation::ethereum_storage_model::caches::account_properties::EthereumAccountProperties;
use crate::system_implementation::ethereum_storage_model::caches::full_storage_cache::EthereumStorageCache;
use crate::system_implementation::ethereum_storage_model::caches::EMPTY_STRING_KECCAK_HASH;
use crate::system_implementation::ethereum_storage_model::compare_bytes32_and_mpt_integer;
use crate::system_implementation::ethereum_storage_model::mpt::{
    BoxInternerCtor, InternerCtor, MPTInternalCapacities, Path,
};
use crate::system_implementation::ethereum_storage_model::LeafValue;
use crate::system_implementation::ethereum_storage_model::{
    EthereumMPT, InterningWordBuffer, PreimagesOracle,
};
use alloc::collections::btree_map::Entry;
use alloc::collections::BTreeMap;
use core::alloc::Allocator;
use core::mem::MaybeUninit;
use crypto::sha3::Keccak256;
use crypto::MiniDigest;
use zk_ee::common_structs::*;
use zk_ee::internal_error;
use zk_ee::memory::stack_trait::StackCtor;
use zk_ee::memory::vec_trait::VecLikeCtor;
use zk_ee::system::errors::internal::InternalError;
use zk_ee::system::logger::Logger;
use zk_ee::system::{IOResultKeeper, Resources};
use zk_ee::system_io_oracle::{IOOracle, STATE_AND_MERKLE_PATHS_SUBSPACE_MASK};
use zk_ee::types_config::EthereumIOTypesConfig;
use zk_ee::utils::{Bytes32, USIZE_SIZE};

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
        // first length
        let expected_bytes: u32 = self
            .0
            .query_serializable(
                ETHEREUM_MPT_PREIMAGE_BYTE_LEN_QUERY_ID,
                &Bytes32::from_array(*key),
            )
            .map_err(|_| ())?;
        let words_buffer_size = (expected_bytes as usize).next_multiple_of(USIZE_SIZE) / USIZE_SIZE;
        assert!(I::SUPPORTS_WORD_LEVEL_INTERNING);
        // NOTE: we leave some slack for 64/32 bit arch mismatches
        let mut buffer = interner.get_word_buffer(words_buffer_size.next_multiple_of(2))?;
        let key = Bytes32::from_array(*key);
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

    pub fn root(&self, hasher: &mut impl MiniDigest<HashOutput = [u8; 32]>) -> [u8; 32] {
        self.mpt.root(hasher)
    }

    pub fn recompute(
        &mut self,
        preimages_oracle: &mut impl PreimagesOracle,
        hasher: &mut impl MiniDigest<HashOutput = [u8; 32]>,
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
        hasher: &mut impl MiniDigest<HashOutput = [u8; 32]>,
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
        hasher: &mut impl MiniDigest<HashOutput = [u8; 32]>,
    ) -> Result<(), ()> {
        self.mpt
            .insert_lazy_value(path, value, preimages_oracle, &mut self.interner, hasher)
    }
}

impl EthereumStoragePersister {
    fn cache_slot_value_as_digits<'a, A: Allocator + Clone>(
        slot: &Bytes32,
        cache: &'a mut BTreeMap<Bytes32, [u8; 64], A>,
        hasher: &mut Keccak256,
    ) -> &'a [u8; 64] {
        match cache.entry(*slot) {
            Entry::Occupied(e) => e.into_mut(),
            Entry::Vacant(e) => {
                hasher.update(slot.as_u8_array_ref());
                let key = hasher.finalize_reset();
                let digits = digits_from_key(&key);
                e.insert(digits)
            }
        }
    }

    fn cache_address_as_digits<'a, A: Allocator + Clone>(
        slot: &BitsOrd160,
        cache: &'a mut BTreeMap<BitsOrd160, [u8; 64], A>,
        hasher: &mut Keccak256,
    ) -> &'a [u8; 64] {
        match cache.entry(*slot) {
            Entry::Occupied(e) => e.into_mut(),
            Entry::Vacant(e) => {
                hasher.update(slot.0.to_be_bytes::<20>());
                let key = hasher.finalize_reset();
                let digits = digits_from_key(&key);
                e.insert(digits)
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
        SC: StackCtor<N>,
        const N: usize,
        VC: VecLikeCtor,
    >(
        &mut self,
        account_cache: &mut EthereumAccountCache<A, R, SC, N>,
        storage_cache: &EthereumStorageCache<A, SC, N, R, P>,
        initial_state_root: &Bytes32,
        oracle: &mut impl IOOracle,
        logger: &mut impl Logger,
        result_keeper: &mut impl IOResultKeeper<EthereumIOTypesConfig>,
        allocator: A,
    ) -> Result<Bytes32, InternalError> {
        // and can actually apply those

        let _ = logger.write_fmt(format_args!("Beginning MTP updates\n"));

        let mut it_fill_initial = storage_cache.net_accesses_iter();
        let mut it_set_final = it_fill_initial.clone();

        let mut preimage_oracle = OracleProxy(oracle);
        let mut key_cache = BTreeMap::<Bytes32, [u8; 64], A>::new_in(allocator.clone());
        let mut hasher = crypto::sha3::Keccak256::new();

        let mut reusable_mpt = MPTWithInterner::<A, VC, BoxInternerCtor>::new_in(allocator.clone());

        use crate::system_implementation::ethereum_storage_model::caches::account_properties::ACCOUNT_LEAF_VALUE_PRE_ENCODING_MAX_LEN;
        let mut account_data_encoding_buffer =
            [MaybeUninit::uninit(); ACCOUNT_LEAF_VALUE_PRE_ENCODING_MAX_LEN];
        let mut slot_value_encoding_buffer =
            [MaybeUninit::uninit(); LEAF_VALUE_PRE_ENCODING_MAX_LEN];

        let mut counter;
        let mut active_address;

        if let Some((addr, value)) = it_fill_initial.next() {
            counter = 1;
            active_address = addr.address;

            let _ = logger.write_fmt(format_args!(
                "Processing initial value for address 0x{:040x}, slot {:?}\n",
                &addr.address.as_uint(),
                &addr.key,
            ));

            let entry = account_cache
                .cache
                .get((&addr.address).into())
                .expect("account with storage address must be cached");
            let initial_root = entry.current().value().storage_root;

            debug_assert!(
                initial_root.is_zero() == false,
                "storage root can not be zero"
            );

            let _ = logger.write_fmt(format_args!(
                "Initial storage root for address 0x{:040x} is {:?}\n",
                addr.address.as_uint(),
                &initial_root,
            ));

            reusable_mpt = reusable_mpt.reinit_with_root(initial_root.as_u8_array());

            if value.initial_value_used {
                let digits =
                    Self::cache_slot_value_as_digits(&addr.key, &mut key_cache, &mut hasher);
                let path = Path::new(digits);
                let initial_expected_value = reusable_mpt
                    .get(path, &mut preimage_oracle, &mut hasher)
                    .map_err(|_| internal_error!("failed to get initial value in MPT"))?;

                assert!(
                    compare_bytes32_and_mpt_integer(&value.initial_value, initial_expected_value),
                    "failed to compare expected storage slot value {:?} vs RLP encoded {:?}\n",
                    &value.initial_value,
                    &initial_expected_value
                );
            } else {
                let _ = logger.write_fmt(format_args!(
                    "Value for address 0x{:040x}, slot {:?} is unobservable\n",
                    &addr.address.as_uint(),
                    &addr.key,
                ));
            }
        } else {
            // Nothing to do
            return Ok(*initial_state_root);
        }

        let mut should_update = false;
        let mut next_pair_to_read_check = None;

        loop {
            match it_fill_initial.next() {
                Some((addr, value)) => {
                    let _ = logger.write_fmt(format_args!(
                        "Processing initial value for address 0x{:040x}, slot {:?}\n",
                        &addr.address.as_uint(),
                        &addr.key,
                    ));

                    if active_address == addr.address {
                        counter += 1;

                        if value.initial_value_used {
                            let digits = Self::cache_slot_value_as_digits(
                                &addr.key,
                                &mut key_cache,
                                &mut hasher,
                            );
                            let path = Path::new(digits);
                            let initial_expected_value = reusable_mpt
                                .get(path, &mut preimage_oracle, &mut hasher)
                                .map_err(|_| {
                                    internal_error!("failed to get initial value in MPT")
                                })?;

                            assert!(
                                compare_bytes32_and_mpt_integer(
                                    &value.initial_value,
                                    initial_expected_value
                                ),
                                "failed to compare expected storage slot value {:?} vs RLP encoded {:?}\n",
                                &value.initial_value,
                                &initial_expected_value
                            );
                        } else {
                            let _ = logger.write_fmt(format_args!(
                                "Value for address 0x{:040x}, slot {:?} is unobservable\n",
                                &addr.address.as_uint(),
                                &addr.key,
                            ));
                        }
                    } else {
                        next_pair_to_read_check = Some((addr, value));
                        should_update = true;
                    }
                }
                None => {
                    should_update = true;
                }
            }

            if should_update {
                should_update = false;

                let _ = logger.write_fmt(format_args!(
                    "Should process {} potential updates for address 0x{:040x}\n",
                    counter,
                    &active_address.as_uint()
                ));

                let mut storage_is_observed = false;
                let mut any_mutation = false;
                for _ in 0..counter {
                    let (addr, v) = unsafe { it_set_final.next().unwrap_unchecked() };

                    let _ = logger.write_fmt(format_args!(
                        "Processing potential updates for address 0x{:040x}, slot {:?}\n",
                        &addr.address.as_uint(),
                        &addr.key,
                    ));

                    if v.initial_value_used {
                        storage_is_observed |= true;

                        debug_assert_eq!(addr.address, active_address);
                        if v.initial_value != v.current_value {
                            any_mutation |= true;

                            // cache hit
                            let digits = Self::cache_slot_value_as_digits(
                                &addr.key,
                                &mut key_cache,
                                &mut hasher,
                            );
                            let path = Path::new(digits);

                            if v.initial_value.is_zero() {
                                // insert

                                let _ = logger.write_fmt(format_args!(
                                    "Will insert value {:?} at slot {:?}\n",
                                    &v.current_value, &addr.key
                                ));

                                // encode value
                                let pre_encoded_value = Self::encode_slot_value(
                                    &v.current_value,
                                    &mut slot_value_encoding_buffer,
                                );

                                reusable_mpt
                                    .insert(
                                        path,
                                        pre_encoded_value,
                                        &mut preimage_oracle,
                                        &mut hasher,
                                    )
                                    .map_err(|_| {
                                        internal_error!("failed to get insert value into MPT")
                                    })?;
                            } else if v.current_value.is_zero() {
                                // delete

                                let _ = logger.write_fmt(format_args!(
                                    "Will delete value {:?} at slot {:?}\n",
                                    &v.initial_value, &addr.key
                                ));

                                reusable_mpt.delete(path).map_err(|_| {
                                    internal_error!("failed to get delete value from MPT")
                                })?;
                            } else {
                                // update

                                let _ = logger.write_fmt(format_args!(
                                    "Will update slot {:?} as {:?} -> {:?}\n",
                                    &addr.key, &v.initial_value, &v.current_value
                                ));

                                // encode value
                                let pre_encoded_value = Self::encode_slot_value(
                                    &v.current_value,
                                    &mut slot_value_encoding_buffer,
                                );

                                reusable_mpt.update(path, pre_encoded_value).map_err(|_| {
                                    internal_error!("failed to get update value in MPT")
                                })?;
                            }
                        } else {
                            let _ = logger.write_fmt(format_args!(
                                "Skipping updates for value for address 0x{:040x}, slot {:?} as there is no net update\n",
                                &addr.address.as_uint(),
                                &addr.key,
                            ));
                        }
                    } else {
                        let _ = logger.write_fmt(format_args!(
                            "Skipping updates for value for address 0x{:040x}, slot {:?} as it was not observed\n",
                            &addr.address.as_uint(),
                            &addr.key,
                        ));
                    }
                }
                if storage_is_observed {
                    let mut e = account_cache
                        .cache
                        .get_mut((&active_address).into())
                        .expect("account with storage address must be cached");
                    e.key_properties_mut().observe();
                }

                // recompute new root
                if any_mutation {
                    let _ = logger.write_fmt(format_args!(
                        "Will update storage root for 0x{:040x}\n",
                        &active_address.as_uint()
                    ));

                    // NOTE: this is fast NOP if no mutations happened
                    reusable_mpt
                        .recompute(&mut preimage_oracle, &mut hasher)
                        .map_err(|_| internal_error!("failed to compute new root for MPT"))?;

                    let mut e = account_cache
                        .cache
                        .get_mut((&active_address).into())
                        .expect("account with storage address must be cached");
                    let new_root = Bytes32::from_array(reusable_mpt.root(&mut hasher));

                    let _ = logger.write_fmt(format_args!(
                        "New storage root for address 0x{:040x} is {:?}\n",
                        active_address.as_uint(),
                        &new_root,
                    ));

                    assert_ne!(new_root, e.current().value().storage_root);
                    e.update(|v| {
                        v.update(|v, _m| {
                            v.storage_root = new_root;

                            Ok(())
                        })
                    })?;
                } else {
                    let _ = logger.write_fmt(format_args!(
                        "Storage root of 0x{:040x} will remain unchanged\n",
                        &active_address.as_uint(),
                    ));
                }

                if let Some((addr, value)) = next_pair_to_read_check.take() {
                    active_address = addr.address;
                    counter = 1;

                    let _ = logger.write_fmt(format_args!(
                        "Setting 0x{:040x} as new active address\n",
                        &addr.address.as_uint()
                    ));

                    // Now we should update MTP for next account, and reset counter
                    // reuse for the next account
                    let entry = account_cache
                        .cache
                        .get((&addr.address).into())
                        .expect("account with storage address must be cached");
                    let initial_root = entry.current().value().storage_root;

                    debug_assert!(
                        initial_root.is_zero() == false,
                        "storage root can not be zero"
                    );
                    reusable_mpt = reusable_mpt.reinit_with_root(initial_root.as_u8_array());

                    if value.initial_value_used {
                        // let _ = logger.write_fmt(format_args!(
                        //     "Initial storage root for address 0x{:040x} is {:?}\n",
                        //     addr.address.as_uint(),
                        //     &initial_root,
                        // ));

                        let digits = Self::cache_slot_value_as_digits(
                            &addr.key,
                            &mut key_cache,
                            &mut hasher,
                        );
                        let path = Path::new(digits);
                        let initial_expected_value = reusable_mpt
                            .get(path, &mut preimage_oracle, &mut hasher)
                            .map_err(|_| internal_error!("failed to get initial value in MPT"))?;

                        assert!(
                            compare_bytes32_and_mpt_integer(
                                &value.initial_value,
                                initial_expected_value
                            ),
                            "failed to compare expected initial storage root value {:?} vs RLP encoded {:?}\n",
                            &value.initial_value,
                            &initial_expected_value
                        );
                    } else {
                        let _ = logger.write_fmt(format_args!(
                            "Value for address 0x{:040x}, slot {:?} is unobservable\n",
                            &addr.address.as_uint(),
                            &addr.key,
                        ));
                    }
                } else {
                    // break out of the loop
                    assert!(it_set_final.next().is_none());
                    break;
                }
            }
        }

        let _ = logger.write_fmt(format_args!("Will update accounts MTP now\n",));

        // now reuse for accounts
        let mut accounts_mpt = reusable_mpt.reinit_with_root(initial_state_root.as_u8_array());

        let mut key_cache = BTreeMap::<BitsOrd160, [u8; 64], A>::new_in(allocator.clone());

        for record in account_cache.cache.iter() {
            let addr = record.key();

            let _ = logger.write_fmt(format_args!(
                "Updating the state of address 0x{:040x}\n",
                addr.0.as_uint()
            ));

            let initial_appearance = record.key_properties().initial_appearance();
            let current_appearance = record.key_properties().current_appearance();

            let initial = record.initial();
            let current = record.current();
            let current_metadata = current.metadata();
            assert!(
                current_metadata.is_marked_for_deconstruction == false,
                "Account 0x{:040x} was marked for deconstruction, but it was not completed",
                addr.0.as_uint()
            );

            match (initial_appearance, current_appearance) {
                (AccountInitialAppearance::Unset, AccountCurrentAppearance::Touched)
                | (AccountInitialAppearance::Retrieved, AccountCurrentAppearance::Touched) => {
                    // whatever it was - it's unobservable, we can just skip it
                    assert_eq!(initial.value(), current.value());

                    // let _ = logger.write_fmt(format_args!(
                    //     "Will skip account state verification for address 0x{:040x}\n",
                    //     addr.0.as_uint()
                    // ));
                }
                (AccountInitialAppearance::Unset, AccountCurrentAppearance::Observed)
                | (AccountInitialAppearance::Retrieved, AccountCurrentAppearance::Observed) => {
                    // we need to check that initial value is the one we claimed in cache

                    // let _ = logger.write_fmt(format_args!(
                    //     "Will retrieve initial account state for address 0x{:040x}\n",
                    //     addr.0.as_uint()
                    // ));

                    let digits = Self::cache_address_as_digits(addr, &mut key_cache, &mut hasher);
                    let path = Path::new(digits);
                    let initial_expected_value = accounts_mpt
                        .get(path, &mut preimage_oracle, &mut hasher)
                        .map_err(|_| {
                            internal_error!("failed to get initial account value in MPT")
                        })?;

                    if initial_appearance == AccountInitialAppearance::Unset {
                        // check that it's empty
                        assert!(initial_expected_value.is_empty());
                    } else {
                        // parse it and compare
                        let parsed =
                            EthereumAccountProperties::parse_from_rlp_bytes(initial_expected_value)
                                .map_err(|_| {
                                    internal_error!("failed to parse initial account value")
                                })?;

                        debug_assert!(
                            initial.value().bytecode_hash.is_zero() == false,
                            "bytecode hash must not be zero for retrieved account"
                        );
                        debug_assert!(
                            initial.value().storage_root.is_zero() == false,
                            "storage root hash must not be zero for retrieved account"
                        );

                        assert_eq!(initial.value(), &parsed);
                    }

                    // let _ = logger
                    //     .write_fmt(format_args!("Leaf initial value for address 0x{:040x} is 0x", addr.0.as_uint()));

                    // let _ = logger
                    //     .log_data(initial_expected_value.iter().copied());

                    // let _ = logger
                    //     .write_fmt(format_args!("\n",));

                    if initial_appearance == AccountInitialAppearance::Unset {
                        let current = current.value();

                        if current == &EthereumAccountProperties::EMPTY_ACCOUNT
                            || current == &EthereumAccountProperties::EMPTY_BUT_EXISTING_ACCOUNT
                        {
                            // empty -> observed -> empty

                            // let _ = logger.write_fmt(format_args!(
                            //     "Will skip empty account insert for address 0x{:040x}\n",
                            //     addr.0.as_uint()
                            // ));
                        } else {
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

                            // we will need to insert
                            // encode - we need slice, that is over list internally
                            let pre_encoded_value = current_value
                                .rlp_encode_for_leaf(&mut account_data_encoding_buffer);

                            // let _ = logger.write_fmt(format_args!(
                            //     "Leaf updated value for address 0x{:040x} is 0x",
                            //     addr.0.as_uint()
                            // ));

                            // let _ = logger.log_data(pre_encoded_value.iter().copied());

                            // let _ = logger.write_fmt(format_args!("\n",));

                            result_keeper.account_state_opaque_encoding(&addr.0, pre_encoded_value);

                            accounts_mpt
                                .insert(path, pre_encoded_value, &mut preimage_oracle, &mut hasher)
                                .map_err(|_| {
                                    internal_error!("failed to get update account value in MPT")
                                })?;
                        }
                    } else {
                        // it's an update potentially, and initial is not empty leaf

                        let initial = initial.value();
                        let current = current.value();

                        debug_assert!(current.bytecode_hash.is_zero() == false);

                        if initial != current {
                            let _ = logger.write_fmt(format_args!(
                                "Will update account state at address 0x{:040x}\n",
                                addr.0.as_uint()
                            ));

                            if current == &EthereumAccountProperties::EMPTY_ACCOUNT
                                || current == &EthereumAccountProperties::EMPTY_BUT_EXISTING_ACCOUNT
                            {
                                // we should delete it

                                let _ = logger.write_fmt(format_args!(
                                    "Will delete leaf for address 0x{:040x}",
                                    addr.0.as_uint()
                                ));

                                accounts_mpt.delete(path).map_err(|_| {
                                    internal_error!("failed to update account value in MPT")
                                })?;
                            } else {
                                // just update

                                // we checked initial, and rolled-over any possible updates on it,
                                // so this step is safe to skip if it's unchanged

                                // encode - we need slice, that is over list internally
                                let pre_encoded_value =
                                    current.rlp_encode_for_leaf(&mut account_data_encoding_buffer);

                                // let _ = logger.write_fmt(format_args!(
                                //     "Leaf updated value for address 0x{:040x} is 0x",
                                //     addr.0.as_uint()
                                // ));

                                // let _ = logger.log_data(pre_encoded_value.iter().copied());

                                // let _ = logger.write_fmt(format_args!("\n",));

                                result_keeper
                                    .account_state_opaque_encoding(&addr.0, pre_encoded_value);

                                accounts_mpt.update(path, pre_encoded_value).map_err(|_| {
                                    internal_error!("failed to update account value in MPT")
                                })?;
                            }
                        } else {
                            // let _ = logger
                            //     .write_fmt(format_args!("No net modification at address 0x{:040x}\n", addr.0.as_uint()));

                            result_keeper
                                .account_state_opaque_encoding(&addr.0, initial_expected_value);
                        }
                    }
                }
            }
        }

        let _ = logger.write_fmt(format_args!("Will recompute state root\n",));

        accounts_mpt
            .recompute(&mut preimage_oracle, &mut hasher)
            .map_err(|_| internal_error!("failed to compute new state root for MPT"))?;

        let _ = logger.write_fmt(format_args!("State MTP was updated\n",));

        Ok(Bytes32::from_array(accounts_mpt.root(&mut hasher)))
    }
}
