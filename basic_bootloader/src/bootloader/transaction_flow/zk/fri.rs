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
//!       source and invoke the in-circuit unified verifier. The
//!       circuit is the final authority; bad proofs make the block
//!       fail to prove.
//!
//! The sequencer's forward run, `eth_call`, and ETH-replay paths all
//! set `VERIFY_FRI_PROOFS = false` and never call
//! [`drive_fri_verification`]. FRI proof validity in those configs is
//! trusted from the admission layer, analogous to how the sequencer
//! trusts `VALIDATE_EOA_SIGNATURE = false` for tx signatures.
#[cfg(any(target_arch = "riscv32", test))]
use crate::bootloader::constants::FRI_STATEMENT_HASH_VERSION;
use crate::bootloader::errors::{InvalidTransaction, TxError};
use crate::bootloader::transaction::Transaction;
#[cfg(any(target_arch = "riscv32", test))]
use crypto::{sha3::Keccak256, MiniDigest};
use zk_ee::oracle::query_ids::FRI_PROOF_QUERY_ID;
use zk_ee::oracle::IOOracle;
use zk_ee::system::constants::MAX_FRI_STATEMENTS_PER_TX;
use zk_ee::system::metadata::basic_metadata::GatewayModeMetadata;
use zk_ee::system::{EthereumLikeTypes, IOSubsystemExt, System};
use zk_ee::utils::Bytes32;

/// Derive the versioned statement hash from the 16 `u32` words the
/// FRI unified verifier returns.
///
/// Format: `keccak256(version_byte || word_0_le || ... || word_15_le)`.
/// The version byte lets us evolve the encoding without risking
/// collisions with previously emitted hashes.
///
/// Used on the RISC-V path after the in-circuit verifier returns, to
/// rebind the derived hash to the claimed `statement_versioned_hash`.
/// Host-mode builds don't run the verifier (admission-layer trust),
/// so this helper is only compiled in when it has a caller.
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
/// and dedup. Returns the unique hash list in declaration order, ready
/// to install on `TxLevelMetadata.verified_fri_statements`.
///
/// This runs in ALL configs — sequencer forward, `eth_call`, recording
/// pass, prover. It does not query the oracle, does not touch the
/// sidecar, and does not run any verifier. Callers that must also
/// verify the proofs drive the oracle query separately via
/// [`drive_fri_verification`].
///
/// Rationale for skipping verification at the sequencer: admission
/// (outside zksync-os) is the FRI gatekeeper, analogous to signature
/// verification. The in-circuit verifier run by the prover is the
/// final authority. Duplicating that work inside the sequencer's
/// bootloader is wasted CPU — same reason
/// `VALIDATE_EOA_SIGNATURE = false` on the forward-sim config skips
/// signature checks.
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
        // Dedup: re-verifying the same hash within a tx produces the
        // same result and the precompile's membership check cannot
        // distinguish "verified once" from "verified N times". We skip
        // redundant slots but do NOT adjust the submitter's gas/native
        // charge — the submitter asked for N slots and pays for N
        // slots (`statement_versioned_hashes_num` in
        // `validation_impl.rs` uses the raw count). `verified` holds
        // the unique set, so `TxLevelMetadata` stores each hash once.
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
/// `verified` and, on the RISC-V guest only, invoke the in-circuit
/// verifier.
///
/// Called only when `Config::VERIFY_FRI_PROOFS == true`:
///   - Proving (RISC-V guest): in-circuit verifier runs; a failure
///     aborts the binary and the block fails to prove.
///   - Prover-input recording pass (host, `ProvingExecutionConfig`):
///     the oracle query is issued purely so `ReadWitnessSource`
///     captures the proof stream. The host-side verifier is NOT run
///     — the circuit is the final authority and any extra host
///     verifier run would be the duplicated admission-layer work
///     Antonio flagged.
///
/// The structural hash list (`verified`) is already populated by
/// [`build_verified_fri_statements_list`]; this function only adds
/// the oracle-side work.
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
/// or invoke the in-circuit verifier (RISC-V guest).
///
/// The oracle query itself is issued in both modes. What differs is
/// what the caller does with the response:
///
/// - **Host (`not(target_arch = "riscv32")`)**: drain so
///   `ReadWitnessSource` captures the full proof stream for the
///   prover to replay later. The host does not verify — the
///   in-circuit verifier is the final authority and the host is not
///   an admission check (that happens upstream).
/// - **RISC-V guest**: only consume the sidecar-present signal (the
///   length prefix). Proof bytes reach the in-circuit verifier
///   through CSR reads directly, not through this iterator. Missing
///   sidecar → `FriProofSidecarMissing`. Proof bytes present →
///   delegate to
///   `full_statement_verifier`, which aborts the binary on a bad
///   proof (the in-circuit verifier has no fallible entry point
///   today).
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
        // On RISC-V the iterator only carries the sidecar-present
        // signal: `Some(length_prefix)` when the sidecar has an entry
        // for this hash, `None` when it doesn't. Proof bytes are
        // consumed by the in-circuit verifier through a separate CSR
        // channel. Calling the verifier with no proof data would
        // otherwise abort the binary.
        response.next().ok_or(TxError::Validation(
            InvalidTransaction::FriProofSidecarMissing,
        ))?;
        drop(response);
        // SAFETY INVARIANT: the non-determinism stream the in-circuit
        // verifier reads here is a replay of what `FriProofResponder`
        // produced during the prover-input recording pass (host
        // mode, `ProvingExecutionConfig`), captured word-for-word by
        // `ReadWitnessSource` (in `oracle_provider`). The bootloader
        // itself no longer runs a host-side verifier — FRI proofs
        // are accepted by the admission layer (outside zksync-os),
        // and the in-circuit verifier below is the final authority.
        //
        // Malformed bytes reaching this point would hit an assertion
        // inside the unified verifier and abort the RISC-V binary —
        // the guest has no fallible-verify primitive today, so the
        // failure mode is "block fails to prove", not "tx rejected".
        // In the current design the possible sources of malformed
        // bytes are (a) a compromised/buggy admission layer, or
        // (b) a recorder bug in `ReadWitnessSource`. Both are
        // sequencer-side operational issues; there is no user-facing
        // path to reach here with adversarially-chosen bytes.
        //
        // TODO: once `full_statement_verifier` exposes a fallible
        // entry point, map its error to `FriProofVerificationFailed`
        // here so block-level proving failures become tx-level
        // rejections.
        // The verifier reads proof bytes from the CSR-backed
        // non-determinism source directly and returns 16 u32 words
        // identifying the proved statement.
        let output = full_statement_verifier::unified_circuit_statement::verify_unrolled_or_unified_circuit_recursion_layer();
        check_statement_hash_matches(&output, statement_versioned_hash)
    }
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
}
