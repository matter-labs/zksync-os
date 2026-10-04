#![feature(allocator_api)]
use basic_system::system_functions::{
    bls12_381::Bls12381PairingCheckPrecompile,
    field_ops::{read_field_hint_request, FieldHintOp, HintAnswer, HintTarget},
};
use callable_oracles::field_hints::{not_identity_claim, NativeFieldOpsQuery};
use crypto::{
    ark_ec::AffineRepr,
    ark_ff::{BigInteger, One, PrimeField},
    bls12_381::{Fq, Fq12, Fq6, Fr, G1Affine, G2Affine},
};
use oracle_provider::{OracleQueryProcessor, RunMode, ZkEENonDeterminismSource};
use zk_ee::{
    oracle::memory_io::host::QuerierMemory,
    reference_implementations::{BaseResources, DecreasingNative},
    system::{base_system_functions::SystemFunctionExt, logger::NullLogger, Resource},
};

struct PairingOracle {
    corrupt: bool,
    force_exact: bool,
    false_identity: bool,
}
impl OracleQueryProcessor for PairingOracle {
    fn supported_memory_query_ids(&self) -> Vec<u32> {
        NativeFieldOpsQuery.supported_memory_query_ids()
    }
    fn process_memory_query(
        &mut self,
        id: u32,
        word: usize,
        memory: &dyn QuerierMemory,
        mode: RunMode,
        native: &mut Vec<u32>,
        guest: &mut Vec<u32>,
    ) {
        let (op, _, _) = read_field_hint_request(memory, word).unwrap();
        if op == FieldHintOp::Bls12381PairingResidueWitness as u32 {
            let mut answer = if self.false_identity {
                let mut words = Vec::new();
                (true, (Fq12::one(), Fq6::one())).write(HintTarget::Native, &mut words);
                words
            } else if self.force_exact {
                not_identity_claim(memory, word)
            } else {
                let mut result = Vec::new();
                NativeFieldOpsQuery.process_memory_query(
                    id,
                    word,
                    memory,
                    mode,
                    &mut result,
                    guest,
                );
                result
            };
            if self.corrupt {
                answer[1] ^= 1;
            }
            native.extend(answer);
        } else {
            NativeFieldOpsQuery.process_memory_query(id, word, memory, mode, native, guest);
        }
    }
}
fn fq(input: Fq, bytes: &mut Vec<u8>) {
    bytes.extend([0u8; 16]);
    bytes.extend(input.into_bigint().to_bytes_be());
}
fn input(pairs: &[(G1Affine, G2Affine)]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for (p, q) in pairs {
        if p.infinity {
            bytes.extend([0u8; 128]);
        } else {
            fq(p.x, &mut bytes);
            fq(p.y, &mut bytes);
        }
        if q.infinity {
            bytes.extend([0u8; 256]);
        } else {
            for value in [q.x.c0, q.x.c1, q.y.c0, q.y.c1] {
                fq(value, &mut bytes);
            }
        }
    }
    bytes
}
fn execute<const ADVICE: bool>(
    pairs: &[(G1Affine, G2Affine)],
    corrupt: bool,
    exact: bool,
    false_identity: bool,
) -> Vec<u8> {
    let mut oracle = ZkEENonDeterminismSource::default();
    oracle.add_external_processor(PairingOracle {
        corrupt,
        force_exact: exact,
        false_identity,
    });
    let mut output = Vec::new();
    let mut resources = BaseResources::<DecreasingNative>::FORMAL_INFINITE;
    Bls12381PairingCheckPrecompile::<ADVICE>::execute(
        &input(pairs),
        &mut output,
        &mut resources,
        &mut oracle,
        &mut NullLogger,
        std::alloc::Global,
    )
    .unwrap();
    output
}
#[test]
fn arbitrary_pairing_hints_match_exact_pairing_and_handle_infinity() {
    let (p, q) = (G1Affine::generator(), G2Affine::generator());
    let p5: G1Affine = (p * Fr::from(5u64)).into();
    let q5: G2Affine = (q * Fr::from(5u64)).into();
    for pairs in [
        vec![(p, q)],
        vec![(p, q), (-p, q)],
        vec![(G1Affine::identity(), q)],
        vec![(p, G2Affine::identity())],
        vec![(p, q); 5],
        vec![(p5, q), (-p, q5)],
        vec![(p, q5)],
        vec![(p5, q), (-p, q), (-p, q), (-p, q), (-p, q), (-p, q)],
    ] {
        let expected = execute::<false>(&pairs, false, false, false);
        assert_eq!(execute::<true>(&pairs, false, false, false), expected);
        assert_eq!(execute::<true>(&pairs, false, true, false), expected);
    }
}
#[test]
#[should_panic(expected = "residue witness")]
fn corrupt_identity_witness_is_rejected() {
    let (p, q) = (G1Affine::generator(), G2Affine::generator());
    execute::<true>(&[(p, q), (-p, q)], true, false, false);
}
#[test]
#[should_panic(expected = "inverse hint is wrong")]
fn corrupt_nonidentity_inverse_is_rejected() {
    execute::<true>(
        &[(G1Affine::generator(), G2Affine::generator())],
        true,
        false,
        false,
    );
}

#[test]
#[should_panic(expected = "residue witness")]
fn a_false_identity_claim_cannot_accept_a_nonidentity_pairing() {
    execute::<true>(
        &[(G1Affine::generator(), G2Affine::generator())],
        false,
        false,
        true,
    );
}
