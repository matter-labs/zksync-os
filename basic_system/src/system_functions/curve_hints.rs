//! Field inversions and square roots of the bn254 and bls12-381 base fields, and inversions of
//! their degree-12 extensions, taken from oracle hints and checked: an inverse with one
//! multiplication, a square root with one squaring. Also the residue witnesses of the pairing
//! checks (`crypto::residue_witness`, after Novakovic and Eagen, "On Proving Pairings",
//! <https://eprint.iacr.org/2024/640>), which replace the final exponentiations; those hints
//! are not checked on their own: the pairing check itself fails for any witness of a
//! non-identity product (see the soundness argument in that module). The exponentiation-based versions cost
//! hundreds of multiplications, which dominates the cost of the point operations that end
//! with one (an affine result, the easy part of a final exponentiation, a point decompression).
//!
//! A wrong hint fails an assertion: hints come from the prover's own oracle, never from
//! external input, so a bad one is a broken prover rather than a case to recover from.

use crate::system_functions::field_ops::{query_field_hint, FieldHintOp};
use crypto::ark_ff::{Field, One, PrimeField, Zero};
use zk_ee::oracle::usize_serialization::UsizeDeserializable;
use zk_ee::oracle::IOOracle;
use zk_ee::utils::Bytes32;

/// The hint encoding of a field element: its base prime field components in a fixed order
/// (for a tower `c0` before `c1` before `c2`, recursively), each as its canonical integer in
/// `LIMBS` 64-bit little-endian limbs (zero-padded above the modulus). Shared with the host
/// processors, and independent of the arkworks `to_base_prime_field_elements` order, which
/// the delegated and the native tower implementations do not agree on.
pub trait HintEncoding: Field {
    /// Limbs per base prime field element: 4 for bn254, 8 for bls12-381 (48 bytes in 64)
    const LIMBS: usize;
    /// Base prime field components of an element
    const COMPONENTS: usize;
    /// Limbs of a whole element
    const TOTAL_LIMBS: usize = Self::LIMBS * Self::COMPONENTS;

    fn write_components(&self, out: &mut [Self::BasePrimeField]);
    fn from_components(components: &[Self::BasePrimeField]) -> Self;
}

macro_rules! prime_field_encoding {
    ($field:ty, $limbs:expr) => {
        impl HintEncoding for $field {
            const LIMBS: usize = $limbs;
            const COMPONENTS: usize = 1;

            fn write_components(&self, out: &mut [Self::BasePrimeField]) {
                out[0] = *self;
            }

            fn from_components(components: &[Self::BasePrimeField]) -> Self {
                components[0]
            }
        }
    };
}

macro_rules! fp12_encoding {
    ($field:ty, $fp6:ty, $fp2:ty, $limbs:expr) => {
        impl HintEncoding for $field {
            const LIMBS: usize = $limbs;
            const COMPONENTS: usize = 12;

            fn write_components(&self, out: &mut [Self::BasePrimeField]) {
                let mut i = 0;
                for fp6 in [&self.c0, &self.c1] {
                    for fp2 in [&fp6.c0, &fp6.c1, &fp6.c2] {
                        out[i] = fp2.c0;
                        out[i + 1] = fp2.c1;
                        i += 2;
                    }
                }
            }

            fn from_components(c: &[Self::BasePrimeField]) -> Self {
                let fp2 = |i: usize| <$fp2>::new(c[i], c[i + 1]);
                let fp6 = |i: usize| <$fp6>::new(fp2(i), fp2(i + 2), fp2(i + 4));
                Self::new(fp6(0), fp6(6))
            }
        }
    };
}

