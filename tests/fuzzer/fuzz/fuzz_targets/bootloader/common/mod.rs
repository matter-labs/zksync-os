use basic_bootloader::bootloader::transaction::abi_encoded::AbiEncodedTransaction;
use basic_system::system_implementation::flat_storage_model::AccountProperties;
use basic_system::system_implementation::flat_storage_model::ACCOUNT_PROPERTIES_STORAGE_ADDRESS;
use basic_system::system_implementation::flat_storage_model::{
    FlatStorageCommitment, TestingTree, TESTING_TREE_HEIGHT,
};
use forward_system::run::test_impl::{InMemoryPreimageSource, InMemoryTree};
use oracle_provider::DummyMemorySource;
use oracle_provider::ZkEENonDeterminismSource;
use rand::{rngs::StdRng, Rng, SeedableRng};
use rig::ruint::aliases::{B160, U256};
use rig::zksync_os_interface::traits::TxListSource;
use std::alloc::Allocator;
use std::alloc::Global;
use std::collections::{HashMap, VecDeque};
use std::convert::TryInto;
use zk_ee::common_structs::derive_flat_storage_key;
use zk_ee::common_structs::ProofData;
use zk_ee::system::metadata::system_metadata::SystemMetadata;
use zk_ee::system::metadata::zk_metadata::{BlockMetadataFromOracle, TxLevelMetadata};
use zk_ee::utils::Bytes32;
use zk_ee::utils::UsizeAlignedByteBox;

#[derive(Debug, Default, Clone)]
pub(crate) struct TransactionData {
    pub(crate) tx_type: u8,
    pub(crate) from: [u8; 20],
    pub(crate) to: [u8; 20],
    pub(crate) gas_limit: U256,
    pub(crate) gas_per_pubdata_limit: U256,
    pub(crate) max_fee_per_gas: U256,
    pub(crate) max_priority_fee_per_gas: U256,
    pub(crate) paymaster: [u8; 20],
    pub(crate) nonce: U256,
    pub(crate) value: U256,
    pub(crate) reserved: [U256; 4],
    pub(crate) data: Vec<u8>,
    pub(crate) signature: Vec<u8>,
    pub(crate) factory_deps: Vec<[u8; 32]>,
    pub(crate) paymaster_input: Vec<u8>,
    pub(crate) reserved_dynamic: Vec<u8>,
}

impl TransactionData {
    fn to_zk_bytes(&self) -> Option<Vec<u8>> {
        // ABI tx head has 19 words:
        // 14 static fields + 5 dynamic offsets.
        let mut head = Vec::<[u8; 32]>::with_capacity(19);
        let mut tail = Vec::<u8>::new();

        head.push(enc_u256(U256::from(self.tx_type)));
        head.push(enc_addr(self.from));
        head.push(enc_addr(self.to));
        head.push(enc_u256(self.gas_limit));
        head.push(enc_u256(self.gas_per_pubdata_limit));
        head.push(enc_u256(self.max_fee_per_gas));
        head.push(enc_u256(self.max_priority_fee_per_gas));
        head.push(enc_addr(self.paymaster));
        head.push(enc_u256(self.nonce));
        head.push(enc_u256(self.value));
        for value in self.reserved {
            head.push(enc_u256(value));
        }

        let head_size = 19 * 32;
        abi_push_bytes(&mut head, &mut tail, &self.data, head_size)?;
        abi_push_bytes(&mut head, &mut tail, &self.signature, head_size)?;
        abi_push_bytes32_array(&mut head, &mut tail, &self.factory_deps, head_size)?;
        abi_push_bytes(&mut head, &mut tail, &self.paymaster_input, head_size)?;
        abi_push_bytes(&mut head, &mut tail, &self.reserved_dynamic, head_size)?;

        let mut out = Vec::with_capacity(head_size + tail.len());
        for w in head {
            out.extend_from_slice(&w);
        }
        out.extend_from_slice(&tail);
        Some(out)
    }
}

