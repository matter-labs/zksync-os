use crate::{colors, init_logger};
use alloy::consensus::Header;
use alloy::signers::local::PrivateKeySigner;
use alloy_rlp::{Decodable, Encodable};
use basic_bootloader::bootloader::block_flow::ethereum::PectraForkHeader;
use basic_bootloader::bootloader::config::BasicBootloaderCallSimulationConfig;
use basic_bootloader::bootloader::config::BasicBootloaderProvingExecutionConfig;
use basic_bootloader::bootloader::constants::MAX_BLOCK_GAS_LIMIT;
use basic_bootloader::bootloader::errors::BootloaderSubsystemError;
use basic_bootloader::bootloader::transaction_flow::ethereum::EthereumTransactionFlow;
use basic_bootloader::bootloader::BasicBootloader;
use basic_system::system_implementation::ethereum_storage_model::caches::account_properties::EthereumAccountProperties;
use basic_system::system_implementation::ethereum_storage_model::vec_trait::VecCtor;
use basic_system::system_implementation::ethereum_storage_model::EthereumMPT;
use basic_system::system_implementation::flat_storage_model::FlatStorageCommitment;
use basic_system::system_implementation::flat_storage_model::{
    address_into_special_storage_key, AccountProperties, ACCOUNT_PROPERTIES_STORAGE_ADDRESS,
    TREE_HEIGHT,
};
use ethers::signers::LocalWallet;
use forward_system::run::query_processors::DACommitmentSchemeResponder;
use forward_system::run::query_processors::EthereumCLResponder;
use forward_system::run::query_processors::EthereumTargetBlockHeaderResponder;
use forward_system::run::query_processors::GenericPreimageResponder;
use forward_system::run::query_processors::InMemoryEthereumInitialAccountStateResponder;
use forward_system::run::query_processors::InMemoryEthereumInitialStorageSlotValueResponder;
use forward_system::run::query_processors::TxDataResponder;
use forward_system::run::query_processors::UARTPrintResponder;
use forward_system::run::result_keeper::ForwardRunningResultKeeper;
use forward_system::run::test_impl::{InMemoryPreimageSource, InMemoryTree, NoopTxCallback};
use forward_system::system::bootloader::run_forward_no_panic;
use forward_system::system::system_types::ethereum::EthereumStorageSystemTypesWithPostOps;
use forward_system::system::system_types::ForwardRunningSystem;
use log::warn;
use log::{debug, info, trace};
use oracle_provider::MemorySource;
use oracle_provider::{ReadWitnessSource, ZkEENonDeterminismSource};
use risc_v_simulator::abstractions::memory::VectorMemoryImpl;
use risc_v_simulator::sim::{DiagnosticsConfig, ProfilerConfig};
use ruint::aliases::{B160, B256, U256};
use std::alloc::Global;
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use zk_ee::common_structs::da_commitment_scheme::DACommitmentScheme;
use zk_ee::common_structs::{derive_flat_storage_key, ProofData};
use zk_ee::system::metadata::zk_metadata::{BlockHashes, BlockMetadataFromOracle};
use zk_ee::system::tracer::NopTracer;
use zk_ee::system::tracer::Tracer;
use zk_ee::utils::Bytes32;
use zksync_os_interface::traits::EncodedTx;
use zksync_os_interface::traits::TxListSource;
use zksync_os_interface::types::BlockOutput;
use zksync_os_interface::types::StorageWrite;

/// Trait for creating oracles with custom configuration
pub trait TestingOracleFactory<const RANDOMIZED_TREE: bool> {
    #[allow(clippy::too_many_arguments)]
    fn create_oracle<M: MemorySource + 'static>(
        &self,
        block_metadata: BlockMetadataFromOracle,
        state_tree: InMemoryTree<RANDOMIZED_TREE>,
        preimage_source: InMemoryPreimageSource,
        tx_source: TxListSource,
        proof_data: Option<ProofData<FlatStorageCommitment<TREE_HEIGHT>>>,
        da_commitment_scheme: Option<DACommitmentScheme>,
        add_uart: bool,
    ) -> ZkEENonDeterminismSource<M>;
}

/// Default oracle factory that uses the existing make_oracle_for_proofs_and_dumps function
pub struct DefaultOracleFactory<const RANDOMIZED_TREE: bool>;

impl<const RANDOMIZED_TREE: bool> TestingOracleFactory<RANDOMIZED_TREE>
    for DefaultOracleFactory<RANDOMIZED_TREE>
{
    fn create_oracle<M: MemorySource + 'static>(
        &self,
        block_metadata: BlockMetadataFromOracle,
        state_tree: InMemoryTree<RANDOMIZED_TREE>,
        preimage_source: InMemoryPreimageSource,
        tx_source: TxListSource,
        proof_data: Option<ProofData<FlatStorageCommitment<TREE_HEIGHT>>>,
        da_commitment_scheme: Option<DACommitmentScheme>,
        add_uart: bool,
    ) -> ZkEENonDeterminismSource<M> {
        forward_system::run::make_oracle_for_proofs_and_dumps(
            block_metadata,
            state_tree,
            preimage_source,
            tx_source,
            proof_data,
            da_commitment_scheme,
            add_uart,
        )
    }
}

