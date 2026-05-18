use super::*;
use alloc::vec::Vec;
use basic_system::system_implementation::caches::storage_access_policy::StorageAccessPolicy;
use basic_system::system_implementation::flat_storage_model::FlatTreeWithAccountsUnderHashesStorageModel;
use basic_system::system_implementation::system::FullIO;
use core::alloc::Allocator;
use crypto::MiniDigest;
use ruint::aliases::{B160, U256};
use system_hooks::addresses_constants::{MESSAGE_ROOT_ADDRESS, SYSTEM_CONTEXT_ADDRESS};
use zk_ee::common_structs::interop_root_storage::InteropRoot;
use zk_ee::memory::stack_trait::StackFactory;
use zk_ee::oracle::IOOracle;
use zk_ee::system::{IOSubsystem, NopResultKeeper, Resource, Resources};
use zk_ee::types_config::SystemIOTypesConfig;
use zk_ee::utils::write_bytes::WriteBytes;
use zk_ee::utils::Bytes32;

pub mod da_commitment_generator;
mod post_tx_op_proving_multiblock_batch;
mod post_tx_op_proving_singleblock_batch;
mod post_tx_op_sequencing;
mod pubdata_compression;
pub mod public_input;

/// Version byte for pubdata encoding format.
/// Version 1: Initial versioned pubdata format
/// Version 2: Remove artifacts_len and artifacts from pubdata; storage diffs
///            use a header + initial/repeated split, with repeated writes
///            referenced by compact tree index.
/// Version 3: v2 body bytes are wrapped in a DEFLATE stream. Header
///            (version + block_hash + timestamp) stays uncompressed so
///            consumers can read them without inflating.
pub const PUBDATA_ENCODING_VERSION: u8 = 3;

/// Adapter that lets the storage / logs body emitters drive a single
/// `Vec<u8, A>` sink through the `WriteBytes` trait. Used internally by
/// `write_pubdata` to collect the v3 body before deflate.
struct VecWriteBytes<'a, A: Allocator>(&'a mut Vec<u8, A>);

impl<A: Allocator> WriteBytes for VecWriteBytes<'_, A> {
    fn write(&mut self, buf: &[u8]) {
        self.0.extend_from_slice(buf);
    }
}

/// Helper method to write the pubdata to the DA commitment generator and result keeper.
///
/// v3 layout: `[VERSION:1][BLOCK_HASH:32][TIMESTAMP:8][DEFLATE(BODY)]`
/// where `BODY` is the byte stream the v2 emitter produces (storage diffs +
/// logs + messages). Header stays uncompressed so consumers can read
/// block_hash / timestamp without inflating.
///
/// Storage diffs are emitted in the v2 compressed format and reference
/// per-block tree indices for repeated writes; the caller must therefore
/// have already run `update_commitment` with the actual state commitment so
/// `io.storage.key_to_index_cache` is populated.
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
    repeated_write_index_encoding_length: u8,
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
    let allocator = A::default();

    // Uncompressed header — keeps `pubdata[0]` the version byte and the
    // 32-byte block hash + 8-byte timestamp inspectable without inflating.
    pubdata_dst.write(&[PUBDATA_ENCODING_VERSION]);
    pubdata_dst.write(block_hash.as_u8_ref());
    pubdata_dst.write(&timestamp.to_be_bytes());

    result_keeper.pubdata(&[PUBDATA_ENCODING_VERSION]);
    result_keeper.pubdata(block_hash.as_u8_ref());
    result_keeper.pubdata(&timestamp.to_be_bytes());

    // Accumulate the v2 body bytes (storage diffs + logs + messages) into
    // a buffer. The body emitters do a dual-write to (pubdata_dst,
    // result_keeper); we suppress the result_keeper mirror here by passing
    // a NopResultKeeper, and capture the bytes via the `pubdata_dst` arg.
    // The compressed body is mirrored to both real sinks below.
    let mut body: Vec<u8, A> = Vec::new_in(allocator.clone());
    let mut body_sink = VecWriteBytes(&mut body);
    let mut nop_rk = NopResultKeeper::<()>::default();

    io.storage.apply_storage_diffs_pubdata(
        &mut nop_rk,
        &mut body_sink,
        &mut io.oracle,
        repeated_write_index_encoding_length,
    );
    io.logs_storage.apply_pubdata(&mut body_sink, &mut nop_rk);
    drop(body_sink);

    let compressed = pubdata_compression::deflate_pubdata_body(&body, allocator);
    pubdata_dst.write(&compressed);
    result_keeper.pubdata(&compressed);
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
}
