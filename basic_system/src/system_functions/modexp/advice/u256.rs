use core::mem::MaybeUninit;
use crypto::{bigint_op_delegation_raw, BigIntOps};

static mut ZERO: MaybeUninit<DelegatedU256> = MaybeUninit::uninit();
static mut ONE: MaybeUninit<DelegatedU256> = MaybeUninit::uninit();

pub(crate) fn init() {
    #[allow(static_mut_refs)]
    unsafe {
        ZERO.write(DelegatedU256::ZERO);
        ONE.write(DelegatedU256::ONE);
    }
}

#[repr(align(32))]
pub(crate) struct DelegatedU256([u64; 4]);

impl core::fmt::Debug for DelegatedU256 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "0x")?;
        for digit in self.0.iter().rev() {
            write!(f, "{digit:016x}")?;
        }

        Ok(())
    }
}

impl DelegatedU256 {
    pub(crate) const ZERO: Self = Self([0; 4]);
    pub(crate) const ONE: Self = Self([1, 0, 0, 0]);

    #[inline(always)]
    unsafe fn copy_from_ptr(source: *const Self) -> Self {
        let mut result = MaybeUninit::<Self>::uninit();
        let _ =
            bigint_op_delegation_raw(result.as_mut_ptr().cast(), source.cast(), BigIntOps::MemCpy);

        unsafe { result.assume_init() }
    }

    pub(crate) fn zero() -> Self {
        unsafe {
            #[allow(static_mut_refs)]
            Self::copy_from_ptr(ZERO.as_ptr())
        }
    }

    pub(crate) unsafe fn from_be_bytes_in_place(input: &[u8; 32], place: &mut MaybeUninit<Self>) {
        unsafe {
            let ptr = place.as_mut_ptr().cast::<u64>();
            let src: *const [u8; 8] = input.as_ptr_range().end.cast();

            ptr.write(u64::from_be_bytes(src.sub(1).read()));
            ptr.add(1).write(u64::from_be_bytes(src.sub(2).read()));
            ptr.add(2).write(u64::from_be_bytes(src.sub(3).read()));
            ptr.add(3).write(u64::from_be_bytes(src.sub(4).read()));
        }
    }

    pub(crate) fn to_be_bytes(&self) -> [u8; 32] {
        let mut res = self.clone();
        res.bytereverse();
        unsafe { core::mem::transmute(res) }
    }

    pub(crate) const fn as_limbs_mut(&mut self) -> &mut [u64; 4] {
        &mut self.0
    }

    pub(crate) fn bytereverse(&mut self) {
        let limbs = self.as_limbs_mut();
        unsafe {
            core::ptr::swap(&mut limbs[0] as *mut u64, &mut limbs[3] as *mut u64);
            core::ptr::swap(&mut limbs[1] as *mut u64, &mut limbs[2] as *mut u64);
        }
        for limb in limbs.iter_mut() {
            *limb = limb.swap_bytes();
        }
    }

    pub(crate) fn is_zero(&self) -> bool {
        #[allow(static_mut_refs)]
        unsafe {
            // equality is non-destructive, so we can cast
            let eq = bigint_op_delegation_raw(
                (self as *const Self).cast_mut().cast(),
                ZERO.as_ptr().cast(),
                BigIntOps::Eq,
            );

            eq != 0
        }
    }

    pub(crate) fn is_one(&self) -> bool {
        #[allow(static_mut_refs)]
        unsafe {
            // equality is non-destructive, so we can cast
            let eq = bigint_op_delegation_raw(
                (self as *const Self).cast_mut().cast(),
                ONE.as_ptr().cast(),
                BigIntOps::Eq,
            );

            eq != 0
        }
    }
}

impl Clone for DelegatedU256 {
    #[inline(always)]
    fn clone(&self) -> Self {
        // custom clone by using precompile
        unsafe { Self::copy_from_ptr(self as *const Self) }
    }

    #[inline(always)]
    fn clone_from(&mut self, source: &Self) {
        unsafe {
            let _ = bigint_op_delegation_raw(
                self.0.as_mut_ptr().cast(),
                (source as *const Self).cast(),
                BigIntOps::MemCpy,
            );
        }
    }
}

impl PartialEq for DelegatedU256 {
    fn eq(&self, other: &Self) -> bool {
        unsafe {
            // equality is non-destructive, so we can cast
            let eq = bigint_op_delegation_raw(
                (self as *const Self).cast_mut().cast(),
                (other as *const Self).cast(),
                BigIntOps::Eq,
            );

            eq != 0
        }
    }
}

impl Eq for DelegatedU256 {}

/// # Safety
/// `src` must be allocated in non ROM.
/// `dst` must be 32 bytes aligned and point to 32 bytes of accessible memory.
pub(crate) unsafe fn write_into_ptr_unchecked(dst: *mut DelegatedU256, source: &DelegatedU256) {
    unsafe {
        bigint_op_delegation_raw(
            dst.cast(),
            (source as *const DelegatedU256).cast(),
            BigIntOps::MemCpy,
        );
    }
}
