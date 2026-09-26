//! Query for field operations hints, such as square root and inverse in secp256k1 fields together with their use for implementing secp256k1 hooks.

use alloc::vec::Vec;
use core::mem::{offset_of, MaybeUninit};
use crypto::secp256k1::field::FieldElement;
use crypto::secp256k1::scalars::Scalar;
use crypto::{bigint_op_delegation_raw, BigIntOps};
use zk_ee::{
    internal_error,
    oracle::{
        memory_io::{
            host::{read_querier_word, QuerierMemory},
            MemoryOracle,
        },
        query_ids::ADVICE_SUBSPACE_MASK,
        IOOracle,
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
/// [`Secp256k1Element`] and `curve_hints::guest_layout`). The answers are in the representation of
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
    /// The prover's claim about a bn254 pairing product (see `crypto::residue_witness` and
    /// `curve_hints::PairingClaim`): the slice of the affine pairs `(G1Affine, G2Affine)` in; an
    /// "is the identity" flag, then for an identity the witness `c`, `d = c^-1` (`Fq12` each) and
    /// the scaling factor (`Fq6`), otherwise the inverse of the Miller loop output (`Fq12`)
    Bn254PairingResidueWitness,
    /// The inverses of the two affine `G2` chains of a bn254 pairing input point (see
    /// `crypto::bn254::curves::g2_affine`): the `G2Affine` point in; a flag and the 93 inverses
    /// (`Fq2`) of the subgroup membership test, then a flag and the 87 inverses of the line
    /// precomputation out. A cleared flag means the chain hit an exceptional case and the
    /// projective computation is to be used.
    Bn254G2PairingInverses,
    /// The prover's claim about the KZG proof pairing product `e(P1, G2) e(P2, tau G2)`: the
    /// `G1Affine` points `[P1, P2]` in; an "is the identity" flag, then for an identity the
    /// witness `d` (`Fq12`) and the scaling factor (`Fq6`), otherwise the inverse of the Miller
    /// loop output (`Fq12`)
    Bls12381KzgResidueWitness,
}

impl FieldHintOp {
    pub fn parse_u32(value: u32) -> Option<Self> {
        const ALL: [FieldHintOp; 11] = [
            FieldHintOp::Secp256k1BaseFieldSqrt,
            FieldHintOp::Secp256k1BaseFieldInverse,
            FieldHintOp::Secp256k1ScalarFieldInverse,
            FieldHintOp::Bn254BaseFieldInverse,
            FieldHintOp::Bn254Fq12Inverse,
            FieldHintOp::Bls12381BaseFieldSqrt,
            FieldHintOp::Bls12381BaseFieldInverse,
            FieldHintOp::Bls12381Fq12Inverse,
            FieldHintOp::Bn254PairingResidueWitness,
            FieldHintOp::Bn254G2PairingInverses,
            FieldHintOp::Bls12381KzgResidueWitness,
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

/// `2^256 mod p` and `2^256 mod n`, the Montgomery form of 1, in little-endian words
const R_MOD_P_WORDS: [u32; 8] = [0x3d1, 0x1, 0, 0, 0, 0, 0, 0];
const R_MOD_N_WORDS: [u32; 8] = [0x2fc9bebf, 0x402da173, 0x50b75fc4, 0x45512319, 0x1, 0, 0, 0];
/// `2^-256 mod p` and `2^-256 mod n`, big-endian
const R_INVERSE_MOD_P: [u8; 32] = [
    0xc9, 0xbd, 0x19, 0x05, 0x15, 0x53, 0x83, 0x99, 0x9c, 0x46, 0xc2, 0xc2, 0x95, 0xf2, 0xb7, 0x61,
    0xbc, 0xb2, 0x23, 0xfe, 0xdc, 0x24, 0xa0, 0x59, 0xd8, 0x38, 0x09, 0x1d, 0x08, 0x68, 0x19, 0x2a,
];
const R_INVERSE_MOD_N: [u8; 32] = [
    0xd9, 0xe8, 0x89, 0x0d, 0x64, 0x94, 0xef, 0x93, 0x89, 0x7f, 0x30, 0xc1, 0x27, 0xcf, 0xab, 0x5d,
    0x3b, 0xbb, 0xd4, 0x56, 0x7f, 0xa5, 0x0c, 0x3c, 0x80, 0xfd, 0x22, 0x93, 0x80, 0x97, 0xc0, 0x16,
];

const fn words_eq(a: &[u32; 8], b: &[u32; 8]) -> bool {
    let mut i = 0;
    while i < 8 {
        if a[i] != b[i] {
            return false;
        }
        i += 1;
    }
    true
}

/// The first 8 words of the representation of `value`
///
/// # Safety
///
/// `T` must be at least 32 bytes, aligned for `u32`, without padding in its first 32 bytes.
const unsafe fn leading_words<T>(value: &T) -> [u32; 8] {
    // SAFETY: guaranteed by the caller
    unsafe { *core::ptr::from_ref(value).cast::<[u32; 8]>() }
}

/// Whether the little-endian `value` is below the little-endian `modulus`
fn is_below(value: &[u32; 8], modulus: &[u32; 8]) -> bool {
    for (value, modulus) in value.iter().zip(modulus).rev() {
        if value != modulus {
            return value < modulus;
        }
    }
    false
}

fn be_bytes(words: &[u32; 8]) -> [u8; 32] {
    let mut bytes = [0u8; 32];
    for (word, out) in words.iter().rev().zip(bytes.as_chunks_mut::<4>().0) {
        *out = word.to_be_bytes();
    }
    bytes
}

fn words_of_be_bytes(bytes: &[u8; 32]) -> [u32; 8] {
    let mut words = [0u32; 8];
    for (word, chunk) in words.iter_mut().rev().zip(bytes.as_chunks::<4>().0) {
        *word = u32::from_be_bytes(*chunk);
    }
    words
}

/// A secp256k1 element, `FieldElement` (modulo `p`) or `Scalar` (modulo `n`), in the secp256k1
/// hints. The operand is the element itself, which the oracle reads where it is, and the answer an
/// integer in 8 little-endian `u32` words, in the representation of the target of the querier.
/// - On the RISC-V guest it is the Montgomery form `x 2^256 mod m`, which is the in-memory
///   representation of both types there (checked at compile time): the element is treated as an
///   opaque 32-byte value, the oracle reads an operand as those words (any 256-bit
///   representative), and writes an answer straight into the destination element, which is then
///   only checked to be below the modulus.
/// - For a querier in this process (native builds, whose representations are private to the
///   types) the oracle reads an operand as a value of the type, and an answer is the canonical
///   integer, through the API of the types.
///
/// The oracle converts between them (see [`HintAnswer::write`]).
pub trait Secp256k1Element: Copy {
    /// The modulus, in little-endian words
    const MODULUS: [u32; 8];
    /// `2^256 mod m`, the Montgomery form of 1, in little-endian words
    const MONTGOMERY_ONE: [u32; 8];
    /// `2^-256 mod m`, big-endian
    const R_INVERSE: [u8; 32];
    /// Whether the representation of the element in this build is its Montgomery words, as on the
    /// RISC-V guest
    const IS_MONTGOMERY_WORDS: bool;

    /// The canonical big-endian bytes of the element
    fn to_be_bytes(self) -> [u8; 32];

    /// The element of canonical big-endian bytes, `None` for the modulus or above
    fn from_be_bytes(bytes: &[u8; 32]) -> Option<Self>;

    /// The product of the elements
    fn multiply(self, rhs: Self) -> Self;

    /// The words of the canonical integer of the element
    fn to_canonical_words(self) -> [u32; 8] {
        words_of_be_bytes(&self.to_be_bytes())
    }

    /// The element of the words of a canonical integer, `None` for the modulus or above
    fn from_canonical_words(words: &[u32; 8]) -> Option<Self> {
        Self::from_be_bytes(&be_bytes(words))
    }

    /// The Montgomery words of the element, below the modulus
    fn to_montgomery_words(self) -> [u32; 8] {
        let r = Self::from_canonical_words(&Self::MONTGOMERY_ONE).expect("below the modulus");
        self.multiply(r).to_canonical_words()
    }

    /// The element of Montgomery words below the modulus
    fn from_montgomery_words(words: &[u32; 8]) -> Self {
        let r_inverse = Self::from_be_bytes(&Self::R_INVERSE).expect("below the modulus");
        Self::from_canonical_words(words)
            .expect("below the modulus")
            .multiply(r_inverse)
    }

    /// The element of the words of an operand of the RISC-V guest: Montgomery words, which may be
    /// any 256-bit representative (below `2^256 < 2 m`)
    fn from_guest_operand_words(words: &[u32; 8]) -> Self {
        let mut reduced = *words;
        if !is_below(&reduced, &Self::MODULUS) {
            let mut borrow = false;
            for (word, modulus) in reduced.iter_mut().zip(&Self::MODULUS) {
                let (difference, borrow_1) = word.overflowing_sub(*modulus);
                let (difference, borrow_2) = difference.overflowing_sub(u32::from(borrow));
                *word = difference;
                borrow = borrow_1 | borrow_2;
            }
        }
        Self::from_montgomery_words(&reduced)
    }

    /// Checks the Montgomery words the oracle answered the RISC-V guest with, in place at `words`,
    /// to be below the modulus, with the bigint delegation (see `check_below_with_delegation`)
    ///
    /// # Safety
    ///
    /// `words` must be valid for reads and writes, and 32-byte aligned: a delegation operand.
    unsafe fn validate(words: *mut [u32; 8]) -> Result<(), InternalError>;
}

/// A 32-byte value as an operand of the bigint delegation, which works on 32-byte aligned memory.
/// As an immutable static it lands in `.rodata`, which the linker places above the ROM bound.
#[repr(C, align(32))]
struct DelegationOperand([u32; 8]);

static FIELD_MODULUS: DelegationOperand =
    DelegationOperand(<FieldElement as Secp256k1Element>::MODULUS);
static SCALAR_MODULUS: DelegationOperand = DelegationOperand(<Scalar as Secp256k1Element>::MODULUS);

/// Checks the value at `words` to be below `modulus` with the bigint delegation: subtracting the
/// modulus borrows exactly then, and adding it back restores the value.
///
/// # Safety
///
/// `words` must be valid for reads and writes, and 32-byte aligned.
#[inline(always)]
unsafe fn check_below_with_delegation(
    words: *mut [u32; 8],
    modulus: &'static DelegationOperand,
) -> Result<(), InternalError> {
    let modulus = core::ptr::from_ref(modulus).cast::<()>();
    // SAFETY: both operands are distinct 32-byte aligned 32-byte values in RAM, and the delegation
    // only writes the first one
    let borrow = unsafe { bigint_op_delegation_raw(words.cast(), modulus, BigIntOps::Sub) } != 0;
    // SAFETY: as above
    unsafe { bigint_op_delegation_raw(words.cast(), modulus, BigIntOps::Add) };
    if borrow {
        Ok(())
    } else {
        Err(internal_error!("the secp256k1 hint is not canonical"))
    }
}

impl Secp256k1Element for FieldElement {
    const MODULUS: [u32; 8] = [
        0xfffffc2f, 0xfffffffe, 0xffffffff, 0xffffffff, 0xffffffff, 0xffffffff, 0xffffffff,
        0xffffffff,
    ];
    const MONTGOMERY_ONE: [u32; 8] = R_MOD_P_WORDS;
    const R_INVERSE: [u8; 32] = R_INVERSE_MOD_P;
    const IS_MONTGOMERY_WORDS: bool = core::mem::size_of::<Self>() == 32
        && core::mem::align_of::<Self>() >= core::mem::align_of::<u32>()
        // SAFETY: the element is 32 bytes, aligned for `u32`: the 4 limbs of the delegated
        // representation, without padding (reading padding would fail the compile-time evaluation,
        // not go unnoticed); `ONE` and `ZERO` then tell whether they are the Montgomery words
        && words_eq(unsafe { &leading_words(&Self::ONE) }, &Self::MONTGOMERY_ONE)
        && words_eq(unsafe { &leading_words(&Self::ZERO) }, &[0; 8]);

    fn to_be_bytes(self) -> [u8; 32] {
        self.to_bytes().into()
    }

    fn from_be_bytes(bytes: &[u8; 32]) -> Option<Self> {
        Self::from_bytes(bytes)
    }

    fn multiply(mut self, rhs: Self) -> Self {
        self.mul_in_place(&rhs);
        self
    }

    unsafe fn validate(words: *mut [u32; 8]) -> Result<(), InternalError> {
        // SAFETY: guaranteed by the caller
        unsafe { check_below_with_delegation(words, &FIELD_MODULUS) }
    }
}

impl Secp256k1Element for Scalar {
    const MODULUS: [u32; 8] = [
        0xd0364141, 0xbfd25e8c, 0xaf48a03b, 0xbaaedce6, 0xfffffffe, 0xffffffff, 0xffffffff,
        0xffffffff,
    ];
    const MONTGOMERY_ONE: [u32; 8] = R_MOD_N_WORDS;
    const R_INVERSE: [u8; 32] = R_INVERSE_MOD_N;
    const IS_MONTGOMERY_WORDS: bool = core::mem::size_of::<Self>() == 32
        && core::mem::align_of::<Self>() >= core::mem::align_of::<u32>()
        // SAFETY: as for `FieldElement`
        && words_eq(unsafe { &leading_words(&Self::ONE) }, &Self::MONTGOMERY_ONE)
        && words_eq(unsafe { &leading_words(&Self::ZERO) }, &[0; 8]);

    fn to_be_bytes(self) -> [u8; 32] {
        self.to_repr().into()
    }

    fn from_be_bytes(bytes: &[u8; 32]) -> Option<Self> {
        is_below(&words_of_be_bytes(bytes), &Self::MODULUS).then(|| scalar_of_be_bytes(bytes))
    }

    fn multiply(mut self, rhs: Self) -> Self {
        self *= rhs;
        self
    }

    unsafe fn validate(words: *mut [u32; 8]) -> Result<(), InternalError> {
        // SAFETY: guaranteed by the caller
        unsafe { check_below_with_delegation(words, &SCALAR_MODULUS) }
    }
}

/// The scalar of a big-endian integer below the order
fn scalar_of_be_bytes(bytes: &[u8; 32]) -> Scalar {
    use crypto::k256::elliptic_curve::scalar::FromUintUnchecked;
    Scalar::from_k256_scalar(crypto::k256::Scalar::from_uint_unchecked(
        crypto::k256::U256::from_be_slice(bytes),
    ))
}

/// Receives an element in the representation of this target (see [`Secp256k1Element`]), validated:
/// on the RISC-V guest straight into the destination.
#[inline(always)]
fn read_secp256k1_element<'a, T: Secp256k1Element, O: MemoryOracle>(
    oracle: &mut O,
    dst: &'a mut MaybeUninit<T>,
) -> Result<&'a mut T, InternalError> {
    #[cfg(target_arch = "riscv32")]
    {
        // a change of the representation in `airbender-crypto` must be noticed here: the element is
        // its Montgomery words, 32-byte aligned as the delegated representation, which `validate`
        // relies on
        const { assert!(T::IS_MONTGOMERY_WORDS && core::mem::align_of::<T>() >= 32) };
        let words = dst.as_mut_ptr().cast::<[u32; 8]>();
        // SAFETY: the element is 8 words, 32-byte aligned (checked above), which initialize it
        unsafe {
            oracle.write_words(words.cast::<u32>(), 8)?;
            T::validate(words)?;
            Ok(dst.assume_init_mut())
        }
    }
    #[cfg(not(target_arch = "riscv32"))]
    {
        let mut words = [0u32; 8];
        // SAFETY: the buffer holds 8 words
        unsafe { oracle.write_words(words.as_mut_ptr(), 8)? };
        let element = T::from_canonical_words(&words)
            .ok_or_else(|| internal_error!("the secp256k1 hint is not canonical"))?;
        Ok(dst.write(element))
    }
}

macro_rules! impl_hint_answer_for_secp256k1_element {
    ($($t:ty),+) => {$(
        impl HintAnswer for $t {
            #[inline(always)]
            fn read<'a, O: MemoryOracle>(
                oracle: &mut O,
                dst: &'a mut MaybeUninit<Self>,
            ) -> Result<&'a mut Self, InternalError> {
                read_secp256k1_element(oracle, dst)
            }

            fn write(&self, target: HintTarget, response: &mut Vec<u32>) {
                response.extend_from_slice(&match target {
                    HintTarget::Native => self.to_canonical_words(),
                    HintTarget::Guest => self.to_montgomery_words(),
                });
            }
        }
    )+};
}

