use alloy::primitives::U256;
use alloy::rpc::types::{trace::geth::CallFrame, Transaction};
use reth_revm::context::ContextTr;
use reth_revm::inspector::InspectCommitEvm;
use reth_revm::{db::CacheDB, Context};
use revm_inspectors::tracing::{TracingInspector, TracingInspectorConfig};
use zksync_os_interface::types::BlockContext;
use zksync_os_interface::types::BlockOutput;
use zksync_os_revm::DefaultZk;
use zksync_os_revm::ZkBuilder;
use zksync_os_revm::ZkSpecId;

use crate::helpers::zk_tx_into_revm_tx;
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
        transactions: Vec<Transaction>,
        block_context: BlockContext,
        block_output: Option<BlockOutput>,
    ) -> anyhow::Result<()> {
        self.run_with_call_traces(transactions, block_context, block_output)
            .map(|_| ())
    }

    pub fn run_with_call_traces(
        &mut self,
        transactions: Vec<Transaction>,
        block_context: BlockContext,
        block_output: Option<BlockOutput>,
    ) -> anyhow::Result<Vec<CallFrame>> {
        let state_provider = RevmStateProvider::new(
            self.state.clone(),
            block_context.block_hashes,
            block_context.block_number - 1,
        );
        let mut cache_db = CacheDB::new(state_provider);
        let mut evm = Context::default()
            .with_db(&mut cache_db)
            .modify_cfg_chained(|cfg| {
                cfg.chain_id = block_context.chain_id;
                cfg.spec = ZkSpecId::AtlasV2; // TODO: make it configurable
            })
            .modify_block_chained(|block| {
                block.number = U256::from(block_context.block_number);
                block.timestamp = U256::from(block_context.timestamp);
                block.beneficiary = block_context.coinbase;
                block.basefee = block_context.eip1559_basefee.saturating_to();
                block.gas_limit = block_context.gas_limit;
                block.prevrandao = Some(block_context.mix_hash.into());
            })
            .build_zk_with_inspector(TracingInspector::new(TracingInspectorConfig::default_geth()));

        let revm_txs: Vec<_> = if let Some(block_output) = block_output.as_ref() {
            transactions
                .iter()
                .zip(&block_output.tx_results)
                .map(|(transaction, tx_output_raw)| {
                    let tx_output = tx_output_raw.as_ref().expect(
                        "block_output of a sealed block must not contain invalid transactions",
                    );

                    zk_tx_into_revm_tx(
                        transaction,
                        Some(tx_output.gas_used),
                        !tx_output.is_success(),
                    )
                })
                .collect()
        } else {
            transactions
                .iter()
                .map(|transaction| zk_tx_into_revm_tx(transaction, None, false))
                .collect()
        };

        let mut call_traces = Vec::with_capacity(revm_txs.len());
        for tx in revm_txs {
            let tx_execution = evm.inspect_tx_commit(tx)?;
            let raw_trace = evm
                .0
                .inspector
                .geth_builder()
                .geth_call_traces(Default::default(), tx_execution.gas_used());
            let trace: CallFrame = serde_json::from_value(serde_json::to_value(raw_trace)?)?;
            call_traces.push(trace);
            evm.0.inspector.fuse();
        }

        if let Some(block_output) = block_output {
            // TODO: maybe it should be a separate function
            let compare_report = CompareReport::build(
                evm.0.db_mut(),
                &block_output.storage_writes,
                &block_output.account_diffs,
            )?;
            if !compare_report.is_empty() {
                println!("************* State mismatch found *************");
                compare_report.log_tracing(100);
            }
        }

        Ok(call_traces)
    }
}
