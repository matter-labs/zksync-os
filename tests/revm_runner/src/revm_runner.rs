use alloy::primitives::U256;
use reth_revm::ExecuteCommitEvm;
use reth_revm::{db::CacheDB, Context};
use zksync_os_interface::types::BlockContext;
use zksync_os_interface::types::BlockOutput;
use zksync_os_revm::DefaultZk;
use zksync_os_revm::ZkBuilder;
use zksync_os_revm::ZkSpecId;
use zksync_os_tests_common::zksync_tx::ZKsyncTxRequest;

use crate::helpers::zk_tx_into_revm_tx;
use crate::revm_state_provider::{RevmStateProvider, ViewState};

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
        transactions: Vec<ZKsyncTxRequest>,
        block_context: BlockContext,
        block_output: Option<BlockOutput>,
    ) -> anyhow::Result<()> {
        let state_provider = RevmStateProvider::new(
            self.state.clone(),
            block_context.block_hashes,
            block_context.block_number,
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
            .build_zk();

        let revm_txs: Vec<_> = if let Some(block_output) = block_output {
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

        let execution_result = evm.transact_many_commit(revm_txs.into_iter())?;

        println!("Execution result: {:#?}", execution_result);

        // TODO: compare execution results, maybe as a separate function
        /*let compare_report = CompareReport::build(
            evm.0.db_mut(),
            &block_output.storage_writes,
            &block_output.account_diffs,
        )?;
        self.handle_report(&replay_record, &compare_report)?;
        */
        Ok(())
    }
}
