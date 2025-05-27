use core::{
    fmt::Debug,
    mem::{size_of, MaybeUninit},
};

/// The int representation is the same for both wasm, native and riscv hosts. It's opaque to allow
/// possible future optimizations to take place.
/// It is `u64` to have sufficient alignment for any primitive representation.
#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct U256Repr(pub(crate) [u64; 4]);

pub trait True {}
pub struct Assert<const B: bool> {}

impl True for Assert<true> {}

/// Converts size in bytes to amount of limbs needed to hols value of that size.
const fn size_to_limbs(size: usize) -> usize {
    if size % 8 == 0 {
        size / 8
    } else {
        size / 8 + 1
    }
}

impl U256Repr {
    pub fn new_zero() -> Self {
        Self([0; 4])
    }

    pub fn bytes_eq<const N: usize>(&self, other: &Self) -> bool {
        for i in 0..(size_to_limbs(N)) {
            if self.0[i] != other.0[i] {
                return false;
            }
        }

        true
    }

    pub fn as_raw_bytes(&self) -> &[u8] {
        unsafe { &*(&self.0 as *const _ as *const [u8; 32]) }
    }

    pub fn as_raw_bytes_mut(&mut self) -> &mut [u8] {
        unsafe { &mut *(&mut self.0 as *mut _ as *mut [u8; 32]) }
    }

    pub fn as_unit_ptr(&self) -> *const () {
        self.0.as_ptr() as *const _
    }

    pub fn as_unit_mut_ptr(&mut self) -> *mut () {
        self.0.as_mut_ptr() as *mut _
    }

    /***** Limbs *****/

    /// Writes `value` into the specified u64 limb.
    pub fn write_u64<const N: usize>(&mut self, value: u64)
    where
        Assert<{ N < 4 }>: True,
    {
        self.0[N] = value
    }

    pub fn as_u8_le_lsb_limbs(&self) -> impl IntoIterator<Item = &u8> {
        let ptr = &self.0 as *const _ as *const [u8; 32];

        unsafe { (*ptr).iter() }
    }

    pub fn as_u8_be_lsb_limbs(&self) -> impl IntoIterator<Item = &u8> {
        let ptr = &self.0 as *const _ as *const [u8; 32];

        unsafe { (*ptr).iter().rev() }
    }

    pub fn as_u8_le_msb_limbs(&self) -> impl IntoIterator<Item = &u8> {
        let ptr = &self.0 as *const _ as *const [u8; 32];

        unsafe { (*ptr).iter().rev() }
    }

    pub fn as_u8_be_msb_limbs(&self) -> impl IntoIterator<Item = &u8> {
        let ptr = &self.0 as *const _ as *const [u8; 32];

        unsafe { (*ptr).iter() }
    }

    pub fn as_u8_le_lsb_limbs_mut(&mut self) -> impl IntoIterator<Item = &mut u8> {
        let ptr = &mut self.0 as *mut _ as *mut [u8; 32];

        unsafe { (*ptr).iter_mut() }
    }

    pub fn as_u8_be_lsb_limbs_mut(&mut self) -> impl IntoIterator<Item = &mut u8> {
        let ptr = &mut self.0 as *mut _ as *mut [u8; 32];

        unsafe { (*ptr).iter_mut().rev() }
    }

    pub fn as_u8_le_msb_limbs_mut(&mut self) -> impl IntoIterator<Item = &mut u8> {
        let ptr = &mut self.0 as *mut _ as *mut [u8; 32];

        unsafe { (*ptr).iter_mut().rev() }
    }

    pub fn as_u8_be_msb_limbs_mut(&mut self) -> impl IntoIterator<Item = &mut u8> {
        let ptr = &mut self.0 as *mut _ as *mut [u8; 32];

        unsafe { (*ptr).iter_mut() }
    }

    /// Returns an iterator for u64 limbs in LE LSB to MSB order.
    pub fn as_u64_le_lsb_limbs(&self) -> impl IntoIterator<Item = &u64> {
        self.0.iter()
    }

    /// Returns an iterator for u64 limbs in BE LSB to MSB order.
    pub fn as_u64_be_lsb_limbs(&self) -> impl IntoIterator<Item = &u64> {
        self.0.iter().rev()
    }

    /// Returns an iterator for u64 limbs in LE MSB to LSB order.
    pub fn as_u64_le_msb_limbs(&self) -> impl IntoIterator<Item = &u64> {
        self.0.iter().rev()
    }

