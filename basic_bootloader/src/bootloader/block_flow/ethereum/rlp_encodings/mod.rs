use crate::bootloader::rlp;
use basic_system::system_implementation::ethereum_storage_model::ByteBuffer;

mod receipt;
mod utils;
pub(crate) use self::receipt::ReceiptEncoder;
pub(crate) use self::utils::*;

pub trait RLPEncodable {
    fn required_buffer_len(&self) -> usize;
    fn encode_into<B: ?Sized + ByteBuffer>(&self, buffer: &mut B);
}

impl<'a, T: RLPEncodable> RLPEncodable for &'a T {
    fn required_buffer_len(&self) -> usize {
        (*self).required_buffer_len()
    }
    fn encode_into<B: ?Sized + ByteBuffer>(&self, buffer: &mut B) {
        (*self).encode_into(buffer);
    }
}

pub trait CachingRLPEncodable {
    // mut allows some caching of length if needed by out internedialte structures
    fn required_buffer_len(&mut self) -> usize;
    fn encode_into<B: ?Sized + ByteBuffer>(&mut self, buffer: &mut B);
}

// To be used and implemented only for a small number of types. This envelope assumes nothing
// about internals - it's implementation can encode as list for convenience
#[derive(Debug)]
pub struct CachingEnvelope<T: RLPEncodable> {
    pub(crate) value: T,
    pub(crate) cached_len: usize,
}

impl<T: RLPEncodable> CachingEnvelope<T> {
    pub fn new(value: T) -> Self {
        Self {
            value,
            cached_len: 0,
        }
    }
}

impl<T: RLPEncodable> CachingRLPEncodable for CachingEnvelope<T> {
    fn required_buffer_len(&mut self) -> usize {
        if self.cached_len == 0 {
            self.cached_len = self.value.required_buffer_len();
        }
        self.cached_len
    }

    fn encode_into<B: ?Sized + ByteBuffer>(&mut self, buffer: &mut B) {
        self.value.encode_into(buffer);
    }
}

pub struct ListElement<T: CachingRLPEncodable, U: CachingRLPEncodable> {
    value: T,
    next: Option<U>,
}

impl<T: CachingRLPEncodable, U: CachingRLPEncodable> ListElement<T, U> {
    pub fn chained(value: T, next: U) -> Self {
        Self {
            value,
            next: Some(next),
        }
    }
}

pub struct ListEnvelope<T: CachingRLPEncodable, U: CachingRLPEncodable> {
    head: ListElement<T, U>,
    cached_len: usize,
}

impl<T: CachingRLPEncodable, U: CachingRLPEncodable> ListEnvelope<T, U> {
    pub fn from_head(head: ListElement<T, U>) -> Self {
        Self {
            cached_len: 0,
            head,
        }
    }
}

pub struct HomogeneousListEnvelope<T: RLPEncodable, I: Iterator<Item = T> + Clone> {
    elements_it: I,
    cached_len: usize,
}

impl<T: RLPEncodable, I: Iterator<Item = T> + Clone> HomogeneousListEnvelope<T, I> {
    pub fn new(elements_it: I) -> Self {
        Self {
            elements_it,
            cached_len: 0,
        }
    }
}

// recursive implementation for lists

impl<T: CachingRLPEncodable, U: CachingRLPEncodable> CachingRLPEncodable for ListElement<T, U> {
    fn required_buffer_len(&mut self) -> usize {
        // list doesn't cache, but we expect it's internals to cache if needed,
        // or ListEnvelope to cache once on top
        let mut total_len = self.value.required_buffer_len();
        if let Some(next) = self.next.as_mut() {
            total_len += next.required_buffer_len();
        }

        total_len
    }

    fn encode_into<B: ?Sized + ByteBuffer>(&mut self, buffer: &mut B) {
        self.value.encode_into(buffer);
        if let Some(next) = self.next.as_mut() {
            next.encode_into(buffer);
        }
    }
}

impl<T: CachingRLPEncodable, U: CachingRLPEncodable> CachingRLPEncodable for ListEnvelope<T, U> {
    fn required_buffer_len(&mut self) -> usize {
        if self.cached_len == 0 {
            self.cached_len = self.head.required_buffer_len();
        }
        // it's a list
        self.cached_len + rlp::estimate_encoding_len_for_payload_length(self.cached_len)
    }

    fn encode_into<B: ?Sized + ByteBuffer>(&mut self, buffer: &mut B) {
        let _ = self.required_buffer_len();
        let payload_len = self.cached_len;
        apply_list_length_encoding(payload_len, buffer);
        self.head.encode_into(buffer);
    }
}

impl<T: RLPEncodable, I: Iterator<Item = T> + Clone> CachingRLPEncodable
    for HomogeneousListEnvelope<T, I>
{
    fn required_buffer_len(&mut self) -> usize {
        if self.cached_len == 0 {
            for el in self.elements_it.clone() {
                self.cached_len += el.required_buffer_len();
            }
        }
        // it's a list
        self.cached_len + rlp::estimate_encoding_len_for_payload_length(self.cached_len)
    }

    fn encode_into<B: ?Sized + ByteBuffer>(&mut self, buffer: &mut B) {
        // just compute it if needed
        let _ = self.required_buffer_len();
        // we need only the payload
        let payload_len = self.cached_len;
        apply_list_length_encoding(payload_len, buffer);
        for el in self.elements_it.clone() {
            el.encode_into(buffer);
        }
    }
}
