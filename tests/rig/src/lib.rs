#![allow(incomplete_features)]
#![feature(allocator_api)]
//!
//! This crate contains infrastructure to write ZKsync OS integration tests.
//! It contains `Chain` - in memory chain state structure with methods to run blocks, change state
//! and few utility methods(in the `utils` module) to encode transactions, load contracts, etc.
//!
use std::str::FromStr;
use std::sync::Once;
pub mod chain;
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
pub use zksync_os_api;
pub use zksync_os_interface;
use zksync_os_interface::types::BlockOutput;
pub use zksync_os_tests_common;
use zksync_os_tests_common::zksync_tx::encoding::ZKsyncOsEncodable;
use zksync_os_tests_common::zksync_tx::ZKsyncTxEnvelope;
pub use zksync_web3_rs;

use crate::chain::RunConfig;

static INIT_LOGGER_ONCE: Once = Once::new();
pub fn init_logger() {
    INIT_LOGGER_ONCE.call_once(env_logger::init);
}

pub trait IntoEncodedTx {
    fn into_encoded_tx(self) -> zksync_os_interface::traits::EncodedTx;
}

impl IntoEncodedTx for zksync_os_interface::traits::EncodedTx {
    fn into_encoded_tx(self) -> zksync_os_interface::traits::EncodedTx {
        self
    }
}

impl IntoEncodedTx for ZKsyncTxEnvelope {
    fn into_encoded_tx(self) -> zksync_os_interface::traits::EncodedTx {
        self.encode()
    }
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

pub struct ZKsyncOSTester<const RANDOMIZED_TREE: bool = false> {
    chain: Chain<RANDOMIZED_TREE>,
    block_context: Option<BlockContext>,
    da_commitment_scheme: Option<DACommitmentScheme>,
    run_config: Option<RunConfig>,
}

impl ZKsyncOSTester<true> {
    pub fn new_with_randomized_tree() -> Self {
        init_logger();

        Self {
            chain: Chain::empty_randomized(None),
            block_context: None,
            da_commitment_scheme: None,
            run_config: None,
        }
    }
}

impl Default for ZKsyncOSTester<false> {
    fn default() -> Self {
        Self::new()
    }
}

impl ZKsyncOSTester<false> {
    pub fn new() -> Self {
        init_logger();

        Self {
            chain: Chain::empty(None),
            block_context: None,
            da_commitment_scheme: None,
            run_config: None,
        }
    }
}

impl<const RANDOMIZED_TREE: bool> ZKsyncOSTester<RANDOMIZED_TREE> {
    pub fn with_chain_id(mut self, chain_id: u64) -> Self {
        self.chain.set_chain_id(chain_id);
        self
    }

    pub fn with_block_hashes(mut self, block_hashes: [ruint::aliases::U256; 256]) -> Self {
        self.chain.set_block_hashes(block_hashes);
        self
    }

    pub fn with_block_number(mut self, block_number: u64) -> Self {
        self.chain.set_last_block_number(
            block_number
                .checked_sub(1)
                .expect("block number should be > 0"),
        );
        self
    }

    pub fn with_block_context(mut self, block_context: BlockContext) -> Self {
        self.block_context = Some(block_context);
        self
    }

    pub fn set_block_context(&mut self, block_context: Option<BlockContext>) -> &mut Self {
        self.block_context = block_context;
        self
    }

    pub fn with_da_commitment_scheme(mut self, da_commitment_scheme: DACommitmentScheme) -> Self {
        self.da_commitment_scheme = Some(da_commitment_scheme);
        self
    }

    pub fn with_run_config(mut self, run_config: RunConfig) -> Self {
        self.run_config = Some(run_config);
        self
    }

    pub fn with_system_contracts(
        mut self,
        with_l1_messenger: bool,
        with_l2_base_token: bool,
        with_contract_deployer: bool,
    ) -> Self {
        crate::testing_utils::install_system_contracts(
            &mut self.chain,
            with_l1_messenger,
            with_l2_base_token,
            with_contract_deployer,
        );
        self
    }

    pub fn with_balance(
        mut self,
        address: alloy::primitives::Address,
        balance: ruint::aliases::U256,
    ) -> Self {
        self.set_balance(address, balance);
        self
    }

    pub fn with_prefunded_account(mut self, address: alloy::primitives::Address) -> Self {
        self.set_balance(
            address,
            ruint::aliases::U256::from(1_000_000_000_000_000_u64),
        );
        self
    }

    pub fn with_evm_contract(
        mut self,
        address: alloy::primitives::Address,
        bytecode: &[u8],
    ) -> Self {
        self.set_evm_contract(address, bytecode);
        self
    }

