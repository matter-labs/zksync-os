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
use zk_ee::common_structs::interop_root_storage::InteropRoot;
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

/// Version byte for pubdata encoding format.
/// Version 1: Initial versioned pubdata format
/// Version 2: Remove artifacts_len and artifacts from pubdata
pub const PUBDATA_ENCODING_VERSION: u8 = 2;

/// Helper method to write the pubdata to the DA commitment generator and result keeper.
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
) {
    // Write version byte first to enable future pubdata format upgrades
    pubdata_dst.write(&[PUBDATA_ENCODING_VERSION]);
    pubdata_dst.write(block_hash.as_u8_ref());
    pubdata_dst.write(&timestamp.to_be_bytes());

    result_keeper.pubdata(&[PUBDATA_ENCODING_VERSION]);
    result_keeper.pubdata(block_hash.as_u8_ref());
    result_keeper.pubdata(&timestamp.to_be_bytes());

    io.storage
        .apply_storage_diffs_pubdata(result_keeper, pubdata_dst, &mut io.oracle);

    io.logs_storage.apply_pubdata(pubdata_dst, result_keeper);
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
/// For each root: rolling_hash = keccak256(old_rolling_hash || chain_id || block_number || root_hash)
pub fn calculate_interop_roots_rolling_hash<'a>(
    old_rolling_hash: Bytes32,
    roots: impl Iterator<Item = &'a InteropRoot>,
    hasher: &mut crypto::sha3::Keccak256,
) -> Bytes32 {
    let mut data = [0u8; 96];

    let mut rolling_hash = old_rolling_hash;
    for root in roots {
        data[0..32].copy_from_slice(&rolling_hash.as_u8_ref());
        data[32..64].copy_from_slice(&root.chain_id.to_be_bytes::<{ U256::BYTES }>());
        data[64..96].copy_from_slice(&root.block_or_batch_number.to_be_bytes::<{ U256::BYTES }>());
        hasher.update(data);

        // Note: now we have only one side
        hasher.update(root.root.as_u8_ref());

        rolling_hash = hasher.finalize_reset().into()
    }

    rolling_hash
}

/// Number of leaves in the chain batch root Merkle tree (fixed height-3 tree = 8 leaves).
pub const CHAIN_BATCH_ROOT_TREE_LEAVES: usize = 8;