    /// Returns an iterator for u64 limbs in BE MSB to LSB order.
    pub fn as_u64_be_msb_limbs(&self) -> impl IntoIterator<Item = &u64> {
        self.0.iter()
    }

    /// Returns a mutable iterator for u64 limbs in LE LSB to MSB order.
    pub fn as_u64_le_lsb_limbs_mut(&mut self) -> impl IntoIterator<Item = &mut u64> {
        self.0.iter_mut()
    }

    /// Returns a mutable iterator for u64 limbs in BE LSB to MSB order.
    pub fn as_u64_be_lsb_limbs_mut(&mut self) -> impl IntoIterator<Item = &mut u64> {
        self.0.iter_mut().rev()
    }

    /// Returns an iterator for u32 limbs in LE LSB to MSB order.
    pub fn as_u32_le_lsb_limbs(&self) -> impl IntoIterator<Item = &u32> {
        let ptr = &self.0 as *const _ as *const [u32; 8];

        let x = unsafe { (*ptr).iter() };
        x
    }

    /// Returns an iterator for u32 limbs in BE LSB to MSB order.
    pub fn as_u32_be_lsb_limbs(&self) -> impl IntoIterator<Item = &u32> {
        let ptr = &self.0 as *const _ as *const [u32; 8];

        unsafe { (*ptr).iter().rev() }
    }

    /// Returns a mutable iterator for u32 limbs in LE LSB to MSB order.
    pub fn as_u32_le_lsb_limbs_mut(&mut self) -> impl IntoIterator<Item = &mut u32> {
        let ptr = &mut self.0 as *mut _ as *mut [u32; 8];

        unsafe { (*ptr).iter_mut() }
    }

    /// Returns a mutable iterator for u32 limbs in BE LSB to MSB order.
    pub fn as_u32_be_lsb_limbs_mut(&mut self) -> impl IntoIterator<Item = &mut u32> {
        let ptr = &mut self.0 as *mut _ as *mut [u32; 8];

        unsafe { (*ptr).iter_mut().rev() }
    }

    /// Returns an iterator for u32 limbs in LE MSB to LSB order.
    pub fn as_u32_le_msb_limbs(&self) -> impl IntoIterator<Item = &u32> {
        let ptr = &self.0 as *const _ as *const [u32; 8];

        unsafe { (*ptr).iter().rev() }
    }

    /// Returns an iterator for u32 limbs in BE MSB to LSB order.
    pub fn as_u32_be_msb_limbs(&self) -> impl IntoIterator<Item = &u32> {
        let ptr = &self.0 as *const _ as *const [u32; 8];

        unsafe { (*ptr).iter() }
    }

    /// Returns a mutable iterator for u32 limbs in LE MSB to LSB order.
    pub fn as_u32_le_msb_limbs_mut(&mut self) -> impl IntoIterator<Item = &mut u32> {
        let ptr = &mut self.0 as *mut _ as *mut [u32; 8];

        unsafe { (*ptr).iter_mut().rev() }
    }

    /// Returns a mutable iterator for u32 limbs in BE MSB to LSB order.
    pub fn as_u32_be_msb_limbs_mut(&mut self) -> impl IntoIterator<Item = &mut u32> {
        let ptr = &mut self.0 as *mut _ as *mut [u32; 8];

        unsafe { (*ptr).iter_mut() }
    }

    /// Returns an iterator for usize limbs in LE LSB to MSB order.
    #[inline(always)]
    pub fn as_usize_le_lsb_limbs(&self) -> impl IntoIterator<Item = &usize> {
        if const { core::mem::size_of::<usize>() == 4 } {
            let ptr = self.0.as_ptr() as *const [usize; 8];

            unsafe { (*ptr).iter() }
        } else if const { core::mem::size_of::<usize>() == 8 } {
            let ptr = self.0.as_ptr() as *const [usize; 4];

            unsafe { (*ptr).iter() }
        } else {
            unreachable!("Uncharted territory.");
        }
    }

    /// Returns an iterator for usize limbs in LE MSB to LSB order.
    #[inline(always)]
    pub fn as_usize_le_msb_limbs(&self) -> impl IntoIterator<Item = &usize> {
        self.as_usize_be_lsb_limbs()
    }

