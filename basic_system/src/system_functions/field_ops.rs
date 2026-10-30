//! The adapter between the oracle side of zksync-os (`zk_ee`) and the hints of `airbender-crypto`
//! (`crypto::hints`), which hold the hinted field operations, their checks, the request and
//! answer encodings (`FieldHintOp`, `HintAnswer`, ...) and the oracle side that answers them
//! (`crypto::hints::answer_query`).
//!
//! The querier side needs none: a zksync-os oracle is a `MemoryOracle` of `airbender_oracle`, the
//! querier of the hints, so the precompiles and the secp256k1 hooks of `airbender-crypto` take it as
//! it is, and the protocol's errors are its `InternalError`s. [`ZkOsQuerierMemory`] is the memory of
//! a querier as the oracle sees it (`zk_ee`'s), as `airbender-crypto`'s [`QuerierMemory`], for the
//! query processors of `callable_oracles`.

use crypto::hints::{HintError, QuerierMemory};
use zk_ee::oracle::memory_io::host::QuerierMemory as ZkEeQuerierMemory;
use zk_ee::system::errors::internal::InternalError;

/// The message of a [`HintError`] for an [`InternalError`] without one: `zk_ee` keeps the
/// messages only with its `error_origins` feature
pub const UNKNOWN_ZK_EE_ERROR: &str = "zk_ee oracle error (its message needs `error_origins`)";

/// The [`HintError`] of an [`InternalError`] of the querier memory: its message, if it is kept
#[inline(always)]
pub fn hint_error(error: InternalError) -> HintError {
    HintError(error.message().unwrap_or(UNKNOWN_ZK_EE_ERROR))
}

/// The memory of a querier as the oracle sees it, `zk_ee`'s [`ZkEeQuerierMemory`], as the
/// [`QuerierMemory`] of `airbender-crypto`: the memory of this process
/// (`zk_ee::oracle::memory_io::host::NativeQuerierMemory`, 8-byte words) for a native run, or the
/// memory of the RISC-V guest (`oracle_provider::GuestMemory` over a `RamPeek`, 4-byte words) for
/// a run of the guest in the simulator. The word size and the reads are the wrapped memory's,
/// with the errors mapped by [`hint_error`].
#[derive(Clone, Copy)]
pub struct ZkOsQuerierMemory<'a>(pub &'a dyn ZkEeQuerierMemory);

impl QuerierMemory for ZkOsQuerierMemory<'_> {
    fn word_size(&self) -> usize {
        self.0.word_size()
    }

    fn read_u32(&self, address: usize) -> Result<u32, HintError> {
        self.0.read_u32(address).map_err(hint_error)
    }
}

/// Oracles that answer the field hints of the tests of the precompiles dishonestly, from the
/// honest answers of `callable_oracles`
#[cfg(test)]
pub(crate) mod test_oracles {
    use super::ZkOsQuerierMemory;
    use alloc::vec::Vec;
    use callable_oracles::field_hints::NativeFieldOpsQuery;
    use crypto::hints::{read_field_hint_request, FieldHintOp};
    use oracle_provider::{OracleQueryProcessor, RunMode, ZkEENonDeterminismSource};
    use zk_ee::oracle::memory_io::host::QuerierMemory;

    /// The op of the hint request at `input_word`
    fn read_op(memory: &dyn QuerierMemory, input_word: usize) -> FieldHintOp {
        let (op, _, _) = read_field_hint_request(&ZkOsQuerierMemory(memory), input_word).unwrap();
        FieldHintOp::parse_u32(op).unwrap()
    }

    /// A processor that answers the field hint queries of the given ops with a corrupted value
    /// (the low bit of the first word flipped), and the others honestly
    struct FlipLowBit {
        ops: Vec<FieldHintOp>,
    }

    impl OracleQueryProcessor for FlipLowBit {
        fn supported_memory_query_ids(&self) -> Vec<u32> {
            NativeFieldOpsQuery.supported_memory_query_ids()
        }

