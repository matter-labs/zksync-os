use core::ops::DerefMut;

use crate::*;
use ruint::aliases::B160;
use zk_ee::system::EthereumLikeTypes;

pub fn bytereverse_u256(value: &mut U256) {
    value.bytereverse();
}

pub fn evm_bytecode_hash(bytecode: &[u8]) -> [u8; 32] {
    use crypto::sha3::Keccak256;
    use crypto::MiniDigest;
    let hash = Keccak256::digest(bytecode);
    let mut result = [0u8; 32];
    result.copy_from_slice(hash.as_slice());

    result
}

impl<S: EthereumLikeTypes> Interpreter<'_, S> {
    #[inline]
    pub(crate) fn cast_to_usize(src: &U256, error_to_set: ExitCode) -> Result<usize, ExitCode> {
        src.try_to_usize().ok_or(error_to_set)
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
    let mut encoding = [0u8; MAX_CREATE_RLP_ENCODING_LEN];
    let mut offset = 0;

    // manual encoding of the list
    if nonce_len == 1 && nonce_bytes[7] < 128 {
        let payload_len = 1 + B160::BYTES + 1;
        encoding[offset] = 0xc0u8 + (payload_len as u8);
        offset += 1;
        encoding[offset] = 0x80u8 + B160::BYTES as u8;
        offset += 1;
        encoding[offset..offset + B160::BYTES].copy_from_slice(&address_bytes);
        offset += B160::BYTES;
        encoding[offset] = nonce_bytes[7];
        offset += 1;
    } else {
        let payload_len = 1 + B160::BYTES + 1 + nonce_len;
        encoding[offset] = 0xc0u8 + (payload_len as u8);
        offset += 1;
        encoding[offset] = 0x80u8 + B160::BYTES as u8;
        offset += 1;
        encoding[offset..offset + B160::BYTES].copy_from_slice(&address_bytes);
        offset += B160::BYTES;
        encoding[offset] = 0x80u8 + (nonce_len as u8);
        offset += 1;
        encoding[offset..offset + nonce_len].copy_from_slice(&nonce_bytes[skip_nonce_len..]);
        offset += nonce_len;
    }

    encoding.into_iter().take(offset)
}

/// Helper to check if an address is an ethereum precompile
#[inline(always)]
pub fn is_precompile(address: &B160) -> bool {
    let limbs = address.as_limbs();
    if limbs[1] != 0u64 || limbs[2] != 0u64 {
        return false;
    }
    let Ok(low) = limbs[0].try_into() else {
        return false;
    };
    precompile_addresses::PRECOMPILE_ADDRESSES_LOWS.contains(&low)
}
