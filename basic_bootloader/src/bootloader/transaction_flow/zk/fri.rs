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
//!   `BasicBootloaderProvingExecutionConfig`. Two sub-behaviors:
//!     - **Host (recording pass)**: issue the oracle query and drain
//!       the response so `ReadWitnessSource` captures the proof
//!       stream the RISC-V guest will replay.
//!     - **RISC-V guest**: drive the CSR-based non-determinism
//!       source and invoke the airbender unified verifier. That
//!       verifier is the final authority; bad proofs make the block
//!       fail to prove.
//!
//! The sequencer's forward run, `eth_call`, and ETH-replay paths all
//! set `VERIFY_FRI_PROOFS = false` and never call
//! [`drive_fri_verification`]. FRI proof validity in those configs is
//! trusted, similar to how the sequencer trusts
//! `VALIDATE_EOA_SIGNATURE = false` for tx signatures.
#[cfg(any(target_arch = "riscv32", test))]
use crate::bootloader::constants::FRI_STATEMENT_HASH_VERSION;
use crate::bootloader::errors::{InvalidTransaction, TxError};
use crate::bootloader::transaction::Transaction;
#[cfg(any(target_arch = "riscv32", test))]
use crypto::{MiniDigest, sha3::Keccak256};
use zk_ee::oracle::IOOracle;
use zk_ee::oracle::query_ids::FRI_PROOF_QUERY_ID;
use zk_ee::system::constants::MAX_FRI_STATEMENTS_PER_TX;
use zk_ee::system::metadata::basic_metadata::GatewayModeMetadata;
use zk_ee::system::{EthereumLikeTypes, IOSubsystemExt, System};
use zk_ee::utils::Bytes32;

/// Derive the versioned statement hash from the 16 `u32` words the
/// FRI unified verifier returns.
#[cfg(any(target_arch = "riscv32", test))]
pub(super) fn statement_versioned_hash_from_verifier_output(output: &[u32; 16]) -> Bytes32 {
    let mut hasher = Keccak256::new();
    hasher.update([FRI_STATEMENT_HASH_VERSION]);
    for word in output.iter() {
        hasher.update(word.to_le_bytes());
    }
    Bytes32::from_array(hasher.finalize())
}

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
/// `verified` and, on the RISC-V guest only, invoke the airbender
/// unified verifier.
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
/// `statement_versioned_hash`, then either drain the response (host)
/// or invoke the airbender unified verifier (RISC-V guest).
pub(super) fn verify_fri_statement<S: EthereumLikeTypes>(
    system: &mut System<S>,
    statement_versioned_hash: Bytes32,
) -> Result<(), TxError>
where
    S::IO: IOSubsystemExt,
{
    #[allow(unused_mut)] // only the RISC-V arm mutates (calls `.next()`)
    let mut response = system
        .io
        .oracle()
        .raw_query(FRI_PROOF_QUERY_ID, &statement_versioned_hash)?;

    #[cfg(not(target_arch = "riscv32"))]
    {
        // Drain the iterator so `ReadWitnessSource` (in
        // `oracle_provider`) captures every word. Without this drain
        // the prover's CSR replay would be missing the proof bytes.
        for _ in response {}
        Ok(())
    }
    #[cfg(target_arch = "riscv32")]
    {
        verify_fri_statement_riscv(response, statement_versioned_hash)
    }
}

#[cfg(target_arch = "riscv32")]
fn verify_fri_statement_riscv<R>(
    mut response: R,
    statement_versioned_hash: Bytes32,
) -> Result<(), TxError>
where
    R: ExactSizeIterator<Item = usize>,
{
    // The witness recorder stores the FRI response as the normal
    // host-u64-to-guest-u32 split of the `u32` stream
    // `[verifier_word_count, verifier_payload...]`.
    //
    // So on RISC-V:
    // - the first guest word is the verifier word count;
    // - the verifier payload follows immediately as raw u32 CSR words;
    // - when the total u32 response length is odd, the final host u64
    //   contributes one trailing zero padding word after the payload.
    let verifier_word_count = begin_fri_verifier_stream(&mut response)?;
    drop(response);
    let output = run_fri_verifier()?;
    finish_fri_verifier_stream_after_verifier(verifier_word_count)?;
    check_statement_hash_matches(&output, statement_versioned_hash)
}

