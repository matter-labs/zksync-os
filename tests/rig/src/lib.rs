#![allow(incomplete_features)]
#![feature(allocator_api)]
//!
//! This crate contains infrastructure to write ZKsync OS integration tests.
//! It contains `Chain` - in memory chain state structure with methods to run blocks, change state
//! and few utility methods(in the `utils` module) to encode transactions, load contracts, etc.
//! `TestingFramework` owns convenience setup behavior (for example, treasury minting),
//! while `Chain` is intended to remain a neutral in-memory state abstraction.
//!
use std::str::FromStr;
use std::sync::Once;
pub mod assertions;
pub mod chain;
pub mod constants;
pub mod evm_bytecode;
pub mod revm_consistency_checker;
pub mod run_config;
pub mod testing_utils;
pub mod utils;

pub use alloy;
use alloy::primitives::address;
use alloy::signers::local::PrivateKeySigner;
pub use alloy_rlp;
pub use alloy_sol_types;
pub use basic_bootloader;
use basic_bootloader::bootloader::errors::BootloaderSubsystemError;
pub use basic_system;
pub use callable_oracles;
pub use chain::BlockContext;
pub use chain::Chain;
#[cfg(feature = "airbender_cli")]
pub use cli_lib;
pub use crypto;
pub use forward_system;
use forward_system::run::convert_alloy::FromAlloy;
use forward_system::system::system_types::ForwardRunningSystem;
#[cfg(feature = "gpu")]
pub use gpu_prover;
pub use log;
pub use oracle_provider;
pub use risc_v_simulator;
pub use risc_v_simulator::sim::ProfilerConfig;
pub use ruint;
pub use system_hooks;
pub use zk_ee;
use zk_ee::common_structs::DACommitmentScheme;
use zk_ee::system::tracer::NopTracer;
use zk_ee::system::tracer::Tracer;
use zk_ee::system::validator::NopTxValidator;
use zk_ee::system::validator::TxValidator;
pub use zksync_os_api;
pub use zksync_os_interface;
use zksync_os_interface::types::BlockOutput;
use zksync_os_revm_runner::revm_runner::RevmRunner;
pub use zksync_os_tests_common;
use zksync_os_tests_common::zksync_tx::encoding::ZKsyncOsEncodable;
use zksync_os_tests_common::zksync_tx::ZKsyncTxEnvelope;

use crate::chain::TestingOracleFactory;
use crate::chain::{BlockExtraStats, RunConfig};
use crate::revm_consistency_checker::{generate_block_context_interface, ChainStateView};

static INIT_LOGGER_ONCE: Once = Once::new();
pub fn init_logger() {
    INIT_LOGGER_ONCE.call_once(env_logger::init);
}

#[allow(dead_code)]
mod colors {
    pub const RESET: &str = "\x1b[0m";

    pub const BLACK: &str = "\x1b[30m";
    pub const RED: &str = "\x1b[31m";
    pub const GREEN: &str = "\x1b[32m";
    pub const YELLOW: &str = "\x1b[33m";
    pub const BLUE: &str = "\x1b[34m";
    pub const MAGENTA: &str = "\x1b[35m";
    pub const CYAN: &str = "\x1b[36m";
    pub const WHITE: &str = "\x1b[37m";

    pub const BRIGHT_BLACK: &str = "\x1b[90m";
    pub const BRIGHT_RED: &str = "\x1b[91m";
    pub const BRIGHT_GREEN: &str = "\x1b[92m";
    pub const BRIGHT_YELLOW: &str = "\x1b[93m";
    pub const BRIGHT_BLUE: &str = "\x1b[94m";
    pub const BRIGHT_MAGENTA: &str = "\x1b[95m";
    pub const BRIGHT_CYAN: &str = "\x1b[96m";
    pub const BRIGHT_WHITE: &str = "\x1b[97m";
}

pub struct LastExecutedBlockInfo {
    pub block_output: BlockOutput,
    pub block_extra_stats: BlockExtraStats,
    pub proof_input: Vec<u32>,
    pub pubdata: Vec<u8>,
}