///
/// In memory chain state, mainly to be used in tests.
///
pub struct Chain<const RANDOMIZED_TREE: bool = false> {
    state_tree: InMemoryTree<RANDOMIZED_TREE>,
    pub preimage_source: InMemoryPreimageSource,
    chain_id: u64,
    previous_block_number: Option<u64>,
    block_hashes: [U256; 256],
    block_timestamp: u64,
}

/// This is a part of the state, which can be controlled by sequencer, other block context values can be determined from the chain state.
pub struct BlockContext {
    pub timestamp: u64,
    pub eip1559_basefee: U256,
    pub pubdata_price: U256,
    pub native_price: U256,
    pub coinbase: B160,
    pub gas_limit: u64,
    pub pubdata_limit: u64,
    pub mix_hash: U256,
    pub blob_fee: U256,
}

impl Default for BlockContext {
    fn default() -> Self {
        Self {
            timestamp: 42,
            eip1559_basefee: U256::from_str_radix("1000", 10).unwrap(),
            pubdata_price: U256::default(),
            native_price: U256::from(10),
            coinbase: B160::default(),
            gas_limit: MAX_BLOCK_GAS_LIMIT,
            pubdata_limit: u64::MAX,
            mix_hash: U256::ONE,
            blob_fee: U256::MAX,
        }
    }
}

#[derive(Default)]
pub struct RunConfig {
    // Config for the profiler
    pub profiler_config: Option<ProfilerConfig>,
    // If set, the witness will be dumped to the given file path
    pub witness_output_file: Option<PathBuf>,
    // Name of risc-v binary to use
    pub app: Option<String>,
    // Only run in forward mode, skip proving run
    pub only_forward: bool,
    // Whether to check that storage diff hashes from forward and proof runs match
    // Only to be used when state-diffs-pi feature is enabled in the binary and
    // only_forward is false
    pub check_storage_diff_hashes: bool,
}

impl Chain<false> {
    ///
    /// Create empty state
    ///
    /// chain_id will be set to testing one(37) if `None` passed
    ///
    pub fn empty(chain_id: Option<u64>) -> Self {
        // TODO: should we init it somewhere else?
        init_logger();
        Self {
            state_tree: InMemoryTree::<false>::empty(),
            preimage_source: InMemoryPreimageSource {
                inner: HashMap::new(),
            },
            chain_id: chain_id.unwrap_or(37),
            previous_block_number: None,
            block_hashes: [U256::ZERO; 256],
            block_timestamp: 0,
        }
    }
}

// Duplication to avoid having to annotate the bool const
impl Chain<true> {
    ///
    /// Create empty state
    ///
    /// chain_id will be set to testing one(37) if `None` passed
    ///
    pub fn empty_randomized(chain_id: Option<u64>) -> Self {
        // TODO: should we init it somewhere else?
        init_logger();
        Self {
            state_tree: InMemoryTree::<true>::empty(),
            preimage_source: InMemoryPreimageSource {
                inner: HashMap::new(),
            },
            chain_id: chain_id.unwrap_or(37),
            previous_block_number: None,
            block_hashes: [U256::ZERO; 256],
            block_timestamp: 0,
        }
    }
}

#[derive(Debug)]
pub struct BlockExtraStats {
    pub computational_native_used: Option<u64>,
    pub effective_used: Option<u64>,
}

impl<const RANDOMIZED_TREE: bool> Chain<RANDOMIZED_TREE> {
    pub fn set_last_block_number(&mut self, prev: u64) {
        self.previous_block_number = Some(prev)
    }

    pub fn next_block_number(&self) -> u64 {
        self.previous_block_number.map(|n| n + 1).unwrap_or(0)
    }

    pub fn set_block_hashes(&mut self, block_hashes: [U256; 256]) {
        self.block_hashes = block_hashes
    }

    /// TODO: duplicated from API, unify.
    /// Runs a block in riscV - using zksync_os binary - and returns the
    /// witness that can be passed to the prover subsystem.
    pub fn run_block_generate_witness<const FLAMEGRAPH: bool>(
        oracle: ZkEENonDeterminismSource<VectorMemoryImpl>,
        app: &Option<String>,
    ) -> Vec<u32> {
        // We'll wrap the source, to collect all the reads.
        let copy_source = ReadWitnessSource::new(oracle);
        let items = copy_source.get_read_items();
        // By default - enable diagnostics is false (which makes the test run faster).
        let path = get_zksync_os_img_path(app);

        let diagnostics_config = if FLAMEGRAPH {
            let mut profiler_config = ProfilerConfig::new("flamegraph.svg".into());
            profiler_config.frequency_recip = 10;

            Some(profiler_config).map(|cfg| {
                let mut diagnostics_cfg = DiagnosticsConfig::new(get_zksync_os_sym_path(app));
                diagnostics_cfg.profiler_config = Some(cfg);
                diagnostics_cfg
            })
        } else {
            None
        };

        let output = zksync_os_runner::run(path, diagnostics_config, 1 << 36, copy_source);

        // We return 0s in case of failure.
        assert_ne!(output, [0u32; 8]);

        let result = items.borrow().clone();
        result
    }