    /// Returns an iterator for usize limbs in LE LSB to MSB order.
    #[inline(always)]
    pub fn as_usize_le_lsb_limbs_mut(&mut self) -> impl IntoIterator<Item = &mut usize> {
        if const { core::mem::size_of::<usize>() == 4 } {
            let ptr = self.0.as_mut_ptr() as *mut [usize; 8];

            unsafe { (*ptr).iter_mut() }
        } else if const { core::mem::size_of::<usize>() == 8 } {
            let ptr = self.0.as_mut_ptr() as *mut [usize; 4];

            unsafe { (*ptr).iter_mut() }
        } else {
            unreachable!("Uncharted territory.");
        }
    }

    /// Returns an iterator for usize limbs in BE LSB to MSB order.
    #[inline(always)]
    pub fn as_usize_be_lsb_limbs(&self) -> impl IntoIterator<Item = &usize> {
        if const { core::mem::size_of::<usize>() == 4 } {
            let ptr = self.0.as_ptr() as *const [usize; 8];

            unsafe { (*ptr).iter().rev() }
        } else if const { core::mem::size_of::<usize>() == 8 } {
            let ptr = self.0.as_ptr() as *const [usize; 4];

            unsafe { (*ptr).iter().rev() }
        } else {
            unreachable!("Uncharted territory.");
        }
    }

    /// Returns an iterator for usize limbs in BE MSB to LSB order.
    #[inline(always)]
    pub fn as_usize_be_msb_limbs(&self) -> impl IntoIterator<Item = &usize> {
        self.as_usize_le_lsb_limbs()
    }

    /***** Endianness *****/

    pub fn swap_endianness_inplace(&mut self) {
        let swap_fn: fn(u64) -> u64 = if cfg!(target_endian = "little") {
            u64::to_be
        } else {
            u64::to_le
        };

        let buf = self.0[0];

        self.0[0] = swap_fn(self.0[3]);
        self.0[3] = swap_fn(buf);

        let buf = self.0[1];

        self.0[1] = swap_fn(self.0[2]);
        self.0[2] = swap_fn(buf);
    }

    pub fn swap_endianness_into(&self, dst: &mut Self) {
        let swap_fn: fn(u64) -> u64 = if cfg!(target_endian = "little") {
            u64::to_be
        } else {
            u64::to_le
        };

        let src = self.0.iter();
        let dst = dst.0.iter_mut().rev();

        src.zip(dst).for_each(|(s, t)| *t = swap_fn(*s));
    }

    /***** Encoding *****/

    pub const ENCODED_SIZE: usize = 32;

    // TODO: this is for le -> be converted write only - rename appropriately.
    pub fn write_into(&self, dst: &mut [MaybeUninit<u8>; size_of::<Self>()]) -> Result<(), &str> {
        const { assert!(size_of::<Self>() == 32) }

        let src = unsafe { &*(&self.0 as *const _ as *const [u32; 8]) };
        let dst = unsafe { &mut *(dst as *mut _ as *mut [u32; 8]) };

        dst[0] = src[7].to_be();
        dst[1] = src[6].to_be();
        dst[2] = src[5].to_be();
        dst[3] = src[4].to_be();
        dst[4] = src[3].to_be();
        dst[5] = src[2].to_be();
        dst[6] = src[1].to_be();
        dst[7] = src[0].to_be();

        Ok(())
    }

    /***** LE - Arithmetics *****/

    pub fn le_add_into(size: usize, left: &Self, right: &Self, result: &mut Self) -> bool {
        // Helping the optimizer to eliminate bound checking.
        // TODO: check if that worked.
        if size > 32 {
            unreachable!()
        }

        let mut limbs_left = left.as_usize_le_lsb_limbs().into_iter();
        let mut limbs_right = right.as_usize_le_lsb_limbs().into_iter();
        let mut limbs_result = result.as_usize_le_lsb_limbs_mut().into_iter();

        let step = core::mem::size_of::<usize>();
        let mut i = size;
        let mut carry = 0;

        loop {
            if i < step {
                break;
            }
            i -= step;

            let a = limbs_left.next().unwrap();
            let b = limbs_right.next().unwrap();

            let (r, of1) = usize::overflowing_add(*a, *b);
            let (r, of2) = usize::overflowing_add(r, carry);

            *limbs_result.next().unwrap() = r;

            carry = (of1 | of2) as usize;
        }

        if i != 0 {
            let a = limbs_left.next().unwrap();
            let b = limbs_right.next().unwrap();

            // OF would not occur, since 1 or more left-most bytes are zero.
            let (r, _) = usize::overflowing_add(*a, *b);
            let (r, _) = usize::overflowing_add(r, carry);

            let mask = (1 << (i * 8)) - 1;

            // Zero out the non-participating bytes.
            *limbs_result.next().unwrap() = r & mask;

            return usize::MAX & !mask != 0;
        }

        carry != 0
    }

