use crate::run::convert_alloy::IntoAlloy;
use zk_ee::system::validator::{
    BeginTxContext, TxValidationError, TxValidationResult, ValidatedTxKind,
};
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

fn into_interface_kind(kind: ValidatedTxKind) -> zksync_os_interface::tracing::ValidatedTxKind {
    use zksync_os_interface::tracing::ValidatedTxKind as Iface;
    match kind {
        ValidatedTxKind::L2 => Iface::L2,
        ValidatedTxKind::L1Priority => Iface::L1Priority,
    }
}

impl<'a, V, S> zk_ee::system::validator::TxValidator<S> for ValidatorWrapped<'a, V>
where
    V: zksync_os_interface::tracing::TxValidator,
    S: SystemTypes + EthereumLikeTypes,
{
    fn begin_tx(&mut self, ctx: &BeginTxContext<'_>) -> TxValidationResult {
        let iface_ctx = zksync_os_interface::tracing::BeginTxContext {
            kind: into_interface_kind(ctx.kind),
            from: ctx.from.into_alloy(),
            to: ctx.to.map(IntoAlloy::into_alloy),
            value: ctx.value,
            calldata: ctx.calldata,
            gas_limit: ctx.gas_limit,
        };
        self.0.begin_tx(&iface_ctx).map_err(map_iface_err)
    }

    fn finish_tx(&mut self) -> TxValidationResult {
        self.0.finish_tx().map_err(map_iface_err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::system::system_types::ForwardRunningSystem;
    use ruint::aliases::{B160, U256};
    use zksync_os_interface::tracing::ValidatedTxKind as IfaceKind;

    #[derive(Default)]
    struct Capturing {
        kinds: Vec<IfaceKind>,
    }

    impl zksync_os_interface::tracing::TxValidator for Capturing {
        fn begin_tx(
            &mut self,
            ctx: &zksync_os_interface::tracing::BeginTxContext<'_>,
        ) -> zksync_os_interface::tracing::TxValidationResult {
            self.kinds.push(ctx.kind);
            match ctx.kind {
                IfaceKind::L2 => Ok(()),
                IfaceKind::L1Priority => {
                    Err(zksync_os_interface::error::InvalidTransaction::FilteredByValidator)
                }
            }
        }
    }

    fn ctx(kind: ValidatedTxKind) -> BeginTxContext<'static> {
        BeginTxContext {
            kind,
            from: B160::ZERO,
            to: None,
            value: U256::ZERO,
            calldata: &[],
            gas_limit: 21_000,
        }
    }

    #[test]
    fn passes_the_transaction_kind_to_the_interface_validator() {
        use zk_ee::system::validator::TxValidator;

        let mut capturing = Capturing::default();
        let mut wrapped = ValidatorWrapped(&mut capturing);

        assert!(TxValidator::<ForwardRunningSystem>::begin_tx(
            &mut wrapped,
            &ctx(ValidatedTxKind::L2)
        )
        .is_ok());
        assert!(matches!(
            TxValidator::<ForwardRunningSystem>::begin_tx(
                &mut wrapped,
                &ctx(ValidatedTxKind::L1Priority)
            ),
            Err(TxValidationError::FilteredByValidator)
        ));

        assert_eq!(capturing.kinds, vec![IfaceKind::L2, IfaceKind::L1Priority]);
    }
}