    ///
    /// Simulate block, do not validate transactions
    ///
    pub fn simulate_block(
        &mut self,
        transactions: Vec<EncodedTx>,
        block_context: Option<BlockContext>,
    ) -> BlockOutput {
        let block_context = block_context.unwrap_or_default();
        let block_metadata = BlockMetadataFromOracle {
            chain_id: self.chain_id,
            block_number: self.next_block_number(),
            block_hashes: BlockHashes(self.block_hashes),
            timestamp: block_context.timestamp,
            eip1559_basefee: block_context.eip1559_basefee,
            pubdata_price: block_context.pubdata_price,
            native_price: block_context.native_price,
            coinbase: block_context.coinbase,
            gas_limit: block_context.gas_limit,
            pubdata_limit: block_context.pubdata_limit,
            mix_hash: block_context.mix_hash,
            blob_fee: block_context.blob_fee,
        };
        let tx_source = TxListSource {
            transactions: transactions.into(),
        };

        let mut nop_tracer = NopTracer::default();

        let block_output: BlockOutput = forward_system::run::run_block_with_oracle_dump_ext::<
            _,
            _,
            _,
            _,
            BasicBootloaderCallSimulationConfig,
        >(
            block_metadata,
            self.state_tree.clone(),
            self.preimage_source.clone(),
            tx_source.clone(),
            NoopTxCallback,
            None,
            None,
            &mut nop_tracer,
        )
        .unwrap();

        trace!(
            "{}Block output:{} \n{:#?}",
            colors::MAGENTA,
            colors::RESET,
            block_output.tx_results
        );
        block_output
    }

    ///
    /// Run block with given transactions and block context.
    /// If block context is `None` default testing values will be used.
    ///
    /// You can also pass a run config.
    ///
    pub fn run_block(
        &mut self,
        transactions: Vec<EncodedTx>,
        block_context: Option<BlockContext>,
        da_commitment_scheme: Option<DACommitmentScheme>,
        run_config: Option<RunConfig>,
    ) -> BlockOutput {
        self.run_block_with_extra_stats(
            transactions,
            block_context,
            da_commitment_scheme,
            run_config,
            &mut NopTracer::default(),
        )
        .unwrap()
        .0
    }

    ///
    /// Run block with given transactions, block context, and custom oracle factory.
    /// If block context is `None` default testing values will be used.
    ///
    /// You can also pass a run config.
    ///
    pub fn run_block_with_oracle_factory<OF: TestingOracleFactory<RANDOMIZED_TREE>>(
        &mut self,
        transactions: Vec<EncodedTx>,
        block_context: Option<BlockContext>,
        da_commitment_scheme: Option<DACommitmentScheme>,
        run_config: Option<RunConfig>,
        oracle_factory: &OF,
    ) -> BlockOutput {
        self.run_block_with_extra_stats_with_oracle_factory(
            transactions,
            block_context,
            da_commitment_scheme,
            run_config,
            &mut NopTracer::default(),
            oracle_factory,
        )
        .unwrap()
        .0
    }

    #[allow(clippy::result_large_err)]
    pub fn run_block_no_panic(
        &mut self,
        transactions: Vec<EncodedTx>,
        block_context: Option<BlockContext>,
        da_commitment_scheme: Option<DACommitmentScheme>,
        run_config: Option<RunConfig>,
    ) -> Result<BlockOutput, BootloaderSubsystemError> {
        let factory = DefaultOracleFactory::<RANDOMIZED_TREE>;
        self.run_inner(
            transactions,
            block_context,
            da_commitment_scheme,
            run_config.unwrap_or_default(),
            &factory,
            &mut NopTracer::default(),
        )
        .map(|r| r.0)
    }

    #[allow(clippy::result_large_err)]
    pub fn run_block_with_extra_stats(
        &mut self,
        transactions: Vec<EncodedTx>,
        block_context: Option<BlockContext>,
        da_commitment_scheme: Option<DACommitmentScheme>,
        run_config: Option<RunConfig>,
        tracer: &mut impl Tracer<ForwardRunningSystem>,
    ) -> Result<(BlockOutput, BlockExtraStats, Vec<u32>), BootloaderSubsystemError> {
        let factory = DefaultOracleFactory::<RANDOMIZED_TREE>;
        self.run_inner(
            transactions,
            block_context,
            da_commitment_scheme,
            run_config.unwrap_or_default(),
            &factory,
            tracer,
        )
    }

