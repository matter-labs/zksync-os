//! The protocol through [`WordChannelOracle`], against an in-process mock of the oracle side that reads
//! the inputs through the addresses exposed by the querier, the way the host reads the guest memory.

use super::dynamic::U32_WORDS_PER_USIZE;
use super::*;
use crate::execution_environment_type::ExecutionEnvironmentType;
use crate::oracle::query_ids::{
    ACCOUNT_AND_STORAGE_SUBSPACE_MASK, GENERIC_PREIMAGE_QUERY_ID, GENERIC_SUBSPACE_MASK,
    INITIAL_STORAGE_SLOT_VALUE_QUERY_ID, NEXT_TX_SIZE_QUERY_ID, U256_DIV_REM_ADVICE_QUERY_ID,
    U256_WIDE_DIV_REM_ADVICE_QUERY_ID,
};
use crate::storage_types::InitialStorageSlotData;
use crate::types_config::EthereumIOTypesConfig;
use crate::utils::Bytes32;
use alloc::alloc::Global;
use alloc::boxed::Box;
use alloc::collections::VecDeque;
use alloc::vec;
use alloc::vec::Vec;
use core::mem::{align_of, offset_of, size_of, MaybeUninit};
use ruint::aliases::{B160, U256, U512};

const TEST_QUERY_ID: u32 = GENERIC_SUBSPACE_MASK | 0x42;

/// Oracle side of the protocol, in process: receives the two words of a query, hands them to a handler
/// that reads the input through the exposed addresses, and serves the response one word at a time.
struct MockHost {
    handler: Box<dyn FnMut(u32, usize) -> Vec<u32>>,
    query_id: Option<u32>,
    response: VecDeque<u32>,
    /// `(query ID, input word)` of every query received
    queries: Vec<(u32, usize)>,
}

impl WordChannel for MockHost {
    fn write_word(&mut self, word: usize) {
        match self.query_id.take() {
            None => {
                assert!(
                    self.response.is_empty(),
                    "previous response not consumed in full"
                );
                self.query_id = Some(u32::try_from(word).unwrap());
            }
            Some(query_id) => {
                self.queries.push((query_id, word));
                self.response = (self.handler)(query_id, word).into();
            }
        }
    }

    fn read_word(&mut self) -> usize {
        self.response
            .pop_front()
            .expect("read past the end of the response") as usize
    }
}

fn mock_oracle(
    handler: impl FnMut(u32, usize) -> Vec<u32> + 'static,
) -> WordChannelOracle<MockHost> {
    WordChannelOracle::new(MockHost {
        handler: Box::new(handler),
        query_id: None,
        response: VecDeque::new(),
        queries: Vec::new(),
    })
}

/// Oracle side read of a value of the querier through its exposed address.
///
/// # Safety
///
/// `address` must be the exposed address of a live `T`.
unsafe fn read_exposed<T>(address: usize) -> T {
    assert!(address.is_multiple_of(align_of::<T>()));
    // SAFETY: guaranteed by the caller
    unsafe { core::ptr::with_exposed_provenance::<T>(address).read() }
}

/// Oracle side encoding of a value without padding, as response words.
fn image<T: ContinuousSerializable>(value: &T) -> Vec<u32> {
    let words = size_of::<T>() / size_of::<u32>();
    // SAFETY: `T` is aligned for `u32` and has no padding, so all its bytes are initialized
    unsafe { core::slice::from_raw_parts(core::ptr::from_ref(value).cast::<u32>(), words) }.to_vec()
}

/// Oracle side encoding of a dynamically sized response: the claim counts `u32` words.
fn dynamic_response(words: &[usize]) -> Vec<u32> {
    let mut response = vec![u32::try_from(words.len() * U32_WORDS_PER_USIZE).unwrap()];
    for word in words {
        for i in 0..U32_WORDS_PER_USIZE {
            response.push((*word as u64 >> (32 * i)) as u32);
        }
    }
    response
}

fn address_of<T>(value: &T) -> usize {
    core::ptr::from_ref(value).addr()
}

