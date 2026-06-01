//!
//! The rlp encoding implementation for hashing.
//! It writes the rlp encoded values directly to hasher without allocating additional memory.
//!
//! There are also methods to estimate the encoding length, useful for lists encoding.
//! The list encoding pipeline should look like:
//! - Estimate encoding length for every list element.
//! - Calculate the list encoding length.
//! - Apply the list encoding length encoding.
//! - Apply encoded elements.
//!

use basic_system::system_implementation::ethereum_storage_model::ByteBuffer;
use basic_system::system_implementation::ethereum_storage_model::LazyEncodable;
use crypto::MiniDigest;
use zk_ee::common_structs::GenericEventContentRef;
use zk_ee::system::MAX_EVENT_TOPICS;
use zk_ee::types_config::EthereumIOTypesConfig;

/// Addresses are encoded as 20 bytes
pub const ADDRESS_ENCODING_LEN: usize = 21;

// methods for the encoding length estimation

///
/// Estimates length of the number rlp encoding.
///
pub fn estimate_number_encoding_len(value: &[u8]) -> usize {
    let first_non_zero_byte = value
        .iter()
        .position(|&byte| byte != 0)
        .unwrap_or(value.len());
    estimate_bytes_encoding_len(&value[first_non_zero_byte..])
}

///
/// Estimates extra length of encoding length of some payload
///
pub const fn estimate_encoding_len_for_payload_length(payload_encoding_len: usize) -> usize {
    if payload_encoding_len <= 55 {
        1
    } else {
        1 + core::mem::size_of::<usize>() - (payload_encoding_len.leading_zeros() / 8) as usize
    }
}

///
/// Estimates length of the bytes rlp encoding.
///
pub fn estimate_bytes_encoding_len(value: &[u8]) -> usize {
    if value.len() == 1 && value[0] < 128 {
        return 1;
    }

    estimate_length_encoding_len(value.len()) + value.len()
}

///
/// Estimates length of the bytes(or list) length rlp encoding.
///
/// **Note that it shouldn't be used for a single byte less than 128.**
///
pub fn estimate_length_encoding_len(length: usize) -> usize {
    if length < 56 {
        1
    } else {
        let length_bytes = length.to_be_bytes();
        let non_zero_byte = length_bytes.iter().position(|&byte| byte != 0).unwrap();
        1 + length_bytes.len() - non_zero_byte
    }
}

// methods to apply the encoding to the hasher

///
/// Applies the number rlp encoding to the hasher.
///
pub fn apply_number_encoding_to_hash(value: &[u8], hasher: &mut impl MiniDigest) {
    // if the value is 0, then it should be encoded as empty bytes
    let first_non_zero_byte = value
        .iter()
        .position(|&byte| byte != 0)
        .unwrap_or(value.len());
    apply_bytes_encoding_to_hash(&value[first_non_zero_byte..], hasher);
}

///
/// Applies the bytes rlp encoding to the hasher.
///
pub fn apply_bytes_encoding_to_hash(value: &[u8], hasher: &mut impl MiniDigest) {
    if value.len() == 1 && value[0] < 128 {
        hasher.update(value);
        return;
    }

    apply_length_encoding_to_hash(value.len(), 128, hasher);
    hasher.update(value);
}

///
/// Applies the list rlp encoding to the hasher.
///
pub fn apply_list_length_encoding_to_hash(length: usize, hasher: &mut impl MiniDigest) {
    apply_length_encoding_to_hash(length, 192, hasher);
}

///
/// Applies the length rlp encoding to the hasher.
/// offset = 128 should be used for bytes, 192 - for list.
///
/// Note that it shouldn't be used for a single byte less than 128.
///
fn apply_length_encoding_to_hash(length: usize, offset: u8, hasher: &mut impl MiniDigest) {
    if length < 56 {
        hasher.update(&[offset + length as u8])
    } else {
        let length_bytes = length.to_be_bytes();
        let non_zero_byte = length_bytes.iter().position(|&byte| byte != 0).unwrap();
        hasher.update(&[offset + 55 + (length_bytes.len() - non_zero_byte) as u8]);
        hasher.update(&length_bytes[non_zero_byte..]);
    }
}

pub trait RLPEncodable {
    fn required_buffer_len(&self) -> usize;
    fn encode_into<B: ?Sized + ByteBuffer>(&self, buffer: &mut B);
}

impl<T: RLPEncodable> RLPEncodable for &T {
    fn required_buffer_len(&self) -> usize {
        (*self).required_buffer_len()
    }

    fn encode_into<B: ?Sized + ByteBuffer>(&self, buffer: &mut B) {
        (*self).encode_into(buffer);
    }
}

pub trait CachingRLPEncodable {
    fn required_buffer_len(&mut self) -> usize;
    fn encode_into<B: ?Sized + ByteBuffer>(&mut self, buffer: &mut B);
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
        self.cached_len + estimate_encoding_len_for_payload_length(self.cached_len)
    }

    fn encode_into<B: ?Sized + ByteBuffer>(&mut self, buffer: &mut B) {
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
        self.cached_len + estimate_encoding_len_for_payload_length(self.cached_len)
    }

    fn encode_into<B: ?Sized + ByteBuffer>(&mut self, buffer: &mut B) {
        let _ = self.required_buffer_len();
        apply_list_length_encoding(self.cached_len, buffer);
        for el in self.elements_it.clone() {
            el.encode_into(buffer);
        }
    }
}