    #[allow(clippy::result_large_err)]
    pub fn run_block_with_extra_stats_with_oracle_factory<
        OF: TestingOracleFactory<RANDOMIZED_TREE>,
    >(
        &mut self,
        transactions: Vec<EncodedTx>,
        block_context: Option<BlockContext>,
        da_commitment_scheme: Option<DACommitmentScheme>,
        run_config: Option<RunConfig>,
        tracer: &mut impl Tracer<ForwardRunningSystem>,
        oracle_factory: &OF,
    ) -> Result<(BlockOutput, BlockExtraStats, Vec<u32>), BootloaderSubsystemError> {
        self.run_inner(
            transactions,
            block_context,
            da_commitment_scheme,
            run_config.unwrap_or_default(),
            oracle_factory,
            tracer,
        )
    }

    #[allow(clippy::result_large_err)]
    fn run_inner<OF: TestingOracleFactory<RANDOMIZED_TREE>>(
        &mut self,
        transactions: Vec<EncodedTx>,
        block_context: Option<BlockContext>,
        da_commitment_scheme: Option<DACommitmentScheme>,
        run_config: RunConfig,
        oracle_factory: &OF,
        tracer: &mut impl Tracer<ForwardRunningSystem>,
    ) -> Result<(BlockOutput, BlockExtraStats, Vec<u32>), BootloaderSubsystemError> {
        let RunConfig {
            profiler_config,
            witness_output_file,
            app,
            only_forward,
            check_storage_diff_hashes,
        } = run_config;
        let block_context = block_context.unwrap_or_default();
        let block_metadata = BlockMetadataFromOracle {
            chain_id: self.chain_id,
            block_number: self.next_block_number(),
            block_hashes: BlockHashes(self.block_hashes),
            timestamp: block_context.timestamp,
            eip1559_basefee: block_context.eip1559_basefee,
            pubdata_price: block_context.pubdata_price,
            native_price: block_context.native_price,
            coinbase: block_context.coinbase,
            gas_limit: block_context.gas_limit,
            pubdata_limit: block_context.pubdata_limit,
            mix_hash: block_context.mix_hash,
            blob_fee: block_context.blob_fee,
        };
        let state_commitment = FlatStorageCommitment::<{ TREE_HEIGHT }> {
            root: *self.state_tree.storage_tree.root(),
            next_free_slot: self.state_tree.storage_tree.next_free_slot,
        };
        let proof_data = ProofData {
            state_root_view: state_commitment,
            last_block_timestamp: self.block_timestamp,
        };
        let tx_source = TxListSource {
            transactions: transactions.into(),
        };

        let da_commitment_scheme =
            da_commitment_scheme.unwrap_or(DACommitmentScheme::BlobsAndPubdataKeccak256);
        let oracle = oracle_factory.create_oracle(
            block_metadata,
            self.state_tree.clone(),
            self.preimage_source.clone(),
            tx_source.clone(),
            Some(proof_data),
            Some(da_commitment_scheme),
            true,
        );

        let forward_oracle = oracle_factory.create_oracle(
            block_metadata,
            self.state_tree.clone(),
            self.preimage_source.clone(),
            tx_source.clone(),
            Some(proof_data),
            Some(da_commitment_scheme),
            true,
        );

        #[cfg(feature = "simulate_witness_gen")]
        let source_for_witness_bench = {
            oracle_factory.create_oracle(
                block_metadata,
                self.state_tree.clone(),
                self.preimage_source.clone(),
                tx_source.clone(),
                Some(proof_data),
                Some(da_commitment_scheme),
                false,
            )
        };

        // forward run
        let mut result_keeper = ForwardRunningResultKeeper::new(NoopTxCallback);

        // we use proving config here for benchmarking,
        // although sequencer can have extra optimizations
        run_forward_no_panic::<BasicBootloaderProvingExecutionConfig>(
            forward_oracle,
            &mut result_keeper,
            tracer,
        )?;

        let block_output: BlockOutput = result_keeper.into();

        trace!(
            "{}Block output:{} \n{:#?}",
            colors::MAGENTA,
            colors::RESET,
            block_output.tx_results
        );
        #[allow(unused_mut)]
        let mut stats = BlockExtraStats {
            computational_native_used: None,
            effective_used: None,
        };

        {
            let native_used: u64 = block_output
                .tx_results
                .iter()
                .map(|res| {
                    res.as_ref()
                        .map(|tx_out| tx_out.computational_native_used)
                        .unwrap_or_default()
                })
                .sum::<u64>();
            stats.computational_native_used = Some(native_used);
        }

        // update state
        self.previous_block_number = Some(self.next_block_number());
        self.block_timestamp = block_context.timestamp;
        for i in 0..255 {
            self.block_hashes[i] = self.block_hashes[i + 1];
        }
        self.block_hashes[255] = U256::from_be_bytes(block_output.header.hash().0);

        for storage_write in block_output.storage_writes.iter() {
            self.state_tree
                .cold_storage
                .insert(storage_write.key.0.into(), storage_write.value.0.into());
            self.state_tree
                .storage_tree
                .insert(&storage_write.key.0.into(), &storage_write.value.0.into());
        }

        for (hash, preimage) in block_output.published_preimages.iter() {
            self.preimage_source
                .inner
                .insert(hash.0.into(), preimage.clone());
        }

        let proof_input = if !only_forward {
            if let Some(path) = witness_output_file {
                let result = Self::run_block_generate_witness::<false>(oracle, &app);
                let mut file = File::create(&path).expect("should create file");
                let witness: Vec<u8> = result.iter().flat_map(|x| x.to_be_bytes()).collect();
                let hex = hex::encode(witness);
                file.write_all(hex.as_bytes())
                    .expect("should write to file");
                result
            } else {
                // We'll wrap the source, to collect all the reads.
                let copy_source = ReadWitnessSource::new(oracle);
                let items = copy_source.get_read_items();

                let diagnostics_config = profiler_config.map(|cfg| {
                    let mut diagnostics_cfg = DiagnosticsConfig::new(get_zksync_os_sym_path(&app));
                    diagnostics_cfg.profiler_config = Some(cfg);
                    diagnostics_cfg
                });

                let now = std::time::Instant::now();
                let (proof_output, block_effective) = {
                    zksync_os_runner::run_and_get_effective_cycles(
                        get_zksync_os_img_path(&app),
                        diagnostics_config,
                        1 << 36,
                        copy_source,
                    )
                };

                info!(
                    "Simulator without witness tracing executed over {:?}",
                    now.elapsed()
                );
                stats.effective_used = block_effective;

                #[cfg(feature = "simulate_witness_gen")]
                {
                    zksync_os_runner::simulate_witness_tracing(
                        get_zksync_os_img_path(),
                        source_for_witness_bench,
                    )
                }

                // dump csr reads if env var set
                if let Ok(output_csr) = std::env::var("CSR_READS_DUMP") {
                    // Save the read elements into a file - that can be later read with the tools/cli from zksync-airbender.
                    let mut file =
                        File::create(&output_csr).expect("Failed to create csr reads file");
                    // Write each u32 as an 8-character hexadecimal string without newlines
                    for num in items.borrow().iter() {
                        write!(file, "{num:08X}").expect("Failed to write to file");
                    }
                    debug!(
                        "Successfully wrote {} u32 csr reads elements to file: {}",
                        items.borrow().len(),
                        output_csr
                    );
                }

                let proof_input = items.borrow().iter().copied().collect::<Vec<u32>>();

                debug!(
                    "{}Proof running output{} = 0x",
                    colors::GREEN,
                    colors::RESET
                );
                for word in proof_output.into_iter() {
                    debug!("{word:08x}");
                }

                // Ensure that proof running didn't fail: check that output is not zero
                assert!(proof_output.into_iter().any(|word| word != 0));
                let proof_output_u8: [u8; 32] = unsafe { core::mem::transmute(proof_output) };

                if check_storage_diff_hashes {
                    // Also ensure that storage diff hash matches
                    use crypto::MiniDigest;
                    let mut hasher = crypto::blake2s::Blake2s256::new();
                    for StorageWrite { key, value, .. } in block_output.storage_writes.iter() {
                        hasher.update(key.0.as_ref());
                        hasher.update(value.0.as_ref());
                    }
                    let forward_storage_diff_hash = hasher.finalize();
                    info!(
                        "Forward storage diff hash: 0x{}",
                        hex::encode(forward_storage_diff_hash.as_ref())
                    );
                    assert_eq!(proof_output_u8, forward_storage_diff_hash);

                    #[cfg(feature = "e2e_proving")]
                    run_prover(items.borrow().as_slice());
                }

                proof_input
            }
        } else {
            vec![]
        };
        Ok((block_output, stats, proof_input))
    }

