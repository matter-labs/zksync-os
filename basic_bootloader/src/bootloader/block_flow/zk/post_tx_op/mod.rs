use super::*;
use basic_system::system_implementation::caches::storage_access_policy::StorageAccessPolicy;
use basic_system::system_implementation::flat_storage_model::FlatTreeWithAccountsUnderHashesStorageModel;
use basic_system::system_implementation::system::FullIO;
use core::alloc::Allocator;
use crypto::MiniDigest;
use ruint::aliases::{B160, U256};
use system_hooks::addresses_constants::{
    L2_INTEROP_COMMITMENT_TREE_ADDRESS, MESSAGE_ROOT_ADDRESS, SYSTEM_CONTEXT_ADDRESS,
};
use zk_ee::common_structs::da_commitment_scheme::PubdataContent;
use zk_ee::common_structs::interop_root_storage::InteropRoot;
use zk_ee::common_structs::merkle_root_in_place;
use zk_ee::memory::stack_trait::StackFactory;
use zk_ee::oracle::IOOracle;
use zk_ee::system::{IOSubsystem, Resource, Resources};
use zk_ee::types_config::SystemIOTypesConfig;
use zk_ee::utils::write_bytes::WriteBytes;
use zk_ee::utils::Bytes32;

pub mod da_commitment_generator;
mod post_tx_op_proving_multiblock_batch;
mod post_tx_op_proving_singleblock_batch;
mod post_tx_op_sequencing;
pub mod public_input;

/// Pubdata encoding version byte, shared by all pubdata contents.
/// Version 1: Initial versioned pubdata format
/// Version 2: Remove artifacts_len and artifacts from pubdata
/// Version 3: A `PubdataContent` mode byte follows the version byte and
/// selects the payload layout (full pubdata vs logs-only)
pub const PUBDATA_ENCODING_VERSION: u8 = 3;

/// Streams the block's pubdata into the DA commitment generator (`pubdata_dst`)
/// and the result keeper.
///
/// The exact same bytes go to both sinks, so the pubdata reported to the
/// sequencer/prover is byte-for-byte what the batch commits to. Every layout
/// starts with the shared two-byte header `[PUBDATA_ENCODING_VERSION, mode]`,
/// where the mode byte is the `PubdataContent` discriminant selecting the
/// payload that follows:
/// - `FullPubdata` (mode 0): the full pubdata —
///   `[block_hash, timestamp, state diffs, logs, message payloads]`.
/// - `LogsOnly` (mode 1): only the mandatory log section —
///   `[logs_count, log records]`. State diffs and message payloads are
///   neither committed nor reported here; the sequencer receives them through
///   the dedicated result-keeper channels (`storage_diffs`, `logs`).
fn write_pubdata<
    DST: WriteBytes + ?Sized,
    A: Allocator + Clone + Default,
    R: Resources,
    P: StorageAccessPolicy<R, Bytes32> + Default,
    SF: StackFactory<N>,
    const N: usize,
    O: IOOracle,
    const PROOF_ENV: bool,
>(
    pubdata_dst: &mut DST,
    result_keeper: &mut impl ResultKeeperExt<EthereumIOTypesConfig, BlockHeader = BlockHeader>,
    block_hash: Bytes32,
    timestamp: u64,
    io: &mut FullIO<
        A,
        R,
        P,
        SF,
        N,
        O,
        FlatTreeWithAccountsUnderHashesStorageModel<A, R, P, SF, N, PROOF_ENV>,
        PROOF_ENV,
    >,
    pubdata_content: PubdataContent,
) {
    // Shared header: the encoding version byte followed by the mode byte.
    let header = [PUBDATA_ENCODING_VERSION, pubdata_content as u8];
    pubdata_dst.write(&header);
    result_keeper.pubdata(&header);
    match pubdata_content {
        PubdataContent::FullPubdata => {
            pubdata_dst.write(block_hash.as_u8_ref());
            pubdata_dst.write(&timestamp.to_be_bytes());
            result_keeper.pubdata(block_hash.as_u8_ref());
            result_keeper.pubdata(&timestamp.to_be_bytes());

            io.storage
                .apply_storage_diffs_pubdata(result_keeper, pubdata_dst, &mut io.oracle);
            // logs then message payloads (matches the pre-split combined encoding).
            io.logs_storage
                .apply_logs_pubdata(pubdata_dst, result_keeper);
            io.logs_storage
                .apply_messages_pubdata(pubdata_dst, result_keeper);
        }
        PubdataContent::LogsOnly => {
            // Only the mandatory L2->L1 log section is committed and reported.
            io.logs_storage
                .apply_logs_pubdata(pubdata_dst, result_keeper);
        }
    }
}

