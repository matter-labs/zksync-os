//! Oracle query processors for field operation hints (square roots and inverses of the
//! secp256k1, bn254 and bls12-381 fields).
//!
//! The hints are served with the memory-based protocol: the input word is the address of the
//! request, which points to the operand where the querier keeps it. The field elements of the
//! operands and of the answers are in the representation of the target of the querier (see
//! `FieldHintOp`, `HintAnswer` and `Secp256k1Element`), which differs between the RISC-V guest and
//! native builds. The run mode tells which the querier is, and whether the guest is answered too,
//! when a native run records the prover input (see `oracle_provider::RunMode`): [`FieldOpsQuery`]
//! and [`NativeFieldOpsQuery`] are the same processor under the names of the two setups.

use basic_system::system_functions::field_ops::{
    read_field_hint_request, FieldHintOp, HintAnswer, HintTarget, FIELD_OPS_ADVISE_QUERY_ID,
};
use oracle_provider::{OracleQueryProcessor, RunMode};
use zk_ee::oracle::memory_io::host::QuerierMemory;
mod impls;

pub(crate) use impls::Operand;

/// The op and the operand of the hint request at `input_word` in the memory of `querier`
fn read_request<'m>(
    memory: &'m dyn QuerierMemory,
    input_word: usize,
    querier: HintTarget,
) -> (FieldHintOp, Operand<'m>) {
    let (op, address, len_u32_words) =
        read_field_hint_request(memory, input_word).expect("must read the field hint request");
    let Some(op) = FieldHintOp::parse_u32(op) else {
        panic!("Unknown field hint op {op}");
    };
    (op, Operand::new(memory, querier, address, len_u32_words))
}

/// The responses of a processor to a hint query: the answer as a querier in this process reads
/// it (a native run), and as the RISC-V guest reads it, for those of the runs it is produced for
struct Responses<'a> {
    native_run: Option<&'a mut Vec<u32>>,
    guest_run: Option<&'a mut Vec<u32>>,
}

impl Responses<'_> {
    fn write<T: HintAnswer>(&mut self, answer: &T) {
        if let Some(response) = self.native_run.as_deref_mut() {
            answer.write(HintTarget::Native, response);
        }
        if let Some(response) = self.guest_run.as_deref_mut() {
            answer.write(HintTarget::Guest, response);
        }
    }
}

/// The answer to the hint `op` on `operand`
fn answer(op: FieldHintOp, operand: &Operand, responses: &mut Responses) {
    match op {
        FieldHintOp::Secp256k1BaseFieldSqrt => {
            responses.write(&impls::secp256k1_base_field_sqrt(operand))
        }
        FieldHintOp::Secp256k1BaseFieldInverse => {
            responses.write(&impls::secp256k1_base_field_inverse(operand))
        }
        FieldHintOp::Secp256k1ScalarFieldInverse => {
            responses.write(&impls::secp256k1_scalar_field_inverse(operand))
        }
        FieldHintOp::Bn254BaseFieldInverse => {
            responses.write(&impls::inverse::<crypto::bn254::Fq>(operand))
        }
        FieldHintOp::Bn254Fq12Inverse => {
            responses.write(&impls::inverse::<crypto::bn254::Fq12>(operand))
        }
        FieldHintOp::Bls12381BaseFieldSqrt => {
            responses.write(&impls::bls12_381_base_field_sqrt(operand))
        }
        FieldHintOp::Bls12381BaseFieldInverse => {
            responses.write(&impls::inverse::<crypto::bls12_381::Fq>(operand))
        }
        FieldHintOp::Bls12381Fq12Inverse => {
            responses.write(&impls::inverse::<crypto::bls12_381::Fq12>(operand))
        }
        FieldHintOp::Bn254PairingResidueWitness => {
            impls::bn254_pairing_residue_witness(operand, false, responses)
        }
        FieldHintOp::Bn254G2PairingInverses => impls::bn254_g2_pairing_inverses(operand, responses),
        FieldHintOp::Bls12381KzgResidueWitness => {
            impls::bls12_381_kzg_residue_witness(operand, false, responses)
        }
        _ => {
            panic!("Unknown field hint op {}", op as u32);
        }
    }
}