impl<A: Allocator> From<&AbiEncodedTransaction<A>> for TransactionData {
    fn from(tx: &AbiEncodedTransaction<A>) -> Self {
        let mut factory_deps = Vec::new();
        for chunk in tx.encoding(tx.factory_deps.clone()).chunks_exact(32) {
            factory_deps.push(chunk.try_into().expect("32-byte chunk"));
        }

        TransactionData {
            tx_type: tx.tx_type.read(),
            from: tx.from.read().to_be_bytes::<20>(),
            to: tx.to.read().to_be_bytes::<20>(),
            gas_limit: U256::from(tx.gas_limit.read()),
            gas_per_pubdata_limit: U256::from(tx.gas_per_pubdata_limit.read()),
            max_fee_per_gas: tx.max_fee_per_gas.read(),
            max_priority_fee_per_gas: tx.max_priority_fee_per_gas.read(),
            paymaster: tx.paymaster.read().to_be_bytes::<20>(),
            nonce: tx.nonce.read(),
            value: tx.value.read(),
            reserved: tx
                .reserved
                .iter()
                .map(|u| u.read())
                .collect::<Vec<U256>>()
                .try_into()
                .expect("4 reserved fields"),
            data: tx.encoding(tx.data.clone()).to_vec(),
            signature: tx.encoding(tx.signature.clone()).to_vec(),
            factory_deps,
            paymaster_input: tx.encoding(tx.paymaster_input.clone()).to_vec(),
            reserved_dynamic: tx.encoding(tx.reserved_dynamic.clone()).to_vec(),
        }
    }
}

#[allow(unused)]
pub fn address_into_special_storage_key(address: &B160) -> Bytes32 {
    let mut key = Bytes32::zero();
    key.as_u8_array_mut()[12..].copy_from_slice(&address.to_be_bytes::<{ B160::BYTES }>());

    key
}

#[allow(unused)]
pub fn mock_oracle() -> (
    zk_ee::system::metadata::zk_metadata::ZkMetadata,
    ZkEENonDeterminismSource<DummyMemorySource>,
) {
    let tree = InMemoryTree::<false> {
        storage_tree: TestingTree::new_in(Global),
        cold_storage: HashMap::new(),
    };
    let init_data = Some(ProofData {
        state_root_view: FlatStorageCommitment::<{ TESTING_TREE_HEIGHT }> {
            root: *tree.storage_tree.root(),
            next_free_slot: tree.storage_tree.next_free_slot,
        },
        last_block_timestamp: 0,
    });

    let block_level = BlockMetadataFromOracle::new_for_test();
    let tx_level = TxLevelMetadata::default();

    let system_metadata = SystemMetadata {
        block_level: block_level.clone(),
        tx_level: tx_level,
        _marker: std::marker::PhantomData,
    };

    (
        system_metadata,
        forward_system::run::make_oracle_for_proofs_and_dumps_for_init_data(
            block_level,
            tree,
            InMemoryPreimageSource {
                inner: HashMap::new(),
            },
            TxListSource {
                transactions: VecDeque::new(),
            },
            init_data,
            None,
            true,
        ),
    )
}

#[allow(unused)]
pub fn mock_oracle_balance(
    address: B160,
    balance: U256,
) -> (
    zk_ee::system::metadata::zk_metadata::ZkMetadata,
    ZkEENonDeterminismSource<DummyMemorySource>,
) {
    let mut tree = InMemoryTree::<false> {
        storage_tree: TestingTree::new_in(Global),
        cold_storage: HashMap::new(),
    };
    let mut preimage_source = InMemoryPreimageSource {
        inner: HashMap::new(),
    };

    let mut account_properties = AccountProperties::TRIVIAL_VALUE;
    account_properties.balance = balance;
    let encoding = account_properties.encoding();
    let properties_hash = account_properties.compute_hash();

    let key = address_into_special_storage_key(&address);
    let flat_key = derive_flat_storage_key(&ACCOUNT_PROPERTIES_STORAGE_ADDRESS, &key);

    tree.cold_storage.insert(flat_key, properties_hash);
    tree.storage_tree.insert(&flat_key, &properties_hash);
    preimage_source
        .inner
        .insert(properties_hash, encoding.to_vec());

    let init_data = Some(ProofData {
        state_root_view: FlatStorageCommitment::<{ TESTING_TREE_HEIGHT }> {
            root: *tree.storage_tree.root(),
            next_free_slot: tree.storage_tree.next_free_slot,
        },
        last_block_timestamp: 0,
    });

    let block_level = BlockMetadataFromOracle::new_for_test();
    let tx_level = TxLevelMetadata::default();
    let system_metadata = SystemMetadata {
        block_level: block_level.clone(),
        tx_level: tx_level,
        _marker: std::marker::PhantomData,
    };
    (
        system_metadata,
        forward_system::run::make_oracle_for_proofs_and_dumps_for_init_data(
            block_level,
            tree,
            preimage_source,
            TxListSource {
                transactions: VecDeque::new(),
            },
            init_data,
            None,
            true,
        ),
    )
}