    pub fn with_storage_slot(
        mut self,
        address: alloy::primitives::Address,
        key: ruint::aliases::U256,
        value: ruint::aliases::B256,
    ) -> Self {
        self.set_storage_slot(address, key, value);
        self
    }

    pub fn with_preimage(mut self, key: zk_ee::utils::Bytes32, value: &[u8]) -> Self {
        self.set_preimage(key, value);
        self
    }

    pub fn with_minted_tokens_to_treasury(mut self) -> Self {
        self.mint_tokens_to_treasury();
        self
    }

    pub fn set_run_config(&mut self, run_config: Option<RunConfig>) -> &mut Self {
        self.run_config = run_config;
        self
    }

    pub fn set_balance(
        &mut self,
        address: alloy::primitives::Address,
        balance: ruint::aliases::U256,
    ) -> &mut Self {
        self.chain
            .set_balance(ruint::aliases::B160::from_alloy(address), balance);
        self
    }

    pub fn set_evm_contract(
        &mut self,
        address: alloy::primitives::Address,
        bytecode: &[u8],
    ) -> &mut Self {
        self.chain
            .set_evm_bytecode(ruint::aliases::B160::from_alloy(address), bytecode);
        self
    }

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

    pub fn set_preimage(&mut self, key: zk_ee::utils::Bytes32, value: &[u8]) -> &mut Self {
        self.chain.set_preimage(key, value);
        self
    }

    pub fn random_signer(&self) -> PrivateKeySigner {
        self.chain.random_signer()
    }

    pub fn prefunded_random_signer(&mut self) -> PrivateKeySigner {
        let signer = self.random_signer();
        self.set_balance(
            signer.address(),
            ruint::aliases::U256::from(1_000_000_000_000_000_u64),
        );
        signer
    }

    pub fn mint_tokens_to_treasury(&mut self) {
        self.chain.mint_tokens_to_treasury();
    }

    pub fn get_account_properties(
        &mut self,
        address: &alloy::primitives::Address,
    ) -> basic_system::system_implementation::flat_storage_model::AccountProperties {
        self.chain
            .get_account_properties(&ruint::aliases::B160::from_alloy(address))
    }

    pub fn get_balance(&mut self, address: &alloy::primitives::Address) -> ruint::aliases::U256 {
        self.chain
            .get_account_properties(&ruint::aliases::B160::from_alloy(address))
            .balance
    }

    pub fn run_block_of_erc20(
        &mut self,
        n: usize,
        block_context: Option<BlockContext>,
    ) -> BlockOutput {
        crate::utils::run_block_of_erc20(&mut self.chain, n, block_context)
    }

    pub fn run_block_of_erc20_with_fee(
        &mut self,
        n: usize,
        block_context: Option<BlockContext>,
        fee: u128,
    ) -> BlockOutput {
        crate::utils::run_block_of_erc20_with_fee(&mut self.chain, n, block_context, fee)
    }

    pub fn execute_block(&mut self, transactions: Vec<ZKsyncTxEnvelope>) -> BlockOutput {
        let encoded_txs = transactions
            .into_iter()
            .map(IntoEncodedTx::into_encoded_tx)
            .collect::<Vec<_>>();
        self.chain.run_block(
            encoded_txs,
            self.block_context.clone(),
            self.da_commitment_scheme,
            self.run_config.clone(),
        )
    }

    pub fn simulate_block(&mut self, transactions: Vec<ZKsyncTxEnvelope>) -> BlockOutput {
        let encoded_txs = transactions
            .into_iter()
            .map(IntoEncodedTx::into_encoded_tx)
            .collect::<Vec<_>>();
        self.chain
            .simulate_block(encoded_txs, self.block_context.clone())
    }

    pub fn execute_block_no_panic(
        &mut self,
        transactions: Vec<ZKsyncTxEnvelope>,
    ) -> Result<BlockOutput, BootloaderSubsystemError> {
        let encoded_txs = transactions
            .into_iter()
            .map(IntoEncodedTx::into_encoded_tx)
            .collect::<Vec<_>>();
        self.chain.run_block_no_panic(
            encoded_txs,
            self.block_context.clone(),
            self.da_commitment_scheme,
            self.run_config.clone(),
        )
    }

    pub fn assert_all_txs_succeeded(&self, block_output: &BlockOutput) {
        assert!(block_output
            .tx_results
            .iter()
            .cloned()
            .enumerate()
            .all(|(i, r)| {
                let success = r.clone().is_ok_and(|o| o.is_success());
                if !success {
                    println!("Transaction {i} failed with: {r:?}")
                }
                success
            }));
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

pub fn common_target_address() -> alloy::primitives::Address {
    address!("4242000000000000000000000000000000000000")
}
