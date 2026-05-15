//! FRI proof handling for `FriProofTx` transactions.
//!
//! This module has two responsibilities that run in different
//! execution configs:
//!
//! - [`build_verified_fri_statements_list`] — structural admission
//!   checks (is_gateway, cap, dedup) that produce the hash list to
//!   install on `TxLevelMetadata`. Runs in every config. Does not
//!   touch the oracle or the verifier.
//!
//! - [`drive_fri_verification`] — runs only when
//!   `Config::VERIFY_FRI_PROOFS == true`, i.e. in
//!   `BasicBootloaderProvingExecutionConfig`. Issues the
//!   `FRI_PROOF_QUERY_ID` oracle query for each claimed statement
//!   hash and hands the response to
//!   [`crate::bootloader::fri_verifier::verify_fri_statement_stream`],
//!   which dispatches to the host or RISC-V verifier.
//!
//! The sequencer's forward run, `eth_call`, and ETH-replay paths all
//! set `VERIFY_FRI_PROOFS = false` and never call
//! [`drive_fri_verification`]. FRI proof validity in those configs is
//! trusted, similar to how the sequencer trusts
//! `VALIDATE_EOA_SIGNATURE = false` for tx signatures.
use crate::bootloader::errors::{InvalidTransaction, TxError};
use crate::bootloader::fri_verifier::verify_fri_statement_stream;
use crate::bootloader::transaction::Transaction;
use zk_ee::oracle::query_ids::FRI_PROOF_QUERY_ID;
use zk_ee::oracle::IOOracle;
use zk_ee::system::constants::MAX_FRI_STATEMENTS_PER_TX;
use zk_ee::system::metadata::basic_metadata::GatewayModeMetadata;
use zk_ee::system::{EthereumLikeTypes, IOSubsystemExt, System};
use zk_ee::utils::Bytes32;

/// Structural admission checks for a `FriProofTx`'s statement-hash
/// list: enforce `is_gateway`, cap against `MAX_FRI_STATEMENTS_PER_TX`,
/// and dedup. Returns the unique hash list in declaration order.
pub(super) fn build_verified_fri_statements_list<S: EthereumLikeTypes>(
    system: &System<S>,
    transaction: &Transaction<S::Allocator>,
) -> Result<arrayvec::ArrayVec<Bytes32, MAX_FRI_STATEMENTS_PER_TX>, TxError>
where
    S::Metadata: GatewayModeMetadata,
{
    if !system.metadata.is_gateway() {
        return Err(TxError::Validation(
            InvalidTransaction::FriProofTxNotSupported,
        ));
    }

    let mut verified = arrayvec::ArrayVec::<Bytes32, MAX_FRI_STATEMENTS_PER_TX>::new();
    if let Some(statement_versioned_hashes) = transaction.statement_versioned_hashes() {
        // Reject the tx before running any verifier work if the hash
        // list is larger than the per-tx cap. This keeps the validator
        // O(cap) and bounds the stack size of `TxLevelMetadata`.
        if statement_versioned_hashes.count > MAX_FRI_STATEMENTS_PER_TX {
            return Err(TxError::Validation(
                InvalidTransaction::TooManyFriStatements,
            ));
        }
        // Dedup verifier work, but not charging.
        for statement_versioned_hash in statement_versioned_hashes.iter() {
            let statement_versioned_hash =
                Bytes32::from_array(*statement_versioned_hash.map_err(TxError::Validation)?);
            if verified.contains(&statement_versioned_hash) {
                continue;
            }
            verified
                .try_push(statement_versioned_hash)
                .expect("cap already checked against MAX_FRI_STATEMENTS_PER_TX");
        }
    }
    Ok(verified)
}

/// Drive the `FRI_PROOF_QUERY_ID` oracle query for each hash in
/// `verified` and dispatch to the target-specific verifier.
pub(super) fn drive_fri_verification<S: EthereumLikeTypes>(
    system: &mut System<S>,
    verified: &[Bytes32],
) -> Result<(), TxError>
where
    S::IO: IOSubsystemExt,
{
    for statement_versioned_hash in verified {
        verify_fri_statement(system, *statement_versioned_hash)?;
    }
    Ok(())
}

/// Issue the `FRI_PROOF_QUERY_ID` oracle query for a single claimed
/// `statement_versioned_hash`, then verify the proof using the
/// target-specific verifier backend.
fn verify_fri_statement<S: EthereumLikeTypes>(
    system: &mut System<S>,
    statement_versioned_hash: Bytes32,
) -> Result<(), TxError>
where
    S::IO: IOSubsystemExt,
{
    let response = system
        .io
        .oracle()
        .raw_query(FRI_PROOF_QUERY_ID, &statement_versioned_hash)?;
    verify_fri_statement_stream(response, statement_versioned_hash)
}
