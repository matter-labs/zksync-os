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

use crate::system_functions::field_ops::{
    query_field_hint, read_hint_answer, send_field_hint_query, FieldHintOp, HintAnswer, HintTarget,
};
use alloc::vec::Vec;
use core::mem::MaybeUninit;
use crypto::ark_ff::{
    BigInteger, CubicExtConfig, CubicExtField, Field, One, PrimeField, QuadExtConfig, QuadExtField,
    Zero,
};
use zk_ee::internal_error;
use zk_ee::oracle::memory_io::MemoryOracle;
use zk_ee::oracle::IOOracle;
use zk_ee::system::errors::internal::InternalError;

/// The base prime field components of a field element in a fixed order (for a tower `c0` before
/// `c1` before `c2`, recursively), independent of the arkworks `to_base_prime_field_elements`
/// order, which the delegated and the native tower implementations do not agree on. It is the
/// order of the components of an element in the memory of the RISC-V guest, which the oracle reads
/// operands from (see [`guest_layout`]), and of the components of an answer on every target (see
/// the `HintAnswer` impls below).
pub trait HintEncoding: Field {
    /// 64-bit limbs of a base prime field element on the RISC-V guest: 4 for bn254, 8 for
    /// bls12-381 (381 bits in 512)
    const LIMBS: usize;
    /// Base prime field components of an element
    const COMPONENTS: usize;
    /// Limbs of a whole element on the RISC-V guest
    const TOTAL_LIMBS: usize = Self::LIMBS * Self::COMPONENTS;

    fn from_components(components: &[Self::BasePrimeField]) -> Self;
}

macro_rules! prime_field_encoding {
    ($field:ty, $limbs:expr) => {
        impl HintEncoding for $field {
            const LIMBS: usize = $limbs;
            const COMPONENTS: usize = 1;

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

            fn from_components(c: &[Self::BasePrimeField]) -> Self {
                let fp2 = |i: usize| <$fp2>::new(c[i], c[i + 1]);
                Self::new(fp2(0), fp2(2), fp2(4))
            }
        }
    };
}

macro_rules! fp2_encoding {
    ($field:ty, $limbs:expr) => {
        impl HintEncoding for $field {
            const LIMBS: usize = $limbs;
            const COMPONENTS: usize = 2;

            fn from_components(c: &[Self::BasePrimeField]) -> Self {
                Self::new(c[0], c[1])
            }
        }
    };
}

prime_field_encoding!(crypto::bn254::Fq, 4);
fp2_encoding!(crypto::bn254::Fq2, 4);
fp2_encoding!(crypto::bls12_381::Fq2, 8);
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

/// Where the oracle finds the parts of an operand in the memory of the RISC-V guest (a querier in
/// this process sends its values, which the oracle reads as such). A base field element is its
/// `HintEncoding::LIMBS` Montgomery limbs (`R = 2^(64 LIMBS)`), any representative (the
/// representation is redundant); an extension field element its base field components, back to
/// back in the `HintEncoding` order; an affine point its coordinates and the flag of the point at
/// infinity at the offsets below; a pair of the bn254 pairing input its two points. `airbender-crypto`
/// asserts the order of the components and of the coordinates at compile time, and the numbers
/// below are asserted on the guest.
pub mod guest_layout {
    /// Byte offsets of the coordinates and of the flag of the point at infinity of an affine
    /// point, and its size in bytes (with padding)
    #[derive(Clone, Copy, Debug)]
    pub struct AffinePoint {
        pub x: usize,
        pub y: usize,
        pub infinity: usize,
        pub size: usize,
    }

    pub const BN254_G1_AFFINE: AffinePoint = AffinePoint {
        x: 0,
        y: 32,
        infinity: 64,
        size: 96,
    };
    pub const BN254_G2_AFFINE: AffinePoint = AffinePoint {
        x: 0,
        y: 64,
        infinity: 128,
        size: 160,
    };
    pub const BLS12_381_G1_AFFINE: AffinePoint = AffinePoint {
        x: 0,
        y: 64,
        infinity: 128,
        size: 160,
    };

    /// Byte offsets of the points of a `(G1Affine, G2Affine)` pair of the bn254 pairing input, and
    /// its size in bytes
    pub const BN254_PAIR_G1: usize = 0;
    pub const BN254_PAIR_G2: usize = 96;
    pub const BN254_PAIR_SIZE: usize = 256;
}

