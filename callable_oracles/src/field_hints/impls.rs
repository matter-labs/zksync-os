use super::Responses;
use basic_system::system_functions::curve_hints::{guest_layout, HintEncoding};
use basic_system::system_functions::field_ops::{HintTarget, Secp256k1Element};
use crypto::ark_ec::short_weierstrass::{Affine, SWCurveConfig};
use crypto::ark_ff::{Field, One, PrimeField, Zero};
use crypto::secp256k1::field::FieldElement;
use crypto::secp256k1::scalars::Scalar;
use zk_ee::oracle::memory_io::host::{read_querier_u32_words, QuerierMemory};

/// The operand of a hint query where the querier keeps it: its address and size in the memory of
/// the querier, in the representation of the target of the querier (see `FieldHintOp`)
pub(crate) struct Operand<'m> {
    memory: &'m dyn QuerierMemory,
    querier: HintTarget,
    address: usize,
    len_u32_words: u32,
}

impl<'m> Operand<'m> {
    pub(crate) fn new(
        memory: &'m dyn QuerierMemory,
        querier: HintTarget,
        address: usize,
        len_u32_words: u32,
    ) -> Self {
        assert!(address != 0, "the operand is not at the null address");
        Self {
            memory,
            querier,
            address,
            len_u32_words,
        }
    }

    /// The size of the operand in bytes
    fn size(&self) -> usize {
        self.len_u32_words as usize * size_of::<u32>()
    }

    /// The operand of a querier in this process, values of `T`
    fn native_values<T: Copy>(&self) -> Vec<T> {
        assert_eq!(self.querier, HintTarget::Native);
        assert!(
            self.size().is_multiple_of(size_of::<T>()),
            "the operand is values of the type"
        );
        assert!(
            self.address.is_multiple_of(align_of::<T>()),
            "the operand is aligned for the type"
        );
        let values = core::ptr::with_exposed_provenance::<T>(self.address);
        // SAFETY: the querier, code of this process, exposes its operand, values of `T`, while it
        // sends the query, i.e. during this call (see `send_field_hint_query`)
        unsafe { core::slice::from_raw_parts(values, self.size() / size_of::<T>()) }.to_vec()
    }

    /// The operand of a querier in this process, a `T`
    fn native_value<T: Copy>(&self) -> T {
        assert_eq!(
            self.size(),
            size_of::<T>(),
            "the operand is a value of the type"
        );
        self.native_values::<T>()[0]
    }

    /// `len` words at `offset` bytes into the operand of the RISC-V guest
    fn guest_words(&self, offset: usize, len: usize) -> Vec<u32> {
        assert_eq!(self.querier, HintTarget::Guest);
        assert!(offset + 4 * len <= self.size(), "within the operand");
        read_querier_u32_words(self.memory, self.address + offset, len)
            .expect("must read the operand")
    }

    /// The byte at `offset` into the operand of the RISC-V guest
    fn guest_byte(&self, offset: usize) -> u8 {
        self.guest_words(offset & !3, 1)[0].to_le_bytes()[offset & 3]
    }

    /// The field element at `offset` bytes into the operand of the RISC-V guest (see
    /// `curve_hints::guest_layout`)
    fn guest_element<F: HintEncoding>(&self, offset: usize) -> F {
        let words = self.guest_words(offset, 2 * F::TOTAL_LIMBS);
        let components: Vec<F::BasePrimeField> = words
            .chunks(2 * F::LIMBS)
            .map(from_guest_montgomery)
            .collect();
        F::from_components(&components)
    }

    /// The affine point at `offset` bytes into the operand of the RISC-V guest
    fn guest_affine<P: SWCurveConfig>(
        &self,
        offset: usize,
        layout: guest_layout::AffinePoint,
    ) -> Affine<P>
    where
        P::BaseField: HintEncoding,
    {
        if self.guest_byte(offset + layout.infinity) != 0 {
            return Affine::identity();
        }
        Affine::new_unchecked(
            self.guest_element(offset + layout.x),
            self.guest_element(offset + layout.y),
        )
    }
}

/// The base field element of its Montgomery limbs on the RISC-V guest (`R = 2^(32 words)`), any
/// representative
fn from_guest_montgomery<F: PrimeField>(words: &[u32]) -> F {
    let bytes: Vec<u8> = words.iter().flat_map(|word| word.to_le_bytes()).collect();
    let r = F::from(2u64).pow([32 * words.len() as u64]);
    F::from_le_bytes_mod_order(&bytes) * r.inverse().expect("a power of two is invertible")
}