pub struct TestingFramework<const RANDOMIZED_TREE: bool = false> {
    chain: Chain<RANDOMIZED_TREE>,
    block_context: Option<BlockContext>,
    da_commitment_scheme: Option<DACommitmentScheme>,
    run_config: Option<RunConfig>,
    // Test-setup convenience flag: when false, each block execution pre-funds treasury.
    // This stays framework-local to keep `Chain` execution APIs neutral.
    skip_minting_tokens_to_treasury: bool,
    last_executed_block_info: Option<LastExecutedBlockInfo>,
    oracle_factory: Option<Box<impl TestingOracleFactory<RANDOMIZED_TREE>>>,
}

impl TestingFramework<true> {
    /// Creates a framework instance backed by a randomized in-memory tree.
    pub fn new_with_randomized_tree() -> Self {
        init_logger();

        Self {
            chain: Chain::empty_randomized(None),
            block_context: None,
            da_commitment_scheme: None,
            run_config: Some(Default::default()),
            skip_minting_tokens_to_treasury: false,
            last_executed_block_info: None,
            oracle_factory: None,
        }
    }
}

impl Default for TestingFramework<false> {
    fn default() -> Self {
        Self::new()
    }
}

impl TestingFramework<false> {
    /// Creates a framework instance backed by the default in-memory tree.
    pub fn new() -> Self {
        init_logger();

        Self {
            chain: Chain::empty(None),
            block_context: None,
            da_commitment_scheme: None,
            run_config: Some(Default::default()),
            skip_minting_tokens_to_treasury: false,
            last_executed_block_info: None,
            oracle_factory: None,
        }
    }
}

impl<const RANDOMIZED_TREE: bool> TestingFramework<RANDOMIZED_TREE> {
    fn revm_consistency_check_enabled(&self) -> bool {
        self.run_config
            .as_ref()
            .is_some_and(|config| config.check_revm_consistency)
    }

    fn run_revm_consistency_check(
        &self,
        pre_block_chain: Chain<RANDOMIZED_TREE>,
        transactions: Vec<ZKsyncTxEnvelope>,
        block_context: BlockContext,
        block_output: &BlockOutput,
    ) -> Result<(), String> {
        let block_context_interface =
            generate_block_context_interface(&pre_block_chain, &block_context);
        let mut revm_runner = RevmRunner::new(ChainStateView {
            chain: pre_block_chain,
        });

        revm_runner
            .run(
                transactions,
                block_context_interface,
                Some(block_output.clone()),
            )
            .map_err(|err| format!("{err:#}"))
    }

    #[allow(clippy::result_large_err)]
    fn execute_block_internal(
        &mut self,
        transactions: Vec<ZKsyncTxEnvelope>,
        tracer: &mut impl Tracer<ForwardRunningSystem>,
        validator: &mut impl TxValidator<ForwardRunningSystem>,
    ) -> Result<BlockOutput, BootloaderSubsystemError> {
        let run_config = self.run_config.clone().unwrap_or_default();
        if !self.skip_minting_tokens_to_treasury {
            self.chain.mint_tokens_to_treasury();
        }

        let should_check_revm_consistency = self.revm_consistency_check_enabled();
        let pre_block_chain = should_check_revm_consistency.then(|| self.chain.clone());
        let transactions_for_revm = should_check_revm_consistency.then(|| transactions.clone());
        let block_context_for_revm =
            should_check_revm_consistency.then(|| self.block_context.clone().unwrap_or_default());

        let encoded_txs = transactions
            .into_iter()
            .map(ZKsyncTxEnvelope::encode)
            .collect::<Vec<_>>();

        let (block_output, block_extra_stats, proof_input, pubdata) =
            if let Some(oracle_factory) = &self.oracle_factory {
                self.chain.run_block_with_extra_stats_with_oracle_factory(
                    encoded_txs,
                    self.block_context.clone(),
                    self.da_commitment_scheme,
                    Some(run_config),
                    tracer,
                    validator,
                    oracle_factory.as_ref(),
                )?
            } else {
                self.chain.run_block_with_extra_stats(
                    encoded_txs,
                    self.block_context.clone(),
                    self.da_commitment_scheme,
                    Some(run_config),
                    tracer,
                    validator,
                )?
            };

        self.last_executed_block_info = Some(LastExecutedBlockInfo {
            block_output: block_output.clone(),
            block_extra_stats,
            proof_input,
            pubdata,
        });

        if let (Some(pre_block_chain), Some(transactions), Some(block_context)) = (
            pre_block_chain,
            transactions_for_revm,
            block_context_for_revm,
        ) {
            self.run_revm_consistency_check(
                pre_block_chain,
                transactions,
                block_context,
                &block_output,
            )
            .map_err(|err| -> BootloaderSubsystemError {
                log::error!("REVM consistency check failed: {err:#}");
                zk_ee::internal_error!("REVM consistency check failed").into()
            })?;
        }

        Ok(block_output)
    }

