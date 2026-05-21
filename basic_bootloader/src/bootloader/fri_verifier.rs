//! Target-specific FRI proof verification driver.
//!
//! Single entry [`verify_fri_statement_stream`] consumes the FRI
//! oracle response iterator and verifies the proof against a claimed
//! `statement_versioned_hash`. Target dispatch is hidden here so
//! callers (the bootloader's tx flow) stay target-agnostic:
//!
//! - On host the iterator is fully drained so `ReadWitnessSource`
//!   captures the proof bytes for the RISC-V replay, then the host
//!   runner from [`crate::bootloader::fri_host_verifier`] verifies
//!   the recovered word slice.
//! - On RISC-V the iterator backs the CSR-driven non-determinism
//!   source that the airbender unified verifier reads. That verifier
//!   is the final authority; bad proofs make the block fail to prove.

use crate::bootloader::constants::FRI_STATEMENT_HASH_VERSION;
use crate::bootloader::errors::{InvalidTransaction, TxError};
use crypto::{sha3::Keccak256, MiniDigest};
use zk_ee::utils::Bytes32;

#[cfg(all(
    not(target_arch = "riscv32"),
    not(all(target_pointer_width = "64", target_endian = "little"))
))]
compile_error!("FRI host verifier response unpacking requires a 64-bit little-endian host target");

/// Derive `statement_versioned_hash` from the 16 `u32` output
/// registers returned by the FRI unified verifier. The hash format is
/// `version || keccak256(output_words_le)[1..]`.
pub(super) fn statement_versioned_hash_from_verifier_output(output: &[u32; 16]) -> Bytes32 {
    let mut hasher = Keccak256::new();
    for word in output.iter() {
        hasher.update(word.to_le_bytes());
    }
    let mut hash = hasher.finalize();
    hash[0] = FRI_STATEMENT_HASH_VERSION;
    Bytes32::from_array(hash)
}

/// Verify a FRI proof oracle response against a claimed
/// `statement_versioned_hash`.
pub(super) fn verify_fri_statement_stream<R>(
    response: R,
    statement_versioned_hash: Bytes32,
) -> Result<(), TxError>
where
    R: ExactSizeIterator<Item = usize>,
{
    #[cfg(not(target_arch = "riscv32"))]
    {
        host::verify(response, statement_versioned_hash)
    }
    #[cfg(target_arch = "riscv32")]
    {
        guest::verify(response, statement_versioned_hash)
    }
}

fn expected_remaining_response_words(verifier_word_count: usize) -> usize {
    verifier_word_count + usize::from(verifier_word_count.is_multiple_of(2))
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
    let expected_remaining = expected_remaining_response_words(verifier_word_count);
    if response.len() != expected_remaining {
        return Err(TxError::Validation(
            InvalidTransaction::FriProofVerificationFailed,
        ));
    }

    Ok(verifier_word_count)
}

#[cfg(not(target_arch = "riscv32"))]
mod host {
    use super::{expected_remaining_response_words, InvalidTransaction, TxError};
    use crate::bootloader::fri_host_verifier::{verify_host_fri_statement, FriHostVerifyError};
    use alloc::vec::Vec;
    use zk_ee::utils::Bytes32;

    pub(super) fn verify<R>(response: R, statement_versioned_hash: Bytes32) -> Result<(), TxError>
    where
        R: ExactSizeIterator<Item = usize>,
    {
        // Collecting consumes the iterator, so `ReadWitnessSource` records
        // the same words the RISC-V guest will replay.
        let response_words = collect_response_words(response);
        let verifier_words = verifier_words_from_response_words(&response_words)?;
        verify_host_fri_statement(verifier_words, statement_versioned_hash)
            .map_err(verify_error_to_tx_error)
    }

    pub(super) fn collect_response_words(
        response: impl ExactSizeIterator<Item = usize>,
    ) -> Vec<u32> {
        let mut words = Vec::with_capacity(response.len().saturating_mul(2));
        for packed in response {
            let packed = packed as u64;
            words.push(packed as u32);
            words.push((packed >> 32) as u32);
        }
        words
    }

    pub(super) fn verifier_words_from_response_words(
        response_words: &[u32],
    ) -> Result<&[u32], TxError> {
        let Some((&verifier_word_count, remaining_words)) = response_words.split_first() else {
            return Err(TxError::Validation(
                InvalidTransaction::FriProofSidecarMissing,
            ));
        };
        let verifier_word_count = verifier_word_count as usize;

        if remaining_words.len() != expected_remaining_response_words(verifier_word_count) {
            return Err(TxError::Validation(
                InvalidTransaction::FriProofVerificationFailed,
            ));
        }

        let (verifier_words, trailing_padding) = remaining_words.split_at(verifier_word_count);
        if verifier_word_count.is_multiple_of(2) && trailing_padding.first().copied() != Some(0) {
            return Err(TxError::Validation(
                InvalidTransaction::FriProofVerificationFailed,
            ));
        }

        Ok(verifier_words)
    }

