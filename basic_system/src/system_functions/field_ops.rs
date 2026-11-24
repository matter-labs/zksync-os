//! Query for field operations hints, such as square roots and inverses. The secp256k1 hints are
//! those of `crypto` (`crypto::hints`), which reach the oracle of the system through
//! [`CryptoHintsOracle`].

use alloc::vec::Vec;
use core::mem::{offset_of, MaybeUninit};
use crypto::hints::HintError;
use zk_ee::{
    internal_error,
    oracle::{
        memory_io::{
            host::{read_querier_word, QuerierMemory},
            MemoryOracle,
        },
        query_ids::ADVICE_SUBSPACE_MASK,
    },
    system::errors::internal::InternalError,
};

pub const FIELD_OPS_ADVISE_QUERY_ID: u32 = ADVICE_SUBSPACE_MASK | 0x11;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct GenericFieldOpsHint<W> {
    pub op: u32,
    pub src_ptr: W,
    pub src_len_u32_words: u32,
}

pub type FieldOpsHint = GenericFieldOpsHint<u32>;
pub type FieldOpsHint64 = GenericFieldOpsHint<u64>;

/// A field hint. The operand of an op is sent by address: the oracle reads the operand where it
/// is, in the representation of the target of the querier, which it knows from the run mode (a
/// querier in this process: the value itself; the RISC-V guest: its memory, see
/// `crypto::hints::Secp256k1Element` and `curve_hints::guest_layout`). The answers are in the representation of
/// the target too (see [`HintAnswer`]).
#[repr(u32)]
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldHintOp {
    /// secp256k1 base field square root: an element in; a candidate root and a "quadratic
    /// non-residue" flag out (the root of the input, or of its negation when the input is not a
    /// square)
    Secp256k1BaseFieldSqrt = 0,
    /// secp256k1 base field inverse: an element in, its inverse out
    Secp256k1BaseFieldInverse,
    /// secp256k1 scalar field inverse: a scalar in, its inverse out
    Secp256k1ScalarFieldInverse,
    /// bn254 base field inverse: an element in, its inverse out
    Bn254BaseFieldInverse,
    /// bn254 `Fq12` inverse: an element in, its inverse out
    Bn254Fq12Inverse,
    /// bls12-381 base field square root: an element in; a candidate root and a "quadratic
    /// non-residue" flag out. As for secp256k1 (`p = 3 mod 4`), the candidate is the root of the
    /// input, or of its negation when the input is not a square.
    Bls12381BaseFieldSqrt,
    /// bls12-381 base field inverse: an element in, its inverse out
    Bls12381BaseFieldInverse,
    /// bls12-381 `Fq12` inverse: an element in, its inverse out
    Bls12381Fq12Inverse,
    /// The prover's claim about a bn254 pairing product (see `crypto::oracle_common::residue_witness` and
    /// `curve_hints::PairingClaim`): the slice of the affine pairs `(G1Affine, G2Affine)` in; an
    /// "is the identity" flag, then for an identity the witness `c`, `d = c^-1` (`Fq12` each) and
    /// the scaling factor (`Fq6`), otherwise the inverse of the Miller loop output (`Fq12`)
    Bn254PairingResidueWitness,
    /// The slopes of the two affine `G2` chains of a bn254 pairing input point (see
    /// `crypto::bn254::curves::g2_affine`): the `G2Affine` point in; a flag and the 93 slopes
    /// (`Fq2`) of the subgroup membership test, then a flag and the 87 slopes of the line
    /// precomputation out. A cleared flag means the chain hit an exceptional case and the
    /// projective computation is to be used.
    Bn254G2PairingSlopes,
    /// The prover's claim about the KZG proof pairing product `e(P1, G2) e(P2, tau G2)`: the
    /// `G1Affine` points `[P1, P2]` in; an "is the identity" flag, then for an identity the
    /// witness `d` (`Fq12`) and the scaling factor (`Fq6`), otherwise the inverse of the Miller
    /// loop output (`Fq12`)
    Bls12381KzgResidueWitness,
    /// secp256k1 base field division: the numerator and the denominator (two adjacent elements)
    /// in, the quotient out
    Secp256k1BaseFieldDivision,
    /// bn254 base field division: the numerator and the denominator (two adjacent elements)
    /// in, the quotient out
    Bn254BaseFieldDivision,
    /// bls12-381 base field division: the numerator and the denominator (two adjacent
    /// elements) in, the quotient out
    Bls12381BaseFieldDivision,
}

impl FieldHintOp {
    pub fn parse_u32(value: u32) -> Option<Self> {
        const ALL: [FieldHintOp; 14] = [
            FieldHintOp::Secp256k1BaseFieldSqrt,
            FieldHintOp::Secp256k1BaseFieldInverse,
            FieldHintOp::Secp256k1ScalarFieldInverse,
            FieldHintOp::Bn254BaseFieldInverse,
            FieldHintOp::Bn254Fq12Inverse,
            FieldHintOp::Bls12381BaseFieldSqrt,
            FieldHintOp::Bls12381BaseFieldInverse,
            FieldHintOp::Bls12381Fq12Inverse,
            FieldHintOp::Bn254PairingResidueWitness,
            FieldHintOp::Bn254G2PairingSlopes,
            FieldHintOp::Bls12381KzgResidueWitness,
            FieldHintOp::Secp256k1BaseFieldDivision,
            FieldHintOp::Bn254BaseFieldDivision,
            FieldHintOp::Bls12381BaseFieldDivision,
        ];
        ALL.into_iter().find(|op| *op as u32 == value)
    }
}

