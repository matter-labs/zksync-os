use alloy::primitives::{Bytes, Log, U256};
use alloy::rpc::types::trace::geth::CallFrame;
use anyhow::{anyhow, bail, Context as AnyhowContext};
use forward_system::run::convert_alloy::IntoAlloy;
use forward_system::run::output::BlockOutput;
use revm::{
    context::{ContextTr, TxEnv},
    context_interface::block::BlobExcessGasAndPrice,
    context_interface::result::HaltReason,
    database::{CacheDB, EmptyDB},
    inspector::InspectCommitEvm,
    DatabaseRef,
};
use revm_inspectors::tracing::{TracingInspector, TracingInspectorConfig};
use zksync_os_interface::traits::AnyBlockContext;
use zksync_os_interface::types::BlockHashes;
use zksync_os_revm::ZKsyncTx;
use zksync_os_revm::{DefaultZk, ZKsyncTxError, ZkBuilder, ZkContext, ZkSpecId};
use zksync_os_tests_common::zksync_tx::ZKsyncTxEnvelope;

use crate::helpers::{
    calculate_excess_blob_gas_from_blob_base_fee, zk_tx_into_revm_tx, BLOB_BASE_FEE_UPDATE_FRACTION,
};
use crate::revm_state_provider::{RevmStateProvider, ViewState};
use crate::storage_diff_comp::CompareReport;

/// Per-transaction execution comparison between REVM and ZKsync OS.
#[derive(Debug)]
pub struct TxComparisonMismatch {
    pub tx_index: usize,
    pub kind: TxMismatchKind,
}

#[derive(Debug)]
pub enum TxMismatchKind {
    /// Success/revert status differs.
    OutcomeMismatch {
        revm_success: bool,
        zk_success: bool,
    },
    /// Return data differs.
    ReturnDataMismatch { revm: Option<Bytes>, zk: Vec<u8> },
    /// Event logs differ.
    LogsMismatch {
        revm_count: usize,
        zk_count: usize,
        first_diff_index: Option<usize>,
    },
}

struct ReplayTx {
    tx: ZKsyncTx<TxEnv>,
    original_tx_index: usize,
}

pub struct RevmRunner<State>
where
    State: ViewState,
{
    state: State,
    spec: ZkSpecId,
    /// When true, REVM computes gas independently instead of using
    /// ZKsync OS's `gas_used` as an override. Best combined with
    /// `unlimited_native` so that gas models are equivalent.
    independent_gas: bool,
}