    fn verify_error_to_tx_error(err: FriHostVerifyError) -> TxError {
        match err {
            FriHostVerifyError::StatementHashMismatch => {
                TxError::Validation(InvalidTransaction::FriProofStatementHashMismatch)
            }
            FriHostVerifyError::VerifierThreadSpawn => {
                TxError::Internal(zk_ee::internal_error!("FRI host verifier thread spawn").into())
            }
            FriHostVerifyError::VerifierRejected
            | FriHostVerifyError::TrailingWords
            | FriHostVerifyError::UnsupportedOpType => {
                TxError::Validation(InvalidTransaction::FriProofVerificationFailed)
            }
        }
    }
}

#[cfg(target_arch = "riscv32")]
mod guest {
    use super::{
        begin_fri_verifier_stream, statement_versioned_hash_from_verifier_output,
        InvalidTransaction, TxError,
    };
    use zk_ee::utils::Bytes32;

    pub(super) fn verify<R>(
        mut response: R,
        statement_versioned_hash: Bytes32,
    ) -> Result<(), TxError>
    where
        R: ExactSizeIterator<Item = usize>,
    {
        // On RISC-V:
        // - the first guest word is the verifier word count;
        // - the verifier payload follows immediately as raw u32 CSR words;
        // - when the total u32 response length is odd, the final host u64
        //   contributes one trailing zero padding word after the payload.
        let verifier_word_count = begin_fri_verifier_stream(&mut response)?;
        drop(response);
        let output = run_verifier()?;
        finish_stream_after_verifier(verifier_word_count)?;
        check_statement_hash_matches(&output, statement_versioned_hash)
    }

    fn finish_stream_after_verifier(verifier_word_count: usize) -> Result<(), TxError> {
        use full_statement_verifier::verifier_common::non_determinism_source::NonDeterminismSource;
        use full_statement_verifier::verifier_common::DefaultNonDeterminismSource;

        if verifier_word_count.is_multiple_of(2) {
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

    #[inline(always)]
    fn run_verifier() -> Result<[u32; 16], TxError> {
        use full_statement_verifier::definitions::OP_VERIFY_UNIFIED_RECURSION_LAYER_IN_UNIFIED_CIRCUIT;
        use full_statement_verifier::verifier_common::non_determinism_source::NonDeterminismSource;
        use full_statement_verifier::verifier_common::DefaultNonDeterminismSource;

        // Pin to unified recursion.
        let op_type = DefaultNonDeterminismSource::read_word();
        if op_type != OP_VERIFY_UNIFIED_RECURSION_LAYER_IN_UNIFIED_CIRCUIT {
            return Err(TxError::Validation(
                InvalidTransaction::FriProofVerificationFailed,
            ));
        }

        Ok(
            full_statement_verifier::unified_circuit_statement::verify_unified_circuit_recursion_layer(
                full_statement_verifier::verifier_common::SecurityModel::Security100,
            ),
        )
    }

    fn check_statement_hash_matches(output: &[u32; 16], expected: Bytes32) -> Result<(), TxError> {
        if statement_versioned_hash_from_verifier_output(output) != expected {
            return Err(TxError::Validation(
                InvalidTransaction::FriProofStatementHashMismatch,
            ));
        }
        Ok(())
    }
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
        let changed = statement_versioned_hash_from_verifier_output(&output);
        assert_ne!(baseline, changed);

        assert_eq!(changed.as_u8_array_ref()[0], FRI_STATEMENT_HASH_VERSION);

        let mut hasher = Keccak256::new();
        for word in output.iter() {
            hasher.update(word.to_le_bytes());
        }
        let without_version = Bytes32::from_array(hasher.finalize());

        assert_eq!(
            &changed.as_u8_array_ref()[1..],
            &without_version.as_u8_array_ref()[1..]
        );
    }

    fn finish_fri_verifier_stream(
        response: &mut impl Iterator<Item = usize>,
        verifier_word_count: usize,
    ) -> Result<(), TxError> {
        if verifier_word_count.is_multiple_of(2) {
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

    #[cfg(not(target_arch = "riscv32"))]
    fn pack_response_words(words: &[u32]) -> alloc::vec::Vec<usize> {
        words
            .chunks(2)
            .map(|chunk| {
                let low = chunk[0] as u64;
                let high = chunk.get(1).copied().unwrap_or_default() as u64;
                (low | (high << 32)) as usize
            })
            .collect()
    }

    #[cfg(not(target_arch = "riscv32"))]
    #[test]
    fn host_response_unpacking_exposes_verifier_words() {
        let packed = pack_response_words(&[3, 11, 22, 33]);
        let response_words = host::collect_response_words(packed.into_iter());

        assert_eq!(response_words, vec![3, 11, 22, 33]);
        assert_eq!(
            host::verifier_words_from_response_words(&response_words).unwrap(),
            &[11, 22, 33]
        );
    }

    #[cfg(not(target_arch = "riscv32"))]
    #[test]
    fn host_response_unpacking_validates_even_count_padding() {
        let packed = pack_response_words(&[4, 11, 22, 33, 44, 0]);
        let response_words = host::collect_response_words(packed.into_iter());

        assert_eq!(
            host::verifier_words_from_response_words(&response_words).unwrap(),
            &[11, 22, 33, 44]
        );

        let packed = pack_response_words(&[4, 11, 22, 33, 44, 9]);
        let response_words = host::collect_response_words(packed.into_iter());
        let err = host::verifier_words_from_response_words(&response_words).unwrap_err();
        assert!(matches!(
            err,
            TxError::Validation(InvalidTransaction::FriProofVerificationFailed)
        ));
    }
}