/// The answer, as a querier in this process reads it, to its pairing request (bn254 or KZG) at
/// `input_word` that claims a non-identity, with the inverse the exact path needs, whatever the
/// product is: for tests of that path
pub fn not_identity_claim(memory: &dyn QuerierMemory, input_word: usize) -> Vec<u32> {
    let mut response = Vec::new();
    let mut responses = Responses {
        native_run: Some(&mut response),
        guest_run: None,
    };
    match read_request(memory, input_word, HintTarget::Native) {
        (FieldHintOp::Bn254PairingResidueWitness, operand) => {
            impls::bn254_pairing_residue_witness(&operand, true, &mut responses)
        }
        (FieldHintOp::Bls12381KzgResidueWitness, operand) => {
            impls::bls12_381_kzg_residue_witness(&operand, true, &mut responses)
        }
        (op, _) => panic!("not a pairing request: {op:?}"),
    }
    response
}

/// Serves a hint query for the runs of `mode`
fn process_field_hint_query(
    query_id: u32,
    input_word: usize,
    memory: &dyn QuerierMemory,
    mode: RunMode,
    native_run_responses: &mut Vec<u32>,
    guest_run_responses: &mut Vec<u32>,
) {
    assert_eq!(query_id, FIELD_OPS_ADVISE_QUERY_ID);
    // the querier, whose memory holds the request and the operand, in its representation
    let (querier, word_size) = match mode {
        RunMode::NativeRunOnly | RunMode::NativeRunSavingForRiscV => {
            (HintTarget::Native, size_of::<usize>())
        }
        RunMode::RiscVRun => (HintTarget::Guest, size_of::<u32>()),
    };
    assert_eq!(
        memory.word_size(),
        word_size,
        "not the querier of the run mode {mode:?}"
    );
    let mut responses = Responses {
        native_run: mode
            .produces_native_run_responses()
            .then_some(native_run_responses),
        guest_run: mode
            .produces_guest_run_responses()
            .then_some(guest_run_responses),
    };
    let (op, operand) = read_request(memory, input_word, querier);
    answer(op, &operand, &mut responses);
}

/// Serves the field hints, set up for the RISC-V guest in the simulated machine.
#[derive(Default)]
pub struct FieldOpsQuery;

impl OracleQueryProcessor for FieldOpsQuery {
    fn supported_memory_query_ids(&self) -> Vec<u32> {
        vec![FIELD_OPS_ADVISE_QUERY_ID]
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
        process_field_hint_query(
            query_id,
            input_word,
            memory,
            mode,
            native_run_responses,
            guest_run_responses,
        );
    }
}

/// Serves the field hints, set up for a querier in this process (host execution).
#[derive(Default)]
pub struct NativeFieldOpsQuery;

impl OracleQueryProcessor for NativeFieldOpsQuery {
    fn supported_memory_query_ids(&self) -> Vec<u32> {
        vec![FIELD_OPS_ADVISE_QUERY_ID]
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
        process_field_hint_query(
            query_id,
            input_word,
            memory,
            mode,
            native_run_responses,
            guest_run_responses,
        );
    }
}

#[cfg(test)]
mod native_query_tests {
    use super::*;
    use crate::test_utils::TestMemorySource;
    use basic_system::system_functions::curve_hints::guest_layout;
    use basic_system::system_functions::field_ops::{FieldOpsHint64, Secp256k1Element};
    use crypto::ark_ec::AffineRepr;
    use crypto::ark_ff::{AdditiveGroup, BigInteger, Field, PrimeField};
    use crypto::secp256k1::field::FieldElement;
    use oracle_provider::GuestMemory;
    use zk_ee::oracle::memory_io::host::NativeQuerierMemory;

    /// The request for the hint `op` on `operand`, of a querier in this process
    fn request<T: ?Sized>(op: FieldHintOp, operand: &T) -> FieldOpsHint64 {
        FieldOpsHint64 {
            op: op as u32,
            src_ptr: core::ptr::from_ref(operand)
                .cast::<u8>()
                .expose_provenance() as u64,
            src_len_u32_words: (size_of_val(operand) / 4) as u32,
        }
    }

    /// The responses of the native run and of the guest run of `mode` to the hint request `hint` of a
    /// querier in this process
    fn process(mode: RunMode, hint: &FieldOpsHint64) -> (Vec<u32>, Vec<u32>) {
        // SAFETY: the request and the operand it points to are alive during the call
        let memory = unsafe { NativeQuerierMemory::new() };
        let (mut native_run, mut guest_run) = (Vec::new(), Vec::new());
        NativeFieldOpsQuery.process_memory_query(
            FIELD_OPS_ADVISE_QUERY_ID,
            core::ptr::from_ref(hint).expose_provenance(),
            &memory,
            mode,
            &mut native_run,
            &mut guest_run,
        );
        (native_run, guest_run)
    }

