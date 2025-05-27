use core::{
    fmt::{Debug, Formatter},
    hash::{Hash, Hasher},
    marker::PhantomData,
    mem::{size_of, transmute},
};

use iwasm_specification::intx::U256Repr;

pub trait Endianness {
    fn limbs_u8_msb(repr: &U256Repr) -> impl IntoIterator<Item = &u8>;
    fn limbs_u8_msb_mut(repr: &mut U256Repr) -> impl IntoIterator<Item = &mut u8>;

    fn limbs_usize_lsb(repr: &U256Repr) -> impl IntoIterator<Item = &usize>;
    fn limbs_usize_msb(repr: &U256Repr) -> impl IntoIterator<Item = &usize>;

    fn flag_byte(repr: &U256Repr) -> u8;
}

#[derive(Debug, Clone)]
pub struct BE {}
impl Endianness for BE {
    fn limbs_u8_msb(repr: &U256Repr) -> impl IntoIterator<Item = &u8> {
        repr.as_u8_be_msb_limbs()
    }

    fn limbs_u8_msb_mut(repr: &mut U256Repr) -> impl IntoIterator<Item = &mut u8> {
        repr.as_u8_be_msb_limbs_mut()
    }

    fn limbs_usize_lsb(repr: &U256Repr) -> impl IntoIterator<Item = &usize> {
        repr.as_usize_be_lsb_limbs()
    }

    fn limbs_usize_msb(repr: &U256Repr) -> impl IntoIterator<Item = &usize> {
        repr.as_usize_be_msb_limbs()
    }

    fn flag_byte(repr: &U256Repr) -> u8 {
        const { assert!(core::mem::size_of::<U256Repr>() == 32) }
        // Safety: has more than one limb.
        unsafe { *repr.as_u8_be_msb_limbs().into_iter().next().unwrap_unchecked() }
    }
}

#[derive(Debug, Clone)]
pub struct LE {}
impl Endianness for LE {
    fn limbs_u8_msb(repr: &U256Repr) -> impl IntoIterator<Item = &u8> {
        repr.as_u8_le_msb_limbs()
    }

    fn limbs_u8_msb_mut(repr: &mut U256Repr) -> impl IntoIterator<Item = &mut u8> {
        repr.as_u8_le_msb_limbs_mut()
    }

    fn limbs_usize_lsb(repr: &U256Repr) -> impl IntoIterator<Item = &usize> {
        repr.as_usize_le_lsb_limbs()
    }

    fn limbs_usize_msb(repr: &U256Repr) -> impl IntoIterator<Item = &usize> {
        repr.as_usize_le_msb_limbs()
    }

    fn flag_byte(repr: &U256Repr) -> u8 {
        const { assert!(core::mem::size_of::<U256Repr>() == 32) }
        // Safety: has more than one limb.
        unsafe { *repr.as_u8_le_msb_limbs().into_iter().next().unwrap_unchecked() }
    }
}

// WARNING: If representation changes to variable size, all unsafe code for `IntX` needs reviewing.
#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct IntX<const N: usize, E: Endianness> {
    pub(crate) repr: U256Repr,
    phantom: PhantomData<[E; N]>,
}

/// Endian agnostic impl
impl<const N: usize, E: Endianness> IntX<N, E>
where
    Assert<{ size_bound(N) }>: IsTrue,
{
    pub fn new() -> Self {
        Self {
            // TODO: check how llvm handles the shadow stack value init
            repr: U256Repr::new_zero(),
            phantom: PhantomData,
        }
    }

    pub fn as_bytes(&self) -> &[u8] {
        self.repr.as_raw_bytes()
    }

    pub fn from_bytes(bytes: [u8; core::mem::size_of::<U256Repr>()]) -> Self {
        let mut new = Self::new();
        crate::sys::intx::fill_bytes(&mut new, bytes);
        new
    }

    pub fn as_ptr(&self) -> *const u8 {
        self.repr.as_raw_bytes().as_ptr()
    }

    pub fn as_mut_ptr(&mut self) -> *mut u8 {
        self.repr.as_raw_bytes_mut().as_mut_ptr()
    }


    pub fn into_u256(self) -> IntX<32, E> {
        // Safety: The representation is the same for all sizes.
        unsafe { transmute(self) }
    }

    pub fn into_size<const M: usize>(mut self) -> IntX<M, E> {
        match N.cmp(&M) {
            core::cmp::Ordering::Greater => {
                // Narrowing
                let mut iter = E::limbs_u8_msb_mut(&mut self.repr).into_iter().peekable();

                // TODO: Verify that this is optimized to usize ops.
                for _ in 0..(32 - N) {
                    let x = iter.next().unwrap();

                    *x = 0;
                }

                drop(iter);

                IntX {
                    repr: self.repr,
                    phantom: PhantomData,
                }
            },
            _ => {
                // Widening, N == M

                const { assert!(core::mem::size_of::<U256Repr>() == 32) }

                IntX {
                    repr: self.repr,
                    phantom: PhantomData,
                }
            }
        }
    }

    pub fn is_zero(&self) -> bool {
        crate::sys::intx::is_zero(self)
    }


}