    /// Builder: sets the chain ID used for block metadata and transaction signing.
    pub fn with_chain_id(mut self, chain_id: u64) -> Self {
        self.chain.set_chain_id(chain_id);
        self
    }

    /// Builder: sets the 256 previous block hashes exposed to execution.
    pub fn with_block_hashes(mut self, block_hashes: [ruint::aliases::U256; 256]) -> Self {
        self.chain.set_block_hashes(block_hashes);
        self
    }

    /// Builder: sets the next block number to execute.
    pub fn with_next_block_number(mut self, block_number: u64) -> Self {
        self.chain.set_last_block_number(
            block_number
                .checked_sub(1)
                .expect("block number should be > 0"),
        );
        self
    }

    /// Builder: sets default block context used by subsequent block execution.
    pub fn with_block_context(mut self, block_context: BlockContext) -> Self {
        self.block_context = Some(block_context);
        self
    }

    /// Setter: replaces the default block context for subsequent block execution.
    pub fn set_block_context(&mut self, block_context: Option<BlockContext>) -> &mut Self {
        self.block_context = block_context;
        self
    }

    /// Builder: sets the DA commitment scheme used for block execution.
    pub fn with_da_commitment_scheme(mut self, da_commitment_scheme: DACommitmentScheme) -> Self {
        self.da_commitment_scheme = Some(da_commitment_scheme);
        self
    }

    /// Builder: sets run-level configuration for forward/proving execution.
    pub fn with_run_config(mut self, run_config: RunConfig) -> Self {
        self.run_config = Some(run_config);
        self
    }

    /// Builder: disables framework-level automatic treasury minting before block execution.
    ///
    /// By default, `TestingFramework` pre-funds treasury before each executed block.
    /// This toggle is setup-only and intentionally not part of `RunConfig`.
    pub fn without_minting_tokens_to_treasury(mut self) -> Self {
        self.skip_minting_tokens_to_treasury = true;
        self
    }

    /// Builder: disables REVM consistency checks for this framework instance.
    pub fn without_revm_consistency_check(mut self) -> Self {
        self.run_config
            .get_or_insert_with(Default::default)
            .disable_revm_consistency_check();
        self
    }

    /// Builder: installs a custom oracle factory for forward/proof runs.
    /// Can be used for testing cases with corrupted or malicious oracles
    pub fn with_custom_oracle_factory(
        mut self,
        oracle_factory: impl TestingOracleFactory<RANDOMIZED_TREE> + 'static,
    ) -> Self {
        self.oracle_factory = Some(Box::new(oracle_factory));
        self
    }

    /// Builder: installs selected system contracts into the in-memory chain state.
    pub fn with_system_contracts(
        mut self,
        with_l1_messenger: bool,
        with_l2_base_token: bool,
    ) -> Self {
        crate::testing_utils::install_system_contracts(
            &mut self.chain,
            with_l1_messenger,
            with_l2_base_token,
        );
        self
    }

    /// Builder: sets account balance for the provided address.
    pub fn with_balance(
        mut self,
        address: alloy::primitives::Address,
        balance: ruint::aliases::U256,
    ) -> Self {
        self.set_balance(address, balance);
        self
    }

    /// Builder: funds account with a fixed default testing balance.
    pub fn with_prefunded_account(mut self, address: alloy::primitives::Address) -> Self {
        self.set_balance(
            address,
            ruint::aliases::U256::from(1_000_000_000_000_000_u64),
        );
        self
    }