    pub fn make_eth_block_oracle<M: MemorySource + 'static>(
        transactions: Vec<EncodedTx>,
        witness: alloy_rpc_types_debug::ExecutionWitness,
        block_header: Header,
        withdrawals: Vec<u8>,
    ) -> ZkEENonDeterminismSource<M> {
        use crypto::MiniDigest;
        use std::collections::BTreeMap;

        let mut headers: Vec<Header> = witness
            .headers
            .iter()
            .map(|el| {
                let mut slice: &[u8] = &el.0;
                Header::decode(&mut slice).unwrap()
            })
            .collect();

        assert!(!headers.is_empty());
        assert!(headers.is_sorted_by(|a, b| a.number < b.number));
        headers.reverse();
        assert_eq!(headers.len(), witness.headers.len());

        let block_number = headers[0].number + 1;
        assert_eq!(block_number, block_header.number);

        let mut headers_encodings: Vec<_> =
            witness.headers.iter().map(|el| el.0.to_vec()).collect();
        headers_encodings.reverse();

        let initial_root = headers[0].state_root;

        let mut preimage_source = InMemoryPreimageSource::default();
        let mut oracle: BTreeMap<Bytes32, Vec<u8>> = BTreeMap::new();

        let mut hasher = crypto::sha3::Keccak256::new();

        // make an oracle
        for el in witness.state.iter() {
            hasher.update(el);
            let hash = hasher.finalize_reset();
            oracle.insert(Bytes32::from_array(hash), el.to_vec());
            preimage_source
                .inner
                .insert(Bytes32::from_array(hash), el.to_vec());
        }

        for el in witness.codes.iter() {
            hasher.update(el);
            let hash = hasher.finalize_reset();
            oracle.insert(Bytes32::from_array(hash), el.to_vec());
            preimage_source
                .inner
                .insert(Bytes32::from_array(hash), el.to_vec());
        }

        // we will do some really bad heuristics here
        use basic_system::system_implementation::ethereum_storage_model::digits_from_key;
        use basic_system::system_implementation::ethereum_storage_model::BoxInterner;
        use basic_system::system_implementation::ethereum_storage_model::Path;

        let mut interner = BoxInterner::with_capacity_in(1 << 26, Global);
        let mut accounts_mpt: EthereumMPT<'_, Global, VecCtor, false> =
            EthereumMPT::new_in(initial_root.0, &mut interner, Global).unwrap();
        let mut account_properties = HashMap::<B160, EthereumAccountProperties>::new();
        for el in witness.keys.iter() {
            if el.len() == 20 {
                hasher.update(el);
                let hash = hasher.finalize_reset();
                let digits = digits_from_key(&hash);
                let path = Path::new(&digits);
                if let Ok(props) = accounts_mpt.get(path, &mut oracle, &mut interner, &mut hasher) {
                    let props = EthereumAccountProperties::parse_from_rlp_bytes(props)
                        .expect("must parse account data");
                    let key = B160::from_be_bytes::<20>(el[..].try_into().unwrap());
                    account_properties.insert(key, props);
                } else {
                    warn!("Account 0x{} is in preimages list, but there is no MTP witness to get it's properties", hex::encode(el));
                }
            }
        }

        info!("Will try to run {} transactions", transactions.len());

        let tx_source = TxListSource {
            transactions: transactions.into(),
        };

        let mut target_header_encoding = vec![];
        block_header.encode(&mut target_header_encoding);

        let target_header_responder = EthereumTargetBlockHeaderResponder {
            target_header: block_header,
            target_header_encoding,
        };
        let tx_data_responder = TxDataResponder {
            tx_source,
            next_tx: None,
            next_tx_format: None,
            next_tx_from: None,
        };
        let da_commitment_scheme_responder = DACommitmentSchemeResponder {
            da_commitment_scheme: Some(DACommitmentScheme::None),
        };
        let preimage_responder = GenericPreimageResponder { preimage_source };
        let initial_account_state_responder = InMemoryEthereumInitialAccountStateResponder::new(
            initial_root.0,
            account_properties.clone(),
            oracle.clone(),
        );
        let initial_values_responder =
            InMemoryEthereumInitialStorageSlotValueResponder::new(account_properties, oracle);

        let cl_responder = EthereumCLResponder {
            withdrawals_list: withdrawals,
            parent_headers_list: headers,
            parent_headers_encodings_list: headers_encodings,
        };

        let mut oracle = ZkEENonDeterminismSource::default();
        oracle.add_external_processor(target_header_responder.clone());
        oracle.add_external_processor(tx_data_responder.clone());
        oracle.add_external_processor(preimage_responder.clone());
        oracle.add_external_processor(initial_account_state_responder.clone());
        oracle.add_external_processor(initial_values_responder.clone());
        oracle.add_external_processor(cl_responder.clone());
        oracle.add_external_processor(da_commitment_scheme_responder);
        oracle.add_external_processor(
            callable_oracles::blob_kzg_commitment::BlobCommitmentAndProofQuery::default(),
        );
        oracle.add_external_processor(callable_oracles::arithmetic::ArithmeticQuery::default());
        oracle.add_external_processor(UARTPrintResponder);

        oracle
    }

    pub fn run_eth_block(
        &mut self,
        transactions: Vec<EncodedTx>,
        witness: alloy_rpc_types_debug::ExecutionWitness,
        block_header: Header,
        withdrawals: Vec<u8>,
    ) -> ForwardRunningResultKeeper<NoopTxCallback, PectraForkHeader> {
        let (result_keeper, _witness) = self.run_eth_block_with_options(
            transactions,
            witness,
            block_header,
            withdrawals,
            Some("eth_stf".to_string()),
            false,
        );
        result_keeper.unwrap()
    }

    #[allow(clippy::too_many_arguments, unused_variables)]
    pub fn run_eth_block_with_options(
        &mut self,
        transactions: Vec<EncodedTx>,
        witness: alloy_rpc_types_debug::ExecutionWitness,
        block_header: Header,
        withdrawals: Vec<u8>,
        app: Option<String>,
        only_forward: bool,
    ) -> (
        Option<ForwardRunningResultKeeper<NoopTxCallback, PectraForkHeader>>,
        Option<Vec<u32>>,
    ) {
        use basic_bootloader::bootloader::config::BasicBootloaderForwardETHLikeConfig;
        use forward_system::run::result_keeper::ForwardRunningResultKeeper;

        let oracle = Self::make_eth_block_oracle(
            transactions.clone(),
            witness.clone(),
            block_header.clone(),
            withdrawals.clone(),
        );

        // Forward run:
        let mut result_keeper = ForwardRunningResultKeeper::new(NoopTxCallback);
        let mut nop_tracer = NopTracer::default();

        BasicBootloader::<
            EthereumStorageSystemTypesWithPostOps<_>,
            EthereumTransactionFlow<EthereumStorageSystemTypesWithPostOps<_>>,
        >::run_prepared::<BasicBootloaderForwardETHLikeConfig>(
            oracle,
            &mut (),
            &mut result_keeper,
            &mut nop_tracer,
        )
        .expect("must succeed");
        let oracle = Self::make_eth_block_oracle(transactions, witness, block_header, withdrawals);

        let copy_source = ReadWitnessSource::new(oracle);
        let items = copy_source.get_read_items();

        let proof_input = if only_forward {
            None
        } else {
            let (proof_output, block_effective) = {
                zksync_os_runner::run_and_get_effective_cycles(
                    get_zksync_os_img_path(&app),
                    None,
                    1 << 36,
                    copy_source,
                )
            };
            Some(items.borrow().iter().copied().collect::<Vec<u32>>())
        };
        (Some(result_keeper), proof_input)
    }

    pub fn get_account_properties(&mut self, address: &B160) -> AccountProperties {
        use forward_system::run::PreimageSource;
        let key = address_into_special_storage_key(address);
        let flat_key = derive_flat_storage_key(&ACCOUNT_PROPERTIES_STORAGE_ADDRESS, &key);
        match self.state_tree.cold_storage.get(&flat_key) {
            None => AccountProperties::default(),
            Some(account_hash) => {
                if account_hash.is_zero() {
                    // Empty (default) account
                    AccountProperties::default()
                } else {
                    // Get from preimage:
                    let encoded = self
                        .preimage_source
                        .get_preimage(*account_hash)
                        .unwrap_or_default();
                    AccountProperties::decode(&encoded.try_into().unwrap())
                }
            }
        }
    }

    ///
    /// Set all properties at once.
    ///
    pub fn set_account_properties(
        &mut self,
        address: B160,
        balance: Option<U256>,
        nonce: Option<u64>,
        bytecode: Option<Vec<u8>>,
    ) {
        use zksync_os_api::helpers::*;
        let mut account_properties = self.get_account_properties(&address);
        if let Some(bytecode) = bytecode {
            let bytecode_and_artifacts = set_properties_code(&mut account_properties, &bytecode);
            // Save bytecode preimage
            self.preimage_source
                .inner
                .insert(account_properties.bytecode_hash, bytecode_and_artifacts);
        }
        if let Some(nominal_token_balance) = balance {
            set_properties_balance(&mut account_properties, nominal_token_balance);
        }
        if let Some(nonce) = nonce {
            set_properties_nonce(&mut account_properties, nonce);
        }

        let encoding = account_properties.encoding();
        let properties_hash = account_properties.compute_hash();

        let key = address_into_special_storage_key(&address);
        let flat_key = derive_flat_storage_key(&ACCOUNT_PROPERTIES_STORAGE_ADDRESS, &key);

        // Save preimage
        self.preimage_source
            .inner
            .insert(properties_hash, encoding.to_vec());
        self.state_tree
            .cold_storage
            .insert(flat_key, properties_hash);
        self.state_tree
            .storage_tree
            .insert(&flat_key, &properties_hash);
    }

    ///
    /// Set a storage slot
    ///
    pub fn set_storage_slot(&mut self, address: B160, key: U256, value: B256) {
        let key = Bytes32::from_u256_be(&key);
        let flat_key = derive_flat_storage_key(&address, &key);

        let value = Bytes32::from_array(value.to_be_bytes());

        self.state_tree.cold_storage.insert(flat_key, value);
        self.state_tree.storage_tree.insert(&flat_key, &value);
    }

    ///
    /// Get value at a storage slot
    ///
    pub fn get_storage_slot(&mut self, address: B160, key: U256) -> Option<&Bytes32> {
        let key = Bytes32::from_u256_be(&key);
        let flat_key = derive_flat_storage_key(&address, &key);

        self.state_tree.cold_storage.get(&flat_key)
    }

    ///
    /// Set given account balance to `balance`.
    ///
    pub fn set_balance(&mut self, address: B160, balance: U256) -> &mut Self {
        let mut account_properties = self.get_account_properties(&address);

        account_properties.balance = balance;
        let encoding = account_properties.encoding();
        let properties_hash = account_properties.compute_hash();

        let key = address_into_special_storage_key(&address);
        let flat_key = derive_flat_storage_key(&ACCOUNT_PROPERTIES_STORAGE_ADDRESS, &key);

        // We are updating both cold storage (hash map) and our storage tree.
        self.state_tree
            .cold_storage
            .insert(flat_key, properties_hash);
        self.state_tree
            .storage_tree
            .insert(&flat_key, &properties_hash);
        self.preimage_source
            .inner
            .insert(properties_hash, encoding.to_vec());
        self
    }

    ///
    /// Set given EVM bytecode on the given address.
    ///
    pub fn set_evm_bytecode(&mut self, address: B160, bytecode: &[u8]) -> &mut Self {
        use zksync_os_api::helpers::*;
        let mut account_properties = self.get_account_properties(&address);

        let bytecode_and_artifacts = set_properties_code(&mut account_properties, bytecode);
        let encoding = account_properties.encoding();
        let properties_hash = account_properties.compute_hash();

        let key = address_into_special_storage_key(&address);
        let flat_key = derive_flat_storage_key(&ACCOUNT_PROPERTIES_STORAGE_ADDRESS, &key);

        // We are updating both cold storage (hash map) and our storage tree.
        self.state_tree
            .cold_storage
            .insert(flat_key, properties_hash);
        self.state_tree
            .storage_tree
            .insert(&flat_key, &properties_hash);
        self.preimage_source
            .inner
            .insert(account_properties.bytecode_hash, bytecode_and_artifacts);
        self.preimage_source
            .inner
            .insert(properties_hash, encoding.to_vec());

        self
    }

    /// Set a preimage, used to test forced deployments
    pub fn set_preimage(&mut self, hash: Bytes32, preimage: &[u8]) -> &mut Self {
        self.preimage_source.inner.insert(hash, preimage.to_vec());
        self
    }

    ///
    /// Generates random ethers local wallet(private key) with chain id.
    ///
    pub fn random_wallet(&self) -> LocalWallet {
        use ethers::signers::Signer;
        let r =
            LocalWallet::new(&mut ethers::core::rand::thread_rng()).with_chain_id(self.chain_id);
        info!("Generated wallet: {r:0x?}");
        r
    }

    ///
    /// Generates random alloy private key signer with chain id.
    ///
    pub fn random_signer(&self) -> PrivateKeySigner {
        use alloy::signers::Signer;
        let r = PrivateKeySigner::random().with_chain_id(Some(self.chain_id));
        info!("Generated wallet: {r:0x?}");
        r
    }
}