#[cfg(target_arch = "riscv32")]
const _: () = {
    use core::mem::{offset_of, size_of};
    use crypto::{bls12_381, bn254};
    use guest_layout::*;

    // base field elements: their Montgomery limbs (the rest of the towers is asserted in
    // `airbender-crypto`)
    assert!(offset_of!(bn254::Fq, 0) == 0);
    assert!(size_of::<bn254::Fq>() == 8 * <bn254::Fq as HintEncoding>::LIMBS);
    assert!(offset_of!(bls12_381::Fq, 0) == 0);
    assert!(size_of::<bls12_381::Fq>() == 8 * <bls12_381::Fq as HintEncoding>::LIMBS);

    macro_rules! assert_affine {
        ($point:ty, $layout:expr) => {
            assert!(offset_of!($point, x) == $layout.x && offset_of!($point, y) == $layout.y);
            assert!(offset_of!($point, infinity) == $layout.infinity);
            assert!(size_of::<$point>() == $layout.size);
        };
    }
    assert_affine!(bn254::G1Affine, BN254_G1_AFFINE);
    assert_affine!(bn254::G2Affine, BN254_G2_AFFINE);
    assert_affine!(bls12_381::G1Affine, BLS12_381_G1_AFFINE);

    type Bn254Pair = (bn254::G1Affine, bn254::G2Affine);
    assert!(offset_of!(Bn254Pair, 0) == BN254_PAIR_G1);
    assert!(offset_of!(Bn254Pair, 1) == BN254_PAIR_G2);
    assert!(size_of::<Bn254Pair>() == BN254_PAIR_SIZE);
};

/// Whether the little-endian `value` is below the little-endian `modulus` (zero above the width of
/// `modulus`)
fn is_below(value: &[u64], modulus: &[u64]) -> bool {
    let (low, high) = value.split_at(value.len().min(modulus.len()));
    if high.iter().any(|limb| *limb != 0) {
        return false;
    }
    for (value, modulus) in low.iter().zip(modulus).rev() {
        if value != modulus {
            return value < modulus;
        }
    }
    false
}

/// The answer of a hint is a base prime field element `x` in the Montgomery form of the target of the
/// querier, its representation there: `x R mod p` for `R = 2^(64 limbs)` with the limbs of the
/// representation, canonical (below the modulus), as its `wire_limbs` little-endian 64-bit limbs (the
/// width of the modulus). It is written straight into the low limbs of the destination element, whose
/// limbs above (on the RISC-V guest, bls12-381 elements take 8 limbs for their 6) are zeroed, and checked
/// to be canonical.
///
/// # Safety
///
/// `limbs` must point to the limbs of the representation of an `F`, all of its bytes, valid for writes.
#[inline(always)]
unsafe fn read_prime_field<F: PrimeField, O: MemoryOracle>(
    oracle: &mut O,
    limbs: *mut u64,
    wire_limbs: usize,
) -> Result<(), InternalError> {
    let num_limbs = <F::BigInt as BigInteger>::NUM_LIMBS;
    debug_assert!(wire_limbs <= num_limbs);
    // SAFETY: the limbs are aligned for `u32`, and `num_limbs` long
    unsafe {
        oracle.write_words(limbs.cast::<u32>(), 2 * wire_limbs)?;
        limbs.add(wire_limbs).write_bytes(0, num_limbs - wire_limbs);
    }
    // SAFETY: just initialized
    let value = unsafe { core::slice::from_raw_parts(limbs, num_limbs) };
    if !is_below(value, F::MODULUS.as_ref()) {
        return Err(internal_error!("the hint is not a canonical field element"));
    }
    Ok(())
}

/// Oracle side of `read_prime_field`, with `limbs` the limbs of the representation of `element` in this
/// build, and `guest_limbs` those of the representation on the RISC-V guest
fn write_prime_field<F: PrimeField>(
    element: &F,
    limbs: &[u64],
    wire_limbs: usize,
    guest_limbs: usize,
    target: HintTarget,
    response: &mut Vec<u32>,
) {
    let mut push = |limbs: &[u64]| {
        debug_assert!(limbs[wire_limbs..].iter().all(|limb| *limb == 0));
        for limb in &limbs[..wire_limbs] {
            response.push(*limb as u32);
            response.push((*limb >> 32) as u32);
        }
    };
    // the Montgomery form of the target: `R = 2^(64 limbs)` with the limbs of its representation
    let target_limbs = match target {
        HintTarget::Native => limbs.len(),
        HintTarget::Guest => guest_limbs,
    };
    if limbs.len() == target_limbs && is_below(limbs, F::MODULUS.as_ref()) {
        // the representation of this build
        push(limbs);
    } else {
        let montgomery_r = F::from(2u64).pow([64 * target_limbs as u64]);
        push((*element * montgomery_r).into_bigint().as_ref());
    }
}

