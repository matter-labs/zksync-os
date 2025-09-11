use super::result_keeper::TxProcessingOutputOwned;
use crate::run::convert::IntoInterface;
use basic_bootloader::bootloader::errors::InvalidTransaction;

pub trait TxResultCallback: 'static {
    fn tx_executed(
        &mut self,
        tx_execution_result: Result<TxProcessingOutputOwned, InvalidTransaction>,
    );
}

impl<T: zksync_os_interface::traits::TxResultCallback> TxResultCallback for T {
    fn tx_executed(
        &mut self,
        tx_execution_result: Result<TxProcessingOutputOwned, InvalidTransaction>,
    ) {
        self.tx_executed(tx_execution_result.map_err(IntoInterface::into_interface))
    }
}
