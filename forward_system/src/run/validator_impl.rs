use zk_ee::system::validator::{TxValidationError, TxValidationResult};
use zk_ee::system::{EthereumLikeTypes, SystemTypes};

pub(crate) struct ValidatorWrapped<'a, V: zksync_os_interface::tracing::TxValidator>(pub &'a mut V);

fn map_iface_err(e: zksync_os_interface::error::InvalidTransaction) -> TxValidationError {
    match e {
        zksync_os_interface::error::InvalidTransaction::FilteredByValidator => {
            TxValidationError::FilteredByValidator
        }
        _ => unreachable!("interface TxValidator must only return FilteredByValidator"),
    }
}

impl<'a, V, S> zk_ee::system::validator::TxValidator<S> for ValidatorWrapped<'a, V>
where
    V: zksync_os_interface::tracing::TxValidator,
    S: SystemTypes + EthereumLikeTypes,
{
    fn begin_tx(&mut self, calldata: &[u8]) -> TxValidationResult {
        self.0.begin_tx(calldata).map_err(map_iface_err)
    }

    fn finish_tx(&mut self) -> TxValidationResult {
        self.0.finish_tx().map_err(map_iface_err)
    }
}
