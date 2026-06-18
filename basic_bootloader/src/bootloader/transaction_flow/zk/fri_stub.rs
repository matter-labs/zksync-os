use crate::bootloader::errors::{InvalidTransaction, TxError};
use crate::bootloader::transaction::Transaction;
use zk_ee::system::constants::MAX_FRI_STATEMENTS_PER_TX;
use zk_ee::system::{EthereumLikeTypes, IOSubsystemExt, System};
use zk_ee::utils::Bytes32;

pub(super) fn build_verified_fri_statements_list<S: EthereumLikeTypes>(
    system: &System<S>,
    transaction: &Transaction<S::Allocator>,
) -> Result<arrayvec::ArrayVec<Bytes32, MAX_FRI_STATEMENTS_PER_TX>, TxError> {
    let _ = (system, transaction);
    Err(TxError::Validation(
        InvalidTransaction::FriProofTxNotSupported,
    ))
}

pub(super) fn drive_fri_verification<S: EthereumLikeTypes>(
    system: &mut System<S>,
    verified: &[Bytes32],
) -> Result<(), TxError>
where
    S::IO: IOSubsystemExt,
{
    let _ = (system, verified);
    Err(TxError::Validation(
        InvalidTransaction::FriProofTxNotSupported,
    ))
}
