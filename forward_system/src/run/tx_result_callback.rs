use super::result_keeper::TxProcessingOutputOwned;
use zksync_os_error::core::tx_valid::ValidationError as InvalidTransaction;

pub trait TxResultCallback: 'static {
    fn tx_executed(
        &mut self,
        tx_execution_result: Result<TxProcessingOutputOwned, InvalidTransaction>,
    );
}