pub(crate) fn serialize_zksync_transaction<A: Allocator>(
    tx: &AbiEncodedTransaction<A>,
) -> Option<Vec<u8>> {
    let tx_data = TransactionData::from(tx);
    tx_data.to_zk_bytes()
}

pub fn mutate_transaction(data: &mut [u8], size: usize, max_size: usize, seed: u32) -> usize {
    if size == 0 || size > data.len() {
        return size.min(data.len());
    }

    let Ok(tx) = parse_abi_encoded_transaction(&data[..size]) else {
        return size;
    };

    let mut tx_data = TransactionData::from(&tx);
    let mut rng = StdRng::seed_from_u64(seed as u64);
    mutate_zksync_transaction(&mut tx_data, &mut rng);

    // Keep invariants enforced by AbiEncodedTransaction::validate_structure.
    tx_data.paymaster = [0u8; 20];
    tx_data.signature.clear();
    tx_data.paymaster_input.clear();
    tx_data.reserved_dynamic.clear();
    tx_data.reserved[2] = U256::ZERO;
    tx_data.reserved[3] = U256::ZERO;

    let Some(serialized_tx) = tx_data.to_zk_bytes() else {
        return size;
    };
    if serialized_tx.len() > max_size || serialized_tx.len() > data.len() {
        return size;
    }
    if parse_abi_encoded_transaction(&serialized_tx).is_err() {
        return size;
    }

    data[..serialized_tx.len()].copy_from_slice(&serialized_tx);
    serialized_tx.len()
}

pub(crate) fn parse_abi_encoded_transaction(
    data: &[u8],
) -> Result<AbiEncodedTransaction<Global>, ()> {
    let buffer = UsizeAlignedByteBox::from_slice_in(data, Global);
    AbiEncodedTransaction::try_from_buffer(buffer)
}

fn mutate_zksync_transaction(tx: &mut TransactionData, rng: &mut StdRng) {
    // Keep tx_type in the supported ABI-only set to maximize valid mutations.
    match rng.gen_range(0..=10) {
        0 => {
            tx.tx_type = match tx.tx_type {
                AbiEncodedTransaction::<Global>::L1_L2_TX_TYPE => {
                    AbiEncodedTransaction::<Global>::UPGRADE_TX_TYPE
                }
                AbiEncodedTransaction::<Global>::UPGRADE_TX_TYPE => {
                    AbiEncodedTransaction::<Global>::L1_L2_TX_TYPE
                }
                _ => AbiEncodedTransaction::<Global>::L1_L2_TX_TYPE,
            };
        }
        1 => mutate_address_inplace(&mut tx.from, rng),
        2 => mutate_address_inplace(&mut tx.to, rng),
        3 => tx.gas_limit = mutate_u64_field(tx.gas_limit, rng),
        4 => tx.gas_per_pubdata_limit = mutate_u32_field(tx.gas_per_pubdata_limit, rng),
        5 => tx.max_fee_per_gas = mutate_u256_vec(tx.max_fee_per_gas, rng),
        6 => tx.max_priority_fee_per_gas = mutate_u256_vec(tx.max_priority_fee_per_gas, rng),
        7 => tx.nonce = mutate_u256_vec(tx.nonce, rng),
        8 => tx.value = mutate_u256_vec(tx.value, rng),
        9 => mutate_bytes_inplace(&mut tx.data, rng),
        10 => mutate_factory_deps_inplace(&mut tx.factory_deps, rng),
        _ => {}
    }
}

fn mutate_bytes_inplace(bytes: &mut Vec<u8>, rng: &mut StdRng) {
    if bytes.is_empty() || rng.gen_bool(0.25) {
        bytes.push(rng.gen::<u8>());
        return;
    }

    let idx = rng.gen_range(0..bytes.len());
    bytes[idx] ^= rng.gen::<u8>();

    if bytes.len() > 1 && rng.gen_bool(0.1) {
        bytes.remove(idx);
    }
}

