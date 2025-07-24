use crate::bootloader::supported_ees::errors::EESubsystemError;
use ruint::aliases::{B160, U256};
use zk_ee::system::{
    errors::{
        internal::InternalError,
        root_cause::{GetRootCause, RootCause},
        runtime::RuntimeError,
        system::SystemError,
    },
    NonceSubsystemError,
};

// Taken from revm, contains changes
///
/// Transaction validation error.
///
#[derive(Debug, Clone)]
pub enum InvalidTransaction {
    /// Failed to decode.
    InvalidEncoding,
    /// Fields set incorrectly in accordance to its type.
    InvalidStructure,
    /// When using the EIP-1559 fee model introduced in the London upgrade, transactions specify two primary fee fields:
    /// - `gas_max_fee`: The maximum total fee a user is willing to pay, inclusive of both base fee and priority fee.
    /// - `gas_priority_fee`: The extra amount a user is willing to give directly to the miner, often referred to as the "tip".
    ///
    /// Provided `gas_priority_fee` exceeds the total `gas_max_fee`.
    PriorityFeeGreaterThanMaxFee,
    /// `basefee` is greater than provided `gas_max_fee`.
    BaseFeeGreaterThanMaxFee,
    /// EIP-1559: `gas_price` is less than `basefee`.
    GasPriceLessThanBasefee,
    /// `gas_limit` in the tx is bigger than `block_gas_limit`.
    CallerGasLimitMoreThanBlock,
    /// Initial gas for a Call is bigger than `gas_limit`.
    ///
    /// Initial gas for a Call contains:
    /// - initial stipend gas
    /// - gas for access list and input data
    CallGasCostMoreThanGasLimit,
    /// EIP-3607 Reject transactions from senders with deployed code
    RejectCallerWithCode,
    /// Transaction account does not have enough amount of ether to cover transferred value and gas_limit*gas_price.
    LackOfFundForMaxFee {
        fee: U256,
        balance: U256,
    },
    /// Overflow payment in transaction.
    OverflowPaymentInTransaction,
    /// Nonce overflows in transaction.
    NonceOverflowInTransaction,
    NonceTooHigh {
        tx: u64,
        state: u64,
    },
    NonceTooLow {
        tx: u64,
        state: u64,
    },
    MalleableSignature,
    IncorrectFrom {
        tx: B160,
        recovered: B160,
    },
    /// EIP-3860: Limit and meter initcode
    CreateInitCodeSizeLimit,
    /// Transaction chain id does not match the config chain id.
    InvalidChainId,
    /// Access list is not supported for blocks before the Berlin hardfork.
    AccessListNotSupported,
    /// Unacceptable gas per pubdata price.
    GasPerPubdataTooHigh,
    /// Block gas limit is too high.
    BlockGasLimitTooHigh,
    /// Protocol upgrade tx should be first in the block.
    UpgradeTxNotFirst,

    /// Call during AA validation reverted
    Revert {
        method: AAMethod,
        output: Option<&'static [u8]>,
    },
    /// Bootloader received insufficient fees
    ReceivedInsufficientFees {
        received: U256,
        required: U256,
    },
    /// Invalid magic returned by validation
    InvalidMagic,
    /// Validation returndata is of invalid length
    InvalidReturndataLength,
    /// Ran out of gas during validation
    OutOfGasDuringValidation,
    /// Ran out of native resources during validation
    OutOfNativeResourcesDuringValidation,
    /// Transaction nonce already used
    NonceUsedAlready,
    /// Nonce not increased after validation
    NonceNotIncreased,
    /// Return data from paymaster is too short
    PaymasterReturnDataTooShort,
    /// Invalid magic in paymaster validation
    PaymasterInvalidMagic,
    /// Paymaster returned invalid context
    PaymasterContextInvalid,
    /// Paymaster context offset is greater than returndata length
    PaymasterContextOffsetTooLong,

    /// Protocol upgrade txs should always be successful.
    // TODO: it's not really a validation error
    UpgradeTxFailed,
}

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
    Internal(BootloaderSubsystemError),
}

impl From<BootloaderSubsystemError> for TxError {
    fn from(v: BootloaderSubsystemError) -> Self {
        Self::Internal(v)
    }
}

impl From<InvalidTransaction> for TxError {
    fn from(value: InvalidTransaction) -> Self {
        TxError::Validation(value)
    }
}

impl From<InternalError> for TxError {
    fn from(e: InternalError) -> Self {
        TxError::Internal(e.into())
    }
}

impl TxError {
    /// Do not implement From to avoid accidentally wrapping
    /// an out of native during Tx execution as a validation error.
    pub fn oon_as_validation(e: BootloaderSubsystemError) -> Self {
        if let RootCause::Runtime(RuntimeError::OutOfNativeResources(_)) = e.root_cause() {
            Self::Validation(InvalidTransaction::OutOfNativeResourcesDuringValidation)
        } else {
            Self::Internal(e)
        }
    }
}

impl From<SystemError> for TxError {
    fn from(e: SystemError) -> Self {
        match e {
            SystemError::LeafRuntime(RuntimeError::OutOfErgs(_)) => {
                TxError::Validation(InvalidTransaction::OutOfGasDuringValidation)
            }
            SystemError::LeafRuntime(RuntimeError::OutOfNativeResources(_)) => {
                Self::Validation(InvalidTransaction::OutOfNativeResourcesDuringValidation)
            }
            SystemError::LeafDefect(e) => TxError::Internal(e.into()),
        }
    }
}

#[macro_export]
macro_rules! revert_on_recoverable {
    ($e:expr) => {
        match $e {
            Ok(x) => Ok(x),
            Err(SystemError::LeafDefect(err)) => Err(err),
            Err(SystemError::LeafRuntime(RuntimeError::OutOfNativeResources(_))) => {
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
            Err(zk_ee::internal_error!($s))
        }
    };
}

zk_ee::define_subsystem!(Bootloader,
      interface BootloaderInterfaceError {
          TopLevelInsufficientBalance,
      },
      cascade WrappedError {
          EEError(EESubsystemError),
          Nonce(NonceSubsystemError),
      }
);