/// Helper method to create block header.
fn form_block_header<S: EthereumLikeTypes>(
    system: &System<S>,
    transactions_root: Bytes32,
    receipts_root: Bytes32,
    block_gas_used: u64,
) -> Result<BlockHeader, BootloaderSubsystemError> {
    let block_number = system.get_block_number();
    let previous_block_hash = if block_number == 0 {
        Bytes32::ZERO
    } else {
        system.get_blockhash(block_number - 1)?
    };
    let beneficiary = system.get_coinbase();
    let gas_limit = system.get_gas_limit();
    let timestamp = system.get_timestamp();
    let consensus_random = system.get_mix_hash()?;
    let base_fee_per_gas = system.get_eip1559_basefee();
    // TODO: add pubdata price and native price
    let base_fee_per_gas = base_fee_per_gas
        .try_into()
        .map_err(|_| internal_error!("base_fee_per_gas exceeds max u64"))?;

    Ok(BlockHeader::new(
        previous_block_hash,
        beneficiary,
        transactions_root,
        receipts_root,
        block_number,
        gas_limit,
        block_gas_used,
        timestamp,
        consensus_random,
        base_fee_per_gas,
    ))
}

/// Calculates a rolling hash over a sequence of interop roots.
/// This creates a cumulative digest that can be verified on settlement layers.
///
/// For each root:
/// rolling_hash = keccak256(old_rolling_hash || chain_id || block_number || timestamp || root_hash),
/// mirroring `ExecutorFacet._verifyDependencyInteropRoots` on the settlement layer
/// (`abi.encodePacked(prev, chainId, blockOrBatchNumber, timestamp, sides)`).
pub fn calculate_interop_roots_rolling_hash<'a>(
    old_rolling_hash: Bytes32,
    roots: impl Iterator<Item = &'a InteropRoot>,
    hasher: &mut crypto::sha3::Keccak256,
) -> Bytes32 {
    let mut data = [0u8; 128];

    let mut rolling_hash = old_rolling_hash;
    for root in roots {
        data[0..32].copy_from_slice(&rolling_hash.as_u8_ref());
        data[32..64].copy_from_slice(&root.chain_id.to_be_bytes::<{ U256::BYTES }>());
        data[64..96].copy_from_slice(&root.block_or_batch_number.to_be_bytes::<{ U256::BYTES }>());
        data[96..128].copy_from_slice(&root.timestamp.to_be_bytes::<{ U256::BYTES }>());
        hasher.update(data);

        // Note: now we have only one side
        hasher.update(root.root.as_u8_ref());

        rolling_hash = hasher.finalize_reset().into()
    }

    rolling_hash
}

/// Height of the chain batch root Merkle tree (capacity `2^3 == 8` leaves): four live commitment
/// leaves followed by four reserved (zero) leaves.
pub const CHAIN_BATCH_ROOT_TREE_HEIGHT: usize = 3;