pub fn apply_u64_encoding<B: ?Sized + ByteBuffer>(value: u64, buffer: &mut B) {
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

pub fn apply_length_encoding<const OFFSET: u8, B: ?Sized + ByteBuffer>(
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

pub fn apply_slice_length_encoding<B: ?Sized + ByteBuffer>(length: usize, buffer: &mut B) {
    apply_length_encoding::<0x80, B>(length, buffer)
}

pub fn apply_list_length_encoding<B: ?Sized + ByteBuffer>(length: usize, buffer: &mut B) {
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
        estimate_number_encoding_len(&self.to_be_bytes())
    }

    fn encode_into<B: ?Sized + ByteBuffer>(&self, buffer: &mut B) {
        apply_u64_encoding(*self, buffer);
    }
}

impl RLPEncodable for [u8; 256] {
    fn required_buffer_len(&self) -> usize {
        3 + self.len()
    }

    fn encode_into<B: ?Sized + ByteBuffer>(&self, buffer: &mut B) {
        buffer.write_slice(&[0xb7 + 2, 0x01, 0x00]);
        buffer.write_slice(self);
    }
}

impl<'a> RLPEncodable for GenericEventContentRef<'a, MAX_EVENT_TOPICS, EthereumIOTypesConfig> {
    fn required_buffer_len(&self) -> usize {
        let payload_len = event_encoding_len_no_outer_list(self);
        payload_len + estimate_encoding_len_for_payload_length(payload_len)
    }

    fn encode_into<B: ?Sized + ByteBuffer>(&self, buffer: &mut B) {
        let payload_len = event_encoding_len_no_outer_list(self);
        apply_list_length_encoding(payload_len, buffer);
        buffer.write_byte(0x80 + 20);
        buffer.write_slice(&self.address.to_be_bytes::<20>());

        let topics_total_len = self.topics.len() * (1 + 32);
        if self.topics.is_empty() {
            buffer.write_byte(0xc0);
        } else if self.topics.len() == 1 {
            buffer.write_byte(0xc0 + 33);
        } else {
            buffer.write_slice(&[0xf7 + 1, topics_total_len as u8]);
        }
        for topic in self.topics.iter() {
            buffer.write_byte(0x80 + 32);
            buffer.write_slice(topic.as_u8_ref());
        }

        apply_slice_length_encoding(self.data.len(), buffer);
        buffer.write_slice(self.data);
    }
}

fn event_encoding_len_no_outer_list(
    el: &GenericEventContentRef<'_, MAX_EVENT_TOPICS, EthereumIOTypesConfig>,
) -> usize {
    let mut total_len = 0;
    total_len += ADDRESS_ENCODING_LEN;

    let topics_concat_len = (1 + 32) * el.topics.len();
    let topics_list_header_len = if topics_concat_len <= 55 {
        1
    } else if topics_concat_len < 256 {
        2
    } else {
        unreachable!()
    };
    total_len += topics_concat_len + topics_list_header_len;
    total_len += el.data.len() + estimate_encoding_len_for_payload_length(el.data.len());

    total_len
}

pub struct ReceiptEncoder<
    'fields,
    'events,
    I: Iterator<Item = GenericEventContentRef<'events, MAX_EVENT_TOPICS, EthereumIOTypesConfig>>
        + Clone,
> {
    tx_type: u8,
    inner: ListEnvelope<
        CachingEnvelope<&'fields bool>,
        ListElement<
            CachingEnvelope<&'fields u64>,
            ListElement<
                CachingEnvelope<&'fields [u8; 256]>,
                HomogeneousListEnvelope<
                    GenericEventContentRef<'events, MAX_EVENT_TOPICS, EthereumIOTypesConfig>,
                    I,
                >,
            >,
        >,
    >,
}

impl<
        'fields,
        'events,
        I: Iterator<
                Item = GenericEventContentRef<'events, MAX_EVENT_TOPICS, EthereumIOTypesConfig>,
            > + Clone,
    > ReceiptEncoder<'fields, 'events, I>
{
    pub fn new_from_fields(
        tx_type: u8,
        status: &'fields bool,
        cumulative_gas_used: &'fields u64,
        bloom: &'fields [u8; 256],
        events_it: I,
    ) -> Self {
        Self {
            tx_type,
            inner: ListEnvelope::from_head(ListElement::chained(
                CachingEnvelope::new(status),
                ListElement::chained(
                    CachingEnvelope::new(cumulative_gas_used),
                    ListElement::chained(
                        CachingEnvelope::new(bloom),
                        HomogeneousListEnvelope::new(events_it),
                    ),
                ),
            )),
        }
    }
}

impl<
        'fields,
        'events,
        I: Iterator<
                Item = GenericEventContentRef<'events, MAX_EVENT_TOPICS, EthereumIOTypesConfig>,
            > + Clone,
    > CachingRLPEncodable for ReceiptEncoder<'fields, 'events, I>
{
    fn required_buffer_len(&mut self) -> usize {
        self.inner.required_buffer_len() + (self.tx_type != 0) as usize
    }

    fn encode_into<B: ?Sized + ByteBuffer>(&mut self, buffer: &mut B) {
        if self.tx_type != 0 {
            buffer.write_byte(self.tx_type);
        }
        self.inner.encode_into(buffer);
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
        let len = self.required_buffer_len();
        assert!(len > 1);
        (len, 0xff)
    }
}
