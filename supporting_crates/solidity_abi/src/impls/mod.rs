use core::alloc::Allocator;

use ruint::aliases::U256;

use crate::codable_trait::*;

pub mod byte_ref;
pub mod codable_array;
pub mod codable_slice;
pub mod string_ref;
pub mod uint_x;

mod test_impls;

pub use self::byte_ref::Bytes;
pub use self::codable_array::Array;
pub use self::codable_slice::Slice;
pub use self::string_ref::SolidityString;
pub use self::uint_x::*;

pub fn u256_to_usize_checked(src: &U256) -> Result<usize, ()> {
    if src.as_limbs()[1] != 0 || src.as_limbs()[2] != 0 || src.as_limbs()[3] != 0 {
        return Err(());
    }

    usize::try_from(src.as_limbs()[0]).map_err(|_| ())
}

pub fn compute_selector_using_buffer<ABIParams: SelectorCodable>(
    buffer: &mut [u8],
    function_name: &str,
) -> Result<[u8; 4], ()> {
    let mut offset = 0;
    let mut is_first = true;
    match append_ascii_str(buffer, &mut offset, function_name) {
        Ok(_) => {}
        Err(_) => {
            return Err(());
        }
    }
    match append_ascii_str(buffer, &mut offset, "(") {
        Ok(_) => {}
        Err(_) => {
            return Err(());
        }
    }
    match ABIParams::append_to_selector(buffer, &mut offset, &mut is_first) {
        Ok(_) => {}
        Err(_) => {
            return Err(());
        }
    }
    match append_ascii_str(buffer, &mut offset, ")") {
        Ok(_) => {}
        Err(_) => {
            return Err(());
        }
    }

    use const_keccak256::keccak256_digest;

    // hack around slice indexing
    if offset >= buffer.len() {
        return Err(());
    }
    let digest_input = unsafe { core::slice::from_raw_parts(buffer.as_ptr(), offset) };
    let output = keccak256_digest(digest_input);
    let selector = [output[0], output[1], output[2], output[3]];

    Ok(selector)
}

pub fn compute_selector<ABIParams: SelectorCodable, A: Allocator>(
    function_name: &str,
    max_buffer_size: usize,
    allocator: A,
) -> Result<[u8; 4], ()> {
    use alloc::vec::Vec;
    let mut buffer = Vec::try_with_capacity_in(max_buffer_size, allocator).map_err(|_| ())?;
    unsafe {
        buffer.set_len(max_buffer_size);
    }

    compute_selector_using_buffer::<ABIParams>(&mut buffer[..], function_name)
}