macro_rules! fp6_encoding {
    ($field:ty, $fp2:ty, $limbs:expr) => {
        impl HintEncoding for $field {
            const LIMBS: usize = $limbs;
            const COMPONENTS: usize = 6;

            fn write_components(&self, out: &mut [Self::BasePrimeField]) {
                for (i, fp2) in [&self.c0, &self.c1, &self.c2].into_iter().enumerate() {
                    out[2 * i] = fp2.c0;
                    out[2 * i + 1] = fp2.c1;
                }
            }

            fn from_components(c: &[Self::BasePrimeField]) -> Self {
                let fp2 = |i: usize| <$fp2>::new(c[i], c[i + 1]);
                Self::new(fp2(0), fp2(2), fp2(4))
            }
        }
    };
}

prime_field_encoding!(crypto::bn254::Fq, 4);
prime_field_encoding!(crypto::bls12_381::Fq, 8);
fp6_encoding!(crypto::bn254::Fq6, crypto::bn254::Fq2, 4);
fp6_encoding!(crypto::bls12_381::Fq6, crypto::bls12_381::Fq2, 8);
fp12_encoding!(
    crypto::bn254::Fq12,
    crypto::bn254::Fq6,
    crypto::bn254::Fq2,
    4
);
fp12_encoding!(
    crypto::bls12_381::Fq12,
    crypto::bls12_381::Fq6,
    crypto::bls12_381::Fq2,
    8
);

/// `value` in the hint encoding; `limbs` must hold `F::TOTAL_LIMBS`
pub fn encode<F: HintEncoding>(value: &F, limbs: &mut [u64]) {
    debug_assert_eq!(limbs.len(), F::TOTAL_LIMBS);
    let mut components = [F::BasePrimeField::zero(); 12];
    value.write_components(&mut components[..F::COMPONENTS]);
    for (component, out) in components[..F::COMPONENTS]
        .iter()
        .zip(limbs.chunks_exact_mut(F::LIMBS))
    {
        let repr = component.into_bigint();
        let repr = repr.as_ref();
        debug_assert!(repr.len() <= F::LIMBS);
        out.fill(0);
        out[..repr.len()].copy_from_slice(repr);
    }
}

/// The element encoded in `limbs` (`F::TOTAL_LIMBS` of them), `None` unless every component is
/// canonical, i.e. below the modulus with zero padding
pub fn decode<F: HintEncoding>(limbs: &[u64]) -> Option<F> {
    debug_assert_eq!(limbs.len(), F::TOTAL_LIMBS);
    let mut components = [F::BasePrimeField::zero(); 12];
    for (component, chunk) in components[..F::COMPONENTS]
        .iter_mut()
        .zip(limbs.chunks_exact(F::LIMBS))
    {
        let mut repr = <F::BasePrimeField as PrimeField>::BigInt::default();
        let width = repr.as_ref().len().min(F::LIMBS);
        if chunk[width..].iter().any(|limb| *limb != 0) {
            return None;
        }
        repr.as_mut()[..width].copy_from_slice(&chunk[..width]);
        *component = F::BasePrimeField::from_bigint(repr)?;
    }
    Some(F::from_components(&components[..F::COMPONENTS]))
}

/// The oracle reads and writes operands as words of `Bytes32`, which are also aligned as the
/// query needs; these are their byte and 64-bit limb views.
fn as_bytes<const N: usize>(words: &[Bytes32; N]) -> &[u8] {
    // SAFETY: `Bytes32` is a plain 32-byte value, so `N` of them are `32 N` initialized bytes
    unsafe { core::slice::from_raw_parts(words.as_ptr().cast::<u8>(), 32 * N) }
}

fn as_limbs<const N: usize>(words: &[Bytes32; N]) -> &[u64] {
    // SAFETY: `Bytes32` is a plain, 8-byte aligned 32-byte value, so `N` of them are `4 N`
    // initialized `u64`
    unsafe { core::slice::from_raw_parts(words.as_ptr().cast::<u64>(), 4 * N) }
}

fn as_limbs_mut<const N: usize>(words: &mut [Bytes32; N]) -> &mut [u64] {
    // SAFETY: as in `as_limbs`, and the words are exclusively borrowed
    unsafe { core::slice::from_raw_parts_mut(words.as_mut_ptr().cast::<u64>(), 4 * N) }
}

