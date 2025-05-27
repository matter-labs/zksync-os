use core::mem::{transmute, MaybeUninit};

use alloc::{self, vec::Vec};

use crate::{abi::ABI_SLOT_SIZE, dev, qol::PipeOp};

/// The data is consists of 3 parts: the state key, metadata and the calldata. Each bit in the state key
/// represents a flag for a particular slot in the call data. The state key is ordered backwards:
/// [ k3, k2, k1, k0, MD, s0, s1, s2, s3] where kn is are keys and sn are slots. The sizes are:
///     - Keys: 1 bit.
///     - Metadata: Word multiple. Currently, 1 word.
///     - Slot: 32 bytes (Solidity ABI slot).
/// Elements are accessed by the index from `s0`:
/// - `sn` is n'th slot from `s0` address to the rights.
/// - `kn` is n'th bit from `k0` address to the left.
pub struct OverlaidCalldata {
    data: Vec<u8>,
    state_key_len: usize,
}

// WARNING: Must have the size defined in `OverlaidCalldata::METADATA_SIZE`.
#[repr(C, align(8))]
pub struct OverlaidCalldataMetadata {
    p1: u32, // reserved
    /// Calldata size in number of slots.
    pub(crate) calldata_slots: u16,
    p2: u16, // reserved
}

impl OverlaidCalldataMetadata {
    /// Safety: `dst` must point to allocated memory.
    pub(crate) unsafe fn init<'a>(
        dst: *mut OverlaidCalldataMetadata,
        calldata_slots: u16,
    ) -> &'a mut Self {
        const {
            assert!(core::mem::size_of::<OverlaidCalldataMetadata>() == 8);
        }

        let md = OverlaidCalldataMetadata {
            p1: 0,
            calldata_slots,
            p2: 0,
        };

        // Safety: Casting `OCM` to `MaybeUninit<OCM>`. The ptr is pointing to allocated memory as
        // required by the function.
        let dst = &mut *(dst as *mut MaybeUninit<OverlaidCalldataMetadata>);

        dst.write(md)
    }
}

/// In the following trait we're defining a 'word' to be equal 8 bytes.
impl OverlaidCalldata {
    /// Metadata size in words.
    // WARNING: The size defines the size of `OverlaidCalldataMetadata`.
    const METADATA_SIZE: usize = 1;

    /// # Safety
    ///
    ///  The `populate` function must write all bytes in the provided slice.
    pub unsafe fn new<F: Fn(&mut [MaybeUninit<u8>])>(
        size: usize,
        populate: F,
    ) -> Result<Self, &'static str> {
        // The host pinky swears that it is.
        if dev!() && size % ABI_SLOT_SIZE != 0 {
            return Err("Calldata isn't multiple of 32 bytes.");
        }

        if size / 32 > u16::MAX as usize {
            return Err("Calldata too large. The limit is u16::MAX slots.");
        }

        let state_key_len = size / ABI_SLOT_SIZE; // Elements == bits.

        // Round up to a word worth of bits.
        // We're rounding to u64 because it has the same alignment as U256, and thus we don't need to
        // take the architecture into account (WASM has alignment of 4, the x64 (test environment)
        // has 8). Otherwise, we would've need to somehow guarantee that the slice is aligned at
        // `state_key_len` byte. Those 4 bytes aren't worth the effort.
        let state_key_len_bits = match state_key_len % 64 {
            0 => state_key_len,
            x => state_key_len - x + 64,
        };

        let state_key_len = state_key_len_bits / 8; // Convert to bytes.
        let metadata_len = Self::METADATA_SIZE * 8; // Convert to bytes.

        let total_len = state_key_len + metadata_len + size;

        let mut v: Vec<MaybeUninit<u8>> = Vec::with_capacity(total_len);
        // Safety: The element type is MaybeUninit and the len is equal to capacity.
        v.set_len(total_len);

        // Init key bytes.
        for i in &mut v[0..state_key_len] {
            i.write(0);
        }

        // Init metadata.
        v.as_mut_ptr()
            .add(state_key_len)
            .cast::<OverlaidCalldataMetadata>()
            // size is in bytes, we need slots.
            .to(|ptr| OverlaidCalldataMetadata::init(ptr, (size / 32) as u16));

        // Write calldata bytes.
        populate(&mut v[state_key_len + metadata_len..]);

        // Safety: `MaybeUninit<u8>` and `u8` have identical layout.
        let v = transmute::<Vec<MaybeUninit<u8>>, Vec<u8>>(v);

        Ok(Self {
            data: v,
            state_key_len,
        })
    }

    pub fn as_mut_ptr(&mut self) -> *mut () {
        unsafe {
            self.data
                .as_mut_ptr()
                .add(self.state_key_len)
                .add(Self::METADATA_SIZE * 8) as *mut _
        }
    }

    /// # Safety
    ///
    /// TODO: add docs
    pub unsafe fn get_metadata<'a>(base_ptr: *const ()) -> &'a OverlaidCalldataMetadata {
        let md_ptr = base_ptr.cast::<OverlaidCalldataMetadata>().sub(1);

        &*md_ptr
    }

    /// # Safety
    ///
    /// TODO: add docs
    pub unsafe fn base_to_key_ptr(base_ptr: *mut ()) -> *mut u32 {
        const { assert!(core::mem::size_of::<OverlaidCalldataMetadata>() % 8 == 0) };
        base_ptr
            .byte_sub(Self::METADATA_SIZE * core::mem::size_of::<OverlaidCalldataMetadata>())
            .cast::<u32>()
            .sub(1) // Move ptr to first u32 to the left of metadata.
    }
}

#[cfg(test)]
mod tests {
    use core::mem::MaybeUninit;

    use super::OverlaidCalldata;

    #[test]
    fn init_size_correct() {
        let f = |x: &mut [MaybeUninit<u8>]| {
            for e in x {
                e.write(0);
            }
        };
        // let f = |vec: &mut _| {  };

        let a = |e, s| {
            let x = unsafe { OverlaidCalldata::new(s, f) }.unwrap();
            assert_eq!(e, x.state_key_len, "Wrong state len for size {}.", s);
            assert_eq!(e + s + 8, x.data.len(), "Wrong data len for size {}.", s);
        };

        a(0, 0 * 32);
        a(8, 1 * 32);
        a(8, 64 * 32);
        a(16, 65 * 32);
        a(16, 128 * 32);
        a(24, 129 * 32);
    }

    #[test]
    fn init_populate() {
        let cd = unsafe {
            OverlaidCalldata::new(4 * 32, |x| {
                for (i, e) in x.iter_mut().enumerate() {
                    e.write(i as u8);
                }
            })
        }
        .unwrap();

        for i in 0..cd.state_key_len {
            assert_eq!(0, cd.data[i]);
        }

        for i in 0..(4 * 32) {
            assert_eq!(i as u8, cd.data[cd.state_key_len + i + 8]);
        }
    }

    #[test]
    fn get_mut_ptr() {
        let mut cd = unsafe {
            OverlaidCalldata::new(64, |x| {
                let ptr = x as *mut _ as *mut u32;

                ptr.write(ptr as u32);
            })
        }
        .unwrap();

        let ptr = cd.as_mut_ptr();

        let raw = unsafe { (ptr as *mut u32).read() };

        assert_eq!(raw, ptr as u32);
    }
}
