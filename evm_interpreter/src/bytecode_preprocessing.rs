use zk_ee::system::{errors::{FatalError, InternalError, SystemError}, Resource, System, SystemTypes};

pub struct BytecodePreprocessingData<S: SystemTypes> {
    pub original_bytecode_len: usize,
    pub jumpdest_bitmap: BitMap<S>,
}

impl<S: SystemTypes> BytecodePreprocessingData<S> {
    pub(crate) fn empty(system: &mut System<S>) -> Self {
        let jumpdest_bitmap = BitMap::<S>::empty(system);
        let original_bytecode_len = 0;

        Self {
            original_bytecode_len,
            jumpdest_bitmap,
        }
    }

    pub fn is_valid_jumpdest(&self, offset: usize) -> bool {
        if offset >= self.original_bytecode_len {
            false
        } else {
            // we are in range of even the extended bytecode, so we are safe
            unsafe { self.jumpdest_bitmap.get_bit_unchecked(offset) }
        }
    }

    pub fn from_raw_bytecode(
        padded_bytecode: &[u8],
        original_len: u32,
        system: &mut System<S>,
        resources: &mut S::Resources,
    ) -> Result<Self, FatalError> {
        use crate::native_resource_constants::BYTECODE_PREPROCESSING_BYTE_NATIVE_COST;
        use zk_ee::system::{Computational, Resources};
        let native_cost = <S::Resources as Resources>::Native::from_computational(
            BYTECODE_PREPROCESSING_BYTE_NATIVE_COST.saturating_mul(padded_bytecode.len() as u64),
        );
        resources
            .charge(&S::Resources::from_native(native_cost))
            .map_err(|e| match e {
                SystemError::Internal(e) => FatalError::Internal(e),
                SystemError::OutOfErgs => {
                    FatalError::Internal(InternalError("OOE when charging only native"))
                }
                SystemError::OutOfNativeResources => FatalError::OutOfNativeResources,
            })?;
        let jump_map = analyze::<S>(padded_bytecode, system)
            .map_err(|_| InternalError("Could not preprocess bytecode"))?;
        let new = Self {
            original_bytecode_len: original_len as usize,
            jumpdest_bitmap: jump_map,
        };

        Ok(new)
    }
}

pub struct BitMap<S: SystemTypes> {
    inner: Vec<usize, S::Allocator>,
}

impl<S: SystemTypes> BitMap<S> {
    pub(crate) fn empty(system: &mut System<S>) -> Self {
        Self {
            inner: Vec::new_in(system.get_allocator()),
        }
    }

    pub(crate) fn allocate_for_bit_capacity(capacity: usize, system: &mut System<S>) -> Self {
        let usize_capacity =
            capacity.next_multiple_of(usize::BITS as usize) / (usize::BITS as usize);
        let mut storage = Vec::with_capacity_in(usize_capacity, system.get_allocator());
        storage.resize(usize_capacity, 0);

        Self { inner: storage }
    }

    /// # Safety
    /// pos must be within the bounds of the bitmap.
    pub(crate) unsafe fn set_bit_on_unchecked(&mut self, pos: usize) {
        let (word_idx, bit_idx) = (pos / usize::BITS as usize, pos % usize::BITS as usize);
        let dst = unsafe { self.inner.get_unchecked_mut(word_idx) };
        *dst |= 1usize << bit_idx;
    }

    /// # Safety
    /// [pos] must be within the bounds of the bitmap.
    pub(crate) unsafe fn get_bit_unchecked(&self, pos: usize) -> bool {
        let (word_idx, bit_idx) = (pos / (usize::BITS as usize), pos % (usize::BITS as usize));
        unsafe { self.inner.get_unchecked(word_idx) & (1usize << bit_idx) != 0 }
    }
}

/// Analyzs bytecode to build a jump map.
fn analyze<S: SystemTypes>(code: &[u8], system: &mut System<S>) -> Result<BitMap<S>, ()> {
    let code_len = code.len();
    let mut jumps = BitMap::<S>::allocate_for_bit_capacity(code_len, system);
    let mut code_iter = code.iter().copied().enumerate();

    use crate::opcodes as opcode;

    while let Some((offset, opcode)) = code_iter.next() {
        if opcode::JUMPDEST == opcode {
            // SAFETY: jumps are max length of the code
            unsafe { jumps.set_bit_on_unchecked(offset) }
            // step by 1 is automatic
        } else {
            let push_bytes = opcode.wrapping_sub(opcode::PUSH1);
            if push_bytes < 32 {
                // we just consumed encoding of "PUSH_X" itself, and now we should
                // consume X bytes after
                // step by 1 is automatic, and then we need to skip push_offset + 1
                match code_iter.advance_by((push_bytes + 1) as usize) {
                    Ok(_) => {
                        // nothing, we continue
                    }
                    Err(_advanced_by) => {
                        // actually we are fine, since bytecode is virtually extendable with zero-pad for EVM
                    }
                }
            } else {
                // step by 1 is automatic
            }
        }
    }

    Ok(jumps)
}