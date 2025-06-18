use crate::utils::*;
use ruint::aliases::B160;

// we can empty 2 compression strategies
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompressionStrategy {
    Add,
    Sub,
}

pub const NUM_EXTRA_BYTES_IN_ENCODING_SCHEME: u8 = 1;
pub const NUM_BYTE_FOR_REPEATED_KEY_ENCODING: u8 = 4;
pub const NUM_BYTE_FOR_NEW_ENCODING: u8 = 32;

impl CompressionStrategy {
    pub fn output_num_bytes(&self, initial_value: &Bytes32, final_value: &Bytes32) -> u8 {
        match self {
            Self::Add => {
                let mut result = initial_value.into_u256_be();
                let of = result.overflowing_add_assign(&final_value.into_u256_be());
                if of {
                    32
                } else {
                    (result.bit_len().next_multiple_of(8) / 8) as u8
                }
            }
            Self::Sub => {
                let mut result = initial_value.into_u256_be();
                let of = result.overflowing_sub_assign(&final_value.into_u256_be());
                if of {
                    32
                } else {
                    (result.bit_len().next_multiple_of(8) / 8) as u8
                }
            }
        }
    }

    pub fn apply_best_strategy(initial_value: &Bytes32, final_value: &Bytes32) -> u8 {
        let mut optimal = 32;
        for strategy in [Self::Add, Self::Sub].iter() {
            optimal = core::cmp::min(
                optimal,
                strategy.output_num_bytes(initial_value, final_value),
            );
        }

        optimal + NUM_EXTRA_BYTES_IN_ENCODING_SCHEME
    }
}

#[derive(Clone, Copy, Debug)]
pub struct PubdataDiffLog {
    pub address: B160,
    pub storage_key: Bytes32,
    pub initial_value: Bytes32,
    pub final_value: Bytes32,
    pub is_new_storage_slot: bool,
}

impl PubdataDiffLog {
    pub const BYTE_LEN: usize = const {
        let size: usize = 20 + 32 * 3 + 4 + 1 + 1;

        size.next_multiple_of(USIZE_SIZE)
    };
    pub fn as_byte_array(&self) -> [u8; Self::BYTE_LEN] {
        let mut dst = [0; Self::BYTE_LEN];

        let mut idx = 0;
        dst[idx..(idx + 20)].copy_from_slice(&self.address.to_le_bytes::<{ B160::BYTES }>());
        idx += 20;
        dst[idx..(idx + 32)].copy_from_slice(self.storage_key.as_u8_array_ref());
        idx += 32;
        dst[idx..(idx + 32)].copy_from_slice(self.initial_value.as_u8_array_ref());
        idx += 32;
        dst[idx..(idx + 32)].copy_from_slice(self.final_value.as_u8_array_ref());
        idx += 32;
        dst[idx] = self.is_new_storage_slot as u8;

        dst
    }
}