    /// The response of a run on the RISC-V guest to its request for the hint `op` on the words
    /// `operand` (at 0x1000 of its memory)
    fn process_guest(op: FieldHintOp, operand: &[u32]) -> Vec<u32> {
        // guest memory: the request (a `FieldOpsHint`) at 0x100, the operand at 0x1000
        let mut memory = TestMemorySource::default();
        memory.insert_u32(0x100, op as u32);
        memory.insert_u32(0x104, 0x1000);
        memory.insert_u32(0x108, operand.len() as u32);
        for (i, word) in operand.iter().enumerate() {
            memory.insert_u32(0x1000 + 4 * i as u32, *word);
        }
        let (mut native_run, mut guest_run) = (Vec::new(), Vec::new());
        FieldOpsQuery.process_memory_query(
            FIELD_OPS_ADVISE_QUERY_ID,
            0x100,
            &GuestMemory(&memory),
            RunMode::RiscVRun,
            &mut native_run,
            &mut guest_run,
        );
        assert!(native_run.is_empty());
        guest_run
    }

    /// An element and its inverse
    fn element_and_inverse() -> (FieldElement, FieldElement) {
        let x = FieldElement::from_bytes(&[7; 32]).unwrap();
        let mut inverse = x;
        inverse.invert_in_place();
        (x, inverse)
    }

    #[test]
    fn a_native_querier_is_answered_for_the_runs_of_the_mode() {
        let (x, inverse) = element_and_inverse();
        let hint = request(FieldHintOp::Secp256k1BaseFieldInverse, &x);

        let (native_run, guest_run) = process(RunMode::NativeRunOnly, &hint);
        assert_eq!(native_run, inverse.to_canonical_words());
        assert!(guest_run.is_empty());

        // a native run that records the prover input also answers the guest
        let (native_run, guest_run) = process(RunMode::NativeRunSavingForRiscV, &hint);
        assert_eq!(native_run, inverse.to_canonical_words());
        assert_eq!(guest_run, inverse.to_montgomery_words());
    }

    #[test]
    fn the_guest_is_answered_in_a_risc_v_run() {
        let (x, inverse) = element_and_inverse();
        let guest_run = process_guest(
            FieldHintOp::Secp256k1BaseFieldInverse,
            &x.to_montgomery_words(),
        );
        assert_eq!(guest_run, inverse.to_montgomery_words());
    }

    /// The limbs of `x` in the memory of the RISC-V guest: its Montgomery form with
    /// `R = 2^(64 limbs)`, plus `extra` moduli (another representative)
    fn guest_limbs<F: PrimeField>(x: F, limbs: usize, extra: u64) -> Vec<u32> {
        let r = F::from(2u64).pow([64 * limbs as u64]);
        let mut value = (x * r).into_bigint();
        for _ in 0..extra {
            value.add_with_carry(&F::MODULUS);
        }
        let mut words: Vec<u32> = value
            .as_ref()
            .iter()
            .flat_map(|limb| [*limb as u32, (*limb >> 32) as u32])
            .collect();
        words.resize(2 * limbs, 0);
        words
    }

    /// Both answers to the guest agree: from the operand of a native run, and from the same
    /// operand in the memory of the guest
    fn assert_same_guest_answers<T: ?Sized>(op: FieldHintOp, native: &T, guest: &[u32]) {
        let (_, from_native) = process(RunMode::NativeRunSavingForRiscV, &request(op, native));
        assert_eq!(process_guest(op, guest), from_native);
    }

    #[test]
    fn guest_elements_are_read_in_their_montgomery_form() {
        let x = crypto::bn254::Fq::from(0x1234_5678u64).square();
        for extra in [0, 1] {
            let guest = guest_limbs(x, 4, extra);
            assert_same_guest_answers(FieldHintOp::Bn254BaseFieldInverse, &x, &guest);
        }
        let y = crypto::bls12_381::Fq::from(0x1234_5678u64).square();
        for extra in [0, 1] {
            let guest = guest_limbs(y, 8, extra);
            assert_same_guest_answers(FieldHintOp::Bls12381BaseFieldInverse, &y, &guest);
            assert_same_guest_answers(FieldHintOp::Bls12381BaseFieldSqrt, &y, &guest);
        }
        let f = crypto::bn254::Fq12::new(
            crypto::bn254::Fq6::new(
                crypto::bn254::Fq2::new(x, x.double()),
                crypto::bn254::Fq2::new(x.square(), x + x.square()),
                crypto::bn254::Fq2::new(x.inverse().unwrap(), x),
            ),
            crypto::bn254::Fq6::new(
                crypto::bn254::Fq2::new(x.double().square(), x),
                crypto::bn254::Fq2::new(x, x.double()),
                crypto::bn254::Fq2::new(x.square().square(), x),
            ),
        );
        // the components in order, `c0` first, recursively
        let mut guest = Vec::new();
        for fq6 in [&f.c0, &f.c1] {
            for fq2 in [&fq6.c0, &fq6.c1, &fq6.c2] {
                guest.extend(guest_limbs(fq2.c0, 4, 0));
                guest.extend(guest_limbs(fq2.c1, 4, 1));
            }
        }
        assert_same_guest_answers(FieldHintOp::Bn254Fq12Inverse, &f, &guest);
    }

