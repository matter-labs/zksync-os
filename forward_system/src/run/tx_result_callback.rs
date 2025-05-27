use super::result_keeper::TxProcessingOutputOwned;
use basic_bootloader::bootloader::errors::InvalidTransaction;

pub trait TxResultCallback: 'static {
    fn tx_executed(
        &mut self,
        tx_execution_result: Result<TxProcessingOutputOwned, InvalidTransaction>,
    );
}