impl<const N: usize> IntX<N, LE>
where
    Assert<{ size_bound(N) }>: IsTrue,
{
    pub fn into_be(self) -> IntX<N, BE> {
        crate::sys::intx::into_be(self)
    }

    pub fn to_be(&self) -> IntX<N, BE> {
        crate::sys::intx::to_be(self)
    }

    pub fn from_usize(value: usize) -> Self {
        let mut new = Self::new();

        // Writing the first lsb limb.
        if core::mem::size_of::<usize>() == 4 {
            *new.repr
                .as_u32_le_lsb_limbs_mut()
                .into_iter()
                .next()
                .unwrap() = value as u32;
        } else {
            *new.repr
                .as_u64_le_lsb_limbs_mut()
                .into_iter()
                .next()
                .unwrap() = value as u64;
        }

        new
    }

    /***** Arithmetics *****/

    pub fn add(&self, other: &Self) -> Self {
        crate::sys::intx::le_add(self, other)
    }

    pub fn sub(&self, other: &Self) -> Self {
        crate::sys::intx::le_sub(self, other)
    }
}

impl<const N: usize> IntX<N, BE>
where
    Assert<{ size_bound(N) }>: IsTrue,
{
    pub fn into_le(self) -> IntX<N, LE> {
        crate::sys::intx::into_le(self)
    }

    pub fn to_le(&self) -> IntX<N, LE> {
        crate::sys::intx::to_le(self)
    }

    pub fn from_usize(value: usize) -> Self {
        let mut new = Self::new();

        // Writing the first lsb limb.
        if size_of::<usize>() == 4 {
            *new.repr
                .as_u32_be_lsb_limbs_mut()
                .into_iter()
                .next()
                .unwrap() = value.to_be() as u32;
        } else {
            *new.repr
                .as_u64_be_lsb_limbs_mut()
                .into_iter()
                .next()
                .unwrap() = value.to_be() as u64;
        }

        new
    }
}

/***** Core trait impls *****/

impl<const N: usize, E: Endianness> Default for IntX<N, E>
where
    Assert<{ size_bound(N) }>: IsTrue,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> Debug for IntX<N, LE> {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.write_fmt(format_args!("IntX<{}, LE>(0x", N,))?;

        for i in self.repr.as_u64_le_msb_limbs() {
            f.write_fmt(format_args!("{:016x?}", i))?;
        }

        f.write_str(")")
    }
}

impl<const N: usize> Debug for IntX<N, BE> {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.write_fmt(format_args!("IntX<{}, BE>(0x", N,))?;

        for i in self.repr.as_u64_be_msb_limbs() {
            // The libs are written in BE, so when read on LE the bytes are swapped implicitly.
            f.write_fmt(format_args!("{:016x?}", i.to_be()))?;
        }

        f.write_str(")")
    }
}

// impl<const N: usize> Into<IntX<N, LE>> for IntX<N, BE>
//     where Assert<{ size_bound(N) }>: IsTrue
// {
//     fn into(self) -> IntX<N, LE> {
//         self.into_le()
//     }
// }
//
// impl<const N: usize> Into<IntX<N, BE>> for IntX<N, LE>
//     where Assert<{ size_bound(N) }>: IsTrue
// {
//     fn into(self) -> IntX<N, BE> {
//         self.into_be()
//     }
// }

impl<const N: usize, E: Endianness> PartialEq for IntX<N, E>
where
    Assert<{ size_bound(N) }>: IsTrue,
{
    fn eq(&self, other: &Self) -> bool {
        crate::sys::intx::partial_eq(self, other)
    }
}

impl<const N: usize, E: Endianness> PartialOrd for IntX<N, E>
where 
    Assert<{ size_bound(N) }>: IsTrue,
{
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<const N: usize, E: Endianness> Eq for IntX<N, E> where Assert<{ size_bound(N) }>: IsTrue {}

impl<const N: usize, E: Endianness> Ord for IntX<N, E> where Assert<{ size_bound(N) }>: IsTrue {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        crate::sys::intx::cmp(self, other)
    }
}