/// Empty-subtree hashes for the chain batch root tree, where entry `i` is the root of an empty
/// subtree of height `i`. The empty (reserved) leaf is `Bytes32::ZERO` and each level doubles up:
/// `entry[i] = keccak256(entry[i - 1] || entry[i - 1])`.
const CHAIN_BATCH_ROOT_EMPTY_SUBTREE_HASHES: [[u8; 32]; CHAIN_BATCH_ROOT_TREE_HEIGHT + 1] = [
    [0u8; 32],
    [
        0xad, 0x32, 0x28, 0xb6, 0x76, 0xf7, 0xd3, 0xcd, 0x42, 0x84, 0xa5, 0x44, 0x3f, 0x17, 0xf1,
        0x96, 0x2b, 0x36, 0xe4, 0x91, 0xb3, 0x0a, 0x40, 0xb2, 0x40, 0x58, 0x49, 0xe5, 0x97, 0xba,
        0x5f, 0xb5,
    ],
    [
        0xb4, 0xc1, 0x19, 0x51, 0x95, 0x7c, 0x6f, 0x8f, 0x64, 0x2c, 0x4a, 0xf6, 0x1c, 0xd6, 0xb2,
        0x46, 0x40, 0xfe, 0xc6, 0xdc, 0x7f, 0xc6, 0x07, 0xee, 0x82, 0x06, 0xa9, 0x9e, 0x92, 0x41,
        0x0d, 0x30,
    ],
    [
        0x21, 0xdd, 0xb9, 0xa3, 0x56, 0x81, 0x5c, 0x3f, 0xac, 0x10, 0x26, 0xb6, 0xde, 0xc5, 0xdf,
        0x31, 0x24, 0xaf, 0xba, 0xdb, 0x48, 0x5c, 0x9b, 0xa5, 0xa3, 0xe3, 0x39, 0x8a, 0x04, 0xb7,
        0xba, 0x85,
    ],
];

/// Builds the chain batch root as a fixed height-3 (8-leaf) keccak256 Merkle tree.
///
/// Leaf layout — the first four carry the live commitments, the last four are reserved (zero) for now:
///   0: l2 logs root
///   1: multichain root
///   2: interop commitment tree (IMT) root at batch begin
///   3: interop commitment tree (IMT) root at batch end
///   4..8: reserved (`Bytes32::ZERO`)
///
pub fn compute_chain_batch_root(
    l2_logs_root: Bytes32,
    multichain_root: Bytes32,
    commitment_tree_root_begin: Bytes32,
    commitment_tree_root_end: Bytes32,
) -> Bytes32 {
    let mut leaves = [
        l2_logs_root,
        multichain_root,
        commitment_tree_root_begin,
        commitment_tree_root_end,
    ];
    let empty_subtree_hashes = CHAIN_BATCH_ROOT_EMPTY_SUBTREE_HASHES.map(Bytes32::from_array);
    merkle_root_in_place::<crypto::sha3::Keccak256>(&mut leaves, &empty_subtree_hashes)
}

///
/// Reads SL chain id from the SystemContext(0x800b) contract.
///
pub fn read_settlement_layer_chain_id<IO: IOSubsystem>(io: &mut IO) -> U256
where
    IO::IOTypes: SystemIOTypesConfig<Address = B160, StorageKey = Bytes32, StorageValue = Bytes32>,
{
    // This helper is intentionally generic over the IO subsystem so it can be
    // reused from bootloader transaction flow code that only has access to
    // `System<S>::io`.
    const SL_CHAIN_ID_STORAGE_SLOT: Bytes32 = Bytes32::ZERO;
    let mut inf_resources = IO::Resources::FORMAL_INFINITE;
    let chain_id = io
        .storage_read::<false>(
            ExecutionEnvironmentType::NoEE,
            &mut inf_resources,
            &SYSTEM_CONTEXT_ADDRESS,
            &SL_CHAIN_ID_STORAGE_SLOT,
        )
        .expect("must read SystemContext SL chain id");
    U256::from_be_bytes(chain_id.as_u8_array())
}

///
/// Reads multichain root from the L2MessageRoot(0x10005) contract.
///
/// Multichain root is the commitment to l2 to l1 logs from the chains that settles on top of current.
/// It's not zero if the current chain is used as the settlement layer.
///
pub fn read_multichain_root<
    A: Allocator + Clone + Default,
    R: Resources,
    P: StorageAccessPolicy<R, Bytes32> + Default,
    SF: StackFactory<N>,
    const N: usize,
    O: IOOracle,
    const PROOF_ENV: bool,
