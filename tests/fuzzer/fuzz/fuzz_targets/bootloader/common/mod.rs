use basic_bootloader::bootloader::constants::TX_OFFSET;
use basic_bootloader::bootloader::transaction::ZkSyncTransaction;
use basic_system::system_implementation::flat_storage_model::AccountProperties;
use basic_system::system_implementation::flat_storage_model::ACCOUNT_PROPERTIES_STORAGE_ADDRESS;
use basic_system::system_implementation::flat_storage_model::{
    FlatStorageCommitment, TestingTree, TESTING_TREE_HEIGHT,
};
use forward_system::run::test_impl::{InMemoryPreimageSource, InMemoryTree, TxListSource};
use forward_system::run::{io_implementer_init_data, ForwardRunningOracle};
use rand::{rngs::StdRng, Rng, SeedableRng};
use rig::ruint::aliases::{B160, U256};
use secp256k1::{Message, Secp256k1, SecretKey};
use std::alloc::Global;
use std::collections::{HashMap, VecDeque};
use std::convert::TryInto;
use web3::ethabi::{encode, Address, Token, Uint};
use zk_ee::common_structs::derive_flat_storage_key;
use zk_ee::system::metadata::BlockMetadataFromOracle;
use zk_ee::utils::Bytes32;

// a test private key from anvil
#[allow(unused)]
const PRIVATE_KEY: [u8; 32] = [
    0xac, 0x09, 0x74, 0xbe, 0xc3, 0x9a, 0x17, 0xe3, 0x6b, 0xa4, 0xa6, 0xb4, 0xd2, 0x38, 0xff, 0x94,
    0x4b, 0xac, 0xb4, 0x78, 0xcb, 0xed, 0x5e, 0xfc, 0xae, 0x78, 0x4d, 0x7b, 0xf4, 0xf2, 0xff, 0x80,
];

#[allow(unused)]
const ACCOUNT: [u8; 32] = [
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xf3, 0x9f, 0xd6, 0xe5,
    0x1a, 0xad, 0x88, 0xf6, 0xf4, 0xce, 0x6a, 0xb8, 0x82, 0x72, 0x79, 0xcf, 0xff, 0xb9, 0x22, 0x66,
];

#[allow(unused)]
const CHAIN_ID: u64 = 37;

// This is a copy of the data structure from era-evm-tester.
// By using this data structure, we can change the fields directly.
#[derive(Debug, Default, Clone)]
pub(crate) struct TransactionData {
    pub(crate) tx_type: u8,
    pub(crate) from: Address,
    pub(crate) to: Option<Address>,
    pub(crate) gas_limit: U256,
    pub(crate) pubdata_price_limit: U256,
    pub(crate) max_fee_per_gas: U256,
    pub(crate) max_priority_fee_per_gas: U256,
    pub(crate) paymaster: Address,
    pub(crate) nonce: U256,
    pub(crate) value: U256,
    // The reserved fields that are unique for different types of transactions.
    // E.g. nonce is currently used in all transaction, but it should not be mandatory
    // in the long run.
    pub(crate) reserved: [U256; 4],
    pub(crate) data: Vec<u8>,
    pub(crate) signature: Vec<u8>,
    // The factory deps provided with the transaction.
    // Whereas it has certain structure, we treat it as a raw bytes.
    #[allow(unused)]
    pub(crate) factory_deps: Vec<u8>,
    pub(crate) paymaster_input: Vec<u8>,
    pub(crate) reserved_dynamic: Vec<u8>,
    //pub(crate) raw_bytes: Option<Vec<u8>>,
}