fn u256(seed: u64) -> U256 {
    U256::from_limbs([
        seed,
        seed.rotate_left(13) ^ 0x0123_4567_89ab_cdef,
        !seed,
        seed.wrapping_mul(0x9e37_79b9_7f4a_7c15),
    ])
}

fn b160(seed: u64) -> B160 {
    B160::from_limbs([seed, !seed, seed & u64::from(u32::MAX)])
}

fn bytes32(seed: u8) -> Bytes32 {
    Bytes32::from_array(core::array::from_fn(|i| seed.wrapping_add(i as u8)))
}

fn to_host(value: &u256::U256) -> U256 {
    U256::from_limbs(*value.as_limbs())
}

fn to_system(value: U256) -> u256::U256 {
    u256::U256::from_limbs(*value.as_limbs())
}

/// Model of `NEXT_TX_SIZE_QUERY_ID`: no input, a short output.
struct NextTxSizeQuery;

impl OracleQuery for NextTxSizeQuery {
    const QUERY_ID: u32 = NEXT_TX_SIZE_QUERY_ID;
    type Input = ();
    type Output = u32;
}

/// A short input, and a short output with a restricted representation.
struct IsEeEnabledQuery;

impl OracleQuery for IsEeEnabledQuery {
    const QUERY_ID: u32 = TEST_QUERY_ID;
    type Input = ExecutionEnvironmentType;
    type Output = bool;
}

/// Model of `U256_DIV_REM_ADVICE_QUERY_ID`: the operands as a composite input, the quotient written in
/// place.
struct DivRemAdviceQuery;

impl OracleQuery for DivRemAdviceQuery {
    const QUERY_ID: u32 = U256_DIV_REM_ADVICE_QUERY_ID;
    type Input = (u256::U256, u256::U256);
    type Output = u256::U256;
}

/// Model of `U256_WIDE_DIV_REM_ADVICE_QUERY_ID`: a composite input of three, and the two halves of the
/// quotient as a composite output.
struct WideDivRemAdviceQuery;

impl OracleQuery for WideDivRemAdviceQuery {
    const QUERY_ID: u32 = U256_WIDE_DIV_REM_ADVICE_QUERY_ID;
    type Input = (u256::U256, u256::U256, u256::U256);
    type Output = (u256::U256, u256::U256);
}

/// Model of `InitialStorageSlotQuery`: the address and the key as a composite input, and the slot data
/// (a `bool` flag, padding, the value) written memcpy-like.
struct InitialStorageSlotQuery;

impl OracleQuery for InitialStorageSlotQuery {
    const QUERY_ID: u32 = INITIAL_STORAGE_SLOT_VALUE_QUERY_ID;
    type Input = (B160, Bytes32);
    type Output = InitialStorageSlotData<EthereumIOTypesConfig>;
}

/// Model of `EthereumAccountPropertiesQuery`: the address passed by address, and the nonce, balance,
/// storage root and bytecode hash as a composite output of four.
struct AccountPropertiesQuery;

const ACCOUNT_PROPERTIES_QUERY_ID: u32 = ACCOUNT_AND_STORAGE_SUBSPACE_MASK | 0x80;

impl OracleQuery for AccountPropertiesQuery {
    const QUERY_ID: u32 = ACCOUNT_PROPERTIES_QUERY_ID;
    type Input = B160;
    type Output = (u64, U256, Bytes32, Bytes32);
}

/// Model of `GENERIC_PREIMAGE_QUERY_ID`: the hash passed by address, and the preimage words.
struct PreimageQuery;

impl DynamicOracleQuery for PreimageQuery {
    const QUERY_ID: u32 = GENERIC_PREIMAGE_QUERY_ID;
    type Input = Bytes32;
}