impl_hint_answer_for_secp256k1_element!(FieldElement, Scalar);

/// Secp256k1 hooks implementation that uses an IOOracle for field operations.
pub struct Secp256k1HooksWithOracle<'a, O: IOOracle> {
    oracle: &'a mut O,
}

impl<'a, O: IOOracle> Secp256k1HooksWithOracle<'a, O> {
    pub fn new(oracle: &'a mut O) -> Self {
        Self { oracle }
    }
}

impl<'a, O: IOOracle> crypto::secp256k1::hooks::Secp256k1Hooks for Secp256k1HooksWithOracle<'a, O> {
    /// An inversion is a hint checked with one multiplication, so the scalar multiplication
    /// makes its table affine (one inversion) instead of carrying a shared denominator
    const FE_INVERT_IS_CHEAP: bool = true;

    fn fe_sqrt_and_assign(&mut self, x: &mut FieldElement) -> bool {
        // Match default hook semantics: sqrt(0) exists and equals 0.
        if x.is_zero() {
            return true;
        }

        let operand = *x;
        let is_quadratic_non_residue: bool =
            self.query_into(FieldHintOp::Secp256k1BaseFieldSqrt, x, |oracle| {
                read_hint_answer(oracle)
            });

        // Verify the oracle's hint is correct.
        // The oracle computes candidate = x^((p+1)/4). For secp256k1's prime p ≡ 3 (mod 4):
        // - If x is a quadratic residue (has a sqrt): candidate² == x
        // - If x is a quadratic non-residue (no sqrt): candidate² == -x
        let mut squared = *x;
        squared.square_in_place();
        if is_quadratic_non_residue == false {
            squared.sub_in_place(&operand);
            assert!(squared.is_zero()); // candidate² - x == 0
        } else {
            squared.add_in_place(&operand);
            assert!(squared.is_zero()); // candidate² + x == 0  (i.e., candidate² == -x)
        }

        // Return true if square root exists (x is a quadratic residue)
        !is_quadratic_non_residue
    }