/// `value` as the hint operand
fn to_words<F: HintEncoding, const N: usize>(value: &F) -> [Bytes32; N] {
    debug_assert_eq!(4 * N, F::TOTAL_LIMBS);
    let mut words = [Bytes32::ZERO; N];
    encode(value, as_limbs_mut(&mut words));
    words
}

/// The element the oracle answered with; panics if the answer is not a canonical element
fn from_words<F: HintEncoding, const N: usize>(words: &[Bytes32; N]) -> F {
    decode(as_limbs(words)).expect("the hint is a canonical field element")
}

/// `a^-1` from a hint, `None` for a zero `a`
fn inverse_from_hint<O: IOOracle, F: HintEncoding, const N: usize>(
    oracle: &mut O,
    op: FieldHintOp,
    a: &F,
) -> Option<F>
where
    [Bytes32; N]: UsizeDeserializable,
{
    if a.is_zero() {
        return None;
    }
    let input: [Bytes32; N] = to_words(a);
    let hint: [Bytes32; N] = query_field_hint(oracle, op, as_bytes(&input));
    let inverse: F = from_words(&hint);
    let mut product = *a;
    product *= &inverse;
    assert!(product.is_one(), "the field inverse hint is wrong");
    Some(inverse)
}

/// `a^-1` in the bn254 base field, `None` for a zero `a`
pub fn bn254_fq_inverse<O: IOOracle>(
    oracle: &mut O,
    a: &crypto::bn254::Fq,
) -> Option<crypto::bn254::Fq> {
    inverse_from_hint::<_, _, 1>(oracle, FieldHintOp::Bn254BaseFieldInverse, a)
}

/// `f^-1` in the bn254 degree-12 extension field, `None` for a zero `f`
pub fn bn254_fq12_inverse<O: IOOracle>(
    oracle: &mut O,
    f: &crypto::bn254::Fq12,
) -> Option<crypto::bn254::Fq12> {
    inverse_from_hint::<_, _, 12>(oracle, FieldHintOp::Bn254Fq12Inverse, f)
}

/// `a^-1` in the bls12-381 base field, `None` for a zero `a`
pub fn bls12_381_fq_inverse<O: IOOracle>(
    oracle: &mut O,
    a: &crypto::bls12_381::Fq,
) -> Option<crypto::bls12_381::Fq> {
    inverse_from_hint::<_, _, 2>(oracle, FieldHintOp::Bls12381BaseFieldInverse, a)
}

/// `f^-1` in the bls12-381 degree-12 extension field, `None` for a zero `f`
pub fn bls12_381_fq12_inverse<O: IOOracle>(
    oracle: &mut O,
    f: &crypto::bls12_381::Fq12,
) -> Option<crypto::bls12_381::Fq12> {
    inverse_from_hint::<_, _, 24>(oracle, FieldHintOp::Bls12381Fq12Inverse, f)
}

// The square root check relies on `-1` being a quadratic non-residue, i.e. on `p = 3 mod 4`
const _: () = assert!(<crypto::bls12_381::Fq as PrimeField>::MODULUS.0[0] & 3 == 3);
const _: () = assert!(<crypto::bn254::Fq as PrimeField>::MODULUS.0[0] & 3 == 3);

/// A square root of `a` in the bls12-381 base field from a hint, `None` if `a` is not a
/// square. Which of the two roots is returned is up to the oracle.
pub fn bls12_381_fq_sqrt<O: IOOracle>(
    oracle: &mut O,
    a: &crypto::bls12_381::Fq,
) -> Option<crypto::bls12_381::Fq> {
    use crypto::bls12_381::Fq;
    if a.is_zero() {
        return Some(Fq::zero());
    }
    let input: [Bytes32; 2] = to_words(a);
    let (candidate, is_non_residue): ([Bytes32; 2], bool) =
        query_field_hint(oracle, FieldHintOp::Bls12381BaseFieldSqrt, as_bytes(&input));
    let candidate: Fq = from_words(&candidate);
    let mut square = candidate;
    square.square_in_place();
    if is_non_residue {
        // `candidate² = -a` proves that `a` is not a square, as `-1` is not one
        square += a;
        assert!(
            square.is_zero(),
            "the non-residue proof of the square root hint is wrong"
        );
        None
    } else {
        assert!(square == *a, "the square root hint is wrong");
        Some(candidate)
    }
}

