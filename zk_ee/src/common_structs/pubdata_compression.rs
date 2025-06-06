//!
//! This module contains utils for pubdata compression that can be reused by different systems/storage models.
//!
use crate::system::IOResultKeeper;
use crate::types_config::SystemIOTypesConfig;
use crate::utils::*;
use crypto::MiniDigest;
use ruint::aliases::U256;

///
/// value diff "Era VM" compression, can be used for contracts storage values and account data fields(nonce and balance).
/// Works for 32 bytes values, numbers encoded/decoded as BE.
///
/// There are 4 compression types:
/// - `Nothing`, final 32 byte value.
/// - `Add`, value increased by specified 0-31 byte value.
/// - `Subtract`, value decreased by specified 0-31 byte value.
/// - `Transform`, final 0-31 byte value, leading zeroes removed.
///
#[derive(PartialEq, Eq)]
pub enum ValueDiffCompressionStrategy {
    Nothing,
    Add,
    Sub,
    Transform,
}

impl ValueDiffCompressionStrategy {
    fn compression_length(&self, initial_value: U256, final_value: U256) -> Option<u8> {
        match self {
            Self::Nothing => Some(33), //full value + metadata byte
            Self::Add => {
                let (result, of) = final_value.overflowing_sub(initial_value);
                let length = (result.bit_len().next_multiple_of(8) / 8) as u8;
                if of || length == 32 {
                    None
                } else {
                    Some(length + 1)
                }
            }
            Self::Sub => {
                let (result, of) = initial_value.overflowing_sub(final_value);
                let length = (result.bit_len().next_multiple_of(8) / 8) as u8;
                if of || length == 32 {
                    None
                } else {
                    Some(length + 1)
                }
            }
            Self::Transform => {
                let length = (final_value.bit_len().next_multiple_of(8) / 8) as u8;
                if length == 32 {
                    None
                } else {
                    Some(length + 1)
                }
            }
        }
    }

    fn compress<IOTypes: SystemIOTypesConfig>(
        &self,
        initial_value: U256,
        final_value: U256,
        hasher: &mut impl MiniDigest,
        result_keeper: &mut impl IOResultKeeper<IOTypes>,
    ) -> Result<(), ()> {
        match self {
            Self::Nothing => {
                let metadata_byte = 0u8;
                hasher.update([metadata_byte]);
                hasher.update(final_value.to_be_bytes::<32>());
                result_keeper.pubdata(&[metadata_byte]);
                result_keeper.pubdata(&final_value.to_be_bytes::<32>());

                Ok(())
            }
            Self::Add => {
                let (result, of) = final_value.overflowing_sub(initial_value);
                let length = (result.bit_len().next_multiple_of(8) / 8) as u8;

                if of || length == 32 {
                    Err(())
                } else {
                    let metadata_byte = (length << 3) | 1;
                    hasher.update([metadata_byte]);
                    hasher.update(&result.to_be_bytes::<32>()[32usize - length as usize..]);
                    result_keeper.pubdata(&[metadata_byte]);
                    result_keeper.pubdata(&result.to_be_bytes::<32>()[32usize - length as usize..]);

                    Ok(())
                }
            }
            Self::Sub => {
                let (result, of) = initial_value.overflowing_sub(final_value);
                let length = (result.bit_len().next_multiple_of(8) / 8) as u8;

                if of || length == 32 {
                    Err(())
                } else {
                    let metadata_byte = (length << 3) | 2;
                    hasher.update([metadata_byte]);
                    hasher.update(&result.to_be_bytes::<32>()[32usize - length as usize..]);
                    result_keeper.pubdata(&[metadata_byte]);
                    result_keeper.pubdata(&result.to_be_bytes::<32>()[32usize - length as usize..]);

                    Ok(())
                }
            }
            Self::Transform => {
                let length = (final_value.bit_len().next_multiple_of(8) / 8) as u8;
                if length == 32 {
                    Err(())
                } else {
                    let metadata_byte = (length << 3) | 3;
                    hasher.update([metadata_byte]);
                    hasher.update(&final_value.to_be_bytes::<32>()[32usize - length as usize..]);
                    result_keeper.pubdata(&[metadata_byte]);
                    result_keeper
                        .pubdata(&final_value.to_be_bytes::<32>()[32usize - length as usize..]);

                    Ok(())
                }
            }
        }
    }

