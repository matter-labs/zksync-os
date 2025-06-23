use crate::utils::evm_bytecode_into_account_properties;
use crate::{colors, init_logger};
use alloy::signers::local::PrivateKeySigner;
use basic_bootloader::bootloader::config::BasicBootloaderForwardSimulationConfig;
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
use forward_system::run::{
    io_implementer_init_data, BatchOutput, ForwardRunningOracle, ForwardRunningOracleAux,
};
use forward_system::system::bootloader::run_forward;
use log::{debug, info, trace};
use oracle_provider::{BasicZkEEOracleWrapper, ReadWitnessSource, ZkEENonDeterminismSource};
use risc_v_simulator::sim::{DiagnosticsConfig, ProfilerConfig};
use ruint::aliases::{B160, B256, U256};
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use zk_ee::common_structs::derive_flat_storage_key;
use zk_ee::system::metadata::{BlockHashes, BlockMetadataFromOracle};
use zk_ee::types_config::EthereumIOTypesConfig;
use zk_ee::utils::Bytes32;

///
/// In memory chain state, mainly to be used in tests.
///
pub struct Chain<const RANDOMIZED_TREE: bool = false> {
    state_tree: InMemoryTree<RANDOMIZED_TREE>,
    preimage_source: InMemoryPreimageSource,
    chain_id: u64,
    block_number: u64,
    block_hashes: [U256; 256],
}

/// This is a part of the state, which can be controlled by sequencer, other block context values can be determined from the chain state.
pub struct BlockContext {
    pub timestamp: u64,
    pub eip1559_basefee: U256,
    pub gas_per_pubdata: U256,
    pub native_price: U256,
    pub coinbase: B160,
}

impl Default for BlockContext {
    fn default() -> Self {
        Self {
            timestamp: 42,
            eip1559_basefee: U256::from_str_radix("1000", 10).unwrap(),
            gas_per_pubdata: U256::default(),
            native_price: U256::from(10),
            coinbase: B160::default(),
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
            block_number: 0,
            block_hashes: [U256::ZERO; 256],
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
            block_number: 0,
            block_hashes: [U256::ZERO; 256],
        }
    }
}

impl<const RANDOMIZED_TREE: bool> Chain<RANDOMIZED_TREE> {
    pub fn set_last_block_number(&mut self, prev: u64) {
        self.block_number = prev
    }

