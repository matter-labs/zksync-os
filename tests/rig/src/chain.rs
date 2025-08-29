use crate::{colors, init_logger};
use alloy::signers::local::PrivateKeySigner;
use basic_bootloader::bootloader::config::BasicBootloaderCallSimulationConfig;
use basic_bootloader::bootloader::config::BasicBootloaderProvingExecutionConfig;
use basic_bootloader::bootloader::constants::MAX_BLOCK_GAS_LIMIT;
use basic_system::system_implementation::flat_storage_model::FlatStorageCommitment;
use basic_system::system_implementation::flat_storage_model::{
    address_into_special_storage_key, AccountProperties, ACCOUNT_PROPERTIES_STORAGE_ADDRESS,
    TREE_HEIGHT,
};
use ethers::signers::LocalWallet;
use forward_system::run::result_keeper::ForwardRunningResultKeeper;
use forward_system::run::test_impl::{
    InMemoryPreimageSource, InMemoryTree, NoopTxCallback, TxListSource,
};
use forward_system::run::{BlockOutput, ForwardRunningOracle};
use forward_system::system::bootloader::run_forward;
use log::{debug, info, trace};
use oracle_provider::{BasicZkEEOracleWrapper, ReadWitnessSource, ZkEENonDeterminismSource};
use risc_v_simulator::sim::{DiagnosticsConfig, ProfilerConfig};
use ruint::aliases::{B160, B256, U256};
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use zk_ee::common_structs::interop_root::InteropRoot;
use zk_ee::common_structs::{derive_flat_storage_key, ProofData};
use zk_ee::system::metadata::{BlockHashes, BlockMetadataFromOracle};
use zk_ee::system::tracer::NopTracer;
use zk_ee::types_config::EthereumIOTypesConfig;
use zk_ee::utils::Bytes32;

///
/// In memory chain state, mainly to be used in tests.
///
pub struct Chain<const RANDOMIZED_TREE: bool = false> {
    state_tree: InMemoryTree<RANDOMIZED_TREE>,
    preimage_source: InMemoryPreimageSource,
    chain_id: u64,
    previous_block_number: Option<u64>,
    block_hashes: [U256; 256],
    block_timestamp: u64,
}

/// This is a part of the state, which can be controlled by sequencer, other block context values can be determined from the chain state.
pub struct BlockContext {
    pub timestamp: u64,
    pub eip1559_basefee: U256,
    pub gas_per_pubdata: U256,
    pub native_price: U256,
    pub coinbase: B160,
    pub gas_limit: u64,
    pub pubdata_limit: u64,
    pub mix_hash: U256,
    pub interop_roots: Vec<InteropRoot>,
}

