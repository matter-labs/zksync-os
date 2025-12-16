use zk_ee::system::{IOSubsystemExt, SystemTypes};

/// NOTE: Such keeper is expected to only bookkeep transactions that were actually included and processed
pub trait BlockTransactionsDataCollector<S: SystemTypes>: core::fmt::Debug
where
    S::IO: IOSubsystemExt,
{
}

#[derive(Debug)]
pub struct NopTransactionDataKeeper;

impl<S: SystemTypes> BlockTransactionsDataCollector<S> for NopTransactionDataKeeper where
    S::IO: IOSubsystemExt
{
}
