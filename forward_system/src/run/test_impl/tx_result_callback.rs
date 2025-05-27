use crate::run::result_keeper::TxProcessingOutputOwned;
use crate::run::TxResultCallback;
use basic_bootloader::bootloader::errors::InvalidTransaction;

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct NoopTxCallback;

impl TxResultCallback for NoopTxCallback {
    fn tx_executed(
        &mut self,
        _tx_execution_result: Result<TxProcessingOutputOwned, InvalidTransaction>,
    ) {
    }
}