#[test]
fn short_words_are_range_checked() {
    assert!(!bool::from_short_word(0).unwrap());
    assert!(bool::from_short_word(1).unwrap());
    assert!(bool::from_short_word(u32::MAX).unwrap());
    assert_eq!(u8::from_short_word(0xff).unwrap(), 0xff);
    assert!(u8::from_short_word(0x100).is_err());
    assert_eq!(u16::from_short_word(0xffff).unwrap(), 0xffff);
    assert!(u16::from_short_word(0x1_0000).is_err());
    assert_eq!(u32::from_short_word(u32::MAX).unwrap(), u32::MAX);
    assert_eq!(
        ExecutionEnvironmentType::from_short_word(1).unwrap(),
        ExecutionEnvironmentType::EVM
    );
    assert!(ExecutionEnvironmentType::from_short_word(2).is_err());
    assert!(ExecutionEnvironmentType::from_short_word(0x101).is_err());
}

#[test]
fn short_inputs_and_outputs_are_single_words() {
    let mut oracle = mock_oracle(|query_id, input| match query_id {
        NEXT_TX_SIZE_QUERY_ID => vec![1234],
        TEST_QUERY_ID if input == ExecutionEnvironmentType::EVM as usize => vec![0x8000_0000],
        TEST_QUERY_ID => vec![0],
        _ => unreachable!(),
    });
    assert_eq!(NextTxSizeQuery::get(&mut oracle, ()).unwrap(), 1234);
    assert!(IsEeEnabledQuery::get(&mut oracle, ExecutionEnvironmentType::EVM).unwrap());
    assert!(!IsEeEnabledQuery::get(&mut oracle, ExecutionEnvironmentType::NoEE).unwrap());
    assert_eq!(
        oracle.channel().queries,
        [
            (NEXT_TX_SIZE_QUERY_ID, 0),
            (TEST_QUERY_ID, ExecutionEnvironmentType::EVM as usize),
            (TEST_QUERY_ID, ExecutionEnvironmentType::NoEE as usize),
        ]
    );
}

#[test]
fn continuous_input_is_passed_by_address_and_output_written_in_place() {
    let hash = bytes32(1);
    let response = bytes32(2);
    let mut oracle = mock_oracle(move |_, input| {
        // SAFETY: the querier passes the address of a live `Bytes32`
        assert_eq!(unsafe { read_exposed::<Bytes32>(input) }, hash);
        image(&response)
    });
    oracle.send_continuous(TEST_QUERY_ID, &hash).unwrap();
    let mut dst = MaybeUninit::<Bytes32>::uninit();
    let dst_address = dst.as_ptr().addr();
    let written = oracle.init(&mut dst).unwrap();

    assert_eq!(*written, response);
    assert_eq!(address_of(written), dst_address);
    // two words for the query, and the response is the eight words of the value, without a prefix
    assert_eq!(
        oracle.channel().queries,
        [(TEST_QUERY_ID, address_of(&hash))]
    );
    assert!(oracle.channel().response.is_empty());
}

#[test]
fn b160_must_fit_in_160_bits() {
    let mut oracle = mock_oracle(|_, _| vec![1, 2, 3, 4, u32::MAX, 0]);
    oracle.send_query(TEST_QUERY_ID, 0).unwrap();
    let mut dst = MaybeUninit::<B160>::uninit();
    assert_eq!(
        *oracle.init(&mut dst).unwrap(),
        B160::from_limbs([2 << 32 | 1, 4 << 32 | 3, u64::from(u32::MAX)])
    );

    let mut oracle = mock_oracle(|_, _| vec![1, 2, 3, 4, 0, 1]);
    oracle.send_query(TEST_QUERY_ID, 0).unwrap();
    assert!(oracle.init(&mut MaybeUninit::<B160>::uninit()).is_err());
}

#[test]
fn arrays_validate_every_element() {
    let values = [b160(1), b160(2)];
    let mut oracle = mock_oracle(move |_, _| image(&values));
    oracle.send_query(TEST_QUERY_ID, 0).unwrap();
    let mut dst = MaybeUninit::<[B160; 2]>::uninit();
    assert_eq!(*oracle.init(&mut dst).unwrap(), values);

    let mut oracle = mock_oracle(|_, _| [image(&b160(1)), vec![0, 0, 0, 0, 0, 1]].concat());
    oracle.send_query(TEST_QUERY_ID, 0).unwrap();
    assert!(oracle
        .init(&mut MaybeUninit::<[B160; 2]>::uninit())
        .is_err());
}

