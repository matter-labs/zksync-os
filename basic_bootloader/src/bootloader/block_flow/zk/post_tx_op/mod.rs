use super::*;
use basic_system::system_implementation::caches::storage_access_policy::StorageAccessPolicy;
use basic_system::system_implementation::flat_storage_model::FlatTreeWithAccountsUnderHashesStorageModel;
use basic_system::system_implementation::system::FullIO;
use core::alloc::Allocator;
use crypto::MiniDigest;
use ruint::aliases::U256;
use system_hooks::addresses_constants::{MESSAGE_ROOT_ADDRESS, SYSTEM_CONTEXT_ADDRESS};
use zk_ee::common_structs::interop_root_storage::InteropRoot;
use zk_ee::memory::stack_trait::StackFactory;
use zk_ee::oracle::IOOracle;
use zk_ee::system::Resources;
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
    tx_rolling_hash: Bytes32,
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
        tx_rolling_hash,
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

///
/// Reads SL chain id from the SystemContext(0x800b) contract.
///
pub fn read_settlement_layer_chain_id<
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
) -> U256 {
    use zk_ee::system::IOSubsystem;
    const SL_CHAIN_ID_STORAGE_SLOT: Bytes32 = Bytes32::ZERO;
    let mut inf_resources = R::FORMAL_INFINITE;
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
}