    /// Builder: deploys EVM bytecode on the given address.
    pub fn with_evm_contract(
        mut self,
        address: alloy::primitives::Address,
        bytecode: &[u8],
    ) -> Self {
        self.set_evm_contract(address, bytecode);
        self
    }

    /// Builder: writes a storage slot value for the given account.
    pub fn with_storage_slot(
        mut self,
        address: alloy::primitives::Address,
        key: ruint::aliases::U256,
        value: ruint::aliases::B256,
    ) -> Self {
        self.set_storage_slot(address, key, value);
        self
    }

    /// Builder: injects a preimage value under the provided hash key.
    pub fn with_preimage(mut self, key: zk_ee::utils::Bytes32, value: &[u8]) -> Self {
        self.set_preimage(key, value);
        self
    }

    /// Builder: mints base tokens to the protocol treasury account.
    pub fn with_minted_tokens_to_treasury(mut self) -> Self {
        self.mint_tokens_to_treasury();
        self
    }

    /// Setter: updates run configuration for subsequent block execution.
    pub fn set_run_config(&mut self, run_config: Option<RunConfig>) -> &mut Self {
        self.run_config = run_config;
        self
    }

    /// Setter: disables framework-level automatic treasury minting for subsequent block executions.
    ///
    /// By default, `TestingFramework` pre-funds treasury before each executed block.
    /// This toggle is setup-only and intentionally not part of `RunConfig`.
    pub fn disable_minting_tokens_to_treasury(&mut self) -> &mut Self {
        self.skip_minting_tokens_to_treasury = true;
        self
    }

    /// Setter: disables REVM consistency checks for subsequent block executions.
    pub fn disable_revm_consistency_check(&mut self) -> &mut Self {
        self.run_config
            .get_or_insert_with(Default::default)
            .disable_revm_consistency_check();
        self
    }

    /// Setter: updates custom oracle factory used for subsequent block execution.
    /// Can be used for testing cases with corrupted or malicious oracles
    pub fn set_custom_oracle_factory(
        &mut self,
        oracle_factory: Option<Box<dyn TestingOracleFactory<RANDOMIZED_TREE>>>,
    ) -> &mut Self {
        self.oracle_factory = oracle_factory;
        self
    }

    /// Setter: sets account balance in chain state.
    pub fn set_balance(
        &mut self,
        address: alloy::primitives::Address,
        balance: ruint::aliases::U256,
    ) -> &mut Self {
        self.chain
            .set_balance(ruint::aliases::B160::from_alloy(address), balance);
        self
    }

    /// Setter: deploys EVM bytecode at the provided address.
    pub fn set_evm_contract(
        &mut self,
        address: alloy::primitives::Address,
        bytecode: &[u8],
    ) -> &mut Self {
        self.chain
            .set_evm_bytecode(ruint::aliases::B160::from_alloy(address), bytecode);
        self
    }

    /// Setter: writes a raw storage slot for the provided account.
    pub fn set_storage_slot(
        &mut self,
        address: alloy::primitives::Address,
        key: ruint::aliases::U256,
        value: ruint::aliases::B256,
    ) -> &mut Self {
        self.chain
            .set_storage_slot(ruint::aliases::B160::from_alloy(address), key, value);
        self
    }

    /// Setter: stores a preimage entry under the provided hash key.
    pub fn set_preimage(&mut self, key: zk_ee::utils::Bytes32, value: &[u8]) -> &mut Self {
        self.chain.set_preimage(key, value);
        self
    }

    /// Returns a random signer configured for the active chain ID.
    pub fn random_signer(&self) -> PrivateKeySigner {
        self.chain.random_signer()
    }

    /// Returns a random signer and prefunds it with the default testing balance.
    pub fn prefunded_random_signer(&mut self) -> PrivateKeySigner {
        let signer = self.random_signer();
        self.set_balance(
            signer.address(),
            ruint::aliases::U256::from(1_000_000_000_000_000_u64),
        );
        signer
    }