/// A field element operand
fn element<F: HintEncoding + Copy>(operand: &Operand) -> F {
    match operand.querier {
        HintTarget::Native => operand.native_value(),
        HintTarget::Guest => {
            assert_eq!(
                operand.size(),
                8 * F::TOTAL_LIMBS,
                "the operand is an element"
            );
            operand.guest_element(0)
        }
    }
}

/// A secp256k1 element operand (see `Secp256k1Element`)
fn secp256k1_element<T: Secp256k1Element>(operand: &Operand) -> T {
    match operand.querier {
        HintTarget::Native => operand.native_value(),
        HintTarget::Guest => {
            assert_eq!(operand.size(), 32, "the operand is an element");
            let words: [u32; 8] = operand.guest_words(0, 8).try_into().expect("8 words");
            T::from_guest_operand_words(&words)
        }
    }
}

/// Computes the square root candidate for a secp256k1 base field element.
///
/// Returns `(candidate, is_quadratic_non_residue)` where:
/// - `candidate` is `input^((p+1)/4)` (the square root if one exists)
/// - `is_quadratic_non_residue` is `true` if `input` has no square root in the field
///
/// When `is_quadratic_non_residue` is false: `candidate² == input`
/// When `is_quadratic_non_residue` is true:  `candidate² == -input`
pub(crate) fn secp256k1_base_field_sqrt(operand: &Operand) -> (FieldElement, bool) {
    let el: FieldElement = secp256k1_element(operand);
    assert!(!el.is_zero());
    let mut candidate = el;
    // sqrt_in_place returns true if the input is a quadratic residue (has a square root)
    let is_quadratic_residue = candidate.sqrt_in_place();
    (candidate, !is_quadratic_residue)
}

pub(crate) fn secp256k1_base_field_inverse(operand: &Operand) -> FieldElement {
    let mut el: FieldElement = secp256k1_element(operand);
    assert!(!el.is_zero());
    el.invert_in_place();
    el
}

pub(crate) fn secp256k1_scalar_field_inverse(operand: &Operand) -> Scalar {
    let mut el: Scalar = secp256k1_element(operand);
    assert!(!el.is_zero());
    el.invert_in_place();
    el
}

/// The inverse of a non-zero field element
pub(crate) fn inverse<F: HintEncoding + Copy>(operand: &Operand) -> F {
    let el: F = element(operand);
    el.inverse().expect("the operand is non-zero")
}

/// The square root candidate of a bls12-381 base field element, as `secp256k1_base_field_sqrt`:
/// `(candidate, is_quadratic_non_residue)` with `candidate² == input` for a square and
/// `candidate² == -input` otherwise (`p = 3 mod 4`, so exactly one of them is a square)
pub(crate) fn bls12_381_base_field_sqrt(operand: &Operand) -> (crypto::bls12_381::Fq, bool) {
    let el: crypto::bls12_381::Fq = element(operand);
    assert!(!el.is_zero());
    match el.sqrt() {
        Some(root) => (root, false),
        None => {
            let root = (-el).sqrt().expect("-1 is not a square, so -el is one");
            (root, true)
        }
    }
}

/// The claim about the bn254 pairing product over the affine pairs of the operand, see
/// `curve_hints::PairingClaim`: the identity flag, then `c`, `d = c^-1` and the scaling factor
/// (`crypto::residue_witness::bn254`) for an identity, or the inverse of the Miller loop output
/// otherwise. `claim_not_identity` forces the latter (for tests of the exact path).
pub(crate) fn bn254_pairing_residue_witness(
    operand: &Operand,
    claim_not_identity: bool,
    responses: &mut Responses,
) {
    use crypto::ark_ec::pairing::{MillerLoopOutput, Pairing};
    use crypto::bn254::curves::{Bn254, G2PreparedNoAlloc};
    use crypto::bn254::{G1Affine, G2Affine};
    use guest_layout::{BN254_G1_AFFINE, BN254_G2_AFFINE, BN254_PAIR_G1, BN254_PAIR_G2};
    let pairs: Vec<(G1Affine, G2Affine)> = match operand.querier {
        HintTarget::Native => operand.native_values(),
        HintTarget::Guest => {
            let size = guest_layout::BN254_PAIR_SIZE;
            assert!(
                operand.size().is_multiple_of(size),
                "the operand is affine pairs"
            );
            (0..operand.size() / size)
                .map(|k| {
                    let pair = k * size;
                    (
                        operand.guest_affine(pair + BN254_PAIR_G1, BN254_G1_AFFINE),
                        operand.guest_affine(pair + BN254_PAIR_G2, BN254_G2_AFFINE),
                    )
                })
                .collect()
        }
    };
    assert!(!pairs.is_empty(), "the operand has pairs");
    // the lines as the verifier computes them (projective; the affine ones from hinted
    // inverses, `g2_affine::prepare_as_verifier`, are not in use): the witness equation holds
    // for the verifier's Miller loop output only
    let prepared: Vec<G2PreparedNoAlloc> = pairs
        .iter()
        .map(|(_, g2)| G2PreparedNoAlloc::from(*g2))
        .collect();
    let f = Bn254::multi_miller_loop_prepared(pairs.iter().map(|(g1, _)| g1), prepared.iter());
    let is_identity = !claim_not_identity
        && Bn254::final_exponentiation(MillerLoopOutput(f))
            .expect("non-zero")
            .0
            .is_one();
    if is_identity {
        let witness = crypto::residue_witness::bn254::witness(&f)
            .expect("the final exponentiation found an identity, which has a witness");
        responses.write(&(true, witness));
    } else {
        responses.write(&(false, f.inverse().expect("non-zero")));
    }
}

