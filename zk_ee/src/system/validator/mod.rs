//! Transaction validation hook system.
//!
//! Provides a trait-based interface for validating transactions at key execution points:
//! - `begin_tx`: Validate before execution
//! - `finish_tx`: Validate after execution

use crate::system::SystemTypes;
use ruint::aliases::{B160, U256};

/// Errors that can occur during transaction validation.
#[derive(Debug)]
pub enum TxValidationError {
    /// Transaction was rejected by the validator
    FilteredByValidator,
}

pub type TxValidationResult = Result<(), TxValidationError>;

/// Context passed to [`TxValidator::begin_tx`] before a transaction is executed.
///
/// Carries the static transaction fields that a policy may inspect when deciding
/// whether to admit a transaction.
pub struct BeginTxContext<'a> {
    pub from: B160,
    pub to: Option<B160>,
    pub value: U256,
    pub calldata: &'a [u8],
    pub gas_limit: u64,
}

pub trait TxValidator<S: SystemTypes> {
    /// Is called before bootloader starts execution of a transaction
    fn begin_tx(&mut self, ctx: &BeginTxContext<'_>) -> TxValidationResult;

    /// Is called after bootloader finishes execution of a transaction
    fn finish_tx(&mut self) -> TxValidationResult;
}

#[derive(Default)]
pub struct NopTxValidator;

impl<S: SystemTypes> TxValidator<S> for NopTxValidator {
    fn begin_tx(&mut self, _ctx: &BeginTxContext<'_>) -> TxValidationResult {
        Ok(())
    }

    fn finish_tx(&mut self) -> TxValidationResult {
        Ok(())
    }
}
