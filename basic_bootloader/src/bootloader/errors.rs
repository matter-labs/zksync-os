use zk_ee::system::errors::{FatalError, InternalError, SystemError, SystemFunctionError};

use zksync_os_error::core::tx_valid::ValidationError as InvalidTransaction;

///
/// Methods called during AA validation
///
#[derive(Debug, Clone)]
pub enum AAMethod {
    /// The account's validation method itself
    AccountValidate,
    /// The account's pay for transaction method
    AccountPayForTransaction,
    /// The account's pre paymaster method
    AccountPrePaymaster,
    /// Paymaster payment
    PaymasterValidateAndPay,
}

///
/// The transaction processing error.
///
#[derive(Debug)]
pub enum TxError {
    /// Failed to validate the transaction,
    /// shouldn't terminate the block execution
    Validation(InvalidTransaction),
    /// Internal error.
    Internal(InternalError),
}

impl From<InvalidTransaction> for TxError {
    fn from(value: InvalidTransaction) -> Self {
        TxError::Validation(value)
    }
}

impl From<InternalError> for TxError {
    fn from(e: InternalError) -> Self {
        TxError::Internal(e)
    }
}

impl TxError {
    /// Do not implement From to avoid accidentally wrapping
    /// an out of native during Tx execution as a validation error.
    pub fn oon_as_validation(e: FatalError) -> Self {
        match e {
            FatalError::Internal(e) => Self::Internal(e),
            FatalError::OutOfNativeResources => {
                Self::Validation(InvalidTransaction::OutOfNativeResourcesDuringValidation)
            }
        }
    }
}

impl From<SystemError> for TxError {
    fn from(e: SystemError) -> Self {
        match e {
            SystemError::OutOfErgs => {
                TxError::Validation(InvalidTransaction::OutOfGasDuringValidation)
            }
            SystemError::OutOfNativeResources => {
                Self::Validation(InvalidTransaction::OutOfNativeResourcesDuringValidation)
            }
            SystemError::Internal(e) => TxError::Internal(e),
        }
    }
}

impl From<SystemFunctionError> for TxError {
    fn from(e: SystemFunctionError) -> Self {
        match e {
            SystemFunctionError::InvalidInput => {
                TxError::Internal(InternalError("Invalid system function input"))
            }
            SystemFunctionError::System(e) => e.into(),
        }
    }
}

#[macro_export]
macro_rules! revert_on_recoverable {
    ($e:expr) => {
        match $e {
            Ok(x) => Ok(x),
            Err(SystemError::Internal(err)) => Err(err),
            Err(SystemError::OutOfResources) => {
                return Ok(ExecutionResult::Revert {
                    output: MemoryRegion::empty_shared(),
                })
            }
        }
    };
}

#[macro_export]
macro_rules! require {
    ($b:expr, $err:expr, $system:expr) => {
        if $b {
            Ok(())
        } else {
            $system
                .get_logger()
                .write_fmt(format_args!("Check failed: {:?}\n", $err))
                .expect("Failed to write log");
            Err($err)
        }
    };
}

#[macro_export]
macro_rules! unless {
    ($b:expr, $err:expr, $system:expr) => {
        if !$b {
            Ok(())
        } else {
            $system
                .get_logger()
                .write_fmt(format_args!("Check failed: {:?}\n", $err))
                .expect("Failed to write log");
            Err($err)
        }
    };
}

#[macro_export]
macro_rules! require_internal {
    ($b:expr, $s:expr, $system:expr) => {
        if $b {
            Ok(())
        } else {
            $system
                .get_logger()
                .write_fmt(format_args!("Check failed: {}\n", $s))
                .expect("Failed to write log");
            Err(InternalError($s))
        }
    };
}
