//! The transaction receipt RLP encoder, plus the encoding of its log entries.
//!
//! Encodes a receipt as `[status, cumulative_gas_used, logs_bloom, [logs...]]`,
//! prefixed by the transaction type byte for typed (non-legacy) transactions.
//! Each log is `[address, [topics...], data]`. Built on the `encodable`
//! framework so it streams into any `WriteBytes` sink (a hasher for the ZK
//! receipts root, an MPT leaf buffer in Ethereum mode).

use super::encodable::{
    CachingEnvelope, CachingRLPEncodable, HomogeneousListEnvelope, ListElement, ListEnvelope,
    RLPEncodable,
};
use core::slice::Iter;
use zk_ee::common_structs::GenericEventContentRef;
use zk_ee::system::MAX_EVENT_TOPICS;
use zk_ee::types_config::EthereumIOTypesConfig;
use zk_ee::utils::{write_bytes::WriteBytes, Bytes32};

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

    fn encode_into<B: ?Sized + WriteBytes>(&mut self, buffer: &mut B) {
        if self.tx_type != 0 {
            buffer.write_byte(self.tx_type);
        }
        self.inner.encode_into(buffer);
    }
}

/// Encoder for a single log entry, as the RLP list `[address, [topics...], data]`.
///
/// Like `ReceiptEncoder`, it is composed from the `encodable` envelopes so the
/// encoding (and its length) is derived from the primitive `RLPEncodable` impls
/// rather than written by hand: the address is a 20-byte string, the topics are
/// a nested list of 32-byte strings, and the data is a byte string. As a
/// `ListEnvelope` it is itself `CachingRLPEncodable`.
pub type EventEncoder<'a> = ListEnvelope<
    CachingEnvelope<[u8; 20]>,
    ListElement<HomogeneousListEnvelope<&'a Bytes32, Iter<'a, Bytes32>>, CachingEnvelope<&'a [u8]>>,
>;

impl<'a> EventEncoder<'a> {
    pub fn new(
        event: &GenericEventContentRef<'a, MAX_EVENT_TOPICS, EthereumIOTypesConfig>,
    ) -> Self {
        ListEnvelope::from_head(ListElement::chained(
            CachingEnvelope::new(event.address.to_be_bytes::<20>()),
            ListElement::chained(
                HomogeneousListEnvelope::new(event.topics.iter()),
                CachingEnvelope::new(event.data),
            ),
        ))
    }
}

impl<'a> RLPEncodable for GenericEventContentRef<'a, MAX_EVENT_TOPICS, EthereumIOTypesConfig> {
    fn required_buffer_len(&self) -> usize {
        EventEncoder::new(self).required_buffer_len()
    }

    fn encode_into<B: ?Sized + WriteBytes>(&self, buffer: &mut B) {
        EventEncoder::new(self).encode_into(buffer);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arrayvec::ArrayVec;
    use ruint::aliases::B160;
    use zk_ee::utils::Bytes32;

    struct VecSink(alloc::vec::Vec<u8>);
    impl WriteBytes for VecSink {
        fn write(&mut self, buf: &[u8]) {
            self.0.extend_from_slice(buf);
        }
    }

    /// Encodes a log `[address(0), [], data]` and checks that the estimated and
    /// actual encoding lengths agree.
    fn encode_log(data: &[u8]) -> alloc::vec::Vec<u8> {
        let address = B160::ZERO;
        let topics: ArrayVec<Bytes32, MAX_EVENT_TOPICS> = ArrayVec::new();
        let event: GenericEventContentRef<'_, MAX_EVENT_TOPICS, EthereumIOTypesConfig> =
            GenericEventContentRef {
                address: &address,
                topics: &topics,
                data,
            };

        let mut sink = VecSink(alloc::vec::Vec::new());
        event.encode_into(&mut sink);
        // The pre-computed length must match what was actually written.
        assert_eq!(event.required_buffer_len(), sink.0.len());
        sink.0
    }

    fn log_prefix() -> alloc::vec::Vec<u8> {
        // address: 20-byte string (0x94 + 20 zero bytes), then empty topics list (0xc0).
        let mut v = alloc::vec::Vec::new();
        v.push(0x94);
        v.extend_from_slice(&[0u8; 20]);
        v.push(0xc0);
        v
    }

    #[test]
    fn log_data_single_low_byte_has_no_prefix() {
        // A single byte < 0x80 is canonical RLP for itself, with no length prefix.
        let mut expected = alloc::vec::Vec::new();
        let body = {
            let mut b = log_prefix();
            b.push(0x05);
            b
        };
        expected.push(0xc0 + body.len() as u8); // 21 + 1 + 1 = 23 payload bytes
        expected.extend_from_slice(&body);

        assert_eq!(encode_log(&[0x05]), expected);
    }

    #[test]
    fn log_data_single_high_byte_is_length_prefixed() {
        // A single byte >= 0x80 keeps the 1-byte string prefix (0x81).
        let mut expected = alloc::vec::Vec::new();
        let body = {
            let mut b = log_prefix();
            b.push(0x81);
            b.push(0x80);
            b
        };
        expected.push(0xc0 + body.len() as u8);
        expected.extend_from_slice(&body);

        assert_eq!(encode_log(&[0x80]), expected);
    }

    #[test]
    fn log_data_empty_is_empty_string() {
        // Empty data encodes as the empty string 0x80.
        let mut expected = alloc::vec::Vec::new();
        let body = {
            let mut b = log_prefix();
            b.push(0x80);
            b
        };
        expected.push(0xc0 + body.len() as u8);
        expected.extend_from_slice(&body);

        assert_eq!(encode_log(&[]), expected);
    }
}