impl<State> RevmRunner<State>
where
    State: ViewState,
{
    pub fn new(state: State) -> Self {
        Self {
            state,
            spec: ZkSpecId::AtlasV3,
            independent_gas: false,
        }
    }

    pub fn with_spec(mut self, spec: ZkSpecId) -> Self {
        self.spec = spec;
        self
    }

    pub fn set_spec(&mut self, spec: ZkSpecId) -> &mut Self {
        self.spec = spec;
        self
    }

    /// Enable independent gas computation: REVM computes gas itself
    /// instead of using ZKsync OS's `gas_used` override.
    pub fn with_independent_gas(mut self, independent: bool) -> Self {
        self.independent_gas = independent;
        self
    }

    pub fn run<BlockContext: AnyBlockContext>(
        &mut self,
        transactions: Vec<ZKsyncTxEnvelope>,
        block_context: BlockContext,
        block_output: Option<BlockOutput>,
    ) -> anyhow::Result<()> {
        let (_traces, _errors, compare_report, tx_mismatches) =
            self.run_with_call_traces(transactions, block_context, block_output)?;

        if let Some(report) = compare_report {
            if !report.is_empty() {
                log::warn!("State mismatch found after REVM replay");
                report.log_tracing(100);
                bail!(
                    "REVM consistency mismatch: storage={} account={}",
                    report.storage.len(),
                    report.accounts.len()
                );
            }
        }

        if !tx_mismatches.is_empty() {
            bail!(
                "REVM consistency mismatch: {} transaction-level divergence(s) (return data, logs, or outcome)",
                tx_mismatches.len()
            );
        }

        Ok(())
    }

    #[allow(clippy::type_complexity)]
    pub fn run_with_call_traces<BlockContext: AnyBlockContext>(
        &mut self,
        transactions: Vec<ZKsyncTxEnvelope>,
        block_context: BlockContext,
        block_output: Option<BlockOutput>,
    ) -> anyhow::Result<(
        Vec<CallFrame>,
        Vec<(usize, ZKsyncTxError)>,
        Option<CompareReport>,
        Vec<TxComparisonMismatch>,
    )> {
        let blob_fee: u64 = block_context
            .blob_fee()
            .try_into()
            .context("Blob fee should fit into u64")?;
        let block_basefee: u64 = block_context
            .eip1559_basefee()
            .try_into()
            .context("Block base fee should fit into u64")?;

        let state_provider = RevmStateProvider::new(
            self.state.clone(),
            BlockHashes(block_context.block_hashes().clone()),
            block_context.block_number().saturating_sub(1),
        );
        let settlement_layer_chain_id = Self::read_settlement_layer_chain_id(self.state.clone())?;

        let blob_excess_gas_and_price = BlobExcessGasAndPrice::new(
            calculate_excess_blob_gas_from_blob_base_fee(blob_fee, BLOB_BASE_FEE_UPDATE_FRACTION),
            BLOB_BASE_FEE_UPDATE_FRACTION
                .try_into()
                .expect("Blob base fee update fraction should fit into u64"),
        );

        let cache_db = CacheDB::new(state_provider);
        let mut evm = ZkContext::<EmptyDB>::default()
            .with_db(cache_db)
            .modify_cfg_chained(|cfg| {
                cfg.chain_id = block_context.chain_id();
                cfg.spec = self.spec;
            })
            .modify_block_chained(|block| {
                block.number = U256::from(block_context.block_number());
                block.timestamp = U256::from(block_context.timestamp());
                block.beneficiary = block_context.coinbase();
                block.basefee = block_basefee;
                block.gas_limit = block_context.gas_limit();
                block.prevrandao = Some(block_context.mix_hash().into());
                block.blob_excess_gas_and_price = Some(blob_excess_gas_and_price);
            })
            .build_zk_with_inspector(TracingInspector::new(TracingInspectorConfig::default_geth()));

        let revm_txs = Self::build_revm_txs(
            &transactions,
            block_output.as_ref(),
            block_context.gas_limit(),
            settlement_layer_chain_id,
            self.independent_gas,
        )?;

        let mut call_traces = Vec::with_capacity(transactions.len());
        let mut invalid_transactions = vec![];
        // Collect per-tx REVM execution results keyed by original tx index.
        let mut revm_results: Vec<(
            usize,
            revm::context_interface::result::ExecutionResult<HaltReason>,
        )> = vec![];
        for replay_tx in revm_txs {
            let tx_execution = match evm.inspect_tx_commit(replay_tx.tx) {
                Ok(res) => res,
                Err(err) => match err {
                    revm::context_interface::result::EVMError::Transaction(e) => {
                        invalid_transactions.push((replay_tx.original_tx_index, e.clone()));
                        continue;
                    }
                    revm::context_interface::result::EVMError::Header(e) => {
                        return Err(anyhow!("Header error: {:?}", e));
                    }
                    revm::context_interface::result::EVMError::Database(e) => {
                        return Err(anyhow!("Database error: {:?}", e));
                    }
                    revm::context_interface::result::EVMError::Custom(e) => {
                        return Err(anyhow!("Other error: {}", e));
                    }
                    revm::context_interface::result::EVMError::CustomAny(e) => {
                        return Err(anyhow!("Other error: {}", e));
                    }
                },
            };
            let trace = evm
                .0
                .inspector
                .geth_builder()
                .geth_call_traces(Default::default(), tx_execution.tx_gas_used());
            call_traces.push(trace);
            revm_results.push((replay_tx.original_tx_index, tx_execution));
            evm.0.inspector.fuse();
        }

        if block_output.is_some() && !invalid_transactions.is_empty() {
            for (idx, err) in invalid_transactions.iter().take(10) {
                log::info!(
                    "REVM rejected tx #{idx} (included in ZKsync OS block as failed): {err:?}"
                );
            }
        }

        let compare_report = if let Some(block_output) = block_output.as_ref() {
            Some(Self::build_compare_report(evm.0.db_mut(), block_output)?)
        } else {
            None
        };

        // Compare per-tx return data and events only when gas overrides
        // are disabled — with force_fail, REVM skips execution so return
        // data is not meaningful.
        let tx_mismatches = if self.independent_gas {
            if let Some(block_output) = block_output.as_ref() {
                Self::compare_tx_results(&revm_results, block_output)
            } else {
                vec![]
            }
        } else {
            vec![]
        };

        if !tx_mismatches.is_empty() {
            for m in &tx_mismatches {
                log::warn!("tx #{} mismatch: {:?}", m.tx_index, m.kind);
            }
        }

        Ok((
            call_traces,
            invalid_transactions,
            compare_report,
            tx_mismatches,
        ))
    }

    fn build_revm_txs(
        transactions: &[ZKsyncTxEnvelope],
        block_output: Option<&BlockOutput>,
        block_gas_limit: u64,
        settlement_layer_chain_id: U256,
        independent_gas: bool,
    ) -> anyhow::Result<Vec<ReplayTx>> {
        if let Some(block_output) = block_output {
            if transactions.len() != block_output.tx_results.len() {
                bail!(
                    "Transactions count ({}) does not match tx_results count ({})",
                    transactions.len(),
                    block_output.tx_results.len()
                );
            }

            let mut revm_txs = Vec::with_capacity(transactions.len());

            for (idx, (transaction, tx_output_raw)) in transactions
                .iter()
                .zip(&block_output.tx_results)
                .enumerate()
            {
                let Ok(tx_output) = tx_output_raw else {
                    log::debug!(
                        "Skipping tx #{idx} in REVM replay because ZKsync OS rejected it: {tx_output_raw:?}"
                    );
                    continue;
                };

                let (gas_override, force_fail) = if independent_gas {
                    (None, false)
                } else {
                    (Some(tx_output.gas_used), !tx_output.is_success())
                };

                let tx_env = zk_tx_into_revm_tx(
                    transaction,
                    gas_override,
                    force_fail,
                    block_gas_limit,
                    Some(settlement_layer_chain_id),
                )
                .with_context(|| format!("Failed to convert tx #{idx} to REVM tx"))?;

                revm_txs.push(ReplayTx {
                    tx: tx_env,
                    original_tx_index: idx,
                });
            }

            Ok(revm_txs)
        } else {
            transactions
                .iter()
                .enumerate()
                .map(|(idx, transaction)| {
                    zk_tx_into_revm_tx(
                        transaction,
                        None,
                        false,
                        block_gas_limit,
                        Some(settlement_layer_chain_id),
                    )
                    .with_context(|| format!("Failed to convert tx #{idx} to REVM tx"))
                    .map(|tx| ReplayTx {
                        tx,
                        original_tx_index: idx,
                    })
                })
                .collect()
        }
    }

    fn build_compare_report<DB>(
        cache_db: &mut CacheDB<DB>,
        block_output: &BlockOutput,
    ) -> anyhow::Result<CompareReport>
    where
        DB: DatabaseRef,
        DB::Error: std::error::Error + Send + Sync + 'static,
    {
        CompareReport::build(
            cache_db,
            &block_output.storage_writes,
            &block_output.account_diffs,
        )
    }

    /// Compare per-tx return data, logs, and success/revert outcome.
    fn compare_tx_results(
        revm_results: &[(
            usize,
            revm::context_interface::result::ExecutionResult<HaltReason>,
        )],
        block_output: &BlockOutput,
    ) -> Vec<TxComparisonMismatch> {
        let mut mismatches = Vec::new();

        for (tx_idx, revm_result) in revm_results {
            let Some(Ok(zk_output)) = block_output.tx_results.get(*tx_idx) else {
                continue;
            };

            // Compare outcome (success vs revert).
            let revm_success = revm_result.is_success();
            let zk_success = zk_output.is_success();
            if revm_success != zk_success {
                mismatches.push(TxComparisonMismatch {
                    tx_index: *tx_idx,
                    kind: TxMismatchKind::OutcomeMismatch {
                        revm_success,
                        zk_success,
                    },
                });
                // If outcome differs, skip return data / logs comparison.
                continue;
            }

            // Compare return data.
            let revm_output = revm_result.output().cloned();
            let zk_output_data = match &zk_output.execution_result {
                zksync_os_interface::types::ExecutionResult::Success(
                    zksync_os_interface::types::ExecutionOutput::Call(data),
                ) => data.clone(),
                zksync_os_interface::types::ExecutionResult::Success(
                    zksync_os_interface::types::ExecutionOutput::Create(data, _),
                ) => data.clone(),
                zksync_os_interface::types::ExecutionResult::Revert(data) => data.clone(),
            };

            let revm_output_bytes = revm_output.as_ref().map(|b| b.as_ref()).unwrap_or(&[]);
            if revm_output_bytes != zk_output_data.as_slice() {
                mismatches.push(TxComparisonMismatch {
                    tx_index: *tx_idx,
                    kind: TxMismatchKind::ReturnDataMismatch {
                        revm: revm_output,
                        zk: zk_output_data,
                    },
                });
            }

            // Compare event logs.
            let revm_logs = revm_result.logs();
            let zk_logs = &zk_output.logs;
            if revm_logs.len() != zk_logs.len() {
                mismatches.push(TxComparisonMismatch {
                    tx_index: *tx_idx,
                    kind: TxMismatchKind::LogsMismatch {
                        revm_count: revm_logs.len(),
                        zk_count: zk_logs.len(),
                        first_diff_index: Some(revm_logs.len().min(zk_logs.len())),
                    },
                });
            } else {
                // Same count — compare individual logs.
                for (i, (revm_log, zk_log)) in revm_logs.iter().zip(zk_logs.iter()).enumerate() {
                    if !logs_match(revm_log, zk_log) {
                        mismatches.push(TxComparisonMismatch {
                            tx_index: *tx_idx,
                            kind: TxMismatchKind::LogsMismatch {
                                revm_count: revm_logs.len(),
                                zk_count: zk_logs.len(),
                                first_diff_index: Some(i),
                            },
                        });
                        break;
                    }
                }
            }
        }

        mismatches
    }

    fn read_settlement_layer_chain_id(mut state: State) -> anyhow::Result<U256> {
        let flat_key = zk_ee::common_structs::derive_flat_storage_key(
            &ruint::aliases::B160::from_limbs([0x800b, 0, 0]),
            &zk_ee::utils::Bytes32::ZERO,
        );
        Ok(U256::from_be_slice(
            state
                .read(flat_key.into_alloy())
                .unwrap_or_default()
                .as_slice(),
        ))
    }
}

/// Compare two logs for equality (address, topics, data).
fn logs_match(revm_log: &Log, zk_log: &Log) -> bool {
    revm_log.address == zk_log.address
        && revm_log.topics() == zk_log.topics()
        && revm_log.data.data == zk_log.data.data
}