impl Default for BlockContext {
    fn default() -> Self {
        Self {
            timestamp: 42,
            eip1559_basefee: U256::from_str_radix("1000", 10).unwrap(),
            gas_per_pubdata: U256::default(),
            native_price: U256::from(10),
            coinbase: B160::default(),
            gas_limit: MAX_BLOCK_GAS_LIMIT,
            pubdata_limit: u64::MAX,
            mix_hash: U256::ONE,
            interop_roots: Default::default(),
        }
    }
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
    pub fn run_block_generate_witness(
        oracle: ForwardRunningOracle<
            InMemoryTree<RANDOMIZED_TREE>,
            InMemoryPreimageSource,
            TxListSource,
        >,
        app: &Option<String>,
    ) -> Vec<u32> {
        let oracle_wrapper =
            BasicZkEEOracleWrapper::<EthereumIOTypesConfig, _>::new(oracle.clone());

        let mut non_determinism_source = ZkEENonDeterminismSource::default();

        non_determinism_source.add_external_processor(oracle_wrapper);
        non_determinism_source.add_external_processor(
            callable_oracles::arithmetic::ArithmeticQuery {
                marker: std::marker::PhantomData,
            },
        );

        // We'll wrap the source, to collect all the reads.
        let copy_source = ReadWitnessSource::new(non_determinism_source);
        let items = copy_source.get_read_items();
        // By default - enable diagnostics is false (which makes the test run faster).
        let path = get_zksync_os_img_path(app);
        let output = zksync_os_runner::run(path, None, 1 << 36, copy_source);

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
        transactions: Vec<Vec<u8>>,
        block_context: Option<BlockContext>,
    ) -> BlockOutput {
        let block_context = block_context.unwrap_or_default();
        let interop_roots = block_context.interop_roots;
        let block_metadata = BlockMetadataFromOracle {
            chain_id: self.chain_id,
            block_number: self.next_block_number(),
            block_hashes: BlockHashes(self.block_hashes),
            timestamp: block_context.timestamp,
            eip1559_basefee: block_context.eip1559_basefee,
            gas_per_pubdata: block_context.gas_per_pubdata,
            native_price: block_context.native_price,
            coinbase: block_context.coinbase,
            gas_limit: block_context.gas_limit,
            pubdata_limit: block_context.pubdata_limit,
            mix_hash: block_context.mix_hash,
        };
        let tx_source = TxListSource {
            transactions: transactions.into(),
        };

        let oracle = ForwardRunningOracle {
            proof_data: None,
            preimage_source: self.preimage_source.clone(),
            tree: self.state_tree.clone(),
            block_metadata,
            next_tx: None,
            tx_source: tx_source.clone(),
            interop_roots,
        };

        // dump oracle if env variable set
        if let Ok(path) = std::env::var("ORACLE_DUMP_FILE") {
            let serialized_oracle = bincode::serialize(&oracle).expect("should serialize");
            let mut file = File::create(&path).expect("should create file");
            file.write_all(&serialized_oracle)
                .expect("should write to file");
            info!("Successfully wrote oracle dump to: {path}");
        }

        // forward run
        let mut result_keeper = ForwardRunningResultKeeper::new(NoopTxCallback);
        let mut nop_tracer = NopTracer::default();

        run_forward::<BasicBootloaderCallSimulationConfig, _, _, _>(
            oracle.clone(),
            &mut result_keeper,
            &mut nop_tracer,
        );

        let block_output: BlockOutput = result_keeper.into();
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
    /// You can also pass profiler config, if you want to enable it.
    ///
    pub fn run_block(
        &mut self,
        transactions: Vec<Vec<u8>>,
        block_context: Option<BlockContext>,
        profiler_config: Option<ProfilerConfig>,
    ) -> BlockOutput {
        self.run_block_with_extra_stats(transactions, block_context, profiler_config, None, None)
            .0
    }

    pub fn run_block_with_extra_stats(
        &mut self,
        transactions: Vec<Vec<u8>>,
        block_context: Option<BlockContext>,
        profiler_config: Option<ProfilerConfig>,
        witness_output_file: Option<PathBuf>,
        app: Option<String>,
    ) -> (BlockOutput, BlockExtraStats, Vec<u32>) {
        let block_context = block_context.unwrap_or_default();
        let interop_roots = block_context.interop_roots;
        let block_metadata = BlockMetadataFromOracle {
            chain_id: self.chain_id,
            block_number: self.next_block_number(),
            block_hashes: BlockHashes(self.block_hashes),
            timestamp: block_context.timestamp,
            eip1559_basefee: block_context.eip1559_basefee,
            gas_per_pubdata: block_context.gas_per_pubdata,
            native_price: block_context.native_price,
            coinbase: block_context.coinbase,
            gas_limit: block_context.gas_limit,
            pubdata_limit: block_context.pubdata_limit,
            mix_hash: block_context.mix_hash,
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

        let oracle = ForwardRunningOracle {
            proof_data: Some(proof_data),
            preimage_source: self.preimage_source.clone(),
            tree: self.state_tree.clone(),
            block_metadata,
            next_tx: None,
            tx_source: tx_source.clone(),
            interop_roots,
        };

        // dump oracle if env variable set
        if let Ok(path) = std::env::var("ORACLE_DUMP_FILE") {
            let serialized_oracle = bincode::serialize(&oracle).expect("should serialize");
            let mut file = File::create(&path).expect("should create file");
            file.write_all(&serialized_oracle)
                .expect("should write to file");
            info!("Successfully wrote oracle dumo to: {path}");
        }

        // forward run
        let mut result_keeper = ForwardRunningResultKeeper::new(NoopTxCallback);
        let mut nop_tracer = NopTracer::default();

        // we use proving config here for benchmarking,
        // although sequencer can have extra optimizations
        run_forward::<BasicBootloaderProvingExecutionConfig, _, _, _>(
            oracle.clone(),
            &mut result_keeper,
            &mut nop_tracer,
        );

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
        self.block_hashes[255] = U256::from_be_bytes(block_output.header.hash());

        for storage_write in block_output.storage_writes.iter() {
            self.state_tree
                .cold_storage
                .insert(storage_write.key, storage_write.value);
            self.state_tree
                .storage_tree
                .insert(&storage_write.key, &storage_write.value);
        }

        for (hash, preimage, _preimage_type) in block_output.published_preimages.iter() {
            self.preimage_source.inner.insert(*hash, preimage.clone());
        }

        let proof_input = if let Some(path) = witness_output_file {
            let result = Self::run_block_generate_witness(oracle.clone(), &app);
            let mut file = File::create(&path).expect("should create file");
            let witness: Vec<u8> = result.iter().flat_map(|x| x.to_be_bytes()).collect();
            let hex = hex::encode(witness);
            file.write_all(hex.as_bytes())
                .expect("should write to file");
            result
        } else {
            // proof run
            let oracle_wrapper = BasicZkEEOracleWrapper::<EthereumIOTypesConfig, _>::new(oracle);

            #[cfg(feature = "simulate_witness_gen")]
            let source_for_witness_bench = {
                let mut non_determinism_source = ZkEENonDeterminismSource::default();
                non_determinism_source.add_external_processor(oracle_wrapper.clone());
                non_determinism_source.add_external_processor(
                    callable_oracles::arithmetic::ArithmeticQuery {
                        marker: std::marker::PhantomData,
                    },
                );

                non_determinism_source
            };

            let mut non_determinism_source = ZkEENonDeterminismSource::default();
            non_determinism_source.add_external_processor(oracle_wrapper);
            non_determinism_source.add_external_processor(
                callable_oracles::arithmetic::ArithmeticQuery {
                    marker: std::marker::PhantomData,
                },
            );
            // We'll wrap the source, to collect all the reads.
            let copy_source = ReadWitnessSource::new(non_determinism_source);
            let items = copy_source.get_read_items();

            let diagnostics_config = profiler_config.map(|cfg| {
                let mut diagnostics_cfg = DiagnosticsConfig::new(get_zksync_os_sym_path(&app));
                diagnostics_cfg.profiler_config = Some(cfg);
                diagnostics_cfg
            });

            let now = std::time::Instant::now();
            let (proof_output, block_effective) = zksync_os_runner::run_and_get_effective_cycles(
                get_zksync_os_img_path(&app),
                diagnostics_config,
                1 << 36,
                copy_source,
            );
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
                let mut file = File::create(&output_csr).expect("Failed to create csr reads file");
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

            #[cfg(feature = "e2e_proving")]
            run_prover(items.borrow().as_slice());

            proof_input
        };
        (block_output, stats, proof_input)
    }

    fn get_account_properties(&mut self, address: &B160) -> AccountProperties {
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
    /// Set given account balance to `balance`.
    ///
    /// **Note, that other account fields will be zeroed out(nonce, code).**
    ///
    pub fn set_balance(&mut self, address: B160, balance: U256) -> &mut Self {
        let mut account_properties = AccountProperties::TRIVIAL_VALUE;
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
    /// **Note, that other account fields will be zeroed out(balance, code).**
    ///
    pub fn set_evm_bytecode(&mut self, address: B160, bytecode: &[u8]) -> &mut Self {
        use zksync_os_api::helpers::*;
        let mut account = AccountProperties::default();
        let bytecode_and_artifacts = set_properties_code(&mut account, bytecode);
        let encoding = account.encoding();
        let properties_hash = account.compute_hash();

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
            .insert(account.bytecode_hash, bytecode_and_artifacts);
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
    let app = app_name.as_deref().unwrap_or("app");
    // let app = app_name.as_deref().unwrap_or("app_debug");
    let filename = format!("{app}.{extension}");
    let zksync_os_path = std::env::var("OVERRIDE_ZKSYNC_OS_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(std::env::var("CARGO_WORKSPACE_DIR").unwrap()).join("zksync_os")
        });
    zksync_os_path.join(filename)
}

fn get_zksync_os_img_path(app_name: &Option<String>) -> PathBuf {
    get_zksync_os_path(app_name, "bin")
}

fn get_zksync_os_sym_path(app_name: &Option<String>) -> PathBuf {
    get_zksync_os_path(app_name, "elf")
}

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
    for el in buffer.array_chunks::<4>() {
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
