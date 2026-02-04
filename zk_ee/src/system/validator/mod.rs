//! Transaction validation hook system.
//!
//! Provides a trait-based interface for validating transactions at key execution points:
//! - `begin_tx`: Validate before execution
//! - `finish_tx`: Validate after execution

use crate::system::SystemTypes;

/// Errors that can occur during transaction validation.
#[derive(Debug)]
pub enum TxValidationError {
    /// Transaction was rejected by the validator
    FilteredByValidator,
}

pub type TxValidationResult = Result<(), TxValidationError>;

pub trait TxValidator<S: SystemTypes> {
    /// Is called before bootloader starts execution of a transaction
    fn begin_tx(&mut self, calldata: &[u8]) -> TxValidationResult;

    /// Is called after bootloader finishes execution of a transaction
    fn finish_tx(&mut self) -> TxValidationResult;
}

#[derive(Default)]
pub struct NopTxValidator;

impl<S: SystemTypes> TxValidator<S> for NopTxValidator {
    fn begin_tx(&mut self, _calldata: &[u8]) -> TxValidationResult {
        Ok(())
    }

    fn finish_tx(&mut self) -> TxValidationResult {
        Ok(())
    }
}