        fn process_memory_query(
            &mut self,
            query_id: u32,
            input_word: usize,
            memory: &dyn QuerierMemory,
            mode: RunMode,
            native_run_responses: &mut Vec<u32>,
            guest_run_responses: &mut Vec<u32>,
        ) {
            let op = read_op(memory, input_word);
            // the response of the native run (the querier of the tests)
            let mut words = Vec::new();
            NativeFieldOpsQuery.process_memory_query(
                query_id,
                input_word,
                memory,
                mode,
                &mut words,
                guest_run_responses,
            );
            if self.ops.contains(&op) {
                // the low word of the first element (past the claim flag of the pairing
                // ops): still a canonical element, so the value check is what fails
                let index = match op {
                    FieldHintOp::Bn254PairingResidueWitness
                    | FieldHintOp::Bls12381KzgResidueWitness
                    | FieldHintOp::Bls12381PairingResidueWitness
                    | FieldHintOp::Secp256k1RecoveryWitness => 1,
                    _ => 0,
                };
                // an answer that is only its flag (no recovery witness) has the flag flipped
                let index = index.min(words.len() - 1);
                words[index] ^= 1;
            }
            native_run_responses.extend(words);
        }
    }

    /// A processor that claims every pairing product not to be the identity, with the
    /// (correct) inverse the slow path needs
    struct ClaimNotIdentity;

    impl OracleQueryProcessor for ClaimNotIdentity {
        fn supported_memory_query_ids(&self) -> Vec<u32> {
            NativeFieldOpsQuery.supported_memory_query_ids()
        }

        fn process_memory_query(
            &mut self,
            query_id: u32,
            input_word: usize,
            memory: &dyn QuerierMemory,
            mode: RunMode,
            native_run_responses: &mut Vec<u32>,
            guest_run_responses: &mut Vec<u32>,
        ) {
            match read_op(memory, input_word) {
                FieldHintOp::Bn254PairingResidueWitness
                | FieldHintOp::Bls12381KzgResidueWitness
                | FieldHintOp::Bls12381PairingResidueWitness => {
                    assert!(mode.produces_native_run_responses());
                    // SAFETY: the tests query from this process (a native run), whose memory
                    // `memory` is, while they send the request of a hinted operation, which
                    // points to live values of the operand types of its op
                    native_run_responses.extend(unsafe {
                        crypto::hints::not_identity_claim(&ZkOsQuerierMemory(memory), input_word)
                    })
                }
                _ => NativeFieldOpsQuery.process_memory_query(
                    query_id,
                    input_word,
                    memory,
                    mode,
                    native_run_responses,
                    guest_run_responses,
                ),
            }
        }
    }

    /// An oracle that claims every pairing product not to be the identity
    pub(crate) fn non_identity_claiming_oracle() -> ZkEENonDeterminismSource {
        let mut oracle = ZkEENonDeterminismSource::default();
        oracle.add_external_processor(ClaimNotIdentity);
        oracle
    }

    /// An oracle that corrupts the answers of `ops` (their first payload word)
    pub(crate) fn lying_oracle(ops: &[FieldHintOp]) -> ZkEENonDeterminismSource {
        let mut oracle = ZkEENonDeterminismSource::default();
        oracle.add_external_processor(FlipLowBit { ops: ops.to_vec() });
        oracle
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;
    use zk_ee::internal_error;

    #[test]
    fn errors_keep_their_messages() {
        let error = internal_error!("a message");
        assert_eq!(
            hint_error(error.clone()).0,
            error.message().unwrap_or(UNKNOWN_ZK_EE_ERROR)
        );
    }

    #[test]
    fn the_querier_memory_is_the_wrapped_one() {
        struct Words(Vec<u32>);

        impl ZkEeQuerierMemory for Words {
            fn word_size(&self) -> usize {
                4
            }

            fn read_u32(&self, address: usize) -> Result<u32, InternalError> {
                self.0
                    .get(address / 4)
                    .copied()
                    .ok_or(internal_error!("out of the guest memory"))
            }
        }

        let words = Words(vec![3, 5]);
        let memory = ZkOsQuerierMemory(&words);
        assert_eq!(memory.word_size(), 4);
        assert_eq!(memory.read_u32(4), Ok(5));
        assert_eq!(
            memory.read_u32(8),
            Err(hint_error(internal_error!("out of the guest memory")))
        );
        // SAFETY: only the word size is read
        let native = unsafe { zk_ee::oracle::memory_io::host::NativeQuerierMemory::new() };
        assert_eq!(ZkOsQuerierMemory(&native).word_size(), 8);
    }
}