/// The claim about `e(p1, G2) e(p2, tau G2)` for the affine points `[p1, p2]` of the operand, as
/// `bn254_pairing_residue_witness`: the identity flag, then `d` and the scaling factor
/// (`crypto::residue_witness::bls12_381`) for an identity, or the inverse of the Miller loop
/// output otherwise
pub(crate) fn bls12_381_kzg_residue_witness(
    operand: &Operand,
    claim_not_identity: bool,
    responses: &mut Responses,
) {
    use crypto::ark_ec::pairing::{MillerLoopOutput, Pairing};
    use crypto::bls12_381::curves::Bls12_381;
    use crypto::bls12_381::{Fq12, G1Affine};
    use guest_layout::BLS12_381_G1_AFFINE;
    let [p1, p2]: [G1Affine; 2] = match operand.querier {
        HintTarget::Native => operand.native_value(),
        HintTarget::Guest => {
            let size = BLS12_381_G1_AFFINE.size;
            assert_eq!(operand.size(), 2 * size, "the operand is two affine points");
            [
                operand.guest_affine(0, BLS12_381_G1_AFFINE),
                operand.guest_affine(size, BLS12_381_G1_AFFINE),
            ]
        }
    };
    let g2 = [
        &crypto::bls12_381::consts::PREPARED_G2_GENERATOR,
        &crypto::bls12_381::consts::PREPARED_G2_BY_TAU,
    ];
    let f = Bls12_381::multi_miller_loop_with_initial(&Fq12::one(), [p1, p2], g2);
    let is_identity = !claim_not_identity
        && Bls12_381::final_exponentiation(MillerLoopOutput(
            Bls12_381::multi_miller_loop_prepared([p1, p2], g2),
        ))
        .expect("non-zero")
        .0
        .is_one();
    if is_identity {
        let witness = crypto::residue_witness::bls12_381::witness(&f)
            .expect("the final exponentiation found an identity, which has a witness");
        responses.write(&(true, witness));
    } else {
        // the inverse of the output of the plain (conjugated) loop, which the exact path runs
        let mut conjugated = f;
        conjugated.conjugate_in_place();
        responses.write(&(false, conjugated.inverse().expect("non-zero")));
    }
}

/// The inverses of the two affine `G2` chains of the pairing input point of the operand, see
/// `curve_hints::bn254_g2_pairing_inverses`: a flag and the subgroup test inverses, a flag
/// and the line precomputation inverses (zeros behind a cleared flag)
pub(crate) fn bn254_g2_pairing_inverses(operand: &Operand, responses: &mut Responses) {
    use crypto::ark_ff::AdditiveGroup;
    use crypto::bn254::curves::g2_affine::{line_inverses, subgroup_inverses};
    use crypto::bn254::{Fq2, G2Affine};
    use guest_layout::BN254_G2_AFFINE;
    let q: G2Affine = match operand.querier {
        HintTarget::Native => operand.native_value(),
        HintTarget::Guest => {
            assert_eq!(
                operand.size(),
                BN254_G2_AFFINE.size,
                "the operand is a point"
            );
            operand.guest_affine(0, BN254_G2_AFFINE)
        }
    };
    fn chain<const N: usize>(inverses: Option<[Fq2; N]>, responses: &mut Responses) {
        match inverses {
            Some(inverses) => responses.write(&(true, inverses)),
            None => responses.write(&(false, [Fq2::ZERO; N])),
        }
    }
    chain(subgroup_inverses(&q), responses);
    chain(line_inverses(&q), responses);
}
