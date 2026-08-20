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
    use alloc::vec::Vec;
    use alloy::consensus::{Eip658Value, Receipt, ReceiptWithBloom};
    use alloy_primitives::{Address, Bloom, Bytes, Log, B256};
    use alloy_rlp::Encodable as _;
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

    // --- alloy cross-checks: our encoders must match a reference RLP library ---

    /// Encodes one log with our `EventEncoder` and with alloy's `Log`, returning
    /// both byte strings. Also asserts our length estimate matches what we wrote.
    fn our_and_alloy_log(addr: [u8; 20], topics: &[[u8; 32]], data: &[u8]) -> (Vec<u8>, Vec<u8>) {
        let address = B160::from_be_bytes::<20>(addr);
        let mut tv: ArrayVec<Bytes32, MAX_EVENT_TOPICS> = ArrayVec::new();
        for t in topics {
            tv.push(Bytes32::from_array(*t));
        }
        let event: GenericEventContentRef<'_, MAX_EVENT_TOPICS, EthereumIOTypesConfig> =
            GenericEventContentRef {
                address: &address,
                topics: &tv,
                data,
            };
        let mut sink = VecSink(Vec::new());
        event.encode_into(&mut sink);
        assert_eq!(event.required_buffer_len(), sink.0.len());

        let alloy_log = Log::new_unchecked(
            Address::from(addr),
            topics.iter().map(|t| B256::from(*t)).collect(),
            Bytes::copy_from_slice(data),
        );
        let mut alloy_enc = Vec::new();
        alloy_log.encode(&mut alloy_enc);

        (sink.0, alloy_enc)
    }

    #[test]
    fn event_encoding_matches_alloy() {
        let addr = [0x11u8; 20];
        let t = [[0x22u8; 32], [0x33u8; 32], [0x44u8; 32], [0x55u8; 32]];
        // Exercises the topics-list length boundary (0 -> 0xc0, 1 -> 0xe1,
        // 2..=4 -> long list) and the data single-byte rule.
        let cases: &[(&[[u8; 32]], &[u8])] = &[
            (&[], &[]),
            (&[], &[0x05]),
            (&t[..1], &[0xde, 0xad]),
            (&t[..2], &[0x01, 0x02, 0x03, 0x04, 0x05]),
            (&t[..3], &[]),
            (&t[..4], &[0u8; 100]),
        ];
        for (topics, data) in cases {
            let (ours, alloy) = our_and_alloy_log(addr, topics, data);
            assert_eq!(
                ours,
                alloy,
                "topics={} data_len={}",
                topics.len(),
                data.len()
            );
        }
    }

    /// Reference encoding of a full receipt via alloy: `type? || rlp([status,
    /// cumulative_gas_used, logs_bloom, [logs...]])`.
    fn alloy_receipt_rlp(
        tx_type: u8,
        status: bool,
        gas: u64,
        bloom: &[u8; 256],
        logs: &[([u8; 20], Vec<[u8; 32]>, Vec<u8>)],
    ) -> Vec<u8> {
        let alloy_logs: Vec<Log> = logs
            .iter()
            .map(|(a, ts, d)| {
                Log::new_unchecked(
                    Address::from(*a),
                    ts.iter().map(|t| B256::from(*t)).collect(),
                    Bytes::copy_from_slice(d),
                )
            })
            .collect();
        let receipt = Receipt {
            status: Eip658Value::Eip658(status),
            cumulative_gas_used: gas,
            logs: alloy_logs,
        };
        let rwb = ReceiptWithBloom {
            receipt,
            logs_bloom: Bloom::from_slice(bloom),
        };
        let mut out = Vec::new();
        if tx_type != 0 {
            out.push(tx_type);
        }
        rwb.encode(&mut out);
        out
    }

    /// Encodes a receipt over the given logs with our `ReceiptEncoder`, returning
    /// `(our_bytes, our_length_estimate, alloy_bytes)` for comparison.
    fn encode_receipt_ours_and_alloy(
        tx_type: u8,
        status: bool,
        gas: u64,
        bloom: &[u8; 256],
        logs: &[([u8; 20], Vec<[u8; 32]>, Vec<u8>)],
    ) -> (Vec<u8>, usize, Vec<u8>) {
        let addrs: Vec<B160> = logs
            .iter()
            .map(|(a, _, _)| B160::from_be_bytes::<20>(*a))
            .collect();
        let topic_vecs: Vec<ArrayVec<Bytes32, MAX_EVENT_TOPICS>> = logs
            .iter()
            .map(|(_, ts, _)| {
                let mut v = ArrayVec::new();
                for t in ts {
                    v.push(Bytes32::from_array(*t));
                }
                v
            })
            .collect();
        let events: Vec<GenericEventContentRef<'_, MAX_EVENT_TOPICS, EthereumIOTypesConfig>> = (0
            ..logs.len())
            .map(|i| GenericEventContentRef {
                address: &addrs[i],
                topics: &topic_vecs[i],
                data: logs[i].2.as_slice(),
            })
            .collect();

        let mut enc =
            ReceiptEncoder::new_from_fields(tx_type, &status, &gas, bloom, events.iter().cloned());
        let mut sink = VecSink(Vec::new());
        enc.encode_into(&mut sink);
        let estimate = enc.required_buffer_len();

        let charged_receipt_hash_native =
            crate::bootloader::constants::RECEIPT_HASH_BASE_NATIVE_COST
                + logs
                    .iter()
                    .map(|(_, topics, data)| {
                        let encoded_log_len_upper_bound = basic_system::system_implementation::flat_storage_model::cost_constants::RECEIPT_LOG_RLP_OVERHEAD_BYTES
                            + 33 * topics.len() as u64
                            + data.len() as u64;
                        encoded_log_len_upper_bound
                            .div_ceil(basic_system::cost_constants::BLAKE2S_CHUNK_SIZE as u64)
                            * basic_system::cost_constants::BLAKE2S_ROUND_NATIVE_COST
                    })
                    .sum::<u64>();
        let actual_receipt_hash_native =
            basic_system::cost_constants::blake2s_native_cost(sink.0.len());
        assert!(
            charged_receipt_hash_native >= actual_receipt_hash_native,
            "decomposed receipt-hash charge must cover the actual encoding"
        );

        let expected = alloy_receipt_rlp(tx_type, status, gas, bloom, logs);
        (sink.0, estimate, expected)
    }

    #[test]
    fn receipt_encoding_matches_alloy() {
        // Two logs: one with topics + multi-byte data, one with no topics and a
        // single low byte of data (exercises the data single-byte rule).
        let logs: alloc::vec::Vec<([u8; 20], Vec<[u8; 32]>, Vec<u8>)> = alloc::vec![
            (
                [0xaau8; 20],
                alloc::vec![[0x01u8; 32], [0x02u8; 32]],
                alloc::vec![0xde, 0xad, 0xbe, 0xef]
            ),
            ([0xbbu8; 20], alloc::vec![], alloc::vec![0x05]),
        ];
        let bloom = [0x07u8; 256];

        // The encoder's contract is purely "prefix the raw type byte for any
        // nonzero `tx_type`" — it is *not* Ethereum-receipt-specific. Cover
        // legacy (type 0, no prefix), the Ethereum typed receipts (1, 2), and the
        // ZKsync-specific type bytes the ZK path can feed (0x7c-0x7f), alongside
        // both status values and a multi-byte cumulative gas.
        for &tx_type in &[0u8, 1, 2, 0x7c, 0x7d, 0x7e, 0x7f] {
            for &status in &[true, false] {
                for &gas in &[0u64, 21_000, 0xffff_ffff] {
                    let (ours, estimate, expected) =
                        encode_receipt_ours_and_alloy(tx_type, status, gas, &bloom, &logs);
                    assert_eq!(estimate, ours.len(), "len tx_type={tx_type}");
                    assert_eq!(
                        ours, expected,
                        "tx_type={tx_type} status={status} gas={gas}"
                    );
                }
            }
        }
    }

    /// Strategy for one random log: 20-byte address, 0..=MAX_EVENT_TOPICS topics,
    /// and an arbitrary data payload.
    fn arb_log() -> impl proptest::strategy::Strategy<Value = ([u8; 20], Vec<[u8; 32]>, Vec<u8>)> {
        use proptest::prelude::*;
        (
            proptest::array::uniform20(any::<u8>()),
            proptest::collection::vec(
                proptest::array::uniform32(any::<u8>()),
                0..=MAX_EVENT_TOPICS,
            ),
            proptest::collection::vec(any::<u8>(), 0..=48),
        )
    }

    proptest::proptest! {
        /// Property test: for arbitrary type byte, status, cumulative gas, bloom
        /// and a list of arbitrary logs, our `ReceiptEncoder` must byte-for-byte
        /// match the canonical alloy RLP encoding, and its length estimate must
        /// equal the bytes actually written.
        #[test]
        fn receipt_encoding_matches_alloy_prop(
            tx_type in proptest::prelude::any::<u8>(),
            status in proptest::prelude::any::<bool>(),
            gas in proptest::prelude::any::<u64>(),
            bloom_bytes in proptest::collection::vec(proptest::prelude::any::<u8>(), 256),
            logs in proptest::collection::vec(arb_log(), 0..=6),
        ) {
            let mut bloom = [0u8; 256];
            bloom.copy_from_slice(&bloom_bytes);

            let (ours, estimate, expected) =
                encode_receipt_ours_and_alloy(tx_type, status, gas, &bloom, &logs);
            proptest::prop_assert_eq!(estimate, ours.len(), "length estimate must match bytes written");
            proptest::prop_assert_eq!(ours, expected);
        }
    }
}