/// Builds the chain batch root as a fixed height-3 (8-leaf) keccak256 Merkle tree.
///
/// Leaf layout — the first four carry the live commitments, the last four are reserved (zero) for now:
///   0: l2 logs root
///   1: multichain root
///   2: interop commitment tree (IMT) root at batch begin
///   3: interop commitment tree (IMT) root at batch end
///   4..8: reserved (`Bytes32::ZERO`)
///
/// Internal nodes are `keccak256(left || right)`; leaves are used directly (no separate leaf-hashing
/// step), matching the existing `l2_logs_root` tree. Because the whole right subtree is zero, this is
/// equivalent to the canonical empty-subtree convention with a zero empty leaf, so reserved leaves can
/// later be populated in place without changing the shape.
pub fn compute_chain_batch_root(
    l2_logs_root: Bytes32,
    multichain_root: Bytes32,
    commitment_tree_root_begin: Bytes32,
    commitment_tree_root_end: Bytes32,
) -> Bytes32 {
    let mut nodes = [
        l2_logs_root,
        multichain_root,
        commitment_tree_root_begin,
        commitment_tree_root_end,
        Bytes32::ZERO,
        Bytes32::ZERO,
        Bytes32::ZERO,
        Bytes32::ZERO,
    ];

    // Reduce level by level in place: 8 -> 4 -> 2 -> 1. Node `i` of the next level hashes children
    // `2*i` and `2*i + 1`; the write index is always below the read indices, so no live value is
    // clobbered before it is consumed.
    let mut width = CHAIN_BATCH_ROOT_TREE_LEAVES;
    while width > 1 {
        let half = width / 2;
        for i in 0..half {
            let mut hasher = crypto::sha3::Keccak256::new();
            hasher.update(nodes[2 * i].as_u8_ref());
            hasher.update(nodes[2 * i + 1].as_u8_ref());
            nodes[i] = Bytes32::from_array(hasher.finalize());
        }
        width = half;
    }

    nodes[0]
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

/// Fixed depth of the interop commitment tree's Indexed Merkle Tree. Mirrors `IMT_DEPTH` in
/// `l1-contracts/contracts/common/libraries/IndexedMerkleTree.sol`.
const INTEROP_COMMITMENT_TREE_IMT_DEPTH: u8 = 32;

/// Storage slot of `_imt.nodes[IMT_DEPTH][0]` — the interop commitment tree's root node — in the
/// L2InteropCommitmentTree(0x10012) contract.
///
/// Derived from the Solidity storage layout (verified with `forge inspect`): `_imt` sits at slot 0;
/// inside `struct IMT` the `bytes32[IMT_DEPTH + 1] zeros` array occupies slots `0..=IMT_DEPTH`, so the
/// `mapping(uint256 => mapping(uint256 => bytes32)) nodes` base slot is `IMT_DEPTH + 1` (= 33). For a
/// nested mapping: `inner = keccak256(pad32(IMT_DEPTH) || pad32(33))`, then
/// `slot = keccak256(pad32(0) || inner)`. Locked against the formula by
/// `commitment_tree_root_slot_matches_layout` below.
const COMMITMENT_TREE_ROOT_NODE_SLOT: [u8; 32] = [
    0xf7, 0x00, 0x9d, 0x13, 0x71, 0x93, 0xf8, 0x68, 0x7e, 0x15, 0x07, 0x32, 0x6b, 0x04, 0x75, 0xde,
    0xd1, 0xa0, 0xd0, 0x9a, 0xa4, 0xd4, 0x61, 0x6a, 0xc9, 0xb1, 0x95, 0xb2, 0xfb, 0x33, 0x3f, 0x81,
];

///
/// Reads the interop commitment tree root from the L2InteropCommitmentTree(0x10012) contract.
///
/// Returns `_imt.nodes[IMT_DEPTH][0]` (the tree root node). When that node has not been written yet —
/// an empty / freshly-seeded tree — it falls back to `_imt.zeros[IMT_DEPTH]` (the last element of the
/// `zeros` array, at slot `IMT_DEPTH`), exactly mirroring `IndexedMerkleTreeLib.root`. On a chain that
/// does not have the tree deployed the reads return zero, so this yields `Bytes32::zero()`.
///
/// Generic over the IO subsystem (like `read_settlement_layer_chain_id`) so it can be called both
/// before the tx loop (batch-begin snapshot) and after it (batch-end snapshot).
///
pub fn read_commitment_tree_root<IO: IOSubsystem>(io: &mut IO) -> Bytes32
where
    IO::IOTypes: SystemIOTypesConfig<Address = B160, StorageKey = Bytes32, StorageValue = Bytes32>,
{
    let mut inf_resources = IO::Resources::FORMAL_INFINITE;

    let root_node = io
        .storage_read::<false>(
            ExecutionEnvironmentType::NoEE,
            &mut inf_resources,
            &L2_INTEROP_COMMITMENT_TREE_ADDRESS,
            &Bytes32::from_array(COMMITMENT_TREE_ROOT_NODE_SLOT),
        )
        .expect("must read InteropCommitmentTree root node");

    if !root_node.is_zero() {
        return root_node;
    }

    // Empty / freshly-seeded tree: the top node is unwritten, so the canonical root is
    // `zeros[IMT_DEPTH]`, stored at slot `IMT_DEPTH` (last element of `bytes32[IMT_DEPTH + 1] zeros`,
    // which starts at slot 0).
    let mut zeros_root_slot = [0u8; 32];
    zeros_root_slot[31] = INTEROP_COMMITMENT_TREE_IMT_DEPTH;
    io.storage_read::<false>(
        ExecutionEnvironmentType::NoEE,
        &mut inf_resources,
        &L2_INTEROP_COMMITMENT_TREE_ADDRESS,
        &Bytes32::from_array(zeros_root_slot),
    )
    .expect("must read InteropCommitmentTree zeros root")
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

    /// Recomputes the interop-commitment-tree root slot from the storage layout and locks the
    /// hardcoded `COMMITMENT_TREE_ROOT_NODE_SLOT` against it. `_imt` is at slot 0; `IMT.zeros` is
    /// `bytes32[IMT_DEPTH + 1]` occupying slots `0..=IMT_DEPTH`, so the `nodes` mapping base slot is
    /// `IMT_DEPTH + 1`. Then `nodes[IMT_DEPTH][0]` follows the nested-mapping rule.
    fn calculate_commitment_tree_root_slot() -> [u8; 32] {
        let nodes_base_slot = INTEROP_COMMITMENT_TREE_IMT_DEPTH + 1;

        let mut level = [0u8; 32];
        level[31] = INTEROP_COMMITMENT_TREE_IMT_DEPTH;
        let mut base = [0u8; 32];
        base[31] = nodes_base_slot;
        // inner mapping slot: keccak256(pad32(IMT_DEPTH) || pad32(nodes_base_slot))
        let mut hasher = crypto::sha3::Keccak256::new();
        hasher.update(level);
        hasher.update(base);
        let inner = hasher.finalize();

        // element [0] of the inner mapping: keccak256(pad32(0) || inner)
        let index = [0u8; 32];
        let mut hasher = crypto::sha3::Keccak256::new();
        hasher.update(index);
        hasher.update(inner);
        hasher.finalize()
    }

    #[test]
    fn commitment_tree_root_slot_matches_layout() {
        assert_eq!(
            calculate_commitment_tree_root_slot(),
            COMMITMENT_TREE_ROOT_NODE_SLOT
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
    fn chain_batch_root_all_zero_is_deterministic() {
        // Sanity: an all-zero input (empty batch) is well defined and non-trivial (non-zero).
        let z = Bytes32::ZERO;
        assert_ne!(compute_chain_batch_root(z, z, z, z), Bytes32::ZERO);
    }
}
