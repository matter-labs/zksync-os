//! FRI proof verification used during the pre-execution validation of
//! `FriProofTx` transactions.
//!
//! The entry point is [`verify_fri_statement`], which takes the
//! `statement_versioned_hash` carried by the transaction and either:
//!   - (host path) reads the proof oracle stream from the sidecar via
//!     `FRI_PROOF_QUERY_ID`, feeds it into the host-side unified
//!     verifier, and rebinds the output hash to the transaction; or
//!   - (RISC-V path) drains the same query so that the bootloader's
//!     CSR-based non-determinism source can serve the stream to the
//!     in-circuit verifier, then rebinds the output hash to the tx.
//!
//! Both paths produce a `[u32; 16]` verifier output that we hash with
//! [`statement_versioned_hash_from_verifier_output`]. The transaction
//! is valid only when that hash matches the one the submitter claimed.
use crate::bootloader::constants::FRI_STATEMENT_HASH_VERSION;
use crate::bootloader::errors::{InvalidTransaction, TxError};
use crate::bootloader::transaction::Transaction;
use crypto::{sha3::Keccak256, MiniDigest};
#[cfg(not(target_arch = "riscv32"))]
use prover::nd_source_std;
#[cfg(not(target_arch = "riscv32"))]
use std::panic::{catch_unwind, AssertUnwindSafe};
use zk_ee::oracle::query_ids::FRI_PROOF_QUERY_ID;
use zk_ee::oracle::IOOracle;
use zk_ee::system::metadata::basic_metadata::GatewayModeMetadata;
use zk_ee::system::{EthereumLikeTypes, IOSubsystemExt, System};
use zk_ee::utils::Bytes32;

/// Derive the versioned statement hash from the 16 `u32` words the
/// FRI unified verifier returns.
///
/// Format: `keccak256(version_byte || word_0_le || ... || word_15_le)`.
/// The version byte lets us evolve the encoding without risking
/// collisions with previously emitted hashes.
pub(super) fn statement_versioned_hash_from_verifier_output(output: &[u32; 16]) -> Bytes32 {
    let mut hasher = Keccak256::new();
    hasher.update([FRI_STATEMENT_HASH_VERSION]);
    for word in output.iter() {
        hasher.update(word.to_le_bytes());
    }
    Bytes32::from_array(hasher.finalize())
}

/// Verify every claimed `statement_versioned_hash` carried by a
/// `FriProofTx` and return the list of hashes, in declaration order,
/// so the caller can install it on the transaction-level metadata
/// together with the rest of the tx context.
///
/// This is called as the very first step of `FriProofTx` validation.
/// Any failure here (non-gateway chain, malformed hash list entry,
/// missing sidecar, bad proof, statement-hash mismatch) is mapped to
/// `TxError::Validation`, which causes the block loop to drop the tx
/// from the block — no fees charged, no state committed, no receipt
/// in the block header. The `tx_results` entry is still recorded so
/// the sequencer can report the failure back to the submitter.
pub(super) fn verify_all_fri_statements<S: EthereumLikeTypes>(
    system: &mut System<S>,
    transaction: &Transaction<S::Allocator>,
) -> Result<alloc::vec::Vec<Bytes32>, TxError>
where
    S::IO: IOSubsystemExt,
    S::Metadata: GatewayModeMetadata,
{
    if !system.metadata.is_gateway() {
        return Err(TxError::Validation(
            InvalidTransaction::FriProofTxNotSupported,
        ));
    }

    let mut verified = alloc::vec::Vec::new();
    if let Some(statement_versioned_hashes) = transaction.statement_versioned_hashes() {
        for statement_versioned_hash in statement_versioned_hashes.iter() {
            let statement_versioned_hash =
                Bytes32::from_array(*statement_versioned_hash.map_err(TxError::Validation)?);
            verify_fri_statement(system, statement_versioned_hash)?;
            verified.push(statement_versioned_hash);
        }
    }
    Ok(verified)
}

/// Verify a single claimed `statement_versioned_hash` for the current
/// transaction.
///
/// Returns `Ok(())` when the FRI verifier accepts the proof and the
/// derived statement hash matches `statement_versioned_hash`. Maps any
/// other outcome to the appropriate `InvalidTransaction` variant.
///
/// In the host path we read the proof stream from the sidecar and run
/// the verifier directly here. In the RISC-V path the verifier runs
/// in-circuit via the non-determinism CSR, so we only drain the oracle
/// response here to keep the host-side query machinery in sync and
/// then call the runtime entry point.
pub(super) fn verify_fri_statement<S: EthereumLikeTypes>(
    system: &mut System<S>,
    statement_versioned_hash: Bytes32,
) -> Result<(), TxError>
where
    S::IO: IOSubsystemExt,
{
    #[cfg(not(target_arch = "riscv32"))]
    {
        let oracle_stream = read_fri_proof_oracle_stream(system, statement_versioned_hash)?;
        let output = run_host_verifier(oracle_stream)?;
        check_statement_hash_matches(&output, statement_versioned_hash)
    }
    #[cfg(target_arch = "riscv32")]
    {
        // On RISC-V the in-circuit verifier reads the proof stream via
        // the CSR-backed non-determinism source, not the host oracle,
        // so we only need to drain the bookkeeping response here.
        drop(
            system
                .io
                .oracle()
                .raw_query(FRI_PROOF_QUERY_ID, &statement_versioned_hash)?,
        );
        let output = crate::bootloader::fri_verifier::run_fri_verifier();
        check_statement_hash_matches(&output, statement_versioned_hash)
    }
}