/// The request of a hint query, as the oracle reads it through the input word, which is the address
/// of a `GenericFieldOpsHint<usize>` of the querier: a [`FieldOpsHint`] on the proving target, a
/// [`FieldOpsHint64`] for a querier in the same process. Returns the op, and the address and the
/// length (in `u32` words) of the operand.
pub fn read_field_hint_request<M: QuerierMemory + ?Sized>(
    memory: &M,
    address: usize,
) -> Result<(u32, usize, u32), InternalError> {
    let (op, src_ptr, src_len_u32_words) = match memory.word_size() {
        4 => (
            offset_of!(FieldOpsHint, op),
            offset_of!(FieldOpsHint, src_ptr),
            offset_of!(FieldOpsHint, src_len_u32_words),
        ),
        8 => (
            offset_of!(FieldOpsHint64, op),
            offset_of!(FieldOpsHint64, src_ptr),
            offset_of!(FieldOpsHint64, src_len_u32_words),
        ),
        _ => return Err(internal_error!("unsupported querier word size")),
    };
    let field = |offset: usize| {
        address
            .checked_add(offset)
            .ok_or_else(|| internal_error!("querier address overflows"))
    };
    Ok((
        memory.read_u32(field(op)?)?,
        read_querier_word(memory, field(src_ptr)?)?,
        memory.read_u32(field(src_len_u32_words)?)?,
    ))
}

/// The target of a querier, whose representation of the answers the oracle writes. The
/// representations of field elements differ between the RISC-V guest and native builds, so the
/// answers do, and a native run that records the prover input is answered in both: it reads the
/// answers of its own target, and records those of the guest, which the guest reads in its place.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HintTarget {
    /// A querier in this process, i.e. of this build (a native run)
    Native,
    /// The RISC-V guest (the proving target)
    Guest,
}

/// A value that field hints answer with. The oracle writes it straight into its destination, as
/// the `u32` words of its representation on the target of the querier, which
/// [`read`](Self::read) then checks to be well-formed. Whether the value is the right answer is up
/// to the caller to check.
pub trait HintAnswer: Sized {
    /// Receives the value from the response to the current hint query into `dst`, and validates it.
    fn read<'a, O: MemoryOracle>(
        oracle: &mut O,
        dst: &'a mut MaybeUninit<Self>,
    ) -> Result<&'a mut Self, InternalError>;

    /// Oracle side: appends the words of the value, as [`read`](Self::read) receives them on
    /// `target`.
    fn write(&self, target: HintTarget, response: &mut Vec<u32>);
}

/// A flag: one word, any non-zero word is `true`.
impl HintAnswer for bool {
    #[inline(always)]
    fn read<'a, O: MemoryOracle>(
        oracle: &mut O,
        dst: &'a mut MaybeUninit<Self>,
    ) -> Result<&'a mut Self, InternalError> {
        Ok(dst.write(oracle.read_short()?))
    }

    fn write(&self, _target: HintTarget, response: &mut Vec<u32>) {
        response.push(u32::from(*self));
    }
}

/// The elements in turn.
impl<T: HintAnswer, const N: usize> HintAnswer for [T; N] {
    #[inline(always)]
    fn read<'a, O: MemoryOracle>(
        oracle: &mut O,
        dst: &'a mut MaybeUninit<Self>,
    ) -> Result<&'a mut Self, InternalError> {
        let first = dst.as_mut_ptr().cast::<MaybeUninit<T>>();
        for i in 0..N {
            // SAFETY: element `i` is inside `dst`, and `MaybeUninit<T>` has the layout of `T`
            T::read(oracle, unsafe { &mut *first.add(i) })?;
        }
        // SAFETY: every element was just initialized
        Ok(unsafe { dst.assume_init_mut() })
    }

    fn write(&self, target: HintTarget, response: &mut Vec<u32>) {
        for element in self {
            element.write(target, response);
        }
    }
}

macro_rules! impl_hint_answer_for_tuple {
    ($($element:ident $index:tt),+) => {
        /// The elements in turn.
        impl<$($element: HintAnswer),+> HintAnswer for ($($element,)+) {
            #[inline(always)]
            fn read<'a, O: MemoryOracle>(
                oracle: &mut O,
                dst: &'a mut MaybeUninit<Self>,
            ) -> Result<&'a mut Self, InternalError> {
                let this = dst.as_mut_ptr();
                // SAFETY: the elements are disjoint places inside `dst`, and `MaybeUninit<E>` has the
                // layout of `E`
                unsafe {
                    $(
                        $element::read(
                            oracle,
                            &mut *(&raw mut (*this).$index).cast::<MaybeUninit<$element>>(),
                        )?;
                    )+
                    // SAFETY: every element was just initialized
                    Ok(dst.assume_init_mut())
                }
            }

            fn write(&self, target: HintTarget, response: &mut Vec<u32>) {
                $(self.$index.write(target, response);)+
            }
        }
    };
}

