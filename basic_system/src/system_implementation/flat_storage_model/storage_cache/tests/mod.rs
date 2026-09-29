use super::*;
use crate::system_implementation::system::EthereumLikeStorageAccessCostModel;
use std::alloc::Global;
use zk_ee::memory::stack_implementations::vec_stack::VecStackFactory;
use zk_ee::oracle::memory_io::host::{ResponseBuffer, WriteQueryOutput};
use zk_ee::oracle::memory_io::MemoryOracle;
use zk_ee::oracle::query_ids::INITIAL_STORAGE_SLOT_VALUE_QUERY_ID;
use zk_ee::oracle::usize_serialization::{UsizeDeserializable, UsizeSerializable};
use zk_ee::reference_implementations::{BaseResources, DecreasingNative};
use zk_ee::storage_types::InitialStorageSlotData;
use zk_ee::system::Resource;

type TestResources = BaseResources<DecreasingNative>;
type TestStorage = NewStorageWithAccountPropertiesUnderHash<
    Global,
    VecStackFactory,
    4,
    TestResources,
    EthereumLikeStorageAccessCostModel,
>;

/// Every queried slot starts at zero.
#[derive(Default)]
struct SlotOracle {
    response: ResponseBuffer,
}

impl MemoryOracle for SlotOracle {
    fn send_query(&mut self, query_id: u32, _input_word: usize) -> Result<(), InternalError> {
        assert_eq!(query_id, INITIAL_STORAGE_SLOT_VALUE_QUERY_ID);
        let slot = InitialStorageSlotData::<EthereumIOTypesConfig> {
            is_new_storage_slot: true,
            initial_value: Bytes32::ZERO,
        };
        let mut response = vec![];
        slot.write_output(&mut response);
        self.response.set(response)
    }

    zk_ee::memory_oracle_response_methods!(response);
}

impl IOOracle for SlotOracle {
    type RawIterator<'a> = core::iter::Empty<usize>;

    fn raw_query<'a, I: UsizeSerializable + UsizeDeserializable>(
        &'a mut self,
        query_type: u32,
        _input: &I,
    ) -> Result<Self::RawIterator<'a>, InternalError> {
        panic!("unexpected query {query_type:#x}")
    }
}

const ADDRESS: B160 = B160::from_limbs([0x1234, 0, 0]);
const V1: Bytes32 =
    Bytes32::from_hex("0000000000000000000000000000000000000000000000000000000000000001");
const V2: Bytes32 =
    Bytes32::from_hex("0000000000000000000000000000000000000000000000000000000000000100");

fn fixture() -> (TestStorage, SlotOracle) {
    let mut storage = NewStorageWithAccountPropertiesUnderHash(
        GenericPubdataAwarePlainStorage::new_from_parts(Global, EthereumLikeStorageAccessCostModel),
    );
    storage.begin_new_tx();
    (storage, SlotOracle::default())
}

fn write(storage: &mut TestStorage, oracle: &mut SlotOracle, key: u8, value: Bytes32) {
    let mut resources = TestResources::FORMAL_INFINITE;
    storage
        .write(
            ExecutionEnvironmentType::NoEE,
            &mut resources,
            &ADDRESS,
            &Bytes32::from_byte_fill(key),
            &value,
            oracle,
        )
        .expect("write");
}

fn slot_diff(from: Bytes32, to: Bytes32) -> u32 {
    32 + ValueDiffCompressionStrategy::optimal_compression_length(&from, &to) as u32
}

#[test]
fn repeated_writes_and_pubdata_calls_count_each_key_once() {
    let (mut storage, mut oracle) = fixture();
    write(&mut storage, &mut oracle, 1, V1);
    write(&mut storage, &mut oracle, 1, V2);
    write(&mut storage, &mut oracle, 2, V1);
    let expected = slot_diff(Bytes32::ZERO, V2) + slot_diff(Bytes32::ZERO, V1);
    assert_eq!(storage.calculate_pubdata_used_by_tx(), expected);
    assert_eq!(storage.calculate_pubdata_used_by_tx(), expected);

    write(&mut storage, &mut oracle, 1, Bytes32::ZERO);
    assert_eq!(
        storage.calculate_pubdata_used_by_tx(),
        slot_diff(Bytes32::ZERO, V1)
    );
    write(&mut storage, &mut oracle, 3, V2);
    assert_eq!(storage.calculate_pubdata_used_by_tx(), expected);
}

#[test]
fn nested_rollback_restores_values_and_kept_frames_preserve_pubdata() {
    let (mut storage, mut oracle) = fixture();
    storage.start_frame();
    write(&mut storage, &mut oracle, 1, V1);
    storage.start_frame();
    write(&mut storage, &mut oracle, 2, V1);
    let inner = storage.start_frame();
    write(&mut storage, &mut oracle, 1, V2);
    write(&mut storage, &mut oracle, 2, V2);
    write(&mut storage, &mut oracle, 3, V2);
    assert_eq!(
        storage.calculate_pubdata_used_by_tx(),
        3 * slot_diff(Bytes32::ZERO, V2)
    );

    storage.finish_frame(Some(&inner)).expect("rollback inner");
    assert_eq!(
        storage.calculate_pubdata_used_by_tx(),
        2 * slot_diff(Bytes32::ZERO, V1)
    );
    storage.finish_frame(None).expect("keep middle");
    storage.finish_frame(None).expect("keep outer");
    assert_eq!(
        storage.calculate_pubdata_used_by_tx(),
        2 * slot_diff(Bytes32::ZERO, V1)
    );
}

#[test]
fn resetting_to_block_initial_or_transaction_start_skips_pubdata() {
    let (mut storage, mut oracle) = fixture();
    write(&mut storage, &mut oracle, 1, V1);
    write(&mut storage, &mut oracle, 1, Bytes32::ZERO);
    assert_eq!(storage.calculate_pubdata_used_by_tx(), 0);

    write(&mut storage, &mut oracle, 1, V1);
    storage.finish_tx().expect("finish tx");
    storage.begin_new_tx();
    assert_eq!(storage.calculate_pubdata_used_by_tx(), 0);
    write(&mut storage, &mut oracle, 1, V2);
    assert_eq!(storage.calculate_pubdata_used_by_tx(), slot_diff(V1, V2));
    write(&mut storage, &mut oracle, 1, V1);
    assert_eq!(storage.calculate_pubdata_used_by_tx(), 0);
    write(&mut storage, &mut oracle, 1, Bytes32::ZERO);
    assert_eq!(storage.calculate_pubdata_used_by_tx(), 0);
}

#[test]
fn reads_unobserved_touches_and_account_properties_skip_pubdata() {
    let (mut storage, mut oracle) = fixture();
    let mut resources = TestResources::FORMAL_INFINITE;
    storage
        .read(
            ExecutionEnvironmentType::NoEE,
            &mut resources,
            &ADDRESS,
            &V1,
            &mut oracle,
        )
        .expect("read");
    // Use the underlying warm-up API: the flat model's touch performs a read.
    storage
        .0
        .apply_touch_impl(
            ExecutionEnvironmentType::NoEE,
            &WarmStorageKey {
                address: ADDRESS,
                key: V2,
            },
            &mut resources,
        )
        .expect("unobserved touch");
    storage
        .write_special_account_property::<AccountAggregateDataHash>(
            ExecutionEnvironmentType::NoEE,
            &mut resources,
            &ADDRESS,
            &V1,
            &mut oracle,
        )
        .expect("account property write");
    assert_eq!(storage.calculate_pubdata_used_by_tx(), 0);
}