    /// Mints base tokens to the protocol treasury account.
    pub fn mint_tokens_to_treasury(&mut self) {
        self.chain.mint_tokens_to_treasury();
    }

    /// Returns decoded account properties for the provided address.
    pub fn get_account_properties(
        &mut self,
        address: &alloy::primitives::Address,
    ) -> basic_system::system_implementation::flat_storage_model::AccountProperties {
        self.chain
            .get_account_properties(&ruint::aliases::B160::from_alloy(address))
    }

    /// Returns native token balance for the provided address.
    pub fn get_balance(&mut self, address: &alloy::primitives::Address) -> ruint::aliases::U256 {
        self.chain
            .get_account_properties(&ruint::aliases::B160::from_alloy(address))
            .balance
    }

    /// Returns raw storage slot value for the provided account and key.
    pub fn get_storage_slot(
        &mut self,
        address: &alloy::primitives::Address,
        key: ruint::aliases::U256,
    ) -> Option<zk_ee::utils::Bytes32> {
        self.chain
            .get_storage_slot(ruint::aliases::B160::from_alloy(address), key)
            .copied()
    }

    /// Returns execution metadata of the most recently executed block, if any.
    pub fn last_executed_block_info(&self) -> Option<&LastExecutedBlockInfo> {
        self.last_executed_block_info.as_ref()
    }

    /// Builds and executes an ERC20 transfer block using default fee settings.
    pub fn run_block_of_erc20(
        &mut self,
        n: usize,
        block_context: Option<BlockContext>,
    ) -> BlockOutput {
        self.run_block_of_erc20_with_fee(n, block_context, 1000)
    }

    /// Builds and executes an ERC20 transfer block with an explicit max fee.
    pub fn run_block_of_erc20_with_fee(
        &mut self,
        n: usize,
        block_context: Option<BlockContext>,
        fee: u128,
    ) -> BlockOutput {
        let transactions = crate::utils::prepare_block_of_erc20_with_fee(&mut self.chain, n, fee);
        let previous_block_context = self.block_context.clone();
        if let Some(block_context) = block_context {
            self.block_context = Some(block_context);
        }

        let output = self.execute_block(transactions);
        self.block_context = previous_block_context;
        self.assert_all_txs_succeeded(&output);
        output
    }

    /// Executes a block using no-op tracer and validator.
    pub fn execute_block(&mut self, transactions: Vec<ZKsyncTxEnvelope>) -> BlockOutput {
        self.execute_block_with_tracing(
            transactions,
            &mut NopTracer::default(),
            &mut NopTxValidator,
        )
    }

    /// Executes a block with custom tracer and validator hooks.
    pub fn execute_block_with_tracing(
        &mut self,
        transactions: Vec<ZKsyncTxEnvelope>,
        tracer: &mut impl Tracer<ForwardRunningSystem>,
        validator: &mut impl TxValidator<ForwardRunningSystem>,
    ) -> BlockOutput {
        self.execute_block_internal(transactions, tracer, validator)
            .unwrap_or_else(|err| panic!("block execution failed: {err:?}"))
    }

    /// Simulate a block in forward mode only.
    ///
    /// This method applies only the explicit/attached block context and intentionally
    /// ignores proving-related configuration such as `run_config`, DA commitment scheme,
    /// and custom oracle factories.
    pub fn simulate_block(&mut self, transactions: Vec<ZKsyncTxEnvelope>) -> BlockOutput {
        let encoded_txs = transactions
            .into_iter()
            .map(ZKsyncTxEnvelope::encode)
            .collect::<Vec<_>>();
        self.chain
            .simulate_block(encoded_txs, self.block_context.clone())
    }

    #[allow(clippy::result_large_err)]
    /// Executes a block and returns a typed error instead of panicking.
    pub fn execute_block_no_panic(
        &mut self,
        transactions: Vec<ZKsyncTxEnvelope>,
    ) -> Result<BlockOutput, BootloaderSubsystemError> {
        let mut tracer = NopTracer::default();
        let mut validator = NopTxValidator;
        self.execute_block_internal(transactions, &mut tracer, &mut validator)
    }