impl_hint_answer_for_tuple!(A 0, B 1);
impl_hint_answer_for_tuple!(A 0, B 1, C 2);
impl_hint_answer_for_tuple!(A 0, B 1, C 2, D 3);

/// Receives a value of the answer to the current hint query.
#[inline(always)]
pub fn read_hint_answer<O: MemoryOracle, R: HintAnswer>(
    oracle: &mut O,
) -> Result<R, InternalError> {
    let mut value = MaybeUninit::uninit();
    R::read(oracle, &mut value)?;
    // SAFETY: initialized by `read`
    Ok(unsafe { value.assume_init() })
}

/// Sends the request for the hint `op` on `operand`, which the oracle reads where it is (see
/// [`FieldHintOp`]). The answer is then received with [`HintAnswer::read`] (or
/// [`read_hint_answer`]), piecemeal if needed, and the query ended with
/// [`MemoryOracle::finish_query`].
#[inline(always)]
pub fn send_field_hint_query<O: MemoryOracle, T: ?Sized>(
    oracle: &mut O,
    op: FieldHintOp,
    operand: &T,
) -> Result<(), InternalError> {
    let size = core::mem::size_of_val(operand);
    debug_assert!(size.is_multiple_of(4) && core::mem::align_of_val(operand) >= 4);
    // the oracle reads the request and the operand while it receives the query: their addresses
    // are exposed
    let request = GenericFieldOpsHint::<usize> {
        op: op as u32,
        src_ptr: core::ptr::from_ref(operand)
            .cast::<u8>()
            .expose_provenance(),
        src_len_u32_words: (size / 4) as u32,
    };
    oracle.send_query(
        FIELD_OPS_ADVISE_QUERY_ID,
        core::ptr::from_ref(&request).expose_provenance(),
    )
}

/// Asks the oracle for the hint `op` on `operand` (see [`send_field_hint_query`]), and receives the
/// answer straight into `dst`. The answer is checked to be well-formed only: it is up to the caller
/// to check that it is right.
#[inline(always)]
pub fn query_field_hint_into<'a, O: MemoryOracle, R: HintAnswer, T: ?Sized>(
    oracle: &mut O,
    op: FieldHintOp,
    operand: &T,
    dst: &'a mut MaybeUninit<R>,
) -> &'a mut R {
    send_field_hint_query(oracle, op, operand).expect("must send the field hint query");
    let answer = R::read(oracle, dst).expect("the hint answer is well-formed");
    oracle
        .finish_query()
        .expect("the hint answer has no excess data");
    answer
}

/// As [`query_field_hint_into`], with the answer returned as a new value.
#[inline(always)]
pub fn query_field_hint<O: MemoryOracle, R: HintAnswer, T: ?Sized>(
    oracle: &mut O,
    op: FieldHintOp,
    operand: &T,
) -> R {
    let mut answer = MaybeUninit::uninit();
    query_field_hint_into(oracle, op, operand, &mut answer);
    // SAFETY: initialized by `query_field_hint_into`
    unsafe { answer.assume_init() }
}

/// The oracle of the system as the oracle of the hints of `crypto` (`crypto::hints::MemoryOracle`),
/// which has the same four operations: the secp256k1 recovery of `crypto` queries it for its
/// witness, divisions, inverses and square root, and checks every answer itself.
pub struct CryptoHintsOracle<'a, O: MemoryOracle>(pub &'a mut O);

impl<O: MemoryOracle> crypto::hints::MemoryOracle for CryptoHintsOracle<'_, O> {
    type Error = HintError;

    #[inline(always)]
    unsafe fn send_query(&mut self, query_id: u32, input_word: usize) -> Result<(), HintError> {
        self.0
            .send_query(query_id, input_word)
            .map_err(|_| HintError("the oracle did not take the query"))
    }

    #[inline(always)]
    fn read_word(&mut self) -> Result<u32, HintError> {
        self.0
            .read_word()
            .map_err(|_| HintError("the oracle response is shorter than expected"))
    }

    #[inline(always)]
    fn read_u64(&mut self) -> Result<u64, HintError> {
        self.0
            .read_u64()
            .map_err(|_| HintError("the oracle response is shorter than expected"))
    }

    #[inline(always)]
    unsafe fn write_words(&mut self, dst: *mut u32, num_words: usize) -> Result<(), HintError> {
        // SAFETY: guaranteed by the caller
        unsafe { self.0.write_words(dst, num_words) }
            .map_err(|_| HintError("the oracle response is shorter than expected"))
    }

    #[inline(always)]
    fn finish_query(&mut self) -> Result<(), HintError> {
        self.0
            .finish_query()
            .map_err(|_| HintError("the oracle response contains excess data"))
    }
}