/// The prover's claim about a pairing product `f`, with the witness it takes to settle it
pub enum PairingClaim<F12, F6> {
    /// `f` is the identity: the residue witness `c`, `d = c^-1` (for bn254; only `d` for
    /// bls12-381, where `c` is unused) and the scaling factor `s`. The claim is settled by the
    /// residue check; a failed check is a broken prover and panics.
    Identity { c: F12, d: F12, s: F6 },
    /// `f` is not the identity: `f^-1`, for the exact final exponentiation that settles the
    /// claim (it would find an identity all the same, so this claim cannot change the answer)
    NotIdentity { f_inverse: F12 },
}

/// The claim and witness of the bn254 pairing product over `pairs` (all non-degenerate,
/// validated) from the oracle, for `crypto::residue_witness::bn254::check` on the Miller loop
/// started at `d`. `c d = 1` is checked here: the loop multiplies by `c` at the negative digits
/// of its count, and the check is only sound for `c = d^-1` (otherwise `f s` would be
/// `d^a c^b` with exponents not divisible by `r`).
pub fn bn254_pairing_residue_witness<O: IOOracle, A: core::alloc::Allocator>(
    oracle: &mut O,
    pairs: &[(crypto::bn254::G1Affine, crypto::bn254::G2Affine)],
    allocator: A,
) -> PairingClaim<crypto::bn254::Fq12, crypto::bn254::Fq6> {
    // 6 base field elements of 4 limbs per pair
    let mut input = alloc::vec::Vec::with_capacity_in(6 * pairs.len(), allocator);
    input.resize(6 * pairs.len(), Bytes32::ZERO);
    // SAFETY: as in `as_limbs`, `Bytes32` is 8-byte aligned plain data
    let limbs = unsafe {
        core::slice::from_raw_parts_mut(input.as_mut_ptr().cast::<u64>(), 24 * pairs.len())
    };
    for ((g1, g2), out) in pairs.iter().zip(limbs.as_chunks_mut::<24>().0) {
        encode(&g1.x, &mut out[0..4]);
        encode(&g1.y, &mut out[4..8]);
        encode(&g2.x.c0, &mut out[8..12]);
        encode(&g2.x.c1, &mut out[12..16]);
        encode(&g2.y.c0, &mut out[16..20]);
        encode(&g2.y.c1, &mut out[20..24]);
    }
    // identity flag, then `c, d, s` for an identity or `f^-1` and zeros otherwise
    let (is_identity, (first, (d, s))): (bool, ([Bytes32; 12], ([Bytes32; 12], [Bytes32; 6]))) =
        query_field_hint(oracle, FieldHintOp::Bn254PairingResidueWitness, unsafe {
            core::slice::from_raw_parts(input.as_ptr().cast::<u8>(), 32 * input.len())
        });
    if is_identity {
        let (c, d, s): (crypto::bn254::Fq12, crypto::bn254::Fq12, crypto::bn254::Fq6) =
            (from_words(&first), from_words(&d), from_words(&s));
        assert!(
            (c * d).is_one(),
            "the residue witness hint is not an inverse pair"
        );
        PairingClaim::Identity { c, d, s }
    } else {
        PairingClaim::NotIdentity {
            f_inverse: from_words(&first),
        }
    }
}