/// `$wire_limbs`: the width of the modulus; `$guest_limbs`: the limbs of the representation on the
/// RISC-V guest, whose `R` is `2^(64 $guest_limbs)`
macro_rules! prime_field_answer {
    ($field:ty, $wire_limbs:expr, $guest_limbs:expr) => {
        const _: () = {
            // the answer fits in every representation, and the modulus in the limbs of the answer
            // (`MODULUS_BIT_SIZE` would not do: the delegated representation counts its bits from
            // its top limb, zero or not)
            let modulus = <$field as PrimeField>::MODULUS.0;
            assert!($wire_limbs <= modulus.len());
            let mut i = $wire_limbs;
            while i < modulus.len() {
                assert!(modulus[i] == 0);
                i += 1;
            }
        };

        impl HintAnswer for $field {
            #[inline(always)]
            fn read<'a, O: MemoryOracle>(
                oracle: &mut O,
                dst: &'a mut MaybeUninit<Self>,
            ) -> Result<&'a mut Self, InternalError> {
                let this = dst.as_mut_ptr();
                // SAFETY: the limbs of the representation (the other field is zero-sized) are a place
                // inside `dst`, which initialize it
                unsafe {
                    let limbs = (&raw mut (*this).0 .0).cast::<u64>();
                    read_prime_field::<Self, O>(oracle, limbs, $wire_limbs)?;
                    Ok(dst.assume_init_mut())
                }
            }

            fn write(&self, target: HintTarget, response: &mut Vec<u32>) {
                write_prime_field(
                    self,
                    &self.0 .0,
                    $wire_limbs,
                    $guest_limbs,
                    target,
                    response,
                );
            }
        }
    };
}

prime_field_answer!(crypto::bn254::Fq, 4, 4);
// 8 limbs of 64 bits for 381 bits on the RISC-V guest, where the delegation works on 256-bit limbs;
// 6 in a native build
prime_field_answer!(crypto::bls12_381::Fq, 6, 8);

/// The components in order, as in `HintEncoding` (`c0` before `c1`, recursively).
impl<P: QuadExtConfig> HintAnswer for QuadExtField<P>
where
    P::BaseField: HintAnswer,
{
    #[inline(always)]
    fn read<'a, O: MemoryOracle>(
        oracle: &mut O,
        dst: &'a mut MaybeUninit<Self>,
    ) -> Result<&'a mut Self, InternalError> {
        let this = dst.as_mut_ptr();
        // SAFETY: the components are disjoint places inside `dst`, which they initialize, and
        // `MaybeUninit<E>` has the layout of `E`
        unsafe {
            P::BaseField::read(oracle, &mut *(&raw mut (*this).c0).cast())?;
            P::BaseField::read(oracle, &mut *(&raw mut (*this).c1).cast())?;
            Ok(dst.assume_init_mut())
        }
    }

    fn write(&self, target: HintTarget, response: &mut Vec<u32>) {
        self.c0.write(target, response);
        self.c1.write(target, response);
    }
}

/// The components in order, as in `HintEncoding` (`c0` before `c1` before `c2`, recursively).
impl<P: CubicExtConfig> HintAnswer for CubicExtField<P>
where
    P::BaseField: HintAnswer,
{
    #[inline(always)]
    fn read<'a, O: MemoryOracle>(
        oracle: &mut O,
        dst: &'a mut MaybeUninit<Self>,
    ) -> Result<&'a mut Self, InternalError> {
        let this = dst.as_mut_ptr();
        // SAFETY: as for the quadratic extension
        unsafe {
            P::BaseField::read(oracle, &mut *(&raw mut (*this).c0).cast())?;
            P::BaseField::read(oracle, &mut *(&raw mut (*this).c1).cast())?;
            P::BaseField::read(oracle, &mut *(&raw mut (*this).c2).cast())?;
            Ok(dst.assume_init_mut())
        }
    }

    fn write(&self, target: HintTarget, response: &mut Vec<u32>) {
        self.c0.write(target, response);
        self.c1.write(target, response);
        self.c2.write(target, response);
    }
}

