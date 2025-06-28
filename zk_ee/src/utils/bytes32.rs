use core::{mem::MaybeUninit, ptr::addr_of_mut};
use ruint::aliases::{B160, U256};

#[cfg(target_pointer_width = "32")]
pub const BYTES32_USIZE_SIZE: usize = 8;

#[cfg(target_pointer_width = "64")]
pub const BYTES32_USIZE_SIZE: usize = 4;

#[repr(align(8))]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "testing", derive(serde::Serialize, serde::Deserialize))]
pub struct Bytes32 {
    inner: [usize; BYTES32_USIZE_SIZE],
}

// we compare as integers to avoid any potential ambiguity

impl Ord for Bytes32 {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.as_u8_array_ref().cmp(other.as_u8_array_ref())
    }
}

impl PartialOrd for Bytes32 {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl core::fmt::Debug for Bytes32 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "0x")?;
        for word in self.inner.iter() {
            #[cfg(target_pointer_width = "32")]
            write!(f, "{:08x}", word.to_be())?;

            #[cfg(target_pointer_width = "64")]
            write!(f, "{:016x}", word.to_be())?;
        }

        Ok(())
    }
}

impl Bytes32 {
    pub const ZERO: Self = Self {
        inner: [0usize; BYTES32_USIZE_SIZE],
    };

    pub const MAX: Self = Self {
        inner: [usize::MAX; BYTES32_USIZE_SIZE],
    };

    pub fn uninit() -> MaybeUninit<Self> {
        MaybeUninit::uninit()
    }

    pub fn from_byte_fill(byte: u8) -> Self {
        let mut buffer = 0usize.to_ne_bytes();
        buffer.fill(byte);
        let init_value = usize::from_ne_bytes(buffer);
        Self {
            inner: [init_value; BYTES32_USIZE_SIZE],
        }
    }

    pub const fn zero() -> Self {
        Self {
            inner: [0usize; BYTES32_USIZE_SIZE],
        }
    }

    pub fn from_array(array: [u8; 32]) -> Self {
        unsafe {
            let mut result = Self::uninit();
            core::ptr::copy_nonoverlapping(
                array.as_ptr(),
                addr_of_mut!((*result.as_mut_ptr()).inner).cast(),
                32,
            );
            result.assume_init()
        }
    }

    pub fn is_zero(&self) -> bool {
        self.inner.iter().all(|el| *el == 0)
    }

    #[allow(clippy::should_implement_trait)]
    pub fn as_ref(&self) -> &[usize] {
        &self.inner
    }

    #[allow(clippy::should_implement_trait)]
    pub fn as_mut(&mut self) -> &mut [usize] {
        &mut self.inner
    }

    pub fn as_array_ref(&self) -> &[usize; BYTES32_USIZE_SIZE] {
        &self.inner
    }

    pub fn as_array_mut(&mut self) -> &mut [usize; BYTES32_USIZE_SIZE] {
        &mut self.inner
    }

    pub fn as_u32_array(self) -> [u32; 8] {
        unsafe { core::mem::transmute(self) }
    }

    pub fn as_u32_array_ref(&self) -> &[u32; 8] {
        unsafe { &*(&self.inner as *const usize).cast::<[u32; 8]>() }
    }

    pub fn as_u32_array_mut(&mut self) -> &mut [u32; 8] {
        unsafe { &mut *(&mut self.inner as *mut usize).cast::<[u32; 8]>() }
    }

    pub fn as_u64_array_ref(&self) -> &[u64; 4] {
        unsafe { &*(&self.inner as *const usize).cast::<[u64; 4]>() }
    }

    pub fn as_u64_array_mut(&mut self) -> &mut [u64; 4] {
        unsafe { &mut *(&mut self.inner as *mut usize).cast::<[u64; 4]>() }
    }

    pub fn as_u8_ref(&self) -> &[u8] {
        unsafe { core::slice::from_raw_parts((&self.inner as *const usize).cast::<u8>(), 32) }
    }

    pub fn as_u8_mut(&mut self) -> &mut [u8] {
        unsafe { core::slice::from_raw_parts_mut((&mut self.inner as *mut usize).cast::<u8>(), 32) }
    }

    pub fn as_u8_array(self) -> [u8; 32] {
        unsafe { core::mem::transmute(self) }
    }

    pub fn as_u8_array_ref(&self) -> &[u8; 32] {
        unsafe { &*(&self.inner as *const usize).cast::<[u8; 32]>() }
    }

    pub fn as_u8_array_mut(&mut self) -> &mut [u8; 32] {
        unsafe { &mut *(&mut self.inner as *mut usize).cast::<[u8; 32]>() }
    }

    pub fn bytereverse(&mut self) {
        cfg_if::cfg_if!(
            if #[cfg(target_endian = "big")] {
                compile_error!("unsupported architecture: big endian arch is not supported")
            } else if #[cfg(target_pointer_width = "32")] {
                self.inner.swap(0, 7);
                self.inner.swap(1, 6);
                self.inner.swap(2, 5);
                self.inner.swap(3, 4);
                for el in self.inner.iter_mut() {
                    *el = el.to_be();
                }
                return;
            } else if #[cfg(target_pointer_width = "64")] {
                self.inner.swap(0, 3);
                self.inner.swap(1, 2);
                for el in self.inner.iter_mut() {
                    *el = el.to_be();
                }
                return;
            } else {
                compile_error!("unsupported architecture")
            }
        );
    }

    pub fn into_u256_le(self) -> U256 {
        cfg_if::cfg_if!(
            if #[cfg(target_endian = "big")] {
                compile_error!("unsupported architecture: big endian arch is not supported")
            } else {
                unsafe {
                    #[allow(clippy::missing_transmute_annotations)]
                    return core::mem::transmute(self);
                }
            }
        );
    }

    pub fn into_u256_be(self) -> U256 {
        U256::from_be_bytes(self.as_u8_array())
    }

    pub fn from_u256_le(value: &U256) -> Self {
        cfg_if::cfg_if!(
            if #[cfg(target_endian = "big")] {
                compile_error!("unsupported architecture: big endian arch is not supported")
            } else {
                unsafe {
                    #[allow(clippy::missing_transmute_annotations)]
                    return core::mem::transmute_copy(value);
                }
            }
        );
    }

    pub fn from_u256_be(value: &U256) -> Self {
        let mut new = Self::uninit();
        unsafe {
            *new.assume_init_mut().as_u8_array_mut() = value.to_be_bytes();
            new.assume_init()
        }
    }
}

// here we assume left-padding of zeroes for future
#[allow(clippy::from_over_into)]
impl Into<B160> for Bytes32 {
    fn into(self) -> B160 {
        // let's hope compiler optimizes it out
        let mut bytes = [0u8; 20];
        bytes.copy_from_slice(&self.as_u8_array_ref()[12..]);
        B160::from_be_bytes(bytes)
    }
}

impl From<B160> for Bytes32 {
    fn from(value: B160) -> Self {
        let mut new = Bytes32::zero();
        new.as_u8_array_mut()[12..].copy_from_slice(&value.to_be_bytes::<{ B160::BYTES }>()[..]);

        new
    }
}

impl From<[u8; 32]> for Bytes32 {
    fn from(value: [u8; 32]) -> Self {
        let mut new = Bytes32::zero();
        new.as_u8_array_mut().copy_from_slice(&value[..]);
        new
    }
}
