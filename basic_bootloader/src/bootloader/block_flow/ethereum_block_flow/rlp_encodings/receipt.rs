use super::*;
use crate::bootloader::block_flow::ethereum_block_flow::rlp_encodings::RLPEncodable;
use crate::bootloader::rlp;
use crate::bootloader::transaction_flow::ethereum::LogsBloom;
use basic_system::system_implementation::ethereum_storage_model::ByteBuffer;
use zk_ee::kv_markers::MAX_EVENT_TOPICS;
use zk_ee::{common_structs::GenericEventContentRef, types_config::EthereumIOTypesConfig};

impl<'a> RLPEncodable for GenericEventContentRef<'a, MAX_EVENT_TOPICS, EthereumIOTypesConfig> {
    fn required_buffer_len(&self) -> usize {
        let payload_len = event_encoding_len_no_outer_list(self);
        // it's a list
        payload_len + rlp::estimate_encoding_len_for_payload_length(payload_len)
    }

    fn encode_into<B: ?Sized + ByteBuffer>(&self, buffer: &mut B) {
        let payload_len = event_encoding_len_no_outer_list(self);
        apply_list_length_encoding(payload_len, buffer);
        // Address
        buffer.write_byte(0x80 + 20);
        buffer.write_slice(&self.address.to_be_bytes::<20>());
        // List of topics
        let topics_total_len = self.topics.len() * (1 + 32); // max 132 bytes
        if self.topics.len() == 0 {
            // empty list
            buffer.write_byte(0xc0);
        } else if self.topics.len() == 1 {
            buffer.write_byte(0xc0 + 33);
        } else {
            buffer.write_slice(&[0xf7 + 1, topics_total_len as u8]); // max 132 bytes
        }
        // topics themselves
        for topic in self.topics.iter() {
            buffer.write_byte(0x80 + 32);
            buffer.write_slice(topic.as_u8_ref());
        }
        // Data
        apply_slice_length_encoding(self.data.len(), buffer);
        buffer.write_slice(self.data);
    }
}

fn event_encoding_len_no_outer_list<'a>(
    el: &GenericEventContentRef<'a, MAX_EVENT_TOPICS, EthereumIOTypesConfig>,
) -> usize {
    let mut total_len = 0;
    // address is fixed
    total_len += rlp::ADDRESS_ENCODING_LEN;
    // topics are a list
    let topics_concat_len = (1 + 32) * el.topics.len();
    let topics_list_header_len = if topics_concat_len <= 55 {
        1
    } else if topics_concat_len < 256 {
        2
    } else {
        unreachable!()
    };
    total_len += topics_concat_len + topics_list_header_len;
    // then data is slice
    total_len += el.data.len() + rlp::estimate_encoding_len_for_payload_length(el.data.len());

    total_len
}

pub(crate) struct ReceiptEncoder<
    'a,
    I: Iterator<Item = GenericEventContentRef<'a, MAX_EVENT_TOPICS, EthereumIOTypesConfig>> + Clone,
> {
    tx_type: u8,
    // type constructor
    inner: ListEnvelope<
        CachingEnvelope<&'a bool>,
        ListElement<
            CachingEnvelope<&'a u64>,
            ListElement<
                CachingEnvelope<&'a LogsBloom>,
                HomogeneousListEnvelope<
                    GenericEventContentRef<'a, MAX_EVENT_TOPICS, EthereumIOTypesConfig>,
                    I,
                >,
            >,
        >,
    >,
}

impl<
        'a,
        I: Iterator<Item = GenericEventContentRef<'a, MAX_EVENT_TOPICS, EthereumIOTypesConfig>>
            + Clone,
    > ReceiptEncoder<'a, I>
{
    pub(crate) fn new_from_fields(
        tx_type: u8,
        status: &'a bool,
        cumulative_gas_used: &'a u64,
        bloom: &'a LogsBloom,
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
        'a,
        I: Iterator<Item = GenericEventContentRef<'a, MAX_EVENT_TOPICS, EthereumIOTypesConfig>>
            + Clone,
    > CachingRLPEncodable for ReceiptEncoder<'a, I>
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
