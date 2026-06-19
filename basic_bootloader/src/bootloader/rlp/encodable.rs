//! The RLP encoder framework: the `RLPEncodable` / `CachingRLPEncodable` traits,
//! list envelopes, and `RLPEncodable` impls for the primitive types.
//!
//! Encoding a list is a two-step dance because the list is length-prefixed:
//! first sum every element's encoded length (`required_buffer_len`, cached where
//! useful), then write the prefix followed by the elements (`encode_into`).

use super::primitives::{
    apply_bytes_encoding, apply_list_length_encoding, apply_number_encoding,
    estimate_bytes_encoding_len, estimate_list_length_encoding_len, estimate_number_encoding_len,
};
use zk_ee::utils::{write_bytes::WriteBytes, Bytes32};

pub trait RLPEncodable {
    fn required_buffer_len(&self) -> usize;
    fn encode_into<B: ?Sized + WriteBytes>(&self, buffer: &mut B);
}

impl<T: ?Sized + RLPEncodable> RLPEncodable for &T {
    fn required_buffer_len(&self) -> usize {
        (*self).required_buffer_len()
    }

    fn encode_into<B: ?Sized + WriteBytes>(&self, buffer: &mut B) {
        (*self).encode_into(buffer);
    }
}

pub trait CachingRLPEncodable {
    fn required_buffer_len(&mut self) -> usize;
    fn encode_into<B: ?Sized + WriteBytes>(&mut self, buffer: &mut B);
}

#[derive(Debug)]
pub struct CachingEnvelope<T: RLPEncodable> {
    value: T,
    cached_len: usize,
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

    fn encode_into<B: ?Sized + WriteBytes>(&mut self, buffer: &mut B) {
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
            head,
            cached_len: 0,
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

impl<T: CachingRLPEncodable, U: CachingRLPEncodable> CachingRLPEncodable for ListElement<T, U> {
    fn required_buffer_len(&mut self) -> usize {
        let mut total_len = self.value.required_buffer_len();
        if let Some(next) = self.next.as_mut() {
            total_len += next.required_buffer_len();
        }

        total_len
    }

    fn encode_into<B: ?Sized + WriteBytes>(&mut self, buffer: &mut B) {
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
        self.cached_len + estimate_list_length_encoding_len(self.cached_len)
    }

    fn encode_into<B: ?Sized + WriteBytes>(&mut self, buffer: &mut B) {
        let _ = self.required_buffer_len();
        apply_list_length_encoding(self.cached_len, buffer);
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
        self.cached_len + estimate_list_length_encoding_len(self.cached_len)
    }

    fn encode_into<B: ?Sized + WriteBytes>(&mut self, buffer: &mut B) {
        let _ = self.required_buffer_len();
        apply_list_length_encoding(self.cached_len, buffer);
        for el in self.elements_it.clone() {
            el.encode_into(buffer);
        }
    }
}

impl RLPEncodable for bool {
    fn required_buffer_len(&self) -> usize {
        let self_u8 = if *self { 1 } else { 0 };
        estimate_number_encoding_len(&[self_u8])
    }

    fn encode_into<B: ?Sized + WriteBytes>(&self, buffer: &mut B) {
        let self_u8 = if *self { 1 } else { 0 };
        apply_number_encoding(&[self_u8], buffer);
    }
}

impl RLPEncodable for u64 {
    fn required_buffer_len(&self) -> usize {
        estimate_number_encoding_len(&self.to_be_bytes())
    }

    fn encode_into<B: ?Sized + WriteBytes>(&self, buffer: &mut B) {
        apply_number_encoding(&self.to_be_bytes(), buffer);
    }
}

impl<const N: usize> RLPEncodable for [u8; N] {
    fn required_buffer_len(&self) -> usize {
        estimate_bytes_encoding_len(self)
    }

    fn encode_into<B: ?Sized + WriteBytes>(&self, buffer: &mut B) {
        apply_bytes_encoding(self, buffer);
    }
}

impl RLPEncodable for [u8] {
    fn required_buffer_len(&self) -> usize {
        estimate_bytes_encoding_len(self)
    }

    fn encode_into<B: ?Sized + WriteBytes>(&self, buffer: &mut B) {
        apply_bytes_encoding(self, buffer);
    }
}

impl RLPEncodable for Bytes32 {
    fn required_buffer_len(&self) -> usize {
        estimate_bytes_encoding_len(self.as_u8_ref())
    }

    fn encode_into<B: ?Sized + WriteBytes>(&self, buffer: &mut B) {
        apply_bytes_encoding(self.as_u8_ref(), buffer);
    }
}
