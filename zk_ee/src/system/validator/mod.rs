//! Transaction validation hook system.
//!
//! Provides a trait-based interface for validating transactions at key execution points:
//! - `begin_tx`: Validate before execution
//! - `finish_tx`: Validate after execution
//!
//! L2 transactions always go through the hooks; a rejected one is dropped from
//! the block. Priority (L1 -> L2) transactions go through them only on chains
//! with `ChainConfig::l1_tx_filtering_enabled`, and a rejected one is
//! force-reverted instead of dropped, since priority operations must be
//! processed and proven (see [`L1TxOperatorDecision`]).
//!
//! The operator's validator only exists in the sequencing run. Witness
//! generation and proving use [`OracleTxValidator`], which replays the
//! decisions the sequencer recorded, so every run rejects at the same point.

use crate::internal_error;
use crate::oracle::query_ids::L1_TX_OPERATOR_DECISION_QUERY_ID;
use crate::oracle::usize_serialization::{UsizeDeserializable, UsizeSerializable};
use crate::oracle::IOOracle;
use crate::system::errors::internal::InternalError;
use crate::system::SystemTypes;
use ruint::aliases::{B160, U256};

/// Errors that can occur during transaction validation.
#[derive(Debug)]
pub enum TxValidationError {
    /// Transaction was rejected by the validator
    FilteredByValidator,
    /// The validator itself failed (e.g. [`OracleTxValidator`] could not read
    /// the recorded decision). Aborts the block like any other internal error.
    Internal(InternalError),
}

impl From<InternalError> for TxValidationError {
    fn from(e: InternalError) -> Self {
        Self::Internal(e)
    }
}

/// The kind of transaction a [`TxValidator`] is consulted about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValidatedTxKind {
    /// An L2 transaction; a rejection drops it from the block.
    L2,
    /// A priority (L1 -> L2) transaction; a rejection force-reverts it.
    L1Priority,
}

pub type TxValidationResult = Result<(), TxValidationError>;

/// Context passed to [`TxValidator::begin_tx`] before a transaction is executed.
///
/// Carries the static transaction fields that a policy may inspect when deciding
/// whether to admit a transaction.
pub struct BeginTxContext<'a> {
    pub kind: ValidatedTxKind,
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

/// Where, if at all, the operator's [`TxValidator`] rejected a priority
/// (L1 -> L2) transaction on a chain with `ChainConfig::l1_tx_filtering_enabled`.
///
/// A rejected priority transaction is not dropped from the block (it must be
/// processed and proven); it is force-reverted with its full gas limit charged.
/// The sequencer records at which point it rejected, and witness generation
/// and proving replay that decision through [`OracleTxValidator`] so the revert
/// happens at the same point in every run.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub enum L1TxOperatorDecision {
    /// Not rejected.
    #[default]
    Accepted = 0,
    /// Rejected by `begin_tx`: the body is not executed.
    RejectedBeforeExecution = 1,
    /// Rejected by `finish_tx`: the executed body is rolled back.
    RejectedAfterExecution = 2,
}

impl TryFrom<u8> for L1TxOperatorDecision {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Accepted),
            1 => Ok(Self::RejectedBeforeExecution),
            2 => Ok(Self::RejectedAfterExecution),
            _ => Err(()),
        }
    }
}

impl UsizeSerializable for L1TxOperatorDecision {
    const USIZE_LEN: usize = <u64 as UsizeSerializable>::USIZE_LEN;

    fn iter(&self) -> impl ExactSizeIterator<Item = usize> {
        cfg_if::cfg_if!(
            if #[cfg(target_endian = "big")] {
                compile_error!("unsupported architecture: big endian arch is not supported")
            } else if #[cfg(target_pointer_width = "32")] {
                return [*self as usize, 0].into_iter();
            } else if #[cfg(target_pointer_width = "64")] {
                return core::iter::once(*self as usize);
            } else {
                compile_error!("unsupported architecture")
            }
        );
    }
}

impl UsizeDeserializable for L1TxOperatorDecision {
    const USIZE_LEN: usize = <Self as UsizeSerializable>::USIZE_LEN;

    fn from_iter(src: &mut impl ExactSizeIterator<Item = usize>) -> Result<Self, InternalError> {
        let word = <u64 as UsizeDeserializable>::from_iter(src)?;
        let raw =
            u8::try_from(word).map_err(|_| internal_error!("Invalid L1 tx operator decision"))?;
        Self::try_from(raw).map_err(|_| internal_error!("Invalid L1 tx operator decision"))
    }
}

impl<S: SystemTypes> TxValidator<S> for NopTxValidator {
    fn begin_tx(&mut self, _ctx: &BeginTxContext<'_>) -> TxValidationResult {
        Ok(())
    }

    fn finish_tx(&mut self) -> TxValidationResult {
        Ok(())
    }
}

/// Validator for witness generation and proving.
///
/// Replays the operator's recorded decisions on priority (L1 -> L2)
/// transactions from the oracle (`L1_TX_OPERATOR_DECISION_QUERY_ID`, one query
/// per priority transaction in block order), so that a force-revert happens at
/// exactly the point the sequencer's live validator rejected. L2 transactions
/// are never rejected here: a filtered L2 transaction never entered the block.
///
/// Owns its own oracle handle; the sequencing run does not use this type.
pub struct OracleTxValidator<O: IOOracle> {
    oracle: O,
    /// Decision fetched in `begin_tx` for the current priority transaction and
    /// answered in `finish_tx`.
    current: L1TxOperatorDecision,
}

impl<O: IOOracle> OracleTxValidator<O> {
    pub fn new(oracle: O) -> Self {
        Self {
            oracle,
            current: L1TxOperatorDecision::Accepted,
        }
    }
}

impl<S: SystemTypes, O: IOOracle> TxValidator<S> for OracleTxValidator<O> {
    fn begin_tx(&mut self, ctx: &BeginTxContext<'_>) -> TxValidationResult {
        self.current = match ctx.kind {
            ValidatedTxKind::L2 => L1TxOperatorDecision::Accepted,
            ValidatedTxKind::L1Priority => self
                .oracle
                .query_with_empty_input(L1_TX_OPERATOR_DECISION_QUERY_ID)?,
        };
        match self.current {
            L1TxOperatorDecision::RejectedBeforeExecution => {
                Err(TxValidationError::FilteredByValidator)
            }
            L1TxOperatorDecision::Accepted | L1TxOperatorDecision::RejectedAfterExecution => Ok(()),
        }
    }

    fn finish_tx(&mut self) -> TxValidationResult {
        match self.current {
            L1TxOperatorDecision::RejectedAfterExecution => {
                Err(TxValidationError::FilteredByValidator)
            }
            L1TxOperatorDecision::Accepted | L1TxOperatorDecision::RejectedBeforeExecution => {
                Ok(())
            }
        }
    }
}