#[test]
fn padding_is_ignored_and_bool_fields_are_normalized() {
    type SlotData = InitialStorageSlotData<EthereumIOTypesConfig>;
    assert_eq!(size_of::<SlotData>(), 40);
    assert_eq!(offset_of!(SlotData, initial_value), 8);

    let (address, key, value) = (b160(3), bytes32(4), bytes32(5));
    for (flag, expected) in [(0u8, false), (1, true), (0xa5, true)] {
        let mut oracle = mock_oracle(move |query_id, input| {
            assert_eq!(query_id, INITIAL_STORAGE_SLOT_VALUE_QUERY_ID);
            // SAFETY: the querier passes the address of the addresses of a live `B160` and `Bytes32`
            let [address_at, key_at] = unsafe { read_exposed::<[usize; 2]>(input) };
            assert_eq!(unsafe { read_exposed::<B160>(address_at) }, address);
            assert_eq!(unsafe { read_exposed::<Bytes32>(key_at) }, key);
            // the flag byte and 7 bytes of padding holding garbage, then the value
            let mut response = vec![u32::from_le_bytes([flag, 0xde, 0xad, 0xbe]), 0xefbe_adde];
            response.extend(image(&value));
            response
        });
        let data = InitialStorageSlotQuery::get(&mut oracle, (&address, &key)).unwrap();
        assert_eq!(data.is_new_storage_slot, expected);
        assert_eq!(data.initial_value, value);
    }
}

#[test]
fn composite_input_is_the_address_of_the_element_addresses() {
    let (a, b, c, d) = (u256(1), bytes32(2), b160(3), [u256(4), u256(5)]);
    let expected = [
        address_of(&a),
        address_of(&b),
        address_of(&c),
        address_of(&d),
    ];
    let mut oracle = mock_oracle(move |_, input| {
        // SAFETY: the querier passes the address of the addresses of live values of these types
        unsafe {
            let addresses = read_exposed::<[usize; 4]>(input);
            assert_eq!(addresses, expected);
            assert_eq!(read_exposed::<U256>(addresses[0]), a);
            assert_eq!(read_exposed::<Bytes32>(addresses[1]), b);
            assert_eq!(read_exposed::<B160>(addresses[2]), c);
            assert_eq!(read_exposed::<[U256; 2]>(addresses[3]), d);
        }
        vec![]
    });
    oracle
        .send_composite(TEST_QUERY_ID, (&a, &b, &c, &d))
        .unwrap();
    oracle.finish_query().unwrap();
    assert_eq!(oracle.channel().queries.len(), 1);
}

/// The host side of the division advice queries.
fn div_rem_host(query_id: u32, input: usize) -> Vec<u32> {
    // SAFETY: the querier passes the address of the addresses of live `u256::U256` operands
    let read = |address| to_host(&unsafe { read_exposed::<u256::U256>(address) });
    let quotient_limbs: Vec<u64> = match query_id {
        U256_DIV_REM_ADVICE_QUERY_ID => {
            // SAFETY: as above
            let [dividend, divisor] = unsafe { read_exposed::<[usize; 2]>(input) };
            (read(dividend) / read(divisor)).as_limbs().to_vec()
        }
        U256_WIDE_DIV_REM_ADVICE_QUERY_ID => {
            // SAFETY: as above
            let [lo, hi, divisor] = unsafe { read_exposed::<[usize; 3]>(input) };
            let dividend: U512 = U512::from(read(lo)) | (U512::from(read(hi)) << 256);
            let quotient: U512 = dividend / U512::from(read(divisor));
            quotient.as_limbs().to_vec()
        }
        _ => unreachable!(),
    };
    quotient_limbs
        .into_iter()
        .flat_map(|limb| [limb as u32, (limb >> 32) as u32])
        .collect()
}