    /// The words of an affine point in the memory of the RISC-V guest
    fn guest_point(
        layout: guest_layout::AffinePoint,
        x: Vec<u32>,
        y: Vec<u32>,
        infinity: bool,
    ) -> Vec<u32> {
        let mut words = vec![0u32; layout.size / 4];
        words[layout.x / 4..][..x.len()].copy_from_slice(&x);
        words[layout.y / 4..][..y.len()].copy_from_slice(&y);
        words[layout.infinity / 4] = u32::from(infinity) << (8 * (layout.infinity % 4));
        words
    }

    fn guest_fq2(x: &crypto::bn254::Fq2) -> Vec<u32> {
        [guest_limbs(x.c0, 4, 0), guest_limbs(x.c1, 4, 0)].concat()
    }

    #[test]
    fn guest_points_are_read_where_the_layout_puts_them() {
        use crypto::{bls12_381, bn254};
        use guest_layout::*;

        let g2 = bn254::G2Affine::generator();
        let guest_g2 = guest_point(BN254_G2_AFFINE, guest_fq2(&g2.x), guest_fq2(&g2.y), false);
        assert_same_guest_answers(FieldHintOp::Bn254G2PairingInverses, &g2, &guest_g2);

        let g1 = (bn254::G1Affine::generator() * bn254::Fr::from(5u64)).into();
        let g1: bn254::G1Affine = g1;
        let guest_g1 = guest_point(
            BN254_G1_AFFINE,
            guest_limbs(g1.x, 4, 0),
            guest_limbs(g1.y, 4, 1),
            false,
        );
        let mut guest_pair = vec![0u32; BN254_PAIR_SIZE / 4];
        guest_pair[BN254_PAIR_G1 / 4..][..guest_g1.len()].copy_from_slice(&guest_g1);
        guest_pair[BN254_PAIR_G2 / 4..][..guest_g2.len()].copy_from_slice(&guest_g2);
        let pairs = [(g1, g2), (g1, g2)];
        assert_same_guest_answers(
            FieldHintOp::Bn254PairingResidueWitness,
            &pairs[..],
            &[guest_pair.clone(), guest_pair].concat(),
        );

        let p = bls12_381::G1Affine::generator();
        let guest_p = guest_point(
            BLS12_381_G1_AFFINE,
            guest_limbs(p.x, 8, 0),
            guest_limbs(p.y, 8, 0),
            false,
        );
        // the point at infinity is the flag
        let infinity = bls12_381::G1Affine::identity();
        let guest_infinity = guest_point(BLS12_381_G1_AFFINE, vec![], vec![], true);
        assert_same_guest_answers(
            FieldHintOp::Bls12381KzgResidueWitness,
            &[p, infinity],
            &[guest_p, guest_infinity].concat(),
        );
    }

    #[test]
    #[should_panic(expected = "not the querier of the run mode")]
    fn a_querier_in_this_process_is_not_answered_in_a_risc_v_run() {
        let (x, _) = element_and_inverse();
        let _ = process(
            RunMode::RiscVRun,
            &request(FieldHintOp::Secp256k1BaseFieldInverse, &x),
        );
    }

    #[test]
    #[should_panic]
    fn native_field_ops_query_rejects_null_query_pointer() {
        // SAFETY: nothing is read at a null address
        let memory = unsafe { NativeQuerierMemory::new() };
        NativeFieldOpsQuery.process_memory_query(
            FIELD_OPS_ADVISE_QUERY_ID,
            0,
            &memory,
            RunMode::NativeRunOnly,
            &mut Vec::new(),
            &mut Vec::new(),
        );
    }

    #[test]
    #[should_panic]
    fn native_field_ops_query_rejects_null_operand_pointer() {
        let hint = FieldOpsHint64 {
            op: FieldHintOp::Secp256k1BaseFieldInverse as u32,
            src_ptr: 0,
            src_len_u32_words: 8,
        };
        let _ = process(RunMode::NativeRunOnly, &hint);
    }
}