/// The claim and witness of `e(p1, G2) e(p2, tau G2)` (the KZG proof check) from the oracle,
/// for `crypto::residue_witness::bls12_381::check` on the Miller loop started at `d` (`c` is
/// not needed by that loop and is left zero)
pub fn bls12_381_kzg_residue_witness<O: IOOracle>(
    oracle: &mut O,
    p1: &crypto::bls12_381::G1Affine,
    p2: &crypto::bls12_381::G1Affine,
) -> PairingClaim<crypto::bls12_381::Fq12, crypto::bls12_381::Fq6> {
    let mut input = [Bytes32::ZERO; 8];
    {
        let limbs = as_limbs_mut(&mut input);
        encode(&p1.x, &mut limbs[0..8]);
        encode(&p1.y, &mut limbs[8..16]);
        encode(&p2.x, &mut limbs[16..24]);
        encode(&p2.y, &mut limbs[24..32]);
    }
    // identity flag, then `d, s` for an identity or `f^-1` and zeros otherwise
    let (is_identity, (first, s)): (bool, ([Bytes32; 24], [Bytes32; 12])) = query_field_hint(
        oracle,
        FieldHintOp::Bls12381KzgResidueWitness,
        as_bytes(&input),
    );
    if is_identity {
        PairingClaim::Identity {
            c: crypto::bls12_381::Fq12::zero(),
            d: from_words(&first),
            s: from_words(&s),
        }
    } else {
        PairingClaim::NotIdentity {
            f_inverse: from_words(&first),
        }
    }
}