#[cfg(any(target_arch = "riscv32", test))]
fn begin_fri_verifier_stream(
    response: &mut impl ExactSizeIterator<Item = usize>,
) -> Result<usize, TxError> {
    let verifier_word_count = response.next().ok_or(TxError::Validation(
        InvalidTransaction::FriProofSidecarMissing,
    ))?;

    // On RISC-V this is the host-declared remaining CSR word count
    // surfaced through the generic oracle iterator.
    let expected_remaining = verifier_word_count + usize::from(verifier_word_count % 2 == 0);
    if response.len() != expected_remaining {
        return Err(TxError::Validation(
            InvalidTransaction::FriProofVerificationFailed,
        ));
    }

    Ok(verifier_word_count)
}

#[cfg(test)]
fn finish_fri_verifier_stream(
    response: &mut impl Iterator<Item = usize>,
    verifier_word_count: usize,
) -> Result<(), TxError> {
    if verifier_word_count % 2 == 0 {
        let trailing_padding = response.next().ok_or(TxError::Validation(
            InvalidTransaction::FriProofVerificationFailed,
        ))?;
        if trailing_padding != 0 {
            return Err(TxError::Validation(
                InvalidTransaction::FriProofVerificationFailed,
            ));
        }
    }

    Ok(())
}

#[cfg(target_arch = "riscv32")]
fn finish_fri_verifier_stream_after_verifier(verifier_word_count: usize) -> Result<(), TxError> {
    use full_statement_verifier::verifier_common::DefaultNonDeterminismSource;
    use full_statement_verifier::verifier_common::non_determinism_source::NonDeterminismSource;

    if verifier_word_count % 2 == 0 {
        // The iterator is dropped before verifier execution; the verifier
        // has consumed the payload from the same CSR stream by now.
        let trailing_padding = DefaultNonDeterminismSource::read_word() as usize;
        if trailing_padding != 0 {
            return Err(TxError::Validation(
                InvalidTransaction::FriProofVerificationFailed,
            ));
        }
    }

    Ok(())
}

#[cfg(target_arch = "riscv32")]
#[inline(always)]
fn run_fri_verifier() -> Result<[u32; 16], TxError> {
    Ok(
        full_statement_verifier::unified_circuit_statement::verify_unrolled_or_unified_circuit_recursion_layer(),
    )
}

#[cfg(target_arch = "riscv32")]
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

    #[test]
    fn fri_verifier_stream_consumes_prefix_and_trailing_padding() {
        let mut response = vec![4usize, 11, 22, 33, 44, 0].into_iter();

        let verifier_word_count = begin_fri_verifier_stream(&mut response).unwrap();
        assert_eq!(verifier_word_count, 4);
        assert_eq!(response.next(), Some(11));
        assert_eq!(response.next(), Some(22));
        assert_eq!(response.next(), Some(33));
        assert_eq!(response.next(), Some(44));
        finish_fri_verifier_stream(&mut response, verifier_word_count).unwrap();
        assert_eq!(response.next(), None);
    }

    #[test]
    fn fri_verifier_stream_rejects_length_mismatch() {
        let mut response = vec![4usize, 11, 22, 33].into_iter();

        let err = begin_fri_verifier_stream(&mut response).unwrap_err();
        assert!(matches!(
            err,
            TxError::Validation(InvalidTransaction::FriProofVerificationFailed)
        ));
    }

    #[test]
    fn fri_verifier_stream_odd_count_has_no_trailing_padding() {
        let mut response = vec![3usize, 11, 22, 33].into_iter();

        let verifier_word_count = begin_fri_verifier_stream(&mut response).unwrap();
        assert_eq!(verifier_word_count, 3);
        for expected in [11usize, 22, 33] {
            assert_eq!(response.next(), Some(expected));
        }
        finish_fri_verifier_stream(&mut response, verifier_word_count).unwrap();
        assert_eq!(response.next(), None);
    }

    #[test]
    fn fri_verifier_stream_rejects_mismatched_remaining_words() {
        let mut response = vec![3usize, 11, 22].into_iter();

        let err = begin_fri_verifier_stream(&mut response).unwrap_err();
        assert!(matches!(
            err,
            TxError::Validation(InvalidTransaction::FriProofVerificationFailed)
        ));
    }

    #[test]
    fn fri_verifier_stream_rejects_extra_remaining_words() {
        let mut response = vec![3usize, 11, 22, 33, 0].into_iter();

        let err = begin_fri_verifier_stream(&mut response).unwrap_err();
        assert!(matches!(
            err,
            TxError::Validation(InvalidTransaction::FriProofVerificationFailed)
        ));
    }
}