fn mutate_factory_deps_inplace(factory_deps: &mut Vec<[u8; 32]>, rng: &mut StdRng) {
    if factory_deps.is_empty() || rng.gen_bool(0.2) {
        let mut new_dep = [0u8; 32];
        rng.fill(new_dep.as_mut_slice());
        factory_deps.push(new_dep);
        return;
    }

    let idx = rng.gen_range(0..factory_deps.len());
    let byte_idx = rng.gen_range(0..32);
    factory_deps[idx][byte_idx] ^= rng.gen::<u8>();

    if factory_deps.len() > 1 && rng.gen_bool(0.1) {
        factory_deps.remove(idx);
    }
}

#[allow(dead_code)]
fn mutate_u256_vec(num: U256, rng: &mut StdRng) -> U256 {
    // Convert the input array to a Vec<u8>
    let mut mutated_bytes: [u8; 32] = num.to_be_bytes();

    // Pick a random byte index
    let idx = rng.gen_range(0..mutated_bytes.len());

    // Mutate the byte
    mutated_bytes[idx] ^= rng.gen::<u8>();

    // Return the mutated number
    U256::from_be_bytes(mutated_bytes)
}

fn mutate_u64_field(num: U256, rng: &mut StdRng) -> U256 {
    mutate_low_bytes(num, 8, rng)
}

fn mutate_u32_field(num: U256, rng: &mut StdRng) -> U256 {
    mutate_low_bytes(num, 4, rng)
}

fn mutate_low_bytes(num: U256, bytes_to_mutate: usize, rng: &mut StdRng) -> U256 {
    let mut mutated_bytes: [u8; 32] = num.to_be_bytes();
    let bytes_to_mutate = bytes_to_mutate.min(mutated_bytes.len());
    if bytes_to_mutate == 0 {
        return num;
    }
    let start = mutated_bytes.len() - bytes_to_mutate;
    let idx = rng.gen_range(start..mutated_bytes.len());
    mutated_bytes[idx] ^= rng.gen::<u8>();
    U256::from_be_bytes(mutated_bytes)
}

#[allow(dead_code)]
fn mutate_u8(num: u8, rng: &mut StdRng) -> u8 {
    num ^ rng.gen::<u8>()
}

#[allow(dead_code)]
fn mutate_address_inplace(addr: &mut [u8; 20], rng: &mut StdRng) {
    let idx = rng.gen_range(0..addr.len());
    addr[idx] ^= rng.gen::<u8>();
}

// helpers
fn word32_be(bytes_be: &[u8]) -> [u8; 32] {
    let mut w = [0u8; 32];
    let n = bytes_be.len().min(32);
    w[32 - n..].copy_from_slice(&bytes_be[bytes_be.len() - n..]);
    w
}

pub(crate) fn enc_addr(a20: [u8; 20]) -> [u8; 32] {
    let mut w = [0u8; 32];
    w[12..].copy_from_slice(&a20);
    w
}

pub(crate) fn enc_u256(u: U256) -> [u8; 32] {
    word32_be(&u.to_be_bytes_vec())
}
pub(crate) fn enc_u32(u: u32) -> [u8; 32] {
    enc_u256(U256::from(u))
}
pub(crate) fn enc_u16(u: u16) -> [u8; 32] {
    enc_u256(U256::from(u))
}

fn pad32(v: &mut Vec<u8>) {
    let pad = (32 - (v.len() % 32)) % 32;
    if pad != 0 {
        v.resize(v.len() + pad, 0);
    }
}

fn checked_u32(value: usize) -> Option<u32> {
    u32::try_from(value).ok()
}

// Push a dynamic `bytes` arg: put its offset in head, then tail = len + data + pad
pub(crate) fn abi_push_bytes(
    head: &mut Vec<[u8; 32]>,
    tail: &mut Vec<u8>,
    data: &[u8],
    head_size_bytes: usize,
) -> Option<()> {
    let offset = checked_u32(head_size_bytes + tail.len())?;
    head.push(enc_u32(offset));
    tail.extend_from_slice(&enc_u32(checked_u32(data.len())?));
    tail.extend_from_slice(data);
    pad32(tail);
    Some(())
}

// Push a dynamic `bytes32[]` arg
pub(crate) fn abi_push_bytes32_array(
    head: &mut Vec<[u8; 32]>,
    tail: &mut Vec<u8>,
    items: &[[u8; 32]],
    head_size_bytes: usize,
) -> Option<()> {
    let offset = checked_u32(head_size_bytes + tail.len())?;
    head.push(enc_u32(offset));
    tail.extend_from_slice(&enc_u32(checked_u32(items.len())?));
    for it in items {
        tail.extend_from_slice(it);
    }
    Some(())
}