#[test]
fn division_advice_round_trip() {
    let mut oracle = mock_oracle(div_rem_host);

    let dividend = to_system(u256(7));
    let divisor = to_system(U256::from(0x1234_5678_9abc_def0u64));
    let mut quotient = MaybeUninit::uninit();
    let quotient =
        DivRemAdviceQuery::get_into(&mut oracle, (&dividend, &divisor), &mut quotient).unwrap();
    assert_eq!(to_host(quotient), to_host(&dividend) / to_host(&divisor));

    let (lo, hi, divisor) = (
        to_system(u256(8)),
        to_system(u256(9) >> 3),
        to_system(u256(10)),
    );
    let (q_lo, q_hi) = WideDivRemAdviceQuery::get(&mut oracle, (&lo, &hi, &divisor)).unwrap();
    let dividend: U512 = U512::from(to_host(&lo)) | (U512::from(to_host(&hi)) << 256);
    let quotient: U512 = U512::from(to_host(&q_lo)) | (U512::from(to_host(&q_hi)) << 256);
    assert_eq!(quotient, dividend / U512::from(to_host(&divisor)));
}

#[test]
fn composite_output_is_written_element_by_element() {
    let account = b160(11);
    let properties = (42u64, u256(12), bytes32(13), bytes32(14));
    let mut oracle = mock_oracle(move |query_id, input| {
        assert_eq!(query_id, ACCOUNT_PROPERTIES_QUERY_ID);
        // SAFETY: the querier passes the address of a live `B160`
        assert_eq!(unsafe { read_exposed::<B160>(input) }, account);
        [
            image(&properties.0),
            image(&properties.1),
            image(&properties.2),
            image(&properties.3),
        ]
        .concat()
    });

    let mut dst = (
        MaybeUninit::uninit(),
        MaybeUninit::uninit(),
        MaybeUninit::uninit(),
        MaybeUninit::uninit(),
    );
    let (nonce, balance, storage_root, bytecode_hash) = AccountPropertiesQuery::get_into(
        &mut oracle,
        &account,
        (&mut dst.0, &mut dst.1, &mut dst.2, &mut dst.3),
    )
    .unwrap();
    assert_eq!(
        (*nonce, *balance, *storage_root, *bytecode_hash),
        properties
    );

    assert_eq!(
        AccountPropertiesQuery::get(&mut oracle, &account).unwrap(),
        properties
    );
}

#[test]
fn composite_output_stops_at_the_first_invalid_element() {
    let mut oracle =
        mock_oracle(|_, _| [image(&u256(1)), vec![0, 0, 0, 0, 0, 1], image(&bytes32(2))].concat());
    oracle.send_query(TEST_QUERY_ID, 0).unwrap();
    let mut dst = (
        MaybeUninit::<U256>::uninit(),
        MaybeUninit::<B160>::uninit(),
        MaybeUninit::<Bytes32>::uninit(),
    );
    assert!(oracle
        .init_composite((&mut dst.0, &mut dst.1, &mut dst.2))
        .is_err());
    // the last element was not read
    assert_eq!(oracle.channel().response.len(), 8);
}

#[test]
fn dynamic_response_must_fit_the_destination() {
    let hash = bytes32(20);
    let preimage = [1usize, 2, 3];
    let host = move |query_id, input| {
        assert_eq!(query_id, GENERIC_PREIMAGE_QUERY_ID);
        // SAFETY: the querier passes the address of a live `Bytes32`
        assert_eq!(unsafe { read_exposed::<Bytes32>(input) }, hash);
        dynamic_response(&preimage)
    };
    let mut oracle = mock_oracle(host);

    // only the claimed words of a longer destination are written
    let mut buffer = [usize::MAX; 5];
    assert_eq!(
        PreimageQuery::get_into(&mut oracle, &hash, &mut buffer[..]).unwrap(),
        3
    );
    assert_eq!(buffer, [1, 2, 3, usize::MAX, usize::MAX]);

    let mut buffer = [MaybeUninit::<usize>::uninit(); 3];
    assert_eq!(
        PreimageQuery::get_into(&mut oracle, &hash, &mut buffer[..]).unwrap(),
        3
    );
    // SAFETY: all three words were written
    assert_eq!(buffer.map(|word| unsafe { word.assume_init() }), preimage);

    // a destination that is too short is rejected before anything is written to it
    let mut buffer = [0usize; 2];
    assert!(PreimageQuery::get_into(&mut oracle, &hash, &mut buffer[..]).is_err());
    assert_eq!(buffer, [0, 0]);
}

