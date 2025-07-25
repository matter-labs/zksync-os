use core::{mem::MaybeUninit, ops::Range};

use ruint::aliases::U256;

use crate::{memory::slice_vec::SliceVec, types_config::SystemIOTypesConfig};

use super::SystemTypes;

pub struct EvmFrameForTracer<'a, S: SystemTypes> {
    /// Instruction pointer
    pub instruction_pointer: usize,
    /// Resources left
    pub resources: &'a S::Resources,
    /// Stack view
    pub stack: EvmStackForTracer<'a>,
    /// Caller address
    pub caller: <S::IOTypes as SystemIOTypesConfig>::Address,
    /// Callee address
    pub address: <S::IOTypes as SystemIOTypesConfig>::Address,
    /// calldata
    pub calldata: &'a [u8],
    /// returndata is available from here if it exists
    pub returndata: &'a [u8],
    /// Heap that belongs to this interpreter frame
    pub heap: &'a SliceVec<'a, u8>,
    /// returndata location serves to save range information at various points
    pub returndata_location: Range<usize>,
    /// Bytecode
    pub bytecode: &'a [u8],
    /// Call value
    pub call_value: &'a U256,
    /// Is interpreter call static.
    pub is_static: bool,
    /// Is interpreter call executing construction code.
    pub is_constructor: bool,
}

pub struct EvmStackForTracer<'a> {
    buffer: &'a [MaybeUninit<U256>; 1024],
    // our length both indicates how many elements are there, and
    // at least how many of them are initialized
    len: usize,
}

impl<'a> EvmStackForTracer<'a> {
    /// # Safety
    /// First `len` elements of buffer are expected to be initialized
    pub unsafe fn from_parts(buffer: &'a [MaybeUninit<U256>; 1024], len: usize) -> Self {
        assert!(len <= 1024);
        Self { buffer, len }
    }

    pub fn to_slice(&self) -> &[U256] {
        unsafe { core::slice::from_raw_parts(self.buffer.as_ptr().cast::<U256>(), self.len) }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn peek_n(&self, index: usize) -> Result<&'a U256, EvmStackError> {
        unsafe {
            if self.len < index + 1 {
                return Err(EvmStackError::StackUnderflow);
            }
            let offset = self.len - (index + 1);
            let p0 = self
                .buffer
                .as_ptr()
                .add(offset)
                .as_ref()
                .expect("Should not be null")
                .assume_init_ref();

            Ok(p0)
        }
    }
}

pub enum EvmStackError {
    StackUnderflow,
}