/// Host path: fetch the flattened `u32` oracle stream for this
/// statement hash from the sidecar. Returns the payload words with the
/// length prefix already stripped and the low/high packing already
/// unpacked.
#[cfg(not(target_arch = "riscv32"))]
fn read_fri_proof_oracle_stream<S: EthereumLikeTypes>(
    system: &mut System<S>,
    statement_versioned_hash: Bytes32,
) -> Result<alloc::vec::Vec<u32>, TxError>
where
    S::IO: IOSubsystemExt,
{
    let mut response = system
        .io
        .oracle()
        .raw_query(FRI_PROOF_QUERY_ID, &statement_versioned_hash)?;

    // Empty response from the responder → sidecar has no entry for this
    // hash (or verifier artifacts are missing on the oracle side). Reject.
    let oracle_stream_len = response.next().ok_or(TxError::Validation(
        InvalidTransaction::FriProofSidecarMissing,
    ))?;

    // Payload is packed two `u32`s per `usize` (low | high << 32).
    if response.len() != oracle_stream_len.div_ceil(2) {
        return Err(TxError::Validation(
            InvalidTransaction::FriProofVerificationFailed,
        ));
    }

    let mut oracle_stream = alloc::vec::Vec::with_capacity(oracle_stream_len);
    for packed_words in response {
        oracle_stream.push(packed_words as u32);
        if oracle_stream.len() < oracle_stream_len {
            oracle_stream.push((packed_words >> 32) as u32);
        }
    }
    Ok(oracle_stream)
}

/// Host path: feed `oracle_stream` into the unified verifier and
/// return its 16-word output.
///
/// The upstream verifier panics on malformed input (it was written
/// assuming trusted input from the prover). Proofs here come from an
/// untrusted sidecar, so we catch the panic and map it to a validation
/// error — required by the bootloader's no-panic-on-external-input
/// policy. We also fail the transaction if the verifier finishes but
/// leaves unread words in the stream, since a valid run consumes it
/// exactly.
///
/// TODO: upstream a fallible entry point in `full_statement_verifier`
/// so we can drop `catch_unwind`.
#[cfg(not(target_arch = "riscv32"))]
fn run_host_verifier(oracle_stream: alloc::vec::Vec<u32>) -> Result<[u32; 16], TxError> {
    nd_source_std::set_iterator(oracle_stream.into_iter());
    let verification_result = catch_unwind(AssertUnwindSafe(|| {
        full_statement_verifier::unified_circuit_statement::verify_unrolled_or_unified_circuit_recursion_layer()
    }));
    let remaining_words = drain_fri_verifier_iterator();
    let output = verification_result
        .map_err(|_| TxError::Validation(InvalidTransaction::FriProofVerificationFailed))?;

    if remaining_words != 0 {
        return Err(TxError::Validation(
            InvalidTransaction::FriProofVerificationFailed,
        ));
    }

    Ok(output)
}

/// Host path: consume any oracle words the verifier left behind so the
/// next tx starts with an empty non-determinism source.
#[cfg(not(target_arch = "riscv32"))]
fn drain_fri_verifier_iterator() -> usize {
    let mut remaining = 0usize;
    while nd_source_std::try_read_word().is_some() {
        remaining += 1;
    }
    remaining
}

fn check_statement_hash_matches(output: &[u32; 16], expected: Bytes32) -> Result<(), TxError> {
    let computed = statement_versioned_hash_from_verifier_output(output);
    if computed != expected {
        return Err(TxError::Validation(
            InvalidTransaction::FriProofStatementHashMismatch,
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statement_hash_depends_on_all_verifier_words_and_version() {
        let mut output = [0u32; 16];
        for (idx, word) in output.iter_mut().enumerate() {
            *word = idx as u32 + 1;
        }

        let baseline = statement_versioned_hash_from_verifier_output(&output);

        output[8] ^= 0xdead_beef;
        let changed_recursion_hash = statement_versioned_hash_from_verifier_output(&output);
        assert_ne!(baseline, changed_recursion_hash);

        let mut hasher = Keccak256::new();
        for word in output.iter() {
            hasher.update(word.to_le_bytes());
        }
        let without_version = Bytes32::from_array(hasher.finalize());
        assert_ne!(changed_recursion_hash, without_version);
    }
}
