use core::ops::DerefMut;

use crate::*;
use ruint::aliases::B160;
use zk_ee::{system::EthereumLikeTypes, utils::exact_size_chain::ExactSizeChain};

pub fn bytereverse_u256(value: &mut U256) {
    // assuming LE
    unsafe {
        let limbs = value.as_limbs_mut();
        core::ptr::swap(&mut limbs[0] as *mut u64, &mut limbs[3] as *mut u64);
        core::ptr::swap(&mut limbs[1] as *mut u64, &mut limbs[2] as *mut u64);
        for limb in limbs.iter_mut() {
            *limb = limb.to_be();
        }
    }
}

pub fn evm_bytecode_hash(bytecode: &[u8]) -> [u8; 32] {
    use crypto::sha3::{Digest, Keccak256};
    let hash = Keccak256::digest(bytecode);
    let mut result = [0u8; 32];
    #[allow(deprecated)]
    result.copy_from_slice(hash.as_slice());

    result
}

impl<S: EthereumLikeTypes> Interpreter<'_, S> {
    #[inline]
    pub(crate) fn cast_to_usize(src: &U256, error_to_set: ExitCode) -> Result<usize, ExitCode> {
        u256_try_to_usize(src).ok_or(error_to_set)
    }

    /// Helper for casting memory offset and length.
    /// If len is zero, offset is ignored.
    pub(crate) fn cast_offset_and_len(
        offset: &U256,
        len: &U256,
        error_to_set: ExitCode,
    ) -> Result<(usize, usize), ExitCode> {
        if len.is_zero() {
            Ok((0, 0))
        } else {
            let offset = Self::cast_to_usize(offset, error_to_set.clone())?;
            let len = Self::cast_to_usize(len, error_to_set)?;
            Ok((offset, len))
        }
    }

    #[inline(always)]
    pub(crate) fn memory_len(&self) -> usize {
        self.heap.len()
    }

    pub(crate) fn clear_last_returndata(&mut self) {
        self.returndata_location = 0..0;
    }

    pub(crate) fn calldata(&'_ self) -> &'_ [u8] {
        self.calldata
    }

    pub(crate) fn heap(&'_ mut self) -> &'_ mut [u8] {
        self.heap.deref_mut()
    }

    pub(crate) fn resize_heap(&mut self, offset: usize, len: usize) -> Result<(), ExitCode> {
        Self::resize_heap_implementation(&mut self.heap, &mut self.gas, offset, len)
    }

    pub(crate) fn resize_heap_implementation<'a>(
        heap: &mut SliceVec<'a, u8>,
        gas: &mut Gas<S>,
        offset: usize,
        len: usize,
    ) -> Result<(), ExitCode> {
        let max_offset = offset.saturating_add(len);
        let new_heap_size = if max_offset > ((u32::MAX - 31) as usize) {
            return Err(ExitCode::EvmError(EvmError::MemoryLimitOOG));
        } else {
            max_offset.next_multiple_of(32)
        };
        let current_heap_size = heap.len();
        if new_heap_size > current_heap_size {
            gas.pay_for_memory_growth(current_heap_size, new_heap_size)?;

            heap.resize(new_heap_size, 0)
                .map_err(|_| ExitCode::EvmError(EvmError::OutOfGas))?;
        }

        Ok(())
    }

    #[inline(always)]
    pub(crate) const fn is_static_frame(&self) -> bool {
        self.is_static
    }
}

pub(crate) const MAX_CREATE_RLP_ENCODING_LEN: usize = 1 + 1 + 20 + 1 + 8;

///
/// Rlp encoding for create.
/// Returns rlp([address, nonce])
///
pub(crate) fn create_quasi_rlp(address: &B160, nonce: u64) -> impl ExactSizeIterator<Item = u8> {
    let address_bytes = address.to_be_bytes::<{ B160::BYTES }>();

    let nonce_bytes = nonce.to_be_bytes();
    let skip_nonce_len = nonce_bytes.iter().take_while(|el| **el == 0).count();
    let nonce_len = 8 - skip_nonce_len;

    // manual encoding of the list
    use either::Either;
    if nonce_len == 1 && nonce_bytes[7] < 128 {
        // we encode
        // - 0xc0 + payload len
        // - 0x80 + 20(address len)
        // - address
        // - one byte nonce

        let payload_len = 1 + B160::BYTES + 1;

        Either::Left(ExactSizeChain::new(
            [
                // payload_len <= 23
                0xc0u8 + (payload_len as u8),
                0x80u8 + B160::BYTES as u8,
            ]
            .into_iter(),
            ExactSizeChain::new(address_bytes.into_iter(), core::iter::once(nonce_bytes[7])),
        ))
    } else {
        // we encode
        // - 0xc0 + payload len
        // - 0x80 + 20(address len)
        // - address
        // - 0x80 + length of nonce
        // - nonce

        let payload_len = 1 + B160::BYTES + 1 + nonce_len;

        Either::Right(ExactSizeChain::new(
            [
                // payload_len <= 30
                0xc0u8 + (payload_len as u8),
                0x80u8 + B160::BYTES as u8,
            ]
            .into_iter(),
            ExactSizeChain::new(
                address_bytes.into_iter(),
                ExactSizeChain::new(
                    // nonce_len <= 8
                    core::iter::once(0x80u8 + (nonce_len as u8)),
                    nonce_bytes.into_iter().skip(skip_nonce_len),
                ),
            ),
        ))
    }
}

/// Helper to check if an address is an ethereum precompile
#[inline(always)]
pub fn is_precompile(address: &B160) -> bool {
    let highest_precompile_address = 10;
    let limbs = address.as_limbs();
    if limbs[1] != 0u64 || limbs[2] != 0u64 {
        return false;
    }
    limbs[0] > 0 && limbs[0] <= highest_precompile_address
}
