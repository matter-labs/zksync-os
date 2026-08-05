// NOTE: it's a structure that will hold lazy value, that can encode itself into interner's buffer
// or hash itself. The encoding trait is dyn-compatible

use crate::system_implementation::ethereum_storage_model::mpt::*;
use crate::system_implementation::ethereum_storage_model::{ByteBuffer, RLPSlice};

/// Trait to encode value into byte buffer. Encoding format doesn't matter,
/// but length of the buffer must be suggested in advance
pub trait LazyEncodable: core::fmt::Debug {
    fn encoding_len_and_first_byte(&self) -> (usize, u8);
    fn encode(&self, into: &mut dyn ByteBuffer);
}

#[derive(Debug)]
pub struct LazyLeafValue<'a> {
    encoder: &'a dyn LazyEncodable,
}

impl<'a> LazyLeafValue<'a> {
    pub fn from_value<T: LazyEncodable + 'a>(value: &'a T) -> Self {
        Self {
            encoder: value as &'a dyn LazyEncodable,
        }
    }
}

#[derive(Debug)]
pub enum LeafValue<'a> {
    Slice {
        value_without_rlp_envelope: &'a [u8],
        cached_encoding_len: usize,
    },
    RLPEnveloped {
        envelope: RLPSlice<'a>,
    },
    LazyEncodable {
        value: LazyLeafValue<'a>,
        cached_encoding_len_with_metadata: u32,
    },
    Used, // Tombstone value to indicate that lazy encodable value was used, so we can drop lifetime
}

impl<'a> LeafValue<'a> {
    pub fn take_value(&mut self) -> Self {
        if let Self::Used = self {
            panic!("value was used already");
        }
        match self {
            Self::Slice {
                value_without_rlp_envelope,
                cached_encoding_len,
            } => Self::Slice {
                value_without_rlp_envelope: *value_without_rlp_envelope,
                cached_encoding_len: *cached_encoding_len,
            },
            Self::RLPEnveloped { envelope } => Self::RLPEnveloped {
                envelope: *envelope,
            },
            a @ Self::LazyEncodable { .. } => core::mem::replace(a, Self::Used),
            Self::Used => {
                panic!("value was used already");
            }
        }
    }

    pub fn from_lazy_encodable<T: LazyEncodable + 'a>(value: &'a T) -> Self {
        Self::LazyEncodable {
            value: LazyLeafValue::from_value(value),
            cached_encoding_len_with_metadata: 0u32,
        }
    }

    pub fn from_pre_encoded(value: &'a [u8]) -> Result<Self, ()> {
        let envelope = RLPSlice::from_slice(value)?;
        Ok(Self::RLPEnveloped { envelope })
    }

    pub fn from_pre_encoded_with_interner(
        value: &[u8],
        interner: &mut (impl Interner<'a> + 'a),
    ) -> Result<Self, ()> {
        let value = interner.intern_slice(value)?;
        Self::from_pre_encoded(value)
    }

    pub fn from_raw_slice(value: &'a [u8]) -> Result<Self, ()> {
        Ok(Self::Slice {
            value_without_rlp_envelope: value,
            cached_encoding_len: 0,
        })
    }

    pub fn from_raw_slice_with_interner(
        value: &[u8],
        interner: &mut (impl Interner<'a> + 'a),
    ) -> Result<Self, ()> {
        let value = interner.intern_slice(value)?;
        Self::from_raw_slice(value)
    }

    const METADATA_SPECIAL_SINGLE_BYTE_CASE_MASK: u32 = 0x80_00_00_00;

    pub(crate) fn data(&self) -> &'a [u8] {
        match self {
            Self::Slice {
                value_without_rlp_envelope,
                ..
            } => *value_without_rlp_envelope,
            Self::RLPEnveloped { envelope } => envelope.data(),
            Self::LazyEncodable { .. } => {
                unreachable!("not used in descending or reading");
            }
            Self::Used => {
                panic!("value was used already");
            }
        }
    }

    // This applies encoding of slice(internal value),
    // unless internal value RLPEnveloped - then just internal value is used
    pub(crate) fn rlp_encoding_length(&mut self) -> usize {
        match self {
            Self::Slice {
                value_without_rlp_envelope,
                cached_encoding_len,
            } => {
                *cached_encoding_len = slice_encoding_len(*value_without_rlp_envelope);

                *cached_encoding_len
            }
            Self::RLPEnveloped { envelope } => envelope.full_encoding().len(),
            Self::LazyEncodable {
                value,
                cached_encoding_len_with_metadata,
            } => {
                let (len, first_byte) = value.encoder.encoding_len_and_first_byte();
                if len == 1 && first_byte < 0x80 {
                    *cached_encoding_len_with_metadata =
                        Self::METADATA_SPECIAL_SINGLE_BYTE_CASE_MASK | 1;

                    1
                } else {
                    *cached_encoding_len_with_metadata = len as u32;
                    if len <= 55 {
                        1 + len
                    } else if len < 1 << 8 {
                        2 + len
                    } else if len < 1 << 16 {
                        3 + len
                    } else if len < 1 << 24 {
                        4 + len
                    } else {
                        unreachable!()
                    }
                }
            }
            Self::Used => {
                panic!("value was used already");
            }
        }
    }

    pub(crate) fn rlp_encode_into(&self, buffer: &mut impl ByteBuffer) {
        match self {
            Self::Slice {
                value_without_rlp_envelope,
                ..
            } => {
                encode_slice_into_buffer(value_without_rlp_envelope, buffer);
            }
            Self::RLPEnveloped { envelope } => {
                buffer.write_slice(envelope.full_encoding());
            }
            Self::LazyEncodable {
                value,
                cached_encoding_len_with_metadata,
            } => {
                let is_special_case = cached_encoding_len_with_metadata
                    & Self::METADATA_SPECIAL_SINGLE_BYTE_CASE_MASK
                    > 0;
                let len = cached_encoding_len_with_metadata
                    & !Self::METADATA_SPECIAL_SINGLE_BYTE_CASE_MASK;
                let len = len as usize;
                if is_special_case {
                    assert_eq!(len, 1);
                    value.encoder.encode(buffer);
                } else {
                    if len <= 55 {
                        buffer.write_byte(0x80 + (len as u8));
                    } else if len < 1 << 8 {
                        buffer.write_slice(&[0xb7 + 1, len as u8]);
                    } else if len < 1 << 16 {
                        buffer.write_slice(&[0xb7 + 2, (len >> 8) as u8, len as u8]);
                    } else if len < 1 << 24 {
                        buffer.write_slice(&[
                            0xb7 + 3,
                            (len >> 16) as u8,
                            (len >> 8) as u8,
                            len as u8,
                        ]);
                    } else {
                        unreachable!()
                    }
                    value.encoder.encode(buffer);
                }
            }
            Self::Used => {
                panic!("value was used already");
            }
        }
    }
}
