use crate::ark_ff_delegation::BigInt;
use crate::bigint_delegation::{u256, DelegatedModParams, DelegatedMontParams};
use core::mem::MaybeUninit;

const _: () = const {
    assert!(core::mem::size_of::<crate::k256::Scalar>() == core::mem::size_of::<ScalarInner>());
};

static mut MODULUS: MaybeUninit<BigInt<4>> = MaybeUninit::uninit();
static mut REDUCTION_CONST: MaybeUninit<BigInt<4>> = MaybeUninit::uninit();

#[derive(Debug, Default)]
pub(super) struct ScalarParams;

impl DelegatedModParams<4> for ScalarParams {
    unsafe fn modulus() -> &'static BigInt<4> {
        unsafe { MODULUS.assume_init_ref() }
    }
}

impl DelegatedMontParams<4> for ScalarParams {
    unsafe fn reduction_const() -> &'static BigInt<4> {
        unsafe { REDUCTION_CONST.assume_init_ref() }
    }
}

#[cfg(any(all(target_arch = "riscv32", feature = "bigint_ops"), test))]
pub fn init() {
    unsafe {
        MODULUS.as_mut_ptr().write(ScalarInner::ORDER.0);
        REDUCTION_CONST
            .as_mut_ptr()
            .write(ScalarInner::REDUCTION_CONST.0);
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct ScalarInner(BigInt<4>);

impl ScalarInner {
    pub(super) const ZERO: Self = Self(BigInt::zero());
    pub(super) const ONE: Self = Self::from_words([4624529908474429119, 4994812053365940164, 1, 0]);
    const ONE_REPR: Self = Self(BigInt::one());
    const R2: Self = Self::from_words([
        9902555850136342848,
        8364476168144746616,
        16616019711348246470,
        11342065889886772165,
    ]); // Montgomery form
    pub(super) const ORDER: Self = Self::from_be_hex_unchecked(super::ORDER_HEX);
    const REDUCTION_CONST: Self = Self::from_be_hex_unchecked(
        "D9E8890D6494EF93897F30C127CFAB5E50A51AC834B9EC244B0DFF665588B13F",
    );
    pub(super) const MINUS_LAMBDA: Self = Self::from_be_hex_unchecked(
        "acd7bfe87aa729c68c5699f9ad96826d8e1af5392b820beecf54734f06a3d4a3",
    );
    pub(super) const MINUS_B1: Self = Self::from_be_hex_unchecked(
        "221208ac9df506c61571b4ae8ac47f711b1c8205faa6ed42c50468d00ad9263c",
    );
    pub(super) const MINUS_B2: Self = Self::from_be_hex_unchecked(
        "c25575eb8e173580176cdf65ba244fce1e8a8dc5f3ba59390cac5e506a144696",
    );

    pub(super) const G1: Self = Self::from_be_bytes_unchecked(&[
        0x30, 0x86, 0xd2, 0x21, 0xa7, 0xd4, 0x6b, 0xcd, 0xe8, 0x6c, 0x90, 0xe4, 0x92, 0x84, 0xeb,
        0x15, 0x3d, 0xaa, 0x8a, 0x14, 0x71, 0xe8, 0xca, 0x7f, 0xe8, 0x93, 0x20, 0x9a, 0x45, 0xdb,
        0xb0, 0x31,
    ]);

    pub(super) const G2: Self = Self::from_be_bytes_unchecked(&[
        0xe4, 0x43, 0x7e, 0xd6, 0x01, 0x0e, 0x88, 0x28, 0x6f, 0x54, 0x7f, 0xa9, 0x0a, 0xbf, 0xe4,
        0xc4, 0x22, 0x12, 0x08, 0xac, 0x9d, 0xf5, 0x06, 0xc6, 0x15, 0x71, 0xb4, 0xae, 0x8a, 0xc4,
        0x7f, 0x71,
    ]);

    const fn from_words(words: [u64; 4]) -> Self {
        Self(BigInt(words))
    }

    pub(super) const fn from_be_bytes_unchecked(bytes: &[u8; 32]) -> Self {
        Self(u256::from_bytes_unchecked(bytes))
    }

    pub(super) fn from_be_bytes(bytes: &[u8; 32]) -> Self {
        let t = Self::from_be_bytes_unchecked(bytes);

        t.to_representation()
    }

    pub(crate) fn from_k256_scalar(s: crate::k256::Scalar) -> Self {
        // SAFETY: it is reduced, but doesn't use Montgomery form
        // we have to copy here because we have stricter alginment than k256::Scalar
        let t: Self = unsafe { core::mem::transmute_copy(&s) };

        t.to_representation()
    }

    pub(super) fn to_representation(mut self) -> Self {
        self.mul_in_place(&Self::R2);

        self
    }

    #[inline(always)]
    pub(super) fn to_integer(mut self) -> Self {
        self.mul_in_place(&Self::ONE_REPR);

        self
    }

    #[cfg(test)]
    pub(super) fn from_u128(n: u128) -> Self {
        Self(BigInt([n as u64, (n >> 64) as u64, 0, 0])).to_representation()
    }

    pub(super) const fn from_be_hex_unchecked(hex: &str) -> Self {
        let bytes = hex.as_bytes();

        assert!(
            bytes.len() == 8 * 4 * 2,
            "hex string is not the expected size"
        );

        let mut res = [0; 4];
        let mut buf = [0u8; 8];
        let mut i = 0;
        let mut err = 0;

        while i < 4 {
            let mut j = 0;
            while j < 8 {
                let offset = (i * 8 + j) * 2;
                let (result, byte_err) = decode_hex_byte([bytes[offset], bytes[offset + 1]]);
                err |= byte_err;
                buf[j] = result;
                j += 1;
            }
            res[3 - i] = u64::from_be_bytes(buf);
            i += 1;
        }

        assert!(err == 0, "invalid hex byte");

        Self::from_words(res)
    }

    pub(super) fn from_be_hex(hex: &str) -> Self {
        Self::from_be_hex_unchecked(hex).to_representation()
    }

    pub(super) fn to_be_bytes(self) -> [u8; 32] {
        let t = self.to_integer();

        u256::to_be_bytes(t.0)
    }

    fn as_words(&self) -> &[u64; 4] {
        &self.0 .0
    }
    // This is only called on the results of decompose and decompose_128, so the input is already in integer form
    #[inline(always)]
    pub(super) fn bits(&self, offset: usize, count: usize) -> u32 {
        // check requested bits must be from the same limb
        debug_assert!((offset + count - 1) >> 6 == offset >> 6);
        let limbs = &self.0 .0;
        ((limbs[offset >> 6] >> (offset & 0x3F)) & ((1 << count) - 1)) as u32
    }

    // This is only called on the results of decompose and decompose_128, so the input is already in integer form
    #[inline(always)]
    pub(super) fn bits_var(&self, offset: usize, count: usize) -> u32 {
        debug_assert!(count <= 32);
        debug_assert!(offset + count <= 256);
        // if all the requested bits are in the same limb
        if (offset + count - 1) >> 6 == offset >> 6 {
            self.bits(offset, count)
        } else {
            debug_assert!((offset >> 6) + 1 < 4);
            let limbs = &self.0 .0;
            (((limbs[offset >> 6] >> (offset & 0x3F))
                | (limbs[(offset >> 6) + 1] << (64 - (offset & 0x3F))))
                & ((1 << count) - 1)) as u32
        }
    }

    // The input should be in montgomerry form, the output is in integer form
    #[inline(always)]
    pub(super) fn decompose_128(&self) -> (Self, Self) {
        let integer_form = self.to_integer();
        let words = integer_form.as_words();

        let r1 = BigInt([words[0], words[1], 0, 0]);
        let r2 = BigInt([words[2], words[3], 0, 0]);

        (Self(r1), Self(r2))
    }

    // The input should be in montgomerry form, the output is in integer form
    pub(super) fn decompose(self) -> (Self, Self) {
        // Not to efficient as we kick out of Montgomery form
        let int_form = self.to_integer();
        let mut c1 = int_form;
        c1.integer_mul_shift_384_vartime(&Self::G1);
        let mut c2 = int_form;
        c2.integer_mul_shift_384_vartime(&Self::G2);

        // now kick back
        c1 = c1.to_representation();
        c2 = c2.to_representation();

        c1.mul_in_place(&Self::MINUS_B1);
        c2.mul_in_place(&Self::MINUS_B2);

        c1.add_in_place(&c2);

        let mut r1 = c1;
        r1.mul_in_place(&Self::MINUS_LAMBDA);
        r1.add_in_place(&self);

        (r1.to_integer(), c1.to_integer())
    }

    #[inline(always)]
    pub(super) fn eq_impl(&self, other: &Self) -> bool {
        unsafe { u256::eq_mod::<ScalarParams>(&self.0, &other.0) }
    }

    fn integer_mul_shift_384_vartime(&mut self, b: &Self) {
        u256::mul_high_assign(&mut self.0, &b.0);

        let words = &self.0 .0;

        let l = words[1];
        self.0 = BigInt([words[2], words[3], 0, 0]);

        if (l >> 63) & 1 != 0 {
            self.add_in_place(&Self::ONE_REPR);
        }
    }

    pub(super) fn mul_in_place(&mut self, rhs: &Self) {
        unsafe {
            u256::mul_assign_montgomery::<ScalarParams>(&mut self.0, &rhs.0);
        }
    }

    pub(super) fn square_in_place(&mut self) {
        unsafe {
            u256::square_assign_montgomery::<ScalarParams>(&mut self.0);
        }
    }

    #[inline(always)]
    pub(super) fn add_in_place(&mut self, rhs: &Self) {
        unsafe {
            u256::add_mod_assign::<ScalarParams>(&mut self.0, &rhs.0);
        }
    }

    #[inline(always)]
    pub(super) fn negate_in_place(&mut self) {
        unsafe {
            u256::neg_mod_assign::<ScalarParams>(&mut self.0);
        }
    }

    #[inline(always)]
    pub(super) fn is_zero(&self) -> bool {
        unsafe { u256::is_zero_mod::<ScalarParams>(&self.0) }
    }
}

/// Decode a single nibble of upper or lower hex
#[inline(always)]
const fn decode_nibble(src: u8) -> u16 {
    let byte = src as i16;
    let mut ret: i16 = -1;

    // 0-9  0x30-0x39
    // if (byte > 0x2f && byte < 0x3a) ret += byte - 0x30 + 1; // -47
    ret += (((0x2fi16 - byte) & (byte - 0x3a)) >> 8) & (byte - 47);
    // A-F  0x41-0x46
    // if (byte > 0x40 && byte < 0x47) ret += byte - 0x41 + 10 + 1; // -54
    ret += (((0x40i16 - byte) & (byte - 0x47)) >> 8) & (byte - 54);
    // a-f  0x61-0x66
    // if (byte > 0x60 && byte < 0x67) ret += byte - 0x61 + 10 + 1; // -86
    ret += (((0x60i16 - byte) & (byte - 0x67)) >> 8) & (byte - 86);

    ret as u16
}

/// Decode a single byte encoded as two hexadecimal characters.
/// Second element of the tuple is non-zero if the `bytes` values are not in the valid range
/// (0-9, a-z, A-Z).
#[inline(always)]
const fn decode_hex_byte(bytes: [u8; 2]) -> (u8, u16) {
    let hi = decode_nibble(bytes[0]);
    let lo = decode_nibble(bytes[1]);
    let byte = (hi << 4) | lo;
    let err = byte >> 8;
    let result = byte as u8;
    (result, err)
}

#[cfg(test)]
impl proptest::arbitrary::Arbitrary for ScalarInner {
    type Parameters = ();

    fn arbitrary_with(_args: Self::Parameters) -> Self::Strategy {
        use proptest::prelude::{any, Strategy};

        any::<u256::U256Wrapper<ScalarParams>>().prop_map(|inner| Self(inner.0).to_representation())
    }

    type Strategy = proptest::arbitrary::Mapped<u256::U256Wrapper<ScalarParams>, Self>;
}

#[cfg(test)]
impl PartialEq for ScalarInner {
    fn eq(&self, other: &Self) -> bool {
        self.eq_impl(other)
    }
}

#[cfg(test)]
impl PartialOrd for ScalarInner {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        self.0.partial_cmp(&other.0)
    }
}

#[cfg(test)]
mod tests {
    use super::ScalarInner;
    use proptest::{prop_assert_eq, proptest};

    fn init() {
        crate::secp256k1::init();
        crate::bigint_delegation::init();
    }

    #[ignore = "requires single threaded runner"]
    #[test]
    fn test_zero() {
        init();
        let zero = ScalarInner::ZERO;
        let order = ScalarInner::ORDER;
        let one = ScalarInner::ONE;

        assert_eq!(order, zero);
        assert!(zero.is_zero());
        assert!(order.is_zero());

        assert_ne!(zero, one);
    }

    #[ignore = "requires single threaded runner"]
    #[test]
    fn test_mul() {
        init();
        proptest!(|(x: ScalarInner, y: ScalarInner, z: ScalarInner)| {
            let mut a = x;
            let mut b = y;
            a.mul_in_place(&y);
            b.mul_in_place(&x);
            prop_assert_eq!(a, b);

            a = x;
            b = y;
            a.mul_in_place(&y);
            a.mul_in_place(&z);
            b.mul_in_place(&z);
            b.mul_in_place(&x);
            prop_assert_eq!(a, b);

            a = x;
            a.mul_in_place(&ScalarInner::ONE);
            prop_assert_eq!(a, x);

            a.mul_in_place(&ScalarInner::ZERO);
            prop_assert_eq!(a, ScalarInner::ZERO);

            a = y;
            b = x;
            let mut c = x;
            a.add_in_place(&z);
            a.mul_in_place(&x);
            b.mul_in_place(&y);
            c.mul_in_place(&z);
            b.add_in_place(&c);
            prop_assert_eq!(a, b);

            a = x;
            b = x;
            a.square_in_place();
            b.mul_in_place(&x);
            prop_assert_eq!(a, b);
        })
    }

    #[ignore = "requires single threaded runner"]
    #[test]
    fn test_add() {
        init();
        proptest!(|(x: ScalarInner, y: ScalarInner, z: ScalarInner)| {
            let mut a = x;
            let mut b = y;
            a.add_in_place(&y);
            b.add_in_place(&x);
            prop_assert_eq!(a, b);

            a = x;
            a.add_in_place(&ScalarInner::ZERO);
            prop_assert_eq!(a, x);

            a = y;
            b = x;
            a.add_in_place(&z);
            a.add_in_place(&x);
            b.add_in_place(&y);
            b.add_in_place(&z);
            prop_assert_eq!(a, b);

            a = x;
            a.negate_in_place();
            a.add_in_place(&x);
            prop_assert_eq!(a, ScalarInner::ZERO);
        })
    }

    #[ignore = "requires single threaded runner"]
    #[test]
    fn to_bytes_round() {
        init();
        proptest!(|(x: ScalarInner)| {
            prop_assert_eq!(ScalarInner::from_be_bytes(&x.to_be_bytes()), x)
        })
    }

    #[ignore = "requires single threaded runner"]
    #[test]
    fn from_bytes_round() {
        init();
        proptest!(|(bytes: [u8; 32])| {
            prop_assert_eq!(ScalarInner::from_be_bytes(&bytes).to_be_bytes(), bytes)
        });
    }

    #[ignore = "requires single threaded runner"]
    #[test]
    fn to_montgomerry_round() {
        init();
        proptest!(|(x: ScalarInner)| {
            prop_assert_eq!(x.to_integer().to_representation(), x);
        })
    }
}