/// `a^-1` from a hint, `None` for a zero `a`
fn inverse_from_hint<O: IOOracle, F: Field + HintAnswer>(
    oracle: &mut O,
    op: FieldHintOp,
    a: &F,
) -> Option<F> {
    if a.is_zero() {
        return None;
    }
    let inverse: F = query_field_hint(oracle, op, a);
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
    inverse_from_hint(oracle, FieldHintOp::Bn254BaseFieldInverse, a)
}

/// `f^-1` in the bn254 degree-12 extension field, `None` for a zero `f`
pub fn bn254_fq12_inverse<O: IOOracle>(
    oracle: &mut O,
    f: &crypto::bn254::Fq12,
) -> Option<crypto::bn254::Fq12> {
    inverse_from_hint(oracle, FieldHintOp::Bn254Fq12Inverse, f)
}

/// `a^-1` in the bls12-381 base field, `None` for a zero `a`
pub fn bls12_381_fq_inverse<O: IOOracle>(
    oracle: &mut O,
    a: &crypto::bls12_381::Fq,
) -> Option<crypto::bls12_381::Fq> {
    inverse_from_hint(oracle, FieldHintOp::Bls12381BaseFieldInverse, a)
}

/// `f^-1` in the bls12-381 degree-12 extension field, `None` for a zero `f`
pub fn bls12_381_fq12_inverse<O: IOOracle>(
    oracle: &mut O,
    f: &crypto::bls12_381::Fq12,
) -> Option<crypto::bls12_381::Fq12> {
    inverse_from_hint(oracle, FieldHintOp::Bls12381Fq12Inverse, f)
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
    let (candidate, is_non_residue): (Fq, bool) =
        query_field_hint(oracle, FieldHintOp::Bls12381BaseFieldSqrt, a);
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
pub fn bn254_pairing_residue_witness<O: IOOracle>(
    oracle: &mut O,
    pairs: &[(crypto::bn254::G1Affine, crypto::bn254::G2Affine)],
) -> PairingClaim<crypto::bn254::Fq12, crypto::bn254::Fq6> {
    use crypto::bn254::{Fq12, Fq6};
    // the identity flag, then `c, d, s` for an identity or `f^-1` otherwise
    send_field_hint_query(oracle, FieldHintOp::Bn254PairingResidueWitness, pairs)
        .expect("must send the field hint query");
    let is_identity: bool = read_hint_answer(oracle).expect("the hint answer is well-formed");
    let claim = if is_identity {
        let (c, d, s): (Fq12, Fq12, Fq6) =
            read_hint_answer(oracle).expect("the hint answer is well-formed");
        PairingClaim::Identity { c, d, s }
    } else {
        PairingClaim::NotIdentity {
            f_inverse: read_hint_answer(oracle).expect("the hint answer is well-formed"),
        }
    };
    oracle
        .finish_query()
        .expect("the hint answer has no excess data");
    if let PairingClaim::Identity { c, d, .. } = &claim {
        assert!(
            (*c * d).is_one(),
            "the residue witness hint is not an inverse pair"
        );
    }
    claim
}

/// The hinted inverses of the affine `G2` chains of one point
/// (`crypto::bn254::curves::g2_affine`), received from the oracle as the chains consume them, so
/// that no array of them is ever moved: a flag and the inverses of the membership test, then a flag
/// and those of the line precomputation. A cleared flag is the prover reporting an exceptional
/// chain (the caller falls back to the projective computation; the words of that chain are zeros
/// and are skipped). The hints are not checked here: each chain checks every inverse against its
/// denominator, and a wrong one is a broken prover. Whatever is left of the answer is skipped on
/// drop, and the query ended, so the oracle stays in step on every path.
pub struct G2InverseHints<'a, O: MemoryOracle> {
    oracle: &'a mut O,
    /// Words of the answer not received yet
    remaining_words: usize,
}

/// Inverses of the subgroup test and of the line precomputation, in base field elements (two per
/// `Fq2`)
pub const G2_SUBGROUP_INVERSE_WORDS: usize =
    2 * crypto::bn254::curves::g2_affine::SUBGROUP_INVERSES;
pub const G2_LINE_INVERSE_WORDS: usize = 2 * crypto::bn254::curves::g2_affine::LINE_INVERSES;