// Copy from era-evm-tester.
// Additionally, we have to convert between different kinds of U256.
impl TransactionData {
    #[allow(dead_code)]
    pub fn abi_encode(self) -> Vec<u8> {
        // do U256 -> Uint conversions
        fn u256_to_uint(u: &U256) -> Uint {
            Uint::from_big_endian(u.to_be_bytes::<32>().as_slice())
        }
        // align vectors to 32 bytes
        fn pad32(data: &[u8]) -> Vec<u8> {
            let mut padded = data.to_vec();
            let len = padded.len();
            if (len % 32) != 0 {
                let pad = 32 - (len % 32);
                padded.extend(vec![0u8; pad]);
            }
            padded
        }
        // produce the actual encoding, as a mix of abi_encode
        // and custom serialization
        let mut res = encode(&[Token::Tuple(vec![
            Token::Uint(Uint::from_big_endian(
                u8::to_be_bytes(self.tx_type).as_slice(),
            )),
            Token::Address(self.from),
            Token::Address(self.to.unwrap_or_default()),
            Token::Uint(u256_to_uint(&self.gas_limit)),
            Token::Uint(u256_to_uint(&self.pubdata_price_limit)),
            Token::Uint(u256_to_uint(&self.max_fee_per_gas)),
            Token::Uint(u256_to_uint(&self.max_priority_fee_per_gas)),
            Token::Address(self.paymaster),
            Token::Uint(u256_to_uint(&self.nonce)),
            Token::Uint(u256_to_uint(&self.value)),
            Token::FixedArray(
                self.reserved
                    .iter()
                    .copied()
                    .map(|u| Token::Uint(u256_to_uint(&u)))
                    .collect(),
            ),
        ])])
        .to_vec();

        // pad the remaining fields, so we can compute their offsets
        let padded_data = pad32(&self.data);
        let padded_signature = pad32(&self.signature);
        let padded_factory_deps = pad32(&self.factory_deps);
        let padded_paymaster_input = pad32(&self.paymaster_input);
        let padded_reserved_dynamic = pad32(&self.reserved_dynamic);

        // the encoded data + 5 offsets
        let data_offset = res.len() + 5 * U256::BYTES;
        assert!(
            data_offset == 19 * U256::BYTES,
            "data offset is {}",
            data_offset
        );
        let signature_offset = data_offset + U256::BYTES + padded_data.len();
        let factory_deps_offset = signature_offset + U256::BYTES + padded_signature.len();
        let paymaster_input_offset = factory_deps_offset + U256::BYTES + padded_factory_deps.len();
        let reserved_dynamic_offset =
            paymaster_input_offset + U256::BYTES + padded_paymaster_input.len();

        // append the offsets
        res.extend(U256::from(data_offset).to_be_bytes::<32>());
        res.extend(U256::from(signature_offset).to_be_bytes::<32>());
        res.extend(U256::from(factory_deps_offset).to_be_bytes::<32>());
        res.extend(U256::from(paymaster_input_offset).to_be_bytes::<32>());
        res.extend(U256::from(reserved_dynamic_offset).to_be_bytes::<32>());

        // append the remaining fields
        let data_len = U256::from(self.data.len()).to_be_bytes::<32>();
        res.extend(data_len);
        res.extend(padded_data);
        let signature_len = U256::from(self.signature.len()).to_be_bytes::<32>();
        res.extend(signature_len);
        res.extend(padded_signature);
        // note that this field is the number of array elements, the elements have u256
        let num_elements = U256::from(self.factory_deps.len() / U256::BYTES).to_be_bytes::<32>();
        res.extend(num_elements);
        res.extend(padded_factory_deps);
        let paymaster_input_len = U256::from(self.paymaster_input.len()).to_be_bytes::<32>();
        res.extend(paymaster_input_len);
        res.extend(padded_paymaster_input);
        let reserved_dynamic_len = U256::from(self.reserved_dynamic.len()).to_be_bytes::<32>();
        res.extend(reserved_dynamic_len);
        res.extend(padded_reserved_dynamic);
        res
    }

    pub fn to_zk_bytes(&self) -> Vec<u8> {
        let mut output = vec![0u8; TX_OFFSET];
        output.extend(self.clone().abi_encode());
        output
    }
}

