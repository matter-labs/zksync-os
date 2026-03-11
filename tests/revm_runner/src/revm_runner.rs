use alloy::primitives::U256;
use alloy::rpc::types::trace::geth::CallFrame;
use anyhow::{anyhow, bail, Context as AnyhowContext};
use revm::{
    context::{ContextTr, TxEnv},
    context_interface::block::BlobExcessGasAndPrice,
    database::{CacheDB, EmptyDB},
    inspector::InspectCommitEvm,
    DatabaseRef,
};
use revm_inspectors::tracing::{TracingInspector, TracingInspectorConfig};
use zksync_os_interface::types::BlockContext;
use zksync_os_interface::types::BlockOutput;
use zksync_os_revm::{DefaultZk, ZKsyncTx, ZkBuilder, ZkContext, ZkSpecId};
use zksync_os_tests_common::zksync_tx::ZKsyncTxEnvelope;

use crate::helpers::{
    calculate_excess_blob_gas_from_blob_base_fee, zk_tx_into_revm_tx, BLOB_BASE_FEE_UPDATE_FRACTION,
};
use crate::revm_state_provider::{RevmStateProvider, ViewState};
use crate::storage_diff_comp::CompareReport;

pub struct RevmRunner<State>
where
    State: ViewState,
{
    state: State,
}

impl<State> RevmRunner<State>
where
    State: ViewState,
{
    pub fn new(state: State) -> Self {
        Self { state }
    }

    pub fn run(
        &mut self,
        transactions: Vec<ZKsyncTxEnvelope>,
        block_context: BlockContext,
        block_output: Option<BlockOutput>,
    ) -> anyhow::Result<()> {
        self.run_with_call_traces(transactions, block_context, block_output)
            .map(|_| ())
    }

    pub fn run_with_call_traces(
        &mut self,
        transactions: Vec<ZKsyncTxEnvelope>,
        block_context: BlockContext,
        block_output: Option<BlockOutput>,
    ) -> anyhow::Result<Vec<CallFrame>> {
        let state_provider = RevmStateProvider::new(
            self.state.clone(),
            block_context.block_hashes,
            block_context.block_number.saturating_sub(1),
        );

        let blob_excess_gas_and_price = BlobExcessGasAndPrice::new(
            calculate_excess_blob_gas_from_blob_base_fee(
                block_context
                    .blob_fee
                    .try_into()
                    .expect("Blob fee should fit into u64"),
                BLOB_BASE_FEE_UPDATE_FRACTION,
            ),
            BLOB_BASE_FEE_UPDATE_FRACTION
                .try_into()
                .expect("Blob base fee update fraction should fit into u64"),
        );

        let cache_db = CacheDB::new(state_provider);
        let mut evm = ZkContext::<EmptyDB>::default()
            .with_db(cache_db)
            .modify_cfg_chained(|cfg| {
                cfg.chain_id = block_context.chain_id;
                cfg.spec = ZkSpecId::AtlasV3; // TODO: make it configurable
            })
            .modify_block_chained(|block| {
                block.number = U256::from(block_context.block_number);
                block.timestamp = U256::from(block_context.timestamp);
                block.beneficiary = block_context.coinbase;
                block.basefee = block_context.eip1559_basefee.saturating_to();
                block.gas_limit = block_context.gas_limit;
                block.prevrandao = Some(block_context.mix_hash.into());
                block.blob_excess_gas_and_price = Some(blob_excess_gas_and_price);
            })
            .build_zk_with_inspector(TracingInspector::new(TracingInspectorConfig::default_geth()));

        let revm_txs = Self::build_revm_txs(&transactions, block_output.as_ref())?;

        let mut call_traces = Vec::with_capacity(revm_txs.len());
        for tx in revm_txs {
            let tx_execution = evm.inspect_tx_commit(tx)?;
            let trace = evm
                .0
                .inspector
                .geth_builder()
                .geth_call_traces(Default::default(), tx_execution.gas_used());
            call_traces.push(trace);
            evm.0.inspector.fuse();
        }

        if let Some(block_output) = block_output.as_ref() {
            Self::compare_state_diffs(evm.0.db_mut(), block_output)?;
        }

        Ok(call_traces)
    }

    fn build_revm_txs(
        transactions: &[ZKsyncTxEnvelope],
        block_output: Option<&BlockOutput>,
    ) -> anyhow::Result<Vec<ZKsyncTx<TxEnv>>> {
        if let Some(block_output) = block_output {
            if transactions.len() != block_output.tx_results.len() {
                bail!(
                    "Transactions count ({}) does not match tx_results count ({})",
                    transactions.len(),
                    block_output.tx_results.len()
                );
            }

            transactions
                .iter()
                .zip(&block_output.tx_results)
                // Ignore invalid transactions - they should be skipped
                .filter(|(_, tx_output_raw)| tx_output_raw.is_ok())
                .enumerate()
                .map(|(idx, (transaction, tx_output_raw))| {
                    let tx_output = tx_output_raw.as_ref().map_err(|e| {
                        anyhow!(
                            "Tx #{idx} is invalid in block output and cannot be replayed: {e:?}"
                        )
                    })?;

                    zk_tx_into_revm_tx(
                        transaction,
                        Some(tx_output.gas_used),
                        !tx_output.is_success(),
                    )
                    .with_context(|| format!("Failed to convert tx #{idx} to REVM tx"))
                })
                .collect()
        } else {
            transactions
                .iter()
                .enumerate()
                .map(|(idx, transaction)| {
                    zk_tx_into_revm_tx(transaction, None, false)
                        .with_context(|| format!("Failed to convert tx #{idx} to REVM tx"))
                })
                .collect()
        }
    }

    fn compare_state_diffs<DB>(
        cache_db: &mut CacheDB<DB>,
        block_output: &BlockOutput,
    ) -> anyhow::Result<()>
    where
        DB: DatabaseRef,
        DB::Error: std::error::Error + Send + Sync + 'static,
    {
        let compare_report = CompareReport::build(
            cache_db,
            &block_output.storage_writes,
            &block_output.account_diffs,
        )?;

        if !compare_report.is_empty() {
            log::warn!("State mismatch found after REVM replay");
            compare_report.log_tracing(100);
            bail!(
                "REVM consistency mismatch: storage={} account={}",
                compare_report.storage.len(),
                compare_report.accounts.len()
            );
        }

        Ok(())
    }
}