    pub fn set_block_hashes(&mut self, block_hashes: [U256; 256]) {
        self.block_hashes = block_hashes
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
    ) -> BatchOutput {
        let block_context = block_context.unwrap_or_default();
        let block_metadata = BlockMetadataFromOracle {
            chain_id: self.chain_id,
            block_number: self.block_number + 1,
            block_hashes: BlockHashes(self.block_hashes),
            timestamp: block_context.timestamp,
            eip1559_basefee: block_context.eip1559_basefee,
            gas_per_pubdata: block_context.gas_per_pubdata,
            native_price: block_context.native_price,
            coinbase: block_context.coinbase,
            gas_limit: MAX_BLOCK_GAS_LIMIT,
        };
        let state_commitment = FlatStorageCommitment::<{ TREE_HEIGHT }> {
            root: *self.state_tree.storage_tree.root(),
            next_free_slot: self.state_tree.storage_tree.next_free_slot,
        };
        let tx_source = TxListSource {
            transactions: transactions.into(),
        };

        let oracle = ForwardRunningOracle {
            io_implementer_init_data: Some(io_implementer_init_data(Some(state_commitment))),
            preimage_source: self.preimage_source.clone(),
            tree: self.state_tree.clone(),
            block_metadata,
            next_tx: None,
            tx_source,
        };

        // dump oracle if env variable set
        if let Ok(path) = std::env::var("ORACLE_DUMP_FILE") {
            let aux_oracle: ForwardRunningOracleAux<
                InMemoryTree<RANDOMIZED_TREE>,
                InMemoryPreimageSource,
                TxListSource,
            > = oracle.clone().into();
            let serialized_oracle = bincode::serialize(&aux_oracle).expect("should serialize");
            let mut file = File::create(&path).expect("should create file");
            file.write_all(&serialized_oracle)
                .expect("should write to file");
            info!("Successfully wrote oracle dumo to: {}", path);
        }

        // forward run
        let mut result_keeper = ForwardRunningResultKeeper::new(NoopTxCallback);

        run_forward::<BasicBootloaderForwardSimulationConfig, _, _, _>(
            oracle.clone(),
            &mut result_keeper,
        );

        let block_output: BatchOutput = result_keeper.into();
        trace!(
            "{}Block output:{} \n{:#?}",
            colors::MAGENTA,
            colors::RESET,
            block_output.tx_results
        );

        #[cfg(feature = "report_native")]
        {
            let native_used: u64 = block_output
                .tx_results
                .iter()
                .map(|res| {
                    res.as_ref()
                        .map(|tx_out| tx_out.native_used)
                        .unwrap_or_default()
                })
                .sum::<u64>();
            info!("Native used: {native_used}")
        }

        // proof run
        let oracle_wrapper = BasicZkEEOracleWrapper::<EthereumIOTypesConfig, _>::new(oracle);

        #[cfg(feature = "simulate_witness_gen")]
        let source_for_witness_bench = {
            let mut non_determinism_source = ZkEENonDeterminismSource::default();
            non_determinism_source.add_external_processor(oracle_wrapper.clone());

            non_determinism_source
        };

        let mut non_determinism_source = ZkEENonDeterminismSource::default();
        non_determinism_source.add_external_processor(oracle_wrapper);

        // We'll wrap the source, to collect all the reads.
        let copy_source = ReadWitnessSource::new(non_determinism_source);
        let items = copy_source.get_read_items();

        let diagnostics_config = profiler_config.map(|cfg| {
            let mut diagnostics_cfg = DiagnosticsConfig::new(get_zksync_os_sym_path());
            diagnostics_cfg.profiler_config = Some(cfg);
            diagnostics_cfg
        });

        let now = std::time::Instant::now();
        let (proof_output, block_effective) = zksync_os_runner::run_and_get_effective_cycles(
            get_zksync_os_img_path(),
            diagnostics_config,
            1 << 30,
            copy_source,
        );
        info!(
            "Simulator without witness tracing executed over {:?}",
            now.elapsed()
        );
        info!("Executed {block_effective:?} effective cycles");

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
                write!(file, "{:08X}", num).expect("Failed to write to file");
            }
            debug!(
                "Successfully wrote {} u32 csr reads elements to file: {}",
                items.borrow().len(),
                output_csr
            );
        }

        debug!(
            "{}Proof running output{} = 0x",
            colors::GREEN,
            colors::RESET
        );
        for word in proof_output.into_iter() {
            debug!("{:08x}", word);
        }

        // Ensure that proof running didn't fail: check that output is not zero
        assert!(proof_output.into_iter().any(|word| word != 0));

        #[cfg(feature = "e2e_proving")]
        run_prover(items.borrow().as_slice());
        // TODO: we also need to update state if we want to execute next block on top

        block_output
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
        let mut account_properties = self.get_account_properties(&address);
        if let Some(bytecode) = bytecode {
            account_properties = evm_bytecode_into_account_properties(&bytecode);
            // Save bytecode preimage
            self.preimage_source
                .inner
                .insert(account_properties.bytecode_hash, bytecode);
        }
        if let Some(nominal_token_balance) = balance {
            account_properties.balance = nominal_token_balance;
        }
        if let Some(nonce) = nonce {
            account_properties.nonce = nonce;
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
        let key = Bytes32::from_u256_be(key);
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
        let account_properties = evm_bytecode_into_account_properties(bytecode);
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
            .insert(account_properties.bytecode_hash, bytecode.to_vec());
        self.preimage_source
            .inner
            .insert(properties_hash, encoding.to_vec());

        self
    }

    ///
    /// Generates random ethers local wallet(private key) with chain id.
    ///
    pub fn random_wallet(&self) -> LocalWallet {
        use ethers::signers::Signer;
        let r =
            LocalWallet::new(&mut ethers::core::rand::thread_rng()).with_chain_id(self.chain_id);
        info!("Generated wallet: {:0x?}", r);
        r
    }

    ///
    /// Generates random alloy private key signer with chain id.
    ///
    pub fn random_signer(&self) -> PrivateKeySigner {
        use alloy::signers::Signer;
        let r = PrivateKeySigner::random().with_chain_id(Some(self.chain_id));
        info!("Generated wallet: {:0x?}", r);
        r
    }
}

// bunch of internal utility methods
fn get_zksync_os_img_path() -> PathBuf {
    PathBuf::from(std::env::var("CARGO_WORKSPACE_DIR").unwrap()).join("zksync_os/app.bin")
}

fn get_zksync_os_sym_path() -> PathBuf {
    PathBuf::from(std::env::var("CARGO_WORKSPACE_DIR").unwrap()).join("zksync_os/app.elf")
}

pub fn is_account_properties_address(address: &B160) -> bool {
    address == &ACCOUNT_PROPERTIES_STORAGE_ADDRESS
}

#[cfg(feature = "e2e_proving")]
fn run_prover(csr_reads: &[u32]) {
    use risc_v_simulator::abstractions::non_determinism::QuasiUARTSource;
    use std::alloc::Global;
    use std::io::Read;

    let mut file = File::open(get_zksync_os_img_path()).expect("must open provided file");
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