/// Convert a &ZkSyncTransaction to TransactionData
impl From<&ZkSyncTransaction<'_>> for TransactionData {
    fn from(tx: &ZkSyncTransaction<'_>) -> Self {
        TransactionData {
            tx_type: tx.tx_type.read(),
            from: Address::from_slice(&tx.encoding(tx.from.clone())[12..32]),
            to: Some(Address::from_slice(&tx.encoding(tx.to.clone())[12..32])),
            gas_limit: U256::from(tx.gas_limit.read()),
            pubdata_price_limit: U256::from(tx.gas_per_pubdata_limit.read()),
            max_fee_per_gas: U256::from(tx.max_fee_per_gas.read()),
            max_priority_fee_per_gas: U256::from(tx.max_priority_fee_per_gas.read()),
            paymaster: Address::from_slice(&tx.encoding(tx.paymaster.clone())[12..32]),
            nonce: U256::from(tx.nonce.read()),
            value: U256::from(tx.value.read()),
            reserved: tx
                .reserved
                .iter()
                .map(|u| U256::from(u.read()))
                .collect::<Vec<U256>>()
                .try_into()
                .unwrap(),
            data: tx.encoding(tx.data.clone()).to_vec(),
            signature: tx.encoding(tx.signature.clone()).to_vec(),
            factory_deps: tx.encoding(tx.factory_deps.clone()).to_vec(),
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
pub fn mock_oracle() -> ForwardRunningOracle<InMemoryTree, InMemoryPreimageSource, TxListSource> {
    let tree = InMemoryTree {
        storage_tree: TestingTree::new_in(Global),
        cold_storage: HashMap::new(),
    };
    ForwardRunningOracle {
        io_implementer_init_data: Some(io_implementer_init_data(Some(FlatStorageCommitment::<
            { TESTING_TREE_HEIGHT },
        > {
            root: *tree.storage_tree.root(),
            next_free_slot: tree.storage_tree.next_free_slot,
        }))),
        preimage_source: InMemoryPreimageSource {
            inner: HashMap::new(),
        },
        tree,
        block_metadata: BlockMetadataFromOracle::new_for_test(),
        next_tx: None,
        tx_source: TxListSource {
            transactions: VecDeque::new(),
        },
    }
}

#[allow(unused)]
pub fn mock_oracle_balance(
    address: B160,
    balance: U256,
) -> ForwardRunningOracle<InMemoryTree, InMemoryPreimageSource, TxListSource> {
    let mut tree = InMemoryTree {
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

    ForwardRunningOracle {
        io_implementer_init_data: Some(io_implementer_init_data(Some(FlatStorageCommitment::<
            { TESTING_TREE_HEIGHT },
        > {
            root: *tree.storage_tree.root(),
            next_free_slot: tree.storage_tree.next_free_slot,
        }))),
        preimage_source,
        tree,
        block_metadata: BlockMetadataFromOracle::new_for_test(),
        next_tx: None,
        tx_source: TxListSource {
            transactions: VecDeque::new(),
        },
    }
}

// TODO: currently internal rust error if uncommented
// pub fn mock_system() -> ForwardRunningSystem<InMemoryTree, InMemoryPreimageSource, TxListSource> {
//     ForwardRunningSystem::init_from_oracle(mock_oracle()).expect("Failed to initialize the mock system")
// }

#[allow(dead_code)]
pub(crate) fn serialize_zksync_transaction<'a>(tx: &ZkSyncTransaction<'a>) -> Vec<u8> {
    let tx_data = TransactionData::from(tx);
    let mut output = vec![0u8; TX_OFFSET];
    output.extend(tx_data.clone().abi_encode());
    output
}

pub fn mutate_transaction(data: &mut [u8], size: usize, max_size: usize, seed: u32) -> usize {
    // Initialize random number generator with a deterministic seed
    let mut rng = StdRng::seed_from_u64(seed as u64);

    // Attempt to decode the input transaction
    let decoded_tx = ZkSyncTransaction::try_from_slice(&mut data[..size]);
    if decoded_tx.is_err() {
        // If decoding fails, return the original size and data
        return size;
    }
    let mut tx = decoded_tx.unwrap();
    // convert tx to TransactionData, so we can freely mutate the fields
    let mut tx_data = TransactionData::from(&tx);

    // Apply random mutations to the transaction.
    mutate_zksync_transaction(&mut tx_data, &mut rng);

    // change the from field to match the private key
    tx_data.from = Address::from_slice(&ACCOUNT[12..32]);

    // convert tx_data back to ZkSyncTransaction, so we can use its functions
    let mut tx_data_bytes = tx_data.to_zk_bytes();
    if let Ok(new_tx) = ZkSyncTransaction::try_from_slice(tx_data_bytes.as_mut_slice()) {
        tx = new_tx;
    } else {
        return size;
    }

    // generate a new signature from the signed hash and the private key
    let signed_hash = if let Ok(h) = tx.calculate_signed_hash(CHAIN_ID) {
        h
    } else {
        return size;
    };
    let msg = Message::from_digest(signed_hash.try_into().unwrap());
    let secp = Secp256k1::new();
    let secret_key = SecretKey::from_slice(&PRIVATE_KEY).expect("expected a valid private key");
    let signature = secp.sign_ecdsa_recoverable(&msg, &secret_key);
    let (rec_id, sig_data) = signature.serialize_compact();
    tx_data.signature = Vec::<u8>::with_capacity(65);
    tx_data.signature.extend(&sig_data[..]);
    tx_data.signature.push(rec_id as u8 + 27);
    assert!(
        tx_data.signature.len() == 65,
        "signature length is {}",
        tx_data.signature.len()
    );

    // Serialize the mutated transaction back into a byte array
    let serialized_tx = tx_data.to_zk_bytes();

    // try to deserialize the transaction again, to see whether it works
    let mut serialized_tx_copy = serialized_tx.clone();
    if let Err(_) = ZkSyncTransaction::try_from_slice(serialized_tx_copy.as_mut_slice()) {
        println!("data          = {}", hex::encode(data));
        println!("serialized_tx = {}", hex::encode(serialized_tx.as_slice()));
        panic!("broken serialization");
    } else {
        (); // OK
    }

    // If the serialized transaction exceeds the max size, return the original data
    if serialized_tx.len() > max_size {
        size
    } else {
        // Update the input data and return the new size
        data[..serialized_tx.len()].copy_from_slice(&serialized_tx);
        serialized_tx.len()
    }
}

// Mutates various parts of the ZkSync transaction
#[allow(dead_code)]
fn mutate_zksync_transaction(tx: &mut TransactionData, rng: &mut StdRng) {
    match rng.gen_range(0..=8) {
        0 => {
            tx.tx_type = mutate_u8(tx.tx_type, rng);
        }
        1 => {
            mutate_address_inplace(&mut tx.from, rng);
        }
        2 => {
            if let Some(addr) = tx.to.as_mut() {
                mutate_address_inplace(addr, rng)
            }
        }
        3 => {
            tx.gas_limit = mutate_u256_vec(tx.gas_limit, rng);
        }
        4 => {
            tx.max_fee_per_gas = mutate_u256_vec(tx.max_fee_per_gas, rng);
        }
        5 => {
            tx.max_priority_fee_per_gas = mutate_u256_vec(tx.max_priority_fee_per_gas, rng);
        }
        6 => {
            tx.nonce = mutate_u256_vec(tx.nonce, rng);
        }
        7 => {
            tx.value = mutate_u256_vec(tx.value, rng);
        }
        8 => { /* No operation */ }
        _ => {}
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

#[allow(dead_code)]
fn mutate_u8(num: u8, rng: &mut StdRng) -> u8 {
    num ^ rng.gen::<u8>()
}

#[allow(dead_code)]
fn mutate_address_inplace(addr: &mut Address, rng: &mut StdRng) {
    let addr_bytes: &mut [u8] = addr.as_bytes_mut();
    let idx = rng.gen_range(0..addr_bytes.len());
    addr_bytes[idx] ^= rng.gen::<u8>();
}
