use zk_ee::system::{IOSubsystemExt, System, SystemTypes};

use super::{transaction::Transaction, BasicTransactionFlow, ExecutionResult};

/// Trait for collecting and tracking data from transactions that are successfully processed in the current block.
///
/// NOTE: Only tracks transactions that were actually included and processed successfully.
pub trait BlockTransactionsDataKeeper<S: SystemTypes, F: BasicTransactionFlow<S>>:
    core::fmt::Debug
where
    S::IO: IOSubsystemExt,
{
    fn record_transaction_results(
        &mut self,
        system: &System<S>,
        transaction: Transaction<S::Allocator>,
        context: &F::TransactionContext,
        result: &ExecutionResult<'_, <S as SystemTypes>::IOTypes>,
    );

    /// Whether the block has the capacity left for a transaction with these limits,
    /// checked before the transaction is validated; the default admits everything
    fn admit_transaction(
        &self,
        _gas_limit: u64,
        _blob_gas: u64,
    ) -> Result<(), crate::bootloader::errors::InvalidTransaction> {
        Ok(())
    }
}

#[derive(Debug)]
pub struct NopTransactionDataKeeper;

impl<S: SystemTypes, F: BasicTransactionFlow<S>> BlockTransactionsDataKeeper<S, F>
    for NopTransactionDataKeeper
where
    S::IO: IOSubsystemExt,
{
    fn record_transaction_results(
        &mut self,
        _system: &System<S>,
        _transaction: Transaction<<S as SystemTypes>::Allocator>,
        _context: &F::TransactionContext,
        _result: &ExecutionResult<'_, <S as SystemTypes>::IOTypes>,
    ) {
        // NOP
    }
}