#[test]
fn dynamic_response_into_vectors_and_boxes() {
    let hash = bytes32(21);
    let preimage = [1usize, 2, 3];
    let host = move |_, _| dynamic_response(&preimage);
    let mut oracle = mock_oracle(host);

    // appended within the spare capacity of a vector
    let mut vector = Vec::with_capacity(4);
    vector.push(7);
    assert_eq!(
        PreimageQuery::get_into(&mut oracle, &hash, &mut vector).unwrap(),
        3
    );
    assert_eq!(vector, [7, 1, 2, 3]);

    let mut boxed: Box<[usize]> = vec![0; 4].into_boxed_slice();
    assert_eq!(
        PreimageQuery::get_into(&mut oracle, &hash, &mut boxed).unwrap(),
        3
    );
    assert_eq!(*boxed, [1, 2, 3, 0]);

    let boxed = PreimageQuery::get_boxed(&mut oracle, &hash, 3, Global).unwrap();
    assert_eq!(*boxed, preimage);

    // without enough spare capacity the vector is left untouched: it is never reallocated
    let mut vector = Vec::with_capacity(3);
    while vector.capacity() - vector.len() > 2 {
        vector.push(7);
    }
    let len = vector.len();
    assert!(PreimageQuery::get_into(&mut mock_oracle(host), &hash, &mut vector).is_err());
    assert_eq!(vector.len(), len);

    assert!(PreimageQuery::get_boxed(&mut mock_oracle(host), &hash, 2, Global).is_err());
}

/// A query whose output is rejected midway is still ended: the in-process oracle drops the rest of the
/// response, as the oracle of the proving target does, and serves the next query.
#[test]
fn a_rejected_output_ends_its_query() {
    struct PairQuery;
    impl OracleQuery for PairQuery {
        const QUERY_ID: u32 = TEST_QUERY_ID;
        type Input = ();
        type Output = (B160, B160);
    }
    struct WordQuery;
    impl OracleQuery for WordQuery {
        const QUERY_ID: u32 = TEST_QUERY_ID + 1;
        type Input = ();
        type Output = u32;
    }
    let mut oracle =
        host::InProcessMemoryOracle::new(|query_id, _, _: &dyn host::QuerierMemory| {
            if query_id == PairQuery::QUERY_ID {
                // the first element does not fit in 160 bits: the second one is not read
                [vec![0, 0, 0, 0, 0, 1], image(&b160(2))].concat()
            } else {
                vec![7]
            }
        });
    assert!(PairQuery::get(&mut oracle, ()).is_err());
    assert_eq!(WordQuery::get(&mut oracle, ()).unwrap(), 7);
}

/// Serves `response_word(i)` at the `i`-th read, and counts the reads. The read at `bad_read`, if any,
/// gets a word that does not fit into `u32` on 64-bit targets.
struct CountingChannel {
    reads: usize,
    bad_read: Option<usize>,
}

impl WordChannel for CountingChannel {
    fn write_word(&mut self, _word: usize) {
        panic!("no query is sent");
    }

    fn read_word(&mut self) -> usize {
        let read = self.reads;
        self.reads += 1;
        if self.bad_read == Some(read) {
            usize::MAX
        } else {
            response_word(read) as usize
        }
    }
}