// bunch of internal utility methods
fn get_zksync_os_path(app_name: &Option<String>, extension: &str) -> PathBuf {
    let app = app_name.as_deref().unwrap_or("for_tests");
    // let app = app_name.as_deref().unwrap_or("app_debug");
    let filename = format!("{app}.{extension}");
    let zksync_os_path = std::env::var("OVERRIDE_ZKSYNC_OS_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(std::env::var("CARGO_WORKSPACE_DIR").unwrap()).join("zksync_os")
        });
    zksync_os_path.join(filename)
}

pub fn get_zksync_os_img_path(app_name: &Option<String>) -> PathBuf {
    get_zksync_os_path(app_name, "bin")
}

fn get_zksync_os_sym_path(app_name: &Option<String>) -> PathBuf {
    get_zksync_os_path(app_name, "elf")
}

// TODO: utils?
pub fn is_account_properties_address(address: &B160) -> bool {
    address == &ACCOUNT_PROPERTIES_STORAGE_ADDRESS
}

#[cfg(feature = "e2e_proving")]
fn run_prover(csr_reads: &[u32]) {
    use risc_v_simulator::abstractions::non_determinism::QuasiUARTSource;
    use std::alloc::Global;
    use std::io::Read;

    let mut file = File::open(get_zksync_os_img_path(&None)).expect("must open provided file");
    let mut buffer = vec![];
    file.read_to_end(&mut buffer).expect("must read the file");
    let mut binary = vec![];
    for el in buffer.as_chunks::<4>().0.iter() {
        binary.push(u32::from_le_bytes(*el));
    }

    use prover_examples::prover::worker::Worker;
    use prover_examples::setups;

    setups::pad_bytecode_for_proving(&mut binary);

    let worker = Worker::new_with_num_threads(8);

    let main_circuit_precomputations =
        setups::get_main_riscv_circuit_setup::<Global, Global>(&binary, &worker);

    let delegation_precomputations =
        setups::all_delegation_circuits_precomputations::<Global, Global>(&worker);

    // TODO: fix
    let mut non_determinism_source = QuasiUARTSource::default();
    for word in csr_reads {
        non_determinism_source.oracle.push_back(*word);
    }

    let _ = prover_examples::prove_image_execution(
        32,
        &binary,
        non_determinism_source,
        &main_circuit_precomputations,
        &delegation_precomputations,
        &worker,
    );

    info!("block proved successfully");
}