    pub fn optimal_compression_length_u256(initial_value: U256, final_value: U256) -> u8 {
        // worst case "Nothing" strategy, always possible to encode
        let mut optimal = Self::Nothing
            .compression_length(initial_value, final_value)
            .unwrap();

        // so we don't check nothing here
        for strategy in [Self::Add, Self::Sub, Self::Transform].iter() {
            if let Some(length) = strategy.compression_length(initial_value, final_value) {
                optimal = core::cmp::min(optimal, length);
            }
        }

        optimal
    }

    pub fn optimal_compression_length(initial_value: &Bytes32, final_value: &Bytes32) -> u8 {
        let initial_value = initial_value.into_u256_be();
        let final_value = final_value.into_u256_be();
        Self::optimal_compression_length_u256(initial_value, final_value)
    }

    pub fn optimal_compression_u256<IOTypes: SystemIOTypesConfig>(
        initial_value: U256,
        final_value: U256,
        hasher: &mut impl MiniDigest,
        result_keeper: &mut impl IOResultKeeper<IOTypes>,
    ) {
        let mut optimal_strategy = Self::Nothing;
        let mut optimal_length = optimal_strategy
            .compression_length(initial_value, final_value)
            .unwrap();

        // nothing already checked
        for strategy in [Self::Add, Self::Sub, Self::Transform] {
            if let Some(length) = strategy.compression_length(initial_value, final_value) {
                if length < optimal_length {
                    optimal_strategy = strategy;
                    optimal_length = length;
                }
            }
        }

        // safe to unwrap here as strategy is checked to be applicable to the current values
        optimal_strategy
            .compress(initial_value, final_value, hasher, result_keeper)
            .unwrap()
    }

    pub fn optimal_compression<IOTypes: SystemIOTypesConfig>(
        initial_value: &Bytes32,
        final_value: &Bytes32,
        hasher: &mut impl MiniDigest,
        result_keeper: &mut impl IOResultKeeper<IOTypes>,
    ) {
        let initial_value = initial_value.into_u256_be();
        let final_value = final_value.into_u256_be();
        Self::optimal_compression_u256(initial_value, final_value, hasher, result_keeper);
    }
}

#[cfg(test)]
mod tests {
    use super::ValueDiffCompressionStrategy;
    use crate::system::IOResultKeeper;
    use crate::types_config::EthereumIOTypesConfig;
    use crate::utils::*;
    use crypto::MiniDigest;

    struct TestResultKeeper {
        pub pubdata: Vec<u8>,
    }

    impl IOResultKeeper<EthereumIOTypesConfig> for TestResultKeeper {
        fn pubdata<'a>(&mut self, value: &'a [u8]) {
            self.pubdata.extend_from_slice(value)
        }
    }

    #[test]
    fn basic_compression_test() {
        let initial = Bytes32::from_array([
            0xff, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0,
        ]);
        let r#final = Bytes32::from_array([
            0xff, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 3,
        ]);

        let optimal_length =
            ValueDiffCompressionStrategy::optimal_compression_length(&initial, &r#final);

        let mut nop_hasher = NopHasher::new();
        let mut result_keeper = TestResultKeeper { pubdata: vec![] };

        ValueDiffCompressionStrategy::optimal_compression(
            &initial,
            &r#final,
            &mut nop_hasher,
            &mut result_keeper,
        );
        let compression = result_keeper.pubdata;

        assert_eq!(optimal_length as usize, compression.len());
        // "Addition" strategy is optimal in this case
        assert_eq!(compression.len(), 2);
        println!("{:?}", compression);
        assert_eq!(compression[0], 0b00001001);
        assert_eq!(compression[1], 3);
    }
}