/// Oracle words of one `Fq2` hint: two elements of 4 limbs
const FQ2_HINT_WORDS: usize = 2 * 2 * 4;

/// Oracle words of the whole answer: two flags and the inverses
const G2_HINT_WORDS: usize = 2
    + (crypto::bn254::curves::g2_affine::SUBGROUP_INVERSES
        + crypto::bn254::curves::g2_affine::LINE_INVERSES)
        * FQ2_HINT_WORDS;

/// The inverses of the affine `G2` chains of `q` (on the twist, not the point at infinity)
/// from the oracle
pub fn bn254_g2_pairing_inverses<'a, O: IOOracle>(
    oracle: &'a mut O,
    q: &crypto::bn254::G2Affine,
) -> G2InverseHints<'a, O> {
    send_field_hint_query(oracle, FieldHintOp::Bn254G2PairingInverses, q)
        .expect("must send the field hint query");
    G2InverseHints {
        oracle,
        remaining_words: G2_HINT_WORDS,
    }
}

impl<'a, O: MemoryOracle> G2InverseHints<'a, O> {
    /// The inverses of the membership test, `None` if the prover reports an exceptional chain
    pub fn subgroup_chain(&mut self) -> Option<HintedChain<'_, 'a, O>> {
        self.chain(crypto::bn254::curves::g2_affine::SUBGROUP_INVERSES)
    }

    /// The inverses of the line precomputation, `None` if the prover reports an exceptional
    /// chain; after the membership test's
    pub fn line_chain(&mut self) -> Option<HintedChain<'_, 'a, O>> {
        self.chain(crypto::bn254::curves::g2_affine::LINE_INVERSES)
    }

    fn chain(&mut self, inverses: usize) -> Option<HintedChain<'_, 'a, O>> {
        self.remaining_words -= 1;
        let present: bool =
            read_hint_answer(self.oracle).expect("the hint response has the flag of the chain");
        if !present {
            self.skip(inverses * FQ2_HINT_WORDS);
            return None;
        }
        Some(HintedChain {
            hints: self,
            remaining: inverses,
        })
    }

    fn skip(&mut self, words: usize) {
        self.remaining_words -= words;
        for _ in 0..words {
            self.oracle
                .read_word()
                .expect("the hint response has the words of the chain");
        }
    }
}

impl<O: MemoryOracle> Drop for G2InverseHints<'_, O> {
    fn drop(&mut self) {
        self.skip(self.remaining_words);
        self.oracle
            .finish_query()
            .expect("the hint answer has no excess data");
    }
}

/// The hints of one chain, in the chain's order
pub struct HintedChain<'c, 'a, O: MemoryOracle> {
    hints: &'c mut G2InverseHints<'a, O>,
    remaining: usize,
}

impl<O: MemoryOracle> crypto::bn254::curves::g2_affine::Inverter for HintedChain<'_, '_, O> {
    fn inverse(&mut self, den: &crypto::bn254::Fq2) -> Option<crypto::bn254::Fq2> {
        debug_assert!(self.remaining > 0, "the chain is longer than its hints");
        self.remaining -= 1;
        self.hints.remaining_words -= FQ2_HINT_WORDS;
        let inverse: crypto::bn254::Fq2 =
            read_hint_answer(self.hints.oracle).expect("the hint is a canonical field element");
        crypto::bn254::curves::g2_affine::verify_inverse(den, inverse)
    }
}