>(
    io: &mut FullIO<
        A,
        R,
        P,
        SF,
        N,
        O,
        FlatTreeWithAccountsUnderHashesStorageModel<A, R, P, SF, N, PROOF_ENV>,
        PROOF_ENV,
    >,
) -> Bytes32 {
    use zk_ee::system::IOSubsystem;

    const SHARED_TREE_HEIGHT_STORAGE_SLOT: [u8; 32] = [
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 4,
    ];
    let mut inf_resources = R::FORMAL_INFINITE;

    // we need to read self._nodes[self._height][0]
    let tree_height = io
        .storage_read::<false>(
            ExecutionEnvironmentType::NoEE,
            &mut inf_resources,
            &MESSAGE_ROOT_ADDRESS,
            &Bytes32::from_array(SHARED_TREE_HEIGHT_STORAGE_SLOT),
        )
        .expect("must read MessageRoot shared tree height");

    let root_slot = calculate_multichain_root_slot(tree_height);

    io.storage_read::<false>(
        ExecutionEnvironmentType::NoEE,
        &mut inf_resources,
        &MESSAGE_ROOT_ADDRESS,
        &root_slot,
    )
    .expect("must read MessageRoot multichain root")
}

/// Storage slot of `_imt.tree._height` in the L2InteropCommitmentTree(0x10012) contract: the IMT
/// is the contract's first state variable and `FullMerkle.FullTree` puts `_height` at offset 0 —
/// a deliberate, consensus-critical storage ABI (see `L2InteropCommitmentTree.sol`).
const COMMITMENT_TREE_HEIGHT_STORAGE_SLOT: [u8; 32] = [0u8; 32];

///
/// Reads the interop commitment tree (IMT) root from the L2InteropCommitmentTree(0x10012) contract.
///
/// Mirrors `read_multichain_root`: the contract keeps the root in its dynamic-height `FullMerkle`
/// engine at `_imt.tree._nodes[_height][0]`, so the read loads `_height` (slot 0) and derives the
/// `_nodes[_height][0]` slot from the `_nodes` base slot 2. On a chain that does not have the tree
/// deployed (or before seeding) both reads return zero storage, so this yields `Bytes32::zero()`.
///
pub fn read_interop_commitment_tree_root<IO: IOSubsystem>(io: &mut IO) -> Bytes32
where
    IO::IOTypes: SystemIOTypesConfig<Address = B160, StorageKey = Bytes32, StorageValue = Bytes32>,
{
    let mut inf_resources = IO::Resources::FORMAL_INFINITE;

    let tree_height = io
        .storage_read::<false>(
            ExecutionEnvironmentType::NoEE,
            &mut inf_resources,
            &L2_INTEROP_COMMITMENT_TREE_ADDRESS,
            &Bytes32::from_array(COMMITMENT_TREE_HEIGHT_STORAGE_SLOT),
        )
        .expect("must read InteropCommitmentTree height");

    let root_slot = calculate_imt_root_slot(tree_height);

    io.storage_read::<false>(
        ExecutionEnvironmentType::NoEE,
        &mut inf_resources,
        &L2_INTEROP_COMMITMENT_TREE_ADDRESS,
        &root_slot,
    )
    .expect("must read InteropCommitmentTree root")
}

///
/// Calculates the storage slot of the IMT root in the L2InteropCommitmentTree(0x10012) contract.
///
/// By convention the slot depends only on `tree_height`. It is the solidity dynamic array access
/// `_imt.tree._nodes[height][0]`, where `_nodes` is located at slot 2 (same derivation as
/// `calculate_multichain_root_slot`, which reads the L2MessageRoot's `FullMerkle` at slot 6).
///
fn calculate_imt_root_slot(tree_height: Bytes32) -> Bytes32 {
    use core::ops::Add;
    // keccak256(0x0000000000000000000000000000000000000000000000000000000000000002)
    const NODES_FIRST_ELEMENT_SLOT: [u8; 32] = [
        0x40, 0x57, 0x87, 0xfa, 0x12, 0xa8, 0x23, 0xe0, 0xf2, 0xb7, 0x63, 0x1c, 0xc4, 0x1b, 0x3b,
        0xa8, 0x82, 0x8b, 0x33, 0x21, 0xca, 0x81, 0x11, 0x11, 0xfa, 0x75, 0xcd, 0x3a, 0xa3, 0xbb,
        0x5a, 0xce,
    ];

    // _nodes[height] slot
    let nodes_height_array_slot = U256::from_be_bytes(NODES_FIRST_ELEMENT_SLOT)
        .add(U256::from_be_bytes(tree_height.as_u8_array()));
    let mut hasher = crypto::sha3::Keccak256::new();
    hasher.update(nodes_height_array_slot.to_be_bytes::<32>());
    // _nodes[height][0]
    Bytes32::from_array(hasher.finalize())
}

