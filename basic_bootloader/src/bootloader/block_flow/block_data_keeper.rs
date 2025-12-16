use zk_ee::system::{IOSubsystemExt, SystemTypes};

/// Trait for collecting and tracking data from transactions that are successfully processed in the current block.
///
/// NOTE: Only tracks transactions that were actually included and processed successfully.
pub trait BlockTransactionsDataKeeper<S: SystemTypes>: core::fmt::Debug
where
    S::IO: IOSubsystemExt,
{
}

#[derive(Debug)]
pub struct NopTransactionDataKeeper;

impl<S: SystemTypes> BlockTransactionsDataKeeper<S> for NopTransactionDataKeeper where
    S::IO: IOSubsystemExt
{
}
