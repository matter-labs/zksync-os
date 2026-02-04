use basic_system::system_implementation::ethereum_storage_model::LazyEncodable;

use super::*;
use crate::bootloader::transaction_flow::ethereum::LogsBloom;

pub(crate) fn apply_u64_encoding<B: ?Sized + ByteBuffer>(value: u64, buffer: &mut B) {
    if value == 0 {
        buffer.write_byte(0x80);
    } else if value < 0x80 {
        buffer.write_byte(value as u8);
    } else {
        let bits = 64 - value.leading_zeros();
        let encoding_bytes = bits.div_ceil(8) as usize;
        let length_bytes = value.to_be_bytes();
        buffer.write_byte(0x80 + encoding_bytes as u8);
        buffer.write_slice(&length_bytes[(8 - encoding_bytes)..]);
    }
}

pub(crate) fn apply_length_encoding<const OFFSET: u8, B: ?Sized + ByteBuffer>(
    length: usize,
    buffer: &mut B,
) {
    if length <= 55 {
        buffer.write_byte(OFFSET + length as u8);
    } else {
        let length_bytes = length.to_be_bytes();
        let non_zero_byte = length_bytes.iter().position(|&byte| byte != 0).unwrap();
        buffer.write_byte(OFFSET + 55 + (length_bytes.len() - non_zero_byte) as u8);
        buffer.write_slice(&length_bytes[non_zero_byte..]);
    }
}

pub(crate) fn apply_slice_length_encoding<B: ?Sized + ByteBuffer>(length: usize, buffer: &mut B) {
    apply_length_encoding::<0x80, B>(length, buffer)
}

pub(crate) fn apply_list_length_encoding<B: ?Sized + ByteBuffer>(length: usize, buffer: &mut B) {
    apply_length_encoding::<0xc0, B>(length, buffer)
}

impl RLPEncodable for bool {
    fn required_buffer_len(&self) -> usize {
        1
    }

    fn encode_into<B: ?Sized + ByteBuffer>(&self, buffer: &mut B) {
        if *self {
            buffer.write_byte(0x01);
        } else {
            buffer.write_byte(0x80);
        }
    }
}

impl RLPEncodable for u64 {
    fn required_buffer_len(&self) -> usize {
        rlp::estimate_number_encoding_len(&self.to_be_bytes())
    }

    fn encode_into<B: ?Sized + ByteBuffer>(&self, buffer: &mut B) {
        apply_u64_encoding(*self, buffer);
    }
}

impl RLPEncodable for LogsBloom {
    fn required_buffer_len(&self) -> usize {
        3 + 256
    }

    fn encode_into<B: ?Sized + ByteBuffer>(&self, buffer: &mut B) {
        buffer.write_slice(&[0xb7 + 2, 0x01, 0x00]);
        buffer.write_slice(self.as_bytes());
    }
}

pub struct CellEnvelope<T: CachingRLPEncodable> {
    value: core::cell::UnsafeCell<T>,
}

impl<T: CachingRLPEncodable> CellEnvelope<T> {
    pub fn new(value: T) -> Self {
        Self {
            value: core::cell::UnsafeCell::new(value),
        }
    }

    pub fn required_buffer_len(&self) -> usize {
        unsafe { self.value.as_mut_unchecked().required_buffer_len() }
    }

    pub fn encode_into<B: ?Sized + ByteBuffer>(&self, buffer: &mut B) {
        unsafe {
            self.value.as_mut_unchecked().encode_into(buffer);
        }
    }
}

impl<T: CachingRLPEncodable> core::fmt::Debug for CellEnvelope<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("CellEnvelope").finish()
    }
}

impl<T: CachingRLPEncodable> LazyEncodable for CellEnvelope<T> {
    fn encode(&self, into: &mut dyn ByteBuffer) {
        self.encode_into(into);
    }

    fn encoding_len_and_first_byte(&self) -> (usize, u8) {
        // For all our
        let len = self.required_buffer_len();
        assert!(len > 1);
        (len, 0xff)
    }
}