///
/// Reads values that must consume the oracle in the same order in both proving
/// flows.
///
pub fn read_batch_context_inputs<
    A: Allocator + Clone + Default,
    R: Resources,
    P: StorageAccessPolicy<R, Bytes32> + Default,
    SF: StackFactory<N>,
    const N: usize,
    O: IOOracle,
    const PROOF_ENV: bool,
>(
    io: &mut FullIO<
        A,
        R,
        P,
        SF,
        N,
        O,
        FlatTreeWithAccountsUnderHashesStorageModel<A, R, P, SF, N, PROOF_ENV>,
        PROOF_ENV,
    >,
) -> (Bytes32, U256) {
    let multichain_root = read_multichain_root(io);
    let settlement_layer_chain_id = read_settlement_layer_chain_id(io);
    if let Some(new_settlement_layer_chain_id) = io.new_settlement_layer_chain_id_storage.value() {
        // If the SL chain id was updated, make sure the updated one matches
        // the one read from storage.
        assert_eq!(new_settlement_layer_chain_id, &settlement_layer_chain_id);
    }

    (multichain_root, settlement_layer_chain_id)
}

///
/// Calculates storage slot of multichain tree root in L2MessageRoot(0x10005) contract.
///
/// By convention storage slot for it should stay the same(depend only on `tree_height`).
/// In fact, it's solidity dynamic array access `_nodes[height][0]`, which is located on slot 6.
///
fn calculate_multichain_root_slot(tree_height: Bytes32) -> Bytes32 {
    use core::ops::Add;
    // keccak256(0x0000000000000000000000000000000000000000000000000000000000000006)
    const NODES_FIRST_ELEMENT_SLOT: [u8; 32] = [
        0xf6, 0x52, 0x22, 0x23, 0x13, 0xe2, 0x84, 0x59, 0x52, 0x8d, 0x92, 0x0b, 0x65, 0x11, 0x5c,
        0x16, 0xc0, 0x4f, 0x3e, 0xfc, 0x82, 0xaa, 0xed, 0xc9, 0x7b, 0xe5, 0x9f, 0x3f, 0x37, 0x7c,
        0x0d, 0x3f,
    ];

    // _nodes[height] slot
    let nodes_height_array_slot = U256::from_be_bytes(NODES_FIRST_ELEMENT_SLOT)
        .add(U256::from_be_bytes(tree_height.as_u8_array()));
    let mut hasher = crypto::sha3::Keccak256::new();
    hasher.update(nodes_height_array_slot.to_be_bytes::<32>());
    // _nodes[height][0]
    Bytes32::from_array(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    // test cases data was made using actual solidity implementation and debug data for read tx:
    // height 1: 0x768c3a22b1e4688c94525eb9bc2cf1ce7601fc9e871dc6e10fc44f0f06340ce1
    // height 3: 0x38ace9b5569ba016113e31884532182bc747997e743c0b7f9c307302b5f83760
    // height 4: 0x35817d789b7a6dbe8b95b0f21e189fb26d3d329de699cac7a267a9568298e0a5
    #[test]
    fn test_calculate_multichain_root_slot_tree_height_1() {
        let tree_height = [
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 1,
        ];
        let root_slot = calculate_multichain_root_slot(Bytes32::from_array(tree_height));

        assert_eq!(
            root_slot.as_u8_array().to_vec(),
            hex::decode("768c3a22b1e4688c94525eb9bc2cf1ce7601fc9e871dc6e10fc44f0f06340ce1")
                .unwrap()
        );
    }

    #[test]
    fn test_calculate_multichain_root_slot_tree_height_3() {
        let tree_height = [
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 3,
        ];
        let root_slot = calculate_multichain_root_slot(Bytes32::from_array(tree_height));

        assert_eq!(
            root_slot.as_u8_array().to_vec(),
            hex::decode("38ace9b5569ba016113e31884532182bc747997e743c0b7f9c307302b5f83760")
                .unwrap()
        );
    }

    #[test]
    fn test_calculate_multichain_root_slot_tree_height_4() {
        let tree_height = [
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 4,
        ];
        let root_slot = calculate_multichain_root_slot(Bytes32::from_array(tree_height));

        assert_eq!(
            root_slot.as_u8_array().to_vec(),
            hex::decode("35817d789b7a6dbe8b95b0f21e189fb26d3d329de699cac7a267a9568298e0a5")
                .unwrap()
        );
    }

    // Expected values: keccak256(keccak256(uint256(2)) + height), cross-checked against the
    // solidity layout lock test (`L2InteropCommitmentTreeStorage.t.sol`).
    #[test]
    fn test_calculate_imt_root_slot_tree_height_0() {
        let root_slot = calculate_imt_root_slot(Bytes32::ZERO);

        assert_eq!(
            root_slot.as_u8_array().to_vec(),
            hex::decode("1ab0c6948a275349ae45a06aad66a8bd65ac18074615d53676c09b67809099e0")
                .unwrap()
        );
    }

    #[test]
    fn test_calculate_imt_root_slot_tree_height_4() {
        let tree_height = [
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 4,
        ];
        let root_slot = calculate_imt_root_slot(Bytes32::from_array(tree_height));

        assert_eq!(
            root_slot.as_u8_array().to_vec(),
            hex::decode("cc034019b449ad16908580172ec972745a229ec6575a8d785eaa22043f92c453")
                .unwrap()
        );
    }

    #[test]
    fn chain_batch_root_is_height3_merkle() {
        fn node(a: &Bytes32, b: &Bytes32) -> Bytes32 {
            let mut h = crypto::sha3::Keccak256::new();
            h.update(a.as_u8_ref());
            h.update(b.as_u8_ref());
            Bytes32::from_array(h.finalize())
        }

        let a = Bytes32::from_byte_fill(1);
        let b = Bytes32::from_byte_fill(2);
        let c = Bytes32::from_byte_fill(3);
        let d = Bytes32::from_byte_fill(4);
        let z = Bytes32::ZERO;

        // Independent recomputation of the height-3 tree with the last four leaves zero.
        let l1 = [node(&a, &b), node(&c, &d), node(&z, &z), node(&z, &z)];
        let l2 = [node(&l1[0], &l1[1]), node(&l1[2], &l1[3])];
        let expected = node(&l2[0], &l2[1]);

        assert_eq!(compute_chain_batch_root(a, b, c, d), expected);
    }

    #[test]
    fn chain_batch_root_empty_hashes_match_recurrence() {
        // Locks the hardcoded table: the empty-subtree recurrence over the zero leaf
        // (`entry[i] = keccak256(entry[i - 1] || entry[i - 1])`) must reproduce it.
        let mut prev = Bytes32::ZERO;
        for entry in CHAIN_BATCH_ROOT_EMPTY_SUBTREE_HASHES {
            assert_eq!(Bytes32::from_array(entry), prev);
            let mut h = crypto::sha3::Keccak256::new();
            h.update(prev.as_u8_ref());
            h.update(prev.as_u8_ref());
            prev = Bytes32::from_array(h.finalize());
        }
    }

    #[test]
    fn chain_batch_root_all_zero_is_deterministic() {
        // Sanity: an all-zero input (empty batch) is well defined and non-trivial (non-zero).
        let z = Bytes32::ZERO;
        assert_ne!(compute_chain_batch_root(z, z, z, z), Bytes32::ZERO);
    }
}
