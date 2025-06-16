use core::ops::DerefMut;

use crate::*;
use ruint::aliases::B160;
use zk_ee::kv_markers::ExactSizeChain;
use zk_ee::system::Ergs;
use zk_ee::system::EthereumLikeTypes;
use zk_ee::system::Resources;

#[inline(always)]
pub(crate) unsafe fn assume(cond: bool) {
    if !cond {
        core::hint::unreachable_unchecked()
    }
}

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
    result.copy_from_slice(hash.as_slice());

    result
}

impl<S: EthereumLikeTypes> Interpreter<'_, S> {
    #[inline]
    pub(crate) fn cast_to_usize(
        &mut self,
        src: &U256,
        error_to_set: ExitCode,
    ) -> Result<usize, ExitCode> {
        u256_try_to_usize(src).ok_or(error_to_set)
    }

    /// Helper for casting memory offset and length.
    /// If len is zero, offset is ignored.
    pub(crate) fn cast_offset_and_len(
        &mut self,
        offset: &U256,
        len: &U256,
        error_to_set: ExitCode,
    ) -> Result<(usize, usize), ExitCode> {
        if len.is_zero() {
            Ok((0, 0))
        } else {
            let offset = self.cast_to_usize(offset, error_to_set)?;
            let len = self.cast_to_usize(len, error_to_set)?;
            Ok((offset, len))
        }
    }

    #[inline(always)]
    pub(crate) fn spend_gas(&mut self, to_spend: u64) -> Result<(), ExitCode> {
        spend_gas_from_resources(&mut self.resources, to_spend)
    }

    #[inline(always)]
    pub(crate) fn spend_gas_and_native(&mut self, gas: u64, native: u64) -> Result<(), ExitCode> {
        spend_gas_and_native_from_resources(&mut self.resources, gas, native)
    }

    #[inline(always)]
    pub(crate) fn gas_left(&self) -> u64 {
        self.resources.ergs().0 / ERGS_PER_GAS
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

    pub(crate) fn returndata(&'_ self) -> &'_ [u8] {
        self.returndata
    }

    pub(crate) fn resize_heap(&mut self, offset: usize, len: usize) -> Result<(), ExitCode> {
        use native_resource_constants::*;
        let max_offset = offset.saturating_add(len);
        let multiple_of_32 = if max_offset > ((u32::MAX - 31) as usize) {
            return Err(ExitCode::MemoryLimitOOG);
        } else {
            max_offset.next_multiple_of(32)
        };
        let current_heap_size = self.memory_len();
        if multiple_of_32 > current_heap_size {
            let net_byte_increase = multiple_of_32 - current_heap_size;
            let new_heap_size_words = multiple_of_32 as u64 / 32;

            let end_cost = crate::gas_constants::MEMORY
                .saturating_mul(new_heap_size_words)
                .saturating_add(new_heap_size_words.saturating_mul(new_heap_size_words) / 512);
            let net_cost_gas = end_cost - self.gas_paid_for_heap_growth;
            let net_cost_native = HEAP_EXPANSION_BASE_NATIVE_COST.saturating_add(
                HEAP_EXPANSION_PER_BYTE_NATIVE_COST.saturating_mul(net_byte_increase as u64),
            );
            self.spend_gas_and_native(net_cost_gas, net_cost_native)?;
            self.gas_paid_for_heap_growth = end_cost;

            self.heap
                .resize(multiple_of_32, 0)
                .map_err(|_| ExitCode::MemoryOOG)?;
        }

        Ok(())
    }

    #[inline(always)]
    pub(crate) const fn is_static_frame(&self) -> bool {
        self.is_static
    }

    #[inline]
    pub fn copy_cost(&mut self, len: u64) -> Result<(u64, u64), ExitCode> {
        let get_cost = |len: u64| -> Option<(u64, u64)> {
            let num_words = len.checked_next_multiple_of(32)? / 32;
            let gas = crate::gas_constants::COPY.checked_mul(num_words)?;
            let native = crate::native_resource_constants::COPY_BYTE_NATIVE_COST
                .checked_mul(len)?
                .checked_add(crate::native_resource_constants::COPY_BASE_NATIVE_COST)?;
            Some((gas, native))
        };
        get_cost(len).ok_or(ExitCode::OutOfGas)
    }

    #[inline]
    pub fn very_low_copy_cost(&mut self, len: u64) -> Result<(u64, u64), ExitCode> {
        let get_cost = |len: u64| -> Option<(u64, u64)> {
            let num_words = len.checked_next_multiple_of(32)? / 32;
            let gas = crate::gas_constants::VERYLOW
                .checked_add(crate::gas_constants::COPY.checked_mul(num_words)?)?;
            let native = crate::native_resource_constants::COPY_BASE_NATIVE_COST
                .checked_mul(len)?
                .checked_add(crate::native_resource_constants::COPY_BASE_NATIVE_COST)?;
            Some((gas, native))
        };
        get_cost(len).ok_or(ExitCode::OutOfGas)
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

#[inline(always)]
pub(crate) fn spend_gas_from_resources<R: Resources>(
    resources: &mut R,
    to_spend: u64,
) -> Result<(), ExitCode> {
    let Some(ergs_cost) = to_spend.checked_mul(ERGS_PER_GAS) else {
        return Err(ExitCode::OutOfGas);
    };
    let resource_cost = R::from_ergs(Ergs(ergs_cost));
    resources.charge(&resource_cost)?;
    Ok(())
}

#[inline(always)]
pub(crate) fn spend_gas_and_native_from_resources<R: Resources>(
    resources: &mut R,
    gas: u64,
    native: u64,
) -> Result<(), ExitCode> {
    use zk_ee::system::Computational;
    let Some(ergs_cost) = gas.checked_mul(ERGS_PER_GAS) else {
        return Err(ExitCode::OutOfGas);
    };
    let resource_cost =
        R::from_ergs_and_native(Ergs(ergs_cost), R::Native::from_computational(native));
    resources.charge(&resource_cost)?;
    Ok(())
}

// Returns the result of subtracting 1/64th gas from
// some resources.
#[inline(always)]
pub(crate) fn apply_63_64_rule(ergs: Ergs) -> Ergs {
    // We need to apply the rule over gas, not ergs
    let gas = ergs.0 / ERGS_PER_GAS;
    Ergs(ergs.0 - (gas / 64) * ERGS_PER_GAS)
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
