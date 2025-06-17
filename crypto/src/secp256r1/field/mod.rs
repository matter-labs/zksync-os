#[cfg(any(all(target_arch = "riscv32", feature="bigint_ops"), test))]
mod fe32_delegation;

mod fe64;

use core::fmt::Debug;

#[cfg(target_pointer_width = "64")]
pub(super) use fe64::FieldElement;
#[cfg(all(target_arch = "riscv32", feature="bigint_ops"))]
pub(super) use fe32_delegation::FieldElement;
pub(super) use fe64::FieldElement as FieldElementConst;

#[cfg(any(all(target_arch = "riscv32", feature ="bigint_ops"), test))]
pub use fe32_delegation::init;

use super::Secp256r1Err;

const MODULUS: [u64; 4] = [18446744073709551615, 4294967295, 0, 18446744069414584321];
const R2: [u64; 4] = [3, 18446744056529682431, 18446744073709551614, 21474836477];
const REDUCTION_CONST: [u64; 4] = [1, 4294967296, 0, 18446744069414584322];

impl FieldElement {
    // montgomerry form 
    pub(super) const HALF: Self = Self::from_words_unchecked([0, 0, 0, 9223372036854775808]);
    // montgomerry form
    pub(super) const EQUATION_A: Self = Self::from_words_unchecked([18446744073709551612, 17179869183, 0, 18446744056529682436]);
    // montgomerry form
    pub(super) const EQUATION_B: Self = Self::from_words_unchecked([15608596021259845087, 12461466548982526096, 16546823903870267094, 15866188208926050356]);

    pub(super) fn from_be_bytes(bytes: &[u8; 32]) -> Result<Self, Secp256r1Err> {
        let val = Self::from_be_bytes_unchecked(bytes);

        if val.overflow() {
            Err(Secp256r1Err::InvalidFieldBytes)
        } else {
            Ok(val.to_representation())
        }
    }
    
    // https://github.com/RustCrypto/elliptic-curves/blob/master/p256/src/arithmetic/field.rs#L118
    pub(super) fn invert_assign(&mut self) {
        let mut t111 = *self;
        t111.square_assign();
        t111 *= &*self;
        t111.square_assign();
        t111 *= &*self;

        let mut t111111 = t111;
        t111111.sqn_assign(3);
        t111111 *= &t111;

        let mut x15 = t111111;
        x15.sqn_assign(6);
        x15 *= &t111111;
        x15.sqn_assign(3);
        x15 *= &t111;

        let mut x16 = x15;
        x16.square_assign();
        x16 *= &*self;

        let mut i53 = x16;
        i53.sqn_assign(16);
        i53 *= &x16;
        i53.sqn_assign(15);

        let mut x47 = x15;
        x47 *= &i53;

        i53.sqn_assign(17);
        i53 *= &*self;
        i53.sqn_assign(143);
        i53 *= &x47;
        i53.sqn_assign(47);

        x47 *= &i53;
        x47.sqn_assign(2);

        *self *= &x47;
    }

    /// Returns self^(2^n) mod p
    fn sqn_assign(&mut self, n: usize) {
        let mut i = 0;
        while i < n {
            self.square_assign();
            i += 1;
        }
    }
}

impl Debug for FieldElement {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("0x")?;
        let bytes = self.to_be_bytes();
        for b in bytes.as_slice().iter() {
            f.write_fmt(format_args!("{:02x}", b))?;
        }
        core::fmt::Result::Ok(())
    }
}

impl FieldElementConst {
    // https://github.com/RustCrypto/elliptic-curves/blob/master/p256/src/arithmetic/field.rs#L118
    pub(super) const fn invert(&self) -> Self {
        // We need to find b such that b * a ≡ 1 mod p. As we are in a prime
        // field, we can apply Fermat's Little Theorem:
        //
        //    a^p         ≡ a mod p
        //    a^(p-1)     ≡ 1 mod p
        //    a^(p-2) * a ≡ 1 mod p
        //
        // Thus inversion can be implemented with a single exponentiation.

        let t111 = self.mul(&self.mul(&self.square()).square());
        let t111111 = t111.mul(&t111.sqn(3));
        let x15 = t111111.sqn(6).mul(&t111111).sqn(3).mul(&t111);
        let x16 = x15.square().mul(self);
        let i53 = x16.sqn(16).mul(&x16).sqn(15);
        let x47 = x15.mul(&i53);
        x47.mul(&i53.sqn(17).mul(self).sqn(143).mul(&x47).sqn(47))
            .sqn(2)
            .mul(self)
    }

     /// Returns self^(2^n) mod p
     const fn sqn(&self, n: usize) -> Self {
        let mut x = *self;
        let mut i = 0;
        while i < n {
            x = x.square();
            i += 1;
        }
        x
    }

    #[cfg(target_pointer_width = "64")]
    pub(super) const fn to_fe(self) -> FieldElement {
        self
    }

    #[cfg(all(target_arch = "riscv32", feature="bigint_ops"))]
    pub(super) const fn to_fe(self) -> FieldElement {
        use crate::ark_ff_delegation::BigInt;

        FieldElement(BigInt::<4>(self.0))
    }
}