    /// Asserts that every transaction in block output completed successfully.
    pub fn assert_all_txs_succeeded(&self, block_output: &BlockOutput) {
        for (i, result) in block_output.tx_results.iter().enumerate() {
            let success = result.as_ref().is_ok_and(|o| o.is_success());
            assert!(success, "Transaction {i} failed with: {result:?}");
        }
    }
}

pub fn tx_succeeded(output: &BlockOutput, idx: usize) -> bool {
    output.tx_results[idx]
        .as_ref()
        .ok()
        .map(|o| o.is_success())
        .unwrap_or(false)
}

pub fn tx_failed(output: &BlockOutput, idx: usize) -> bool {
    !tx_succeeded(output, idx)
}

pub fn signer_from_key(key: &str) -> PrivateKeySigner {
    PrivateKeySigner::from_str(key).unwrap()
}

pub const PRIMARY_TEST_PK: &str =
    "dcf2cbdd171a21c480aa7f53d77f31bb102282b3ff099c78e3118b37348c72f7";
pub const SECONDARY_TEST_PK: &str =
    "a226d3a5c8c408741c3446c762aee8dff742f21e381a0e5ab85a96c5c00100be";
pub const TERTIARY_TEST_PK: &str =
    "abcdebdd171a21c480aa7f53d77f31bb102282b3ff099c78e3118b37348c72f7";

pub fn testing_signer(index: u64) -> PrivateKeySigner {
    match index {
        0 => signer_from_key(PRIMARY_TEST_PK),
        1 => signer_from_key(SECONDARY_TEST_PK),
        2 => signer_from_key(TERTIARY_TEST_PK),
        _ => panic!("unsupported testing signer index: {index}"),
    }
}

pub fn common_target_address() -> alloy::primitives::Address {
    address!("4242000000000000000000000000000000000000")
}

#[cfg(test)]
mod tests {
    use super::{chain::RunConfig, TestingFramework};
    use forward_system::run::convert_alloy::IntoAlloy;
    use ruint::aliases::U256;
    use system_hooks::addresses_constants::BASE_TOKEN_HOLDER_ADDRESS;

    #[test]
    fn builder_disables_revm_consistency_check() {
        let tester = TestingFramework::new().without_revm_consistency_check();
        let run_config = tester.run_config.expect("run config should be set");
        assert!(!run_config.check_revm_consistency);
    }

    #[test]
    fn setter_disables_revm_consistency_check_even_if_run_config_is_none() {
        let mut tester = TestingFramework::new();
        tester.set_run_config(None);
        tester.disable_revm_consistency_check();

        let run_config = tester.run_config.expect("run config should be set");
        assert!(!run_config.check_revm_consistency);
    }

    #[test]
    fn setter_overrides_enabled_revm_consistency_check() {
        let mut tester = TestingFramework::new().with_run_config({
            let mut run_config = RunConfig::default();
            run_config.enable_revm_consistency_check();
            run_config
        });
        tester.disable_revm_consistency_check();

        let run_config = tester.run_config.expect("run config should be set");
        assert!(!run_config.check_revm_consistency);
    }

    #[test]
    fn testing_framework_mints_treasury_by_default() {
        let treasury = BASE_TOKEN_HOLDER_ADDRESS.into_alloy();
        let mut tester = TestingFramework::new().with_run_config(RunConfig::without_riscv_run());
        assert_eq!(tester.get_balance(&treasury), U256::ZERO);

        let _ = tester.execute_block(vec![]);

        let max_treasury_balance = (U256::ONE << 128) - U256::ONE;
        assert_eq!(tester.get_balance(&treasury), max_treasury_balance);
    }

    #[test]
    fn testing_framework_can_disable_automatic_treasury_minting() {
        let treasury = BASE_TOKEN_HOLDER_ADDRESS.into_alloy();
        let mut tester = TestingFramework::new()
            .without_minting_tokens_to_treasury()
            .with_run_config(RunConfig::without_riscv_run());
        assert_eq!(tester.get_balance(&treasury), U256::ZERO);

        let _ = tester.execute_block(vec![]);

        assert_eq!(tester.get_balance(&treasury), U256::ZERO);
    }
}