impl<const N: usize, E: Endianness> Hash for IntX<N, E>
where
    Assert<{ size_bound(N) }>: IsTrue,
{
    fn hash<H: Hasher>(&self, state: &mut H) {
        for limb in self.repr.as_u32_le_lsb_limbs() {
            limb.hash(state);
        }
    }
}

impl<const N: usize> IntX<N, LE>
where
    Assert<{ size_bound(N) }>: IsTrue,
{
    pub fn overflowing_add_assign(&mut self, _other: &Self) -> bool {
        todo!()
    }

    pub fn div_assign(&mut self, _other: &Self) {
        todo!()
    }

    pub fn checked_div_assign(&mut self, other: &Self) -> bool {
        if other.is_zero() {
            return false;
        }
        self.div_assign(other);

        true
    }

    pub fn set_zero(&mut self) {
        todo!()
    }


    pub fn overflowing_shr(&mut self, rhs: usize) -> bool {
        if rhs >= 256 {
            let of = !self.is_zero();
            self.set_zero();

            return of;
        }

        // S::uintx_overflowing_shr_assign(self, rhs as u32)
        todo!()
    }

    pub fn overflowing_shl(&mut self, rhs: usize) -> bool {
        if rhs >= 256 {
            let of = !self.is_zero();
            self.set_zero();

            return of;
        }

        // S::uintx_overflowing_shl_assign(self, rhs as u32)
        todo!()
    }

    // #[must_use]
    // pub fn overflowing_shr(&mut self, rhs: usize) -> bool {
    //     let (limbs, bits) = (rhs / 64, rhs % 64);
    //     if limbs >= Self::LIMBS {
    //         let of = !self.is_zero();
    //         self.zero_out();

    //         return of;
    //     }
    //     if bits == 0 {
    //         // Check for overflow
    //         let mut overflow = false;
    //         for i in 0..limbs {
    //             overflow |= self.repr.as_mut().0[i] != 0;
    //         }

    //         // Shift
    //         for i in 0..(Self::LIMBS - limbs) {
    //             self.repr.as_mut().0[i] = self.repr.as_mut().0[i + limbs];
    //         }
    //         for i in (Self::LIMBS - limbs)..Self::LIMBS {
    //             self.repr.as_mut().0[i] = 0;
    //         }
    //         return overflow;
    //     }

    //     // Check for overflow
    //     let mut overflow = false;
    //     for i in 0..limbs {
    //         overflow |= self.repr.as_mut().0[i] != 0;
    //     }
    //     overflow |= self.repr.as_mut().0[limbs] >> bits != 0;

    //     // Shift
    //     for i in 0..(Self::LIMBS - limbs - 1) {
    //         self.repr.as_mut().0[i] = self.repr.as_mut().0[i + limbs] >> bits;
    //         self.repr.as_mut().0[i] |= self.repr.as_mut().0[i + limbs + 1] << (64 - bits);
    //     }
    //     self.repr.as_mut().0[Self::LIMBS - limbs - 1] = self.repr.as_mut().0[Self::LIMBS - 1] >> bits;
    //     for i in (Self::LIMBS - limbs)..Self::LIMBS {
    //         self.repr.as_mut().0[i] = 0;
    //     }
    //     overflow
    // }
}

// pub type U256Impl<S> = IntX<S, 32>;

// Utils
pub struct Assert<const B: bool> {}
pub trait IsTrue: 'static {}
impl IsTrue for Assert<true> {}

pub const fn size_bound(n: usize) -> bool {
    n > 0 && n <= 32
}

#[cfg(test)]
mod tests {
    use crate::types::ints::{U256, U256BE};
    use rand::RngCore;

    #[test]
    fn as_bytes_from_bytes() {
        let mut bytes = [0; 32];
        rand::thread_rng().fill_bytes(&mut bytes);

        println!("{:?}", bytes);
        assert_eq!(bytes, U256::from_bytes(bytes).as_bytes());
    }

    #[test]
    fn from_hex_be() {
        let mut bytes = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut bytes);

        let mut hex = [char::default(); 64];
        for (i, b) in bytes.iter().enumerate() {
            let cs = format!("{:02x?}", b);
            let mut iter = cs.chars();
            hex[i * 2] = iter.next().unwrap();
            hex[i * 2 + 1] = iter.next().unwrap();
        }
        let hex = hex.iter().collect::<String>();
        assert_eq!(bytes, U256BE::from_hex(hex.as_str()).as_bytes());
    }
}