/// The hinted inverse of the Miller loop output `f`, checked
pub fn checked_inverse<F: Field>(f: &F, f_inverse: F) -> Option<F> {
    assert!(
        (*f * f_inverse).is_one(),
        "the Miller loop inverse hint is wrong"
    );
    Some(f_inverse)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use callable_oracles::field_hints::NativeFieldOpsQuery;
    use oracle_provider::ZkEENonDeterminismSource;

    fn oracle() -> ZkEENonDeterminismSource {
        let mut oracle = ZkEENonDeterminismSource::default();
        oracle.add_external_processor(NativeFieldOpsQuery);
        oracle
    }

    /// Deterministic field elements spread over the field: powers of a large seed
    fn elements<F: PrimeField>(count: usize) -> Vec<F> {
        let seed = F::from(0x9e37_79b9_7f4a_7c15u64) * F::from(0xd1b5_4a32_d192_ed03u64);
        let mut acc = seed;
        (0..count)
            .map(|_| {
                acc = acc * seed + F::from(3u64);
                acc
            })
            .collect()
    }

    fn extension_elements<E: Field>(count: usize) -> Vec<E> {
        let base = elements::<E::BasePrimeField>(count * E::extension_degree() as usize);
        base.chunks(E::extension_degree() as usize)
            .map(|c| E::from_base_prime_field_elems(c.iter().copied()).unwrap())
            .collect()
    }

    #[test]
    fn encoding_round_trips_and_rejects_non_canonical() {
        for a in elements::<crypto::bls12_381::Fq>(5) {
            let mut limbs = [0u64; 8];
            encode(&a, &mut limbs);
            assert_eq!(limbs[6], 0);
            assert_eq!(decode::<crypto::bls12_381::Fq>(&limbs), Some(a));
            limbs[7] = 1;
            assert_eq!(decode::<crypto::bls12_381::Fq>(&limbs), None);
        }
        for f in extension_elements::<crypto::bn254::Fq12>(3) {
            let mut limbs = [0u64; 48];
            encode(&f, &mut limbs);
            assert_eq!(decode::<crypto::bn254::Fq12>(&limbs), Some(f));
        }
        // the modulus itself is not canonical
        let mut limbs = [0u64; 4];
        limbs.copy_from_slice(<crypto::bn254::Fq as PrimeField>::MODULUS.as_ref());
        assert_eq!(decode::<crypto::bn254::Fq>(&limbs), None);
    }

    #[test]
    fn inverses_match_the_field() {
        let mut oracle = oracle();
        for a in elements::<crypto::bn254::Fq>(20) {
            assert_eq!(bn254_fq_inverse(&mut oracle, &a), a.inverse());
        }
        for f in extension_elements::<crypto::bn254::Fq12>(5) {
            assert_eq!(bn254_fq12_inverse(&mut oracle, &f), f.inverse());
        }
        for a in elements::<crypto::bls12_381::Fq>(20) {
            assert_eq!(bls12_381_fq_inverse(&mut oracle, &a), a.inverse());
        }
        for f in extension_elements::<crypto::bls12_381::Fq12>(5) {
            assert_eq!(bls12_381_fq12_inverse(&mut oracle, &f), f.inverse());
        }
        assert_eq!(
            bn254_fq_inverse(&mut oracle, &crypto::bn254::Fq::zero()),
            None
        );
        assert_eq!(
            bn254_fq12_inverse(&mut oracle, &crypto::bn254::Fq12::zero()),
            None
        );
        assert_eq!(
            bls12_381_fq_inverse(&mut oracle, &crypto::bls12_381::Fq::zero()),
            None
        );
        assert_eq!(
            bls12_381_fq12_inverse(&mut oracle, &crypto::bls12_381::Fq12::zero()),
            None
        );
        let one = crypto::bls12_381::Fq::one();
        assert_eq!(bls12_381_fq_inverse(&mut oracle, &one), Some(one));
        let minus_one = -crypto::bn254::Fq::one();
        assert_eq!(bn254_fq_inverse(&mut oracle, &minus_one), Some(minus_one));
    }

    #[test]
    fn square_roots_match_the_field() {
        let mut oracle = oracle();
        let mut squares = 0;
        for a in elements::<crypto::bls12_381::Fq>(40) {
            let hinted = bls12_381_fq_sqrt(&mut oracle, &a);
            let reference = a.sqrt();
            assert_eq!(hinted.is_some(), reference.is_some());
            if let Some(root) = hinted {
                squares += 1;
                assert_eq!(root.square(), a);
            }
        }
        assert!(squares > 5 && squares < 35, "{squares} squares of 40");
        let zero = crypto::bls12_381::Fq::zero();
        assert_eq!(bls12_381_fq_sqrt(&mut oracle, &zero), Some(zero));
        let four = crypto::bls12_381::Fq::from(4u64);
        let root = bls12_381_fq_sqrt(&mut oracle, &four).unwrap();
        assert!(
            root == crypto::bls12_381::Fq::from(2u64) || root == -crypto::bls12_381::Fq::from(2u64)
        );
    }

    /// A processor that answers the field hint queries of the given ops with a corrupted value
    /// (the low bit of the first word flipped), and the others honestly
    pub struct FlipLowBit {
        inner: NativeFieldOpsQuery,
        ops: Vec<FieldHintOp>,
        /// Corrupt the last word of the answer instead of the first payload word
        last_word: bool,
    }

    impl oracle_provider::OracleQueryProcessor for FlipLowBit {
        fn supported_query_ids(&self) -> Vec<u32> {
            self.inner.supported_query_ids()
        }

        fn process_buffered_query(
            &mut self,
            query_id: u32,
            query: Vec<usize>,
            memory: &dyn oracle_provider::RamPeek,
        ) -> Box<dyn ExactSizeIterator<Item = usize> + 'static + Send + Sync> {
            use crate::system_functions::field_ops::FieldOpsHint64;
            // SAFETY: the query is the address of the request the caller built
            let request = unsafe { (query[0] as *const FieldOpsHint64).read() };
            let op = FieldHintOp::parse_u32(request.op).unwrap();
            let mut words: Vec<usize> = self
                .inner
                .process_buffered_query(query_id, query, memory)
                .collect();
            if self.ops.contains(&op) {
                // the low limb of the first element (past the claim flag of the pairing
                // ops): still a canonical element, so the value check is what fails
                let index = match op {
                    _ if self.last_word => words.len() - 1,
                    FieldHintOp::Bn254PairingResidueWitness
                    | FieldHintOp::Bls12381KzgResidueWitness => 1,
                    _ => 0,
                };
                words[index] ^= 1;
            }
            Box::new(words.into_iter())
        }
    }

    /// A processor that claims every pairing product not to be the identity, with the
    /// (correct) inverse the slow path needs
    pub struct ClaimNotIdentity(NativeFieldOpsQuery);

    impl oracle_provider::OracleQueryProcessor for ClaimNotIdentity {
        fn supported_query_ids(&self) -> Vec<u32> {
            self.0.supported_query_ids()
        }

        fn process_buffered_query(
            &mut self,
            query_id: u32,
            query: Vec<usize>,
            memory: &dyn oracle_provider::RamPeek,
        ) -> Box<dyn ExactSizeIterator<Item = usize> + 'static + Send + Sync> {
            use crate::system_functions::field_ops::FieldOpsHint64;
            // SAFETY: the query is the address of the request the caller built
            let request = unsafe { (query[0] as *const FieldOpsHint64).read() };
            let op = FieldHintOp::parse_u32(request.op).unwrap();
            let words: Vec<usize> = match op {
                FieldHintOp::Bn254PairingResidueWitness => {
                    callable_oracles::field_hints::bn254_pairing_not_identity_claim(
                        request.src_ptr,
                        request.src_len_u32_words,
                    )
                }
                FieldHintOp::Bls12381KzgResidueWitness => {
                    callable_oracles::field_hints::bls12_381_kzg_not_identity_claim(
                        request.src_ptr,
                        request.src_len_u32_words,
                    )
                }
                _ => return self.0.process_buffered_query(query_id, query, memory),
            };
            Box::new(words.into_iter())
        }
    }

    /// An oracle that claims every pairing product not to be the identity
    pub fn non_identity_claiming_oracle() -> ZkEENonDeterminismSource {
        let mut oracle = ZkEENonDeterminismSource::default();
        oracle.add_external_processor(ClaimNotIdentity(NativeFieldOpsQuery));
        oracle
    }

    /// An oracle that corrupts the answers of `ops` (their first payload word)
    pub fn lying_oracle(ops: &[FieldHintOp]) -> ZkEENonDeterminismSource {
        let mut oracle = ZkEENonDeterminismSource::default();
        oracle.add_external_processor(FlipLowBit {
            inner: NativeFieldOpsQuery,
            ops: ops.to_vec(),
            last_word: false,
        });
        oracle
    }

    /// An oracle that corrupts the last word of the answers of `ops`
    pub fn lying_oracle_last_word(ops: &[FieldHintOp]) -> ZkEENonDeterminismSource {
        let mut oracle = ZkEENonDeterminismSource::default();
        oracle.add_external_processor(FlipLowBit {
            inner: NativeFieldOpsQuery,
            ops: ops.to_vec(),
            last_word: true,
        });
        oracle
    }

    fn all_lying() -> ZkEENonDeterminismSource {
        lying_oracle(&[
            FieldHintOp::Bn254BaseFieldInverse,
            FieldHintOp::Bn254Fq12Inverse,
            FieldHintOp::Bls12381BaseFieldSqrt,
            FieldHintOp::Bls12381BaseFieldInverse,
            FieldHintOp::Bls12381Fq12Inverse,
        ])
    }

    #[test]
    #[should_panic(expected = "inverse hint is wrong")]
    fn wrong_bn254_inverse_is_rejected() {
        let a = crypto::bn254::Fq::from(7u64);
        let _ = bn254_fq_inverse(&mut all_lying(), &a);
    }

    #[test]
    #[should_panic(expected = "inverse hint is wrong")]
    fn wrong_bls12_381_fq12_inverse_is_rejected() {
        let f = extension_elements::<crypto::bls12_381::Fq12>(1)[0];
        let _ = bls12_381_fq12_inverse(&mut all_lying(), &f);
    }

    #[test]
    #[should_panic(expected = "square root hint is wrong")]
    fn wrong_square_root_is_rejected() {
        // 4 is a square; the lying oracle returns a wrong root
        let a = crypto::bls12_381::Fq::from(4u64);
        let _ = bls12_381_fq_sqrt(&mut all_lying(), &a);
    }
}