/// The claim and witness of `e(p1, G2) e(p2, tau G2)` (the KZG proof check, `points` are
/// `[p1, p2]`) from the oracle, for `crypto::residue_witness::bls12_381::check` on the Miller loop
/// started at `d` (`c` is not needed by that loop and is left zero)
pub fn bls12_381_kzg_residue_witness<O: IOOracle>(
    oracle: &mut O,
    points: &[crypto::bls12_381::G1Affine; 2],
) -> PairingClaim<crypto::bls12_381::Fq12, crypto::bls12_381::Fq6> {
    use crypto::bls12_381::{Fq12, Fq6};
    // the identity flag, then `d, s` for an identity or `f^-1` otherwise
    send_field_hint_query(oracle, FieldHintOp::Bls12381KzgResidueWitness, points)
        .expect("must send the field hint query");
    let is_identity: bool = read_hint_answer(oracle).expect("the hint answer is well-formed");
    let claim = if is_identity {
        let (d, s): (Fq12, Fq6) = read_hint_answer(oracle).expect("the hint answer is well-formed");
        PairingClaim::Identity {
            c: Fq12::zero(),
            d,
            s,
        }
    } else {
        PairingClaim::NotIdentity {
            f_inverse: read_hint_answer(oracle).expect("the hint answer is well-formed"),
        }
    };
    oracle
        .finish_query()
        .expect("the hint answer has no excess data");
    claim
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
    use zk_ee::oracle::memory_io::host::QuerierMemory;

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

    fn answer_words<T: HintAnswer>(value: &T, target: HintTarget) -> Vec<u32> {
        let mut response = Vec::new();
        value.write(target, &mut response);
        response
    }

    fn limb_words(limbs: &[u64]) -> Vec<u32> {
        limbs
            .iter()
            .flat_map(|limb| [*limb as u32, (*limb >> 32) as u32])
            .collect()
    }

    #[test]
    fn answers_follow_the_target() {
        for a in elements::<crypto::bn254::Fq>(5) {
            // `R = 2^256` on every target
            let montgomery_form = a * crypto::bn254::Fq::from(2u64).pow([256]);
            let expected = limb_words(montgomery_form.into_bigint().as_ref());
            assert_eq!(answer_words(&a, HintTarget::Native), expected);
            assert_eq!(answer_words(&a, HintTarget::Guest), expected);
        }
        for a in elements::<crypto::bls12_381::Fq>(5) {
            // `R = 2^384` in this build, `2^512` on the RISC-V guest; 6 limbs on both
            let r = |bits: u64| crypto::bls12_381::Fq::from(2u64).pow([bits]);
            let native = limb_words((a * r(384)).into_bigint().as_ref());
            let guest = limb_words((a * r(512)).into_bigint().as_ref());
            assert_eq!(answer_words(&a, HintTarget::Native), native);
            assert_eq!(answer_words(&a, HintTarget::Guest), guest);
            assert_eq!(guest.len(), 12);
            let f = crypto::bls12_381::Fq2::new(a, -a);
            assert_eq!(
                answer_words(&f, HintTarget::Guest),
                [guest, limb_words(((-a) * r(512)).into_bigint().as_ref())].concat()
            );
        }
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

    /// The op of the hint request at `input_word`, as the type of this crate (`callable_oracles`
    /// has its own `basic_system`)
    fn read_op(memory: &dyn QuerierMemory, input_word: usize) -> FieldHintOp {
        let (op, _, _) =
            crate::system_functions::field_ops::read_field_hint_request(memory, input_word)
                .unwrap();
        FieldHintOp::parse_u32(op).unwrap()
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
        fn supported_memory_query_ids(&self) -> Vec<u32> {
            self.inner.supported_memory_query_ids()
        }

        fn process_memory_query(
            &mut self,
            query_id: u32,
            input_word: usize,
            memory: &dyn QuerierMemory,
            mode: oracle_provider::RunMode,
            native_run_responses: &mut Vec<u32>,
            guest_run_responses: &mut Vec<u32>,
        ) {
            let op = read_op(memory, input_word);
            // the response of the native run (the querier of the tests)
            let mut words = Vec::new();
            self.inner.process_memory_query(
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
                    _ if self.last_word => words.len() - 1,
                    FieldHintOp::Bn254PairingResidueWitness
                    | FieldHintOp::Bls12381KzgResidueWitness => 1,
                    _ => 0,
                };
                words[index] ^= 1;
            }
            native_run_responses.extend(words);
        }
    }

    /// A processor that claims every pairing product not to be the identity, with the
    /// (correct) inverse the slow path needs
    pub struct ClaimNotIdentity(NativeFieldOpsQuery);

    impl oracle_provider::OracleQueryProcessor for ClaimNotIdentity {
        fn supported_memory_query_ids(&self) -> Vec<u32> {
            self.0.supported_memory_query_ids()
        }

        fn process_memory_query(
            &mut self,
            query_id: u32,
            input_word: usize,
            memory: &dyn QuerierMemory,
            mode: oracle_provider::RunMode,
            native_run_responses: &mut Vec<u32>,
            guest_run_responses: &mut Vec<u32>,
        ) {
            match read_op(memory, input_word) {
                FieldHintOp::Bn254PairingResidueWitness
                | FieldHintOp::Bls12381KzgResidueWitness => native_run_responses.extend(
                    callable_oracles::field_hints::not_identity_claim(memory, input_word),
                ),
                _ => self.0.process_memory_query(
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