fn counting_oracle(bad_read: Option<usize>) -> WordChannelOracle<CountingChannel> {
    WordChannelOracle::new(CountingChannel { reads: 0, bad_read })
}

fn response_word(i: usize) -> u32 {
    (i as u32).wrapping_mul(0x9e37_79b9) ^ 0x5a5a_5a5a
}

/// A destination word that was not written.
const SENTINEL: u32 = 0xdead_beef;

/// Every length, and those around the unrolled block sizes: the `i`-th word read goes to word `i` of
/// the destination, and nothing is read or written past the length.
#[test]
fn write_words_writes_every_length_in_order() {
    for num_words in (0..=70).chain([127, 128, 129, 511, 512, 513, 1000]) {
        let mut oracle = counting_oracle(None);
        let mut buffer = vec![SENTINEL; num_words + 3];
        // SAFETY: the buffer holds more than `num_words` words
        unsafe { oracle.write_words(buffer.as_mut_ptr(), num_words) }.unwrap();
        assert_eq!(oracle.channel().reads, num_words);
        for (i, word) in buffer.iter().enumerate() {
            let expected = if i < num_words {
                response_word(i)
            } else {
                SENTINEL
            };
            assert_eq!(*word, expected, "word {i} of {num_words}");
        }
    }
}

/// A word that does not fit into `u32` stops the transfer: the words before it are written, it is read
/// and not written, and nothing after it is read.
#[cfg(target_pointer_width = "64")]
#[test]
fn write_words_stops_at_the_first_word_that_does_not_fit() {
    for num_words in [1, 5, 16, 17, 32, 33, 40, 64, 65] {
        for bad_read in 0..num_words {
            let mut oracle = counting_oracle(Some(bad_read));
            let mut buffer = vec![SENTINEL; num_words];
            // SAFETY: the buffer holds `num_words` words
            assert!(unsafe { oracle.write_words(buffer.as_mut_ptr(), num_words) }.is_err());
            assert_eq!(oracle.channel().reads, bad_read + 1);
            for (i, word) in buffer.iter().enumerate() {
                let expected = if i < bad_read {
                    response_word(i)
                } else {
                    SENTINEL
                };
                assert_eq!(
                    *word, expected,
                    "word {i} of {num_words}, bad read {bad_read}"
                );
            }
        }
    }
}

/// Dynamically sized responses around the unrolled block sizes, through the whole protocol: one query
/// each, the claimed words land at the start of the destination, and the response is consumed in full.
#[test]
fn dynamic_responses_around_the_block_sizes() {
    let hash = bytes32(22);
    for len in [0, 1, 2, 3, 7, 8, 9, 15, 16, 17, 31, 32, 33, 63, 64, 65, 100] {
        let preimage: Vec<usize> = (0..len as u64)
            .map(|i| i.wrapping_mul(0x9e37_79b9_7f4a_7c15) as usize)
            .collect();
        let response = dynamic_response(&preimage);
        let mut oracle = mock_oracle(move |query_id, input| {
            assert_eq!(query_id, GENERIC_PREIMAGE_QUERY_ID);
            // SAFETY: the querier passes the address of a live `Bytes32`
            assert_eq!(unsafe { read_exposed::<Bytes32>(input) }, hash);
            response.clone()
        });
        for spare in [0, 5] {
            let mut buffer = vec![MaybeUninit::new(usize::MAX); len + spare];
            assert_eq!(
                PreimageQuery::get_into(&mut oracle, &hash, &mut buffer[..]).unwrap(),
                len
            );
            assert!(oracle.channel().response.is_empty());
            // SAFETY: every word was initialized, with `usize::MAX` or by the oracle
            let buffer: Vec<usize> = buffer
                .iter()
                .map(|word| unsafe { word.assume_init() })
                .collect();
            assert_eq!(buffer[..len], preimage);
            assert!(buffer[len..].iter().all(|&word| word == usize::MAX));
        }
        assert_eq!(
            oracle.channel().queries,
            [(GENERIC_PREIMAGE_QUERY_ID, address_of(&hash)); 2]
        );
    }
}