    pub fn le_sub_into(size: usize, left: &Self, right: &Self, result: &mut Self) -> bool {
        if size > 32 {
            unreachable!()
        }

        let mut limbs_left = left.as_usize_le_lsb_limbs().into_iter();
        let mut limbs_right = right.as_usize_le_lsb_limbs().into_iter();
        let mut limbs_result = result.as_usize_le_lsb_limbs_mut().into_iter();

        let step = core::mem::size_of::<usize>();
        let mut i = size;
        let mut carry = 0;

        loop {
            if i < step {
                break;
            }
            i -= step;

            let a = limbs_left.next().unwrap();
            let b = limbs_right.next().unwrap();

            let (r, of1) = usize::overflowing_sub(*a, *b);
            let (r, of2) = usize::overflowing_sub(r, carry);

            *limbs_result.next().unwrap() = r;

            carry = (of1 | of2) as usize;
        }

        if i != 0 {
            let a = limbs_left.next().unwrap();
            let b = limbs_right.next().unwrap();

            let (r, of1) = usize::overflowing_sub(*a, *b);
            let (r, of2) = usize::overflowing_sub(r, carry);

            *limbs_result.next().unwrap() =
                // Zero out the non-participating bytes
                r & ((1 << (i * 8)) - 1);

            carry = (of1 | of2) as usize;
        }

        carry != 0
    }
}

impl Debug for U256Repr {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("0x")?;

        if f.alternate() {
            for i in self.as_u64_le_msb_limbs() {
                f.write_fmt(format_args!("{:016x?}", i))?
            }
        } else {
            for i in self.0 {
                f.write_fmt(format_args!("{:016x?}", i))?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {

    // #[test]
    // fn write_reservation_le() {
    //     let mut bytes = [1; 128];
    //
    //     let slice = bytes.as_mut_slice().split_at_mut(64).0.try_into().unwrap();
    //
    //     let r: Reservation<64> = Reservation { slice };
    //
    //     let i = U256Repr::new_zero();
    //
    //     let r = i.write_reservation_le(r).unwrap();
    //
    //     let r = i.write_reservation_le(r);
    //
    //
    //     println!("{:?}", bytes);
    // }

    use super::U256Repr;

    #[test]
    fn add_max_size_no_of() {
        let mut a = U256Repr::new_zero();
        let mut b = U256Repr::new_zero();

        for x in a.as_usize_le_lsb_limbs_mut() {
            *x = 1;
        }
        for x in b.as_usize_le_lsb_limbs_mut() {
            *x = 1;
        }

        let mut dst = U256Repr::new_zero();

        let of = U256Repr::le_add_into(32, &a, &b, &mut dst);

        let mut expect = U256Repr::new_zero();
        for x in expect.as_usize_le_lsb_limbs_mut() {
            *x = 2;
        }

        assert!(!of);
        assert_eq!(dst.as_raw_bytes(), expect.as_raw_bytes());
    }

    #[test]
    fn add_max_size_with_of() {
        let mut a = U256Repr::new_zero();
        let mut b = U256Repr::new_zero();

        assert_eq!(8, core::mem::size_of::<usize>());

        for x in a.as_usize_le_lsb_limbs_mut() {
            *x = 0xf000000000000000;
        }
        for x in b.as_usize_le_lsb_limbs_mut() {
            *x = 0x1000000000000000;
        }

        let mut dst = U256Repr::new_zero();

        let of = U256Repr::le_add_into(32, &a, &b, &mut dst);

        let mut expect = U256Repr::new_zero();
        for (x, y) in expect
            .as_usize_le_lsb_limbs_mut()
            .into_iter()
            .zip([0usize, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1])
        {
            *x = y;
        }

        assert!(of);
        assert_eq!(dst.as_raw_bytes(), expect.as_raw_bytes());
    }

    #[test]
    fn add_usize_mod_size_no_of() {}

    #[test]
    fn add_usize_mod_size_with_of() {}
}
