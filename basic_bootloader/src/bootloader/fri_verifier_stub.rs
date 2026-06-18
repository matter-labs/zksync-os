use crate::bootloader::errors::{InvalidTransaction, TxError};
use zk_ee::utils::Bytes32;

/// Verify a FRI proof oracle response against a claimed
/// `statement_versioned_hash`.
#[allow(dead_code)]
pub(super) fn verify_fri_statement_stream<R>(
    response: R,
    statement_versioned_hash: Bytes32,
) -> Result<(), TxError>
where
    R: ExactSizeIterator<Item = usize>,
{
    let _ = (response, statement_versioned_hash);
    Err(TxError::Validation(
        InvalidTransaction::FriProofTxNotSupported,
    ))
}