    fn fe_invert_and_assign(&mut self, x: &mut FieldElement) {
        // Match default hook semantics: invert(0) == 0.
        if x.is_zero() {
            return;
        }

        // the operand, multiplied by the answer below
        let mut product = *x;
        self.query_into(FieldHintOp::Secp256k1BaseFieldInverse, x, |_| Ok(()));

        // we must check that hint was correct
        product.mul_in_place(x);
        assert!(product.is_one());
    }

    fn scalar_invert_and_assign(&mut self, x: &mut Scalar) {
        // Match default hook semantics: invert(0) == 0.
        if x.is_zero() {
            return;
        }

        // the operand, multiplied by the answer below
        let mut product = *x;
        self.query_into(FieldHintOp::Secp256k1ScalarFieldInverse, x, |_| Ok(()));

        // we must check that hint was correct
        product *= &*x;
        assert!(product.is_one());
    }
}

impl<'a, O: IOOracle> Secp256k1HooksWithOracle<'a, O> {
    /// Asks for the hint `op` on `x`, and receives the answer, an element, into `x` (on the RISC-V
    /// guest straight, see [`Secp256k1Element`]), then the rest of the answer with `rest`. The
    /// oracle reads the operand when it receives the query, before the answer overwrites it.
    #[inline(always)]
    fn query_into<T: Secp256k1Element + HintAnswer, R>(
        &mut self,
        op: FieldHintOp,
        x: &mut T,
        rest: impl FnOnce(&mut O) -> Result<R, InternalError>,
    ) -> R {
        send_field_hint_query(self.oracle, op, &*x).expect("must send the field hint query");
        // SAFETY: `MaybeUninit<T>` has the layout of `T`, and `read` writes a valid `T` into it, or
        // on the RISC-V guest the oracle's words, which are a valid `T` too (plain integers), so `x`
        // stays valid even if the answer is rejected
        let dst = unsafe { &mut *core::ptr::from_mut(x).cast::<MaybeUninit<T>>() };
        T::read(self.oracle, dst).expect("the hint answer is well-formed");
        let rest = rest(self.oracle).expect("the hint answer is well-formed");
        self.oracle
            .finish_query()
            .expect("the hint answer has no excess data");
        rest
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use callable_oracles::field_hints::NativeFieldOpsQuery;
    use crypto::secp256k1::hooks::{DefaultSecp256k1Hooks, Secp256k1Hooks};
    use oracle_provider::ZkEENonDeterminismSource;
    use proptest::{prop_assert, prop_assert_eq, proptest};

    fn create_oracle_with_field_ops() -> ZkEENonDeterminismSource {
        let mut oracle = ZkEENonDeterminismSource::default();
        oracle.add_external_processor(NativeFieldOpsQuery);
        oracle
    }

    /// The integer of little-endian words plus the modulus, if it stays below `2^256`
    fn plus_modulus(words: &[u32; 8], modulus: &[u32; 8]) -> Option<[u32; 8]> {
        let mut sum = [0u32; 8];
        let mut carry = 0u64;
        for ((out, word), modulus) in sum.iter_mut().zip(words).zip(modulus) {
            let t = u64::from(*word) + u64::from(*modulus) + carry;
            *out = t as u32;
            carry = t >> 32;
        }
        (carry == 0).then_some(sum)
    }

    #[test]
    fn secp256k1_montgomery_words_round_trip() {
        // the Montgomery form of 1 is `2^256` reduced; `from_montgomery_words` multiplies by the
        // inverse constants, so this also checks them
        assert_eq!(FieldElement::ONE.to_montgomery_words(), R_MOD_P_WORDS);
        assert_eq!(Scalar::ONE.to_montgomery_words(), R_MOD_N_WORDS);
        assert_eq!(
            FieldElement::from_montgomery_words(&R_MOD_P_WORDS).to_bytes(),
            FieldElement::ONE.to_bytes()
        );
        assert_eq!(
            Scalar::from_montgomery_words(&R_MOD_N_WORDS).to_repr(),
            Scalar::ONE.to_repr()
        );
        proptest!(|(bytes: [u8; 32])| {
            if let Some(fe) = FieldElement::from_bytes(&bytes) {
                let words = fe.to_montgomery_words();
                prop_assert!(is_below(&words, &FieldElement::MODULUS));
                prop_assert_eq!(FieldElement::from_montgomery_words(&words).to_bytes(), fe.to_bytes());
                // an operand of the guest may be any representative below 2^256
                if let Some(unreduced) = plus_modulus(&words, &FieldElement::MODULUS) {
                    prop_assert_eq!(FieldElement::from_guest_operand_words(&unreduced).to_bytes(), fe.to_bytes());
                }
                let words = fe.to_canonical_words();
                prop_assert_eq!(be_bytes(&words), <[u8; 32]>::from(fe.to_bytes()));
                prop_assert_eq!(FieldElement::from_canonical_words(&words).map(|fe| fe.to_bytes()), Some(fe.to_bytes()));
            }
            use crypto::k256::elliptic_curve::Curve;
            if crypto::k256::U256::from_be_slice(&bytes) < crypto::k256::Secp256k1::ORDER {
                let scalar = scalar_of_be_bytes(&bytes);
                let words = scalar.to_montgomery_words();
                prop_assert!(is_below(&words, &Scalar::MODULUS));
                prop_assert_eq!(Scalar::from_montgomery_words(&words).to_repr(), scalar.to_repr());
                if let Some(unreduced) = plus_modulus(&words, &Scalar::MODULUS) {
                    prop_assert_eq!(Scalar::from_guest_operand_words(&unreduced).to_repr(), scalar.to_repr());
                }
                let words = scalar.to_canonical_words();
                prop_assert_eq!(be_bytes(&words), bytes);
                prop_assert_eq!(Scalar::from_canonical_words(&words).map(|s| s.to_repr()), Some(scalar.to_repr()));
            }
        });
    }

    /// The delegated check of the words of a guest answer, and the words it leaves
    fn validate<T: Secp256k1Element>(words: [u32; 8]) -> (bool, [u32; 8]) {
        let mut operand = DelegationOperand(words);
        // SAFETY: the words are a 32-byte aligned local
        let valid = unsafe { T::validate(&raw mut operand.0) }.is_ok();
        (valid, operand.0)
    }

    fn below<T: Secp256k1Element>(words: [u32; 8]) -> bool {
        let (valid, left) = validate::<T>(words);
        // the check leaves the value as it was
        assert_eq!(left, words);
        valid
    }

    #[test]
    fn secp256k1_answers_must_be_below_the_modulus() {
        fn check<T: Secp256k1Element>() {
            let mut below_modulus = T::MODULUS;
            below_modulus[0] -= 1;
            assert!(!below::<T>(T::MODULUS));
            assert!(!below::<T>([u32::MAX; 8]));
            assert!(below::<T>(below_modulus));
            assert!(below::<T>([0; 8]));
            assert!(below::<T>(T::MONTGOMERY_ONE));
            assert!(T::from_canonical_words(&T::MODULUS).is_none());
            assert!(T::from_canonical_words(&below_modulus).is_some());
        }
        check::<FieldElement>();
        check::<Scalar>();
        proptest!(|(words: [u32; 8])| {
            prop_assert_eq!(below::<FieldElement>(words), is_below(&words, &FieldElement::MODULUS));
            prop_assert_eq!(below::<Scalar>(words), is_below(&words, &Scalar::MODULUS));
        });
    }

    fn answer_words<T: HintAnswer>(value: &T, target: HintTarget) -> Vec<u32> {
        let mut response = Vec::new();
        value.write(target, &mut response);
        response
    }

    #[test]
    fn secp256k1_answers_follow_the_target() {
        let fe = FieldElement::from_bytes(&[7; 32]).unwrap();
        let scalar = scalar_of_be_bytes(&[7; 32]);
        assert_eq!(
            answer_words(&fe, HintTarget::Native),
            fe.to_canonical_words()
        );
        assert_eq!(
            answer_words(&fe, HintTarget::Guest),
            fe.to_montgomery_words()
        );
        assert_eq!(
            answer_words(&scalar, HintTarget::Native),
            scalar.to_canonical_words()
        );
        assert_eq!(
            answer_words(&scalar, HintTarget::Guest),
            scalar.to_montgomery_words()
        );
        assert_ne!(fe.to_canonical_words(), fe.to_montgomery_words());
    }

    #[test]
    fn test_fe_sqrt_oracle_matches_default() {
        proptest!(|(bytes: [u8; 32])| {
            let Some(fe) = FieldElement::from_bytes(&bytes) else {
                return Ok(());
            };
            if fe.normalizes_to_zero() {
                return Ok(());
            }

            let mut fe_default = fe;
            let result_default = DefaultSecp256k1Hooks.fe_sqrt_and_assign(&mut fe_default);

            let mut oracle = create_oracle_with_field_ops();
            let mut fe_oracle = fe;
            let result_oracle = Secp256k1HooksWithOracle::new(&mut oracle)
                .fe_sqrt_and_assign(&mut fe_oracle);

            prop_assert_eq!(result_default, result_oracle, "sqrt existence should match");
            prop_assert_eq!(fe_default.to_bytes(), fe_oracle.to_bytes(), "sqrt values should match");
        });
    }

    #[test]
    fn test_fe_invert_oracle_matches_default() {
        proptest!(|(bytes: [u8; 32])| {
            let Some(fe) = FieldElement::from_bytes(&bytes) else {
                return Ok(());
            };
            if fe.normalizes_to_zero() {
                return Ok(());
            }

            let mut fe_default = fe;
            DefaultSecp256k1Hooks.fe_invert_and_assign(&mut fe_default);

            let mut oracle = create_oracle_with_field_ops();
            let mut fe_oracle = fe;
            Secp256k1HooksWithOracle::new(&mut oracle).fe_invert_and_assign(&mut fe_oracle);

            prop_assert_eq!(fe_default.to_bytes(), fe_oracle.to_bytes(), "inverse values should match");
        });
    }

    #[test]
    fn test_scalar_invert_oracle_matches_default() {
        proptest!(|(bytes: [u8; 32])| {
            use crypto::k256::elliptic_curve::scalar::FromUintUnchecked;
            use crypto::k256::elliptic_curve::Curve;
            use crypto::k256::U256;

            let val = U256::from_be_slice(&bytes);
            if val >= crypto::k256::Secp256k1::ORDER || val == U256::ZERO {
                return Ok(());
            }

            let scalar = Scalar::from_k256_scalar(
                crypto::k256::Scalar::from_uint_unchecked(val)
            );

            let mut scalar_default = scalar;
            DefaultSecp256k1Hooks.scalar_invert_and_assign(&mut scalar_default);

            let mut oracle = create_oracle_with_field_ops();
            let mut scalar_oracle = scalar;
            Secp256k1HooksWithOracle::new(&mut oracle).scalar_invert_and_assign(&mut scalar_oracle);

            prop_assert_eq!(scalar_default.to_repr(), scalar_oracle.to_repr(), "scalar inverse values should match");
        });
    }

    /// Tests that verify the validation logic catches lying oracles.
    /// These tests ensure that incorrect oracle responses are rejected.
    mod malicious_oracle_tests {
        use super::*;
        use oracle_provider::OracleQueryProcessor;
        use proptest::prop_assert;

        /// Ways to corrupt oracle responses
        enum Corruption {
            /// Return all zeros
            ReturnZero,
            /// Flip the least significant bit of the result
            FlipLsb,
            /// Add 1 to the result (wrapping)
            AddOne,
            /// Return a fixed arbitrary value
            ReturnArbitrary([u8; 32]),
        }

        impl Corruption {
            fn apply(&self, data: &mut [u8]) {
                match self {
                    Corruption::ReturnZero => data.fill(0),
                    Corruption::FlipLsb => {
                        if !data.is_empty() {
                            data[data.len() - 1] ^= 1;
                        }
                    }
                    Corruption::AddOne => {
                        // Add 1 with carry propagation (big-endian)
                        let mut carry = 1u16;
                        for byte in data.iter_mut().rev() {
                            let sum = *byte as u16 + carry;
                            *byte = sum as u8;
                            carry = sum >> 8;
                        }
                    }
                    Corruption::ReturnArbitrary(val) => {
                        data.copy_from_slice(val);
                    }
                }
            }
        }

        /// A malicious oracle processor that wraps a correct one and corrupts its output
        struct LyingFieldOpsQuery {
            inner: callable_oracles::field_hints::NativeFieldOpsQuery,
            corruption: Corruption,
            /// If set, lie about sqrt existence (flip the boolean)
            lie_about_sqrt_existence: bool,
        }

        impl LyingFieldOpsQuery {
            fn new(corruption: Corruption) -> Self {
                Self {
                    inner: callable_oracles::field_hints::NativeFieldOpsQuery,
                    corruption,
                    lie_about_sqrt_existence: false,
                }
            }

            fn with_sqrt_existence_lie(mut self) -> Self {
                self.lie_about_sqrt_existence = true;
                self
            }
        }

        impl OracleQueryProcessor for LyingFieldOpsQuery {
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
                // Get the correct response of the native run (the querier of the tests)
                let mut correct_response = Vec::new();
                self.inner.process_memory_query(
                    query_id,
                    input_word,
                    memory,
                    mode,
                    &mut correct_response,
                    guest_run_responses,
                );

                // Determine if this is a sqrt query (returns an element + bool) or inverse query (returns an element)
                // sqrt response: 8 words for the element + 1 word for bool = 9 words
                // inverse response: 8 words for the element
                let is_sqrt_query = correct_response.len() == 9;

                let mut corrupted = correct_response.clone();

                if is_sqrt_query && self.lie_about_sqrt_existence {
                    // Flip the boolean (last element)
                    corrupted[8] ^= 1;
                } else {
                    // Corrupt the element (the first 8 words), as the big-endian bytes of the
                    // integer of its words
                    let words: [u32; 8] = corrupted[..8].try_into().unwrap();
                    let mut bytes = be_bytes(&words);
                    self.corruption.apply(&mut bytes);
                    corrupted[..8].copy_from_slice(&words_of_be_bytes(&bytes));
                }

                native_run_responses.extend(corrupted);
            }
        }

        fn create_lying_oracle(corruption: Corruption) -> ZkEENonDeterminismSource {
            let mut oracle = ZkEENonDeterminismSource::default();
            oracle.add_external_processor(LyingFieldOpsQuery::new(corruption));
            oracle
        }

        fn create_sqrt_existence_lying_oracle() -> ZkEENonDeterminismSource {
            let mut oracle = ZkEENonDeterminismSource::default();
            oracle.add_external_processor(
                LyingFieldOpsQuery::new(Corruption::ReturnZero).with_sqrt_existence_lie(),
            );
            oracle
        }

        // A known valid field element for testing (small value, definitely in field)
        fn test_field_element() -> FieldElement {
            let mut bytes = [0u8; 32];
            bytes[31] = 7; // Small non-zero value
            FieldElement::from_bytes(&bytes).unwrap()
        }

        fn test_scalar() -> Scalar {
            use crypto::k256::elliptic_curve::scalar::FromUintUnchecked;
            let mut bytes = [0u8; 32];
            bytes[31] = 7;
            Scalar::from_k256_scalar(crypto::k256::Scalar::from_uint_unchecked(
                crypto::k256::U256::from_be_slice(&bytes),
            ))
        }

        // ============ fe_invert tests ============

        #[test]
        #[should_panic]
        fn test_fe_invert_rejects_zero_answer() {
            let mut oracle = create_lying_oracle(Corruption::ReturnZero);
            let mut fe = test_field_element();
            Secp256k1HooksWithOracle::new(&mut oracle).fe_invert_and_assign(&mut fe);
        }

        #[test]
        #[should_panic]
        fn test_fe_invert_rejects_flipped_bit() {
            let mut oracle = create_lying_oracle(Corruption::FlipLsb);
            let mut fe = test_field_element();
            Secp256k1HooksWithOracle::new(&mut oracle).fe_invert_and_assign(&mut fe);
        }

        #[test]
        #[should_panic]
        fn test_fe_invert_rejects_off_by_one() {
            let mut oracle = create_lying_oracle(Corruption::AddOne);
            let mut fe = test_field_element();
            Secp256k1HooksWithOracle::new(&mut oracle).fe_invert_and_assign(&mut fe);
        }

        #[test]
        #[should_panic]
        fn test_fe_invert_rejects_arbitrary_value() {
            let arbitrary = [0x42u8; 32];
            let mut oracle = create_lying_oracle(Corruption::ReturnArbitrary(arbitrary));
            let mut fe = test_field_element();
            Secp256k1HooksWithOracle::new(&mut oracle).fe_invert_and_assign(&mut fe);
        }

        // ============ fe_sqrt tests ============

        #[test]
        #[should_panic]
        fn test_fe_sqrt_rejects_wrong_sqrt_value() {
            let mut oracle = create_lying_oracle(Corruption::FlipLsb);
            let mut fe = test_field_element();
            Secp256k1HooksWithOracle::new(&mut oracle).fe_sqrt_and_assign(&mut fe);
        }

        #[test]
        #[should_panic]
        fn test_fe_sqrt_rejects_zero_answer() {
            let mut oracle = create_lying_oracle(Corruption::ReturnZero);
            let mut fe = test_field_element();
            Secp256k1HooksWithOracle::new(&mut oracle).fe_sqrt_and_assign(&mut fe);
        }

        #[test]
        #[should_panic]
        fn test_fe_sqrt_rejects_lie_about_existence() {
            // This test uses an oracle that returns the correct sqrt value but lies
            // about whether a sqrt exists (flips the boolean)
            let mut oracle = create_sqrt_existence_lying_oracle();
            let mut fe = test_field_element();
            Secp256k1HooksWithOracle::new(&mut oracle).fe_sqrt_and_assign(&mut fe);
        }

        // ============ scalar_invert tests ============

        #[test]
        #[should_panic]
        fn test_scalar_invert_rejects_zero_answer() {
            let mut oracle = create_lying_oracle(Corruption::ReturnZero);
            let mut scalar = test_scalar();
            Secp256k1HooksWithOracle::new(&mut oracle).scalar_invert_and_assign(&mut scalar);
        }

        #[test]
        #[should_panic]
        fn test_scalar_invert_rejects_flipped_bit() {
            let mut oracle = create_lying_oracle(Corruption::FlipLsb);
            let mut scalar = test_scalar();
            Secp256k1HooksWithOracle::new(&mut oracle).scalar_invert_and_assign(&mut scalar);
        }

        #[test]
        #[should_panic]
        fn test_scalar_invert_rejects_off_by_one() {
            let mut oracle = create_lying_oracle(Corruption::AddOne);
            let mut scalar = test_scalar();
            Secp256k1HooksWithOracle::new(&mut oracle).scalar_invert_and_assign(&mut scalar);
        }

        #[test]
        #[should_panic]
        fn test_scalar_invert_rejects_arbitrary_value() {
            let arbitrary = [0x42u8; 32];
            let mut oracle = create_lying_oracle(Corruption::ReturnArbitrary(arbitrary));
            let mut scalar = test_scalar();
            Secp256k1HooksWithOracle::new(&mut oracle).scalar_invert_and_assign(&mut scalar);
        }

        // ============ Proptest: random corruptions should be rejected ============

        #[test]
        fn test_fe_invert_rejects_random_corruptions() {
            proptest!(|(bytes: [u8; 32], corruption_bytes: [u8; 32])| {
                let Some(fe) = FieldElement::from_bytes(&bytes) else {
                    return Ok(());
                };
                if fe.normalizes_to_zero() {
                    return Ok(());
                }

                // Get the correct inverse first
                let mut correct_fe = fe;
                let mut correct_oracle = create_oracle_with_field_ops();
                Secp256k1HooksWithOracle::new(&mut correct_oracle).fe_invert_and_assign(&mut correct_fe);
                // the answer on the wire: its canonical integer
                let correct_inverse = be_bytes(&correct_fe.to_canonical_words());

                // Skip if random corruption happens to equal the correct answer
                if corruption_bytes == correct_inverse {
                    return Ok(());
                }

                // Now try with the corrupted oracle
                let mut lying_oracle = create_lying_oracle(Corruption::ReturnArbitrary(corruption_bytes));
                let mut test_fe = fe;
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    Secp256k1HooksWithOracle::new(&mut lying_oracle).fe_invert_and_assign(&mut test_fe);
                }));

                // The validation should have caught the lie (panicked)
                prop_assert!(result.is_err(), "Oracle lie was not detected for input {:?}", bytes);
            });
        }

        #[test]
        fn test_scalar_invert_rejects_random_corruptions() {
            proptest!(|(bytes: [u8; 32], corruption_bytes: [u8; 32])| {
                use crypto::k256::elliptic_curve::scalar::FromUintUnchecked;
                use crypto::k256::elliptic_curve::Curve;
                use crypto::k256::U256;

                let val = U256::from_be_slice(&bytes);
                if val >= crypto::k256::Secp256k1::ORDER || val == U256::ZERO {
                    return Ok(());
                }

                let scalar = Scalar::from_k256_scalar(
                    crypto::k256::Scalar::from_uint_unchecked(val)
                );

                // Get the correct inverse first
                let mut correct_scalar = scalar;
                let mut correct_oracle = create_oracle_with_field_ops();
                Secp256k1HooksWithOracle::new(&mut correct_oracle).scalar_invert_and_assign(&mut correct_scalar);
                // the answer on the wire: its canonical integer
                let correct_inverse = be_bytes(&correct_scalar.to_canonical_words());

                // Skip if random corruption happens to equal the correct answer
                if corruption_bytes == correct_inverse {
                    return Ok(());
                }

                // Also skip if corruption_bytes >= ORDER (would fail the validation instead)
                let corruption_val = U256::from_be_slice(&corruption_bytes);
                if corruption_val >= crypto::k256::Secp256k1::ORDER {
                    return Ok(());
                }

                // Now try with the corrupted oracle
                let mut lying_oracle = create_lying_oracle(Corruption::ReturnArbitrary(corruption_bytes));
                let mut test_scalar = scalar;
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    Secp256k1HooksWithOracle::new(&mut lying_oracle).scalar_invert_and_assign(&mut test_scalar);
                }));

                // The validation should have caught the lie (panicked)
                prop_assert!(result.is_err(), "Oracle lie was not detected for input {:?}", bytes);
            });
        }

        #[test]
        fn test_fe_sqrt_rejects_random_corruptions() {
            proptest!(|(bytes: [u8; 32], corruption_bytes: [u8; 32], flip_bool: bool)| {
                let Some(fe) = FieldElement::from_bytes(&bytes) else {
                    return Ok(());
                };
                if fe.normalizes_to_zero() {
                    return Ok(());
                }

                // Get the correct result first
                let mut correct_fe = fe;
                let mut correct_oracle = create_oracle_with_field_ops();
                let correct_exists = Secp256k1HooksWithOracle::new(&mut correct_oracle)
                    .fe_sqrt_and_assign(&mut correct_fe);
                let correct_sqrt = be_bytes(&correct_fe.to_canonical_words());

                // Test 1: Corrupt the sqrt candidate value
                // Skip if random corruption happens to equal the correct answer
                if corruption_bytes != correct_sqrt {
                    let mut lying_oracle = create_lying_oracle(Corruption::ReturnArbitrary(corruption_bytes));
                    let mut test_fe = fe;
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        Secp256k1HooksWithOracle::new(&mut lying_oracle).fe_sqrt_and_assign(&mut test_fe);
                    }));

                    // The validation should have caught the lie (panicked)
                    prop_assert!(result.is_err(),
                        "Oracle lie about sqrt candidate was not detected for input {:?}", bytes);
                }

                // Test 2: Lie about sqrt existence (flip the boolean)
                // Only test if flip_bool is true to reduce test cases
                if flip_bool {
                    let mut lying_oracle = create_sqrt_existence_lying_oracle();
                    let mut test_fe = fe;
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        Secp256k1HooksWithOracle::new(&mut lying_oracle).fe_sqrt_and_assign(&mut test_fe);
                    }));

                    // The validation should have caught the lie about existence (panicked)
                    prop_assert!(result.is_err(),
                        "Oracle lie about sqrt existence was not detected for input {:?} (is_qr={})",
                        bytes, correct_exists);
                }
            });
        }
    }

    mod zero_input_regression_tests {
        use super::*;
        use oracle_provider::{OracleQueryProcessor, ZkEENonDeterminismSource};

        /// A processor that should never be reached in these tests.
        /// Zero-input hook behavior is expected to short-circuit before oracle queries.
        struct PanickingFieldOpsQuery;

        impl OracleQueryProcessor for PanickingFieldOpsQuery {
            fn supported_memory_query_ids(&self) -> Vec<u32> {
                vec![FIELD_OPS_ADVISE_QUERY_ID]
            }

            fn process_memory_query(
                &mut self,
                query_id: u32,
                _input_word: usize,
                _memory: &dyn QuerierMemory,
                _mode: oracle_provider::RunMode,
                _native_run_responses: &mut Vec<u32>,
                _guest_run_responses: &mut Vec<u32>,
            ) {
                panic!("field ops oracle should not be queried for zero input, query_id=0x{query_id:08x}");
            }
        }

        fn create_panicking_field_ops_oracle() -> ZkEENonDeterminismSource {
            let mut oracle = ZkEENonDeterminismSource::default();
            oracle.add_external_processor(PanickingFieldOpsQuery);
            oracle
        }

        fn zero_field_element() -> FieldElement {
            FieldElement::from_bytes(&[0u8; 32]).expect("zero is a valid field element")
        }

        #[test]
        fn test_fe_invert_zero_does_not_query_oracle() {
            let mut fe_default = zero_field_element();
            DefaultSecp256k1Hooks.fe_invert_and_assign(&mut fe_default);
            assert!(fe_default.normalizes_to_zero());

            let mut fe_oracle = zero_field_element();
            let mut oracle = create_panicking_field_ops_oracle();
            Secp256k1HooksWithOracle::new(&mut oracle).fe_invert_and_assign(&mut fe_oracle);
            assert!(fe_oracle.normalizes_to_zero());
        }

        #[test]
        fn test_scalar_invert_zero_does_not_query_oracle() {
            let mut scalar_default = Scalar::ZERO;
            DefaultSecp256k1Hooks.scalar_invert_and_assign(&mut scalar_default);
            assert!(scalar_default.is_zero());

            let mut scalar_oracle = Scalar::ZERO;
            let mut oracle = create_panicking_field_ops_oracle();
            Secp256k1HooksWithOracle::new(&mut oracle).scalar_invert_and_assign(&mut scalar_oracle);
            assert!(scalar_oracle.is_zero());
        }

        #[test]
        fn test_fe_sqrt_zero_does_not_query_oracle_and_matches_default() {
            let mut fe_default = zero_field_element();
            let exists_default = DefaultSecp256k1Hooks.fe_sqrt_and_assign(&mut fe_default);
            assert!(exists_default);
            assert!(fe_default.normalizes_to_zero());

            let mut fe_oracle = zero_field_element();
            let mut oracle = create_panicking_field_ops_oracle();
            let exists_oracle =
                Secp256k1HooksWithOracle::new(&mut oracle).fe_sqrt_and_assign(&mut fe_oracle);
            assert!(exists_oracle);
            assert!(fe_oracle.normalizes_to_zero());
        }
    }
}
