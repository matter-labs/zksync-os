//! Bridge that lets the generic RLP encoders feed the Ethereum-mode MPT.
//!
//! Used, for example, to build the Ethereum-mode transactions/receipts tries:
//! a `ReceiptEncoder` becomes a `CellEnvelope` leaf in the receipts MPT.
//!
//! The MPT stores leaf values as `LazyEncodable`, encoding them into its own
//! `ByteBuffer` sink. `CellEnvelope` wraps a `CachingRLPEncodable` (e.g. a
//! `ReceiptEncoder`) as such a leaf. The small `ByteBufferAsWriteBytes` adapter
//! lets the `WriteBytes`-based RLP encoders write into the MPT's `ByteBuffer`,
//! so the generic RLP code stays decoupled from the MPT.

use crate::bootloader::rlp::CachingRLPEncodable;
use basic_system::system_implementation::ethereum_storage_model::{ByteBuffer, LazyEncodable};
use zk_ee::utils::write_bytes::WriteBytes;

/// Adapts an MPT `ByteBuffer` sink to the `WriteBytes` sink used by RLP encoders.
struct ByteBufferAsWriteBytes<'a>(&'a mut dyn ByteBuffer);

impl WriteBytes for ByteBufferAsWriteBytes<'_> {
    fn write(&mut self, buf: &[u8]) {
        self.0.write_slice(buf);
    }

    fn write_byte(&mut self, byte: u8) {
        self.0.write_byte(byte);
    }
}

/// Wraps a `CachingRLPEncodable` so it can be stored as an MPT leaf value.
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

    pub fn encode_into<B: ?Sized + WriteBytes>(&self, buffer: &mut B) {
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
        self.encode_into(&mut ByteBufferAsWriteBytes(into));
    }

    fn encoding_len_and_first_byte(&self) -> (usize, u8) {
        let len = self.required_buffer_len();
        assert!(len > 1);
        (len, 0xff)
    }
}
