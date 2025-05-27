//!
//! Bunch of methods needed to decode a transaction.
//!

use super::u256be_ptr::U256BEPtr;
use ruint::aliases::U256;
use zk_ee::utils::u256_try_to_usize;

#[allow(dead_code)]
///
/// Decodes `bytes32[]`, returns slice to its content and total encoding length (with length)
///
pub fn decode_bytes32_array(slice: &[u8]) -> Result<(&[u8], usize), ()> {
    let (length, slice) = U256BEPtr::try_from_slice(slice)?;
    let length = u256_try_to_usize(&length.read()).ok_or(())?;
    let slice_len = length.checked_mul(U256::BYTES).ok_or(())?;
    let encoding_length = slice_len.checked_add(U256::BYTES).ok_or(())?;
    if slice.len() < slice_len {
        return Err(());
    }
    Ok((&slice[..slice_len], encoding_length))
}

#[allow(dead_code)]
///
/// Decodes `bytes`, returns slice to its content and total encoding length (with padding and length)
///
pub fn decode_bytes(slice: &[u8]) -> Result<(&[u8], usize), ()> {
    let (length, slice) = U256BEPtr::try_from_slice(slice)?;
    let length = u256_try_to_usize(&length.read()).ok_or(())?;

    let length_words = length.div_ceil(U256::BYTES);
    let padded_len = length_words.checked_mul(U256::BYTES).ok_or(())?;

    if slice.len() < padded_len {
        return Err(());
    }

    let encoding_len = padded_len.checked_add(U256::BYTES).ok_or(())?;

    // check that it's padded with zeroes
    if length % U256::BYTES != 0 {
        let zero_bytes = U256::BYTES - (length % U256::BYTES);
        #[allow(clippy::needless_range_loop)]
        for i in padded_len - zero_bytes..padded_len {
            if slice[i] != 0 {
                return Err(());
            }
        }
    }

    let bytes = &slice[..length];
    Ok((bytes, encoding_len))
}
