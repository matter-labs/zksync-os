use core::str::FromStr;

use super::*;
use ruint::aliases::{B160, U256};

#[test]
fn test_unit_serialization() {
    let unit = ();
    assert_eq!(unit.word_len(), 0);
    let mut iter = unit.to_word_vec().into_iter();
    assert_eq!(iter.len(), 0);
    assert_eq!(iter.next(), None);

    let mut empty_iter = core::iter::empty();
    let _ = <() as WordDeserializable>::read_words(&mut empty_iter).unwrap();
}

#[test]
fn test_bool_serialization() {
    let val = true;
    let mut iter = val.to_word_vec().into_iter();
    let deserialized = bool::read_words(&mut iter).unwrap();
    assert_eq!(deserialized, true);

    let val = false;
    let mut iter = val.to_word_vec().into_iter();
    let deserialized = bool::read_words(&mut iter).unwrap();
    assert_eq!(deserialized, false);
}

#[test]
fn test_u8_serialization() {
    let val = 255u8;
    assert_eq!(val.word_len(), 0u64.word_len());

    let mut iter = val.to_word_vec().into_iter();
    let deserialized = u8::read_words(&mut iter).unwrap();
    assert_eq!(deserialized, 255);
}

#[test]
fn test_u8_overflow_detection() {
    let large_value = 256usize;
    let mut iter = core::iter::once(large_value);
    let result = u8::read_words(&mut iter);
    assert!(result.is_err());
}

#[test]
fn test_u32_serialization() {
    let val = 0x12345678u32;
    assert_eq!(val.word_len(), 0u64.word_len());

    let mut iter = val.to_word_vec().into_iter();
    let deserialized = u32::read_words(&mut iter).unwrap();
    assert_eq!(deserialized, 0x12345678);
}

#[test]
fn test_u32_overflow_detection() {
    let large_value = (u32::MAX as u64 + 1) as usize;
    let mut iter = core::iter::once(large_value);
    let result = u32::read_words(&mut iter);
    assert!(result.is_err());
}

#[test]
fn test_u64_serialization() {
    let val = 0x123456789ABCDEFu64;
    let mut iter = val.to_word_vec().into_iter();
    let deserialized = u64::read_words(&mut iter).unwrap();
    assert_eq!(deserialized, 0x123456789ABCDEF);
}

#[cfg(target_pointer_width = "64")]
#[test]
fn test_u64_single_word_on_64bit() {
    assert_eq!(0u64.word_len(), 1);

    let val = 0x123456789ABCDEFu64;
    let mut iter = val.to_word_vec().into_iter();
    assert_eq!(iter.len(), 1);
    assert_eq!(iter.next(), Some(0x123456789ABCDEF));
    assert_eq!(iter.next(), None);
}

#[cfg(target_pointer_width = "32")]
#[test]
fn test_u64_two_words_on_32bit() {
    assert_eq!(0u64.word_len(), 2);

    let val = 0x123456789ABCDEFu64;
    let mut iter = val.to_word_vec().into_iter();
    assert_eq!(iter.len(), 2);

    let low = iter.next().unwrap();
    let high = iter.next().unwrap();
    assert_eq!(iter.next(), None);

    let reconstructed = ((high as u64) << 32) | (low as u64);
    assert_eq!(reconstructed, 0x123456789ABCDEF);
}

#[test]
fn test_u256_serialization() {
    let val = U256::from_str("0x123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF")
        .unwrap();

    let mut iter = val.to_word_vec().into_iter();
    let deserialized = U256::read_words(&mut iter).unwrap();
    assert_eq!(deserialized, val);
}

#[test]
fn test_u256_length() {
    assert_eq!(U256::ZERO.word_len(), 0u64.word_len() * 4);
}

#[test]
fn test_b160_serialization() {
    let val = B160::from_str("0x1234567890123456789012345678901234567890").unwrap();

    let mut iter = val.to_word_vec().into_iter();
    let deserialized = B160::read_words(&mut iter).unwrap();
    assert_eq!(deserialized, val);
}

#[test]
fn test_b160_insufficient_data() {
    let mut iter = core::iter::once(42usize);
    let result = B160::read_words(&mut iter);
    assert!(result.is_err());
}

#[test]
fn test_tuple_serialization() {
    let val = (42u32, 100u64);

    let serialized = WordSerializable::to_word_vec(&val);
    let mut iter = serialized.into_iter();

    let deserialized = <(u32, u64) as WordDeserializable>::read_words(&mut iter).unwrap();
    assert_eq!(deserialized, (42, 100));
}

#[test]
fn test_tuple_length() {
    assert_eq!(
        WordSerializable::word_len(&(42u32, 100u64)),
        WordSerializable::word_len(&42u32) + WordSerializable::word_len(&100u64)
    );
}

#[test]
fn test_array_length() {
    assert_eq!(
        WordSerializable::word_len(&[1u32, 2, 3, 4, 5]),
        WordSerializable::word_len(&1u32) * 5
    );
}

#[test]
fn test_composed_word_serialization_no_chain_needed() {
    let value = (42u32, 100u64);
    let serialized = WordSerializable::to_word_vec(&value);
    assert_eq!(serialized.len(), WordSerializable::word_len(&value));

    let mut iter = serialized.into_iter();
    let deserialized = <(u32, u64) as WordDeserializable>::read_words(&mut iter).unwrap();
    assert_eq!(deserialized, value);
}

#[test]
fn test_bool_invalid_values() {
    let mut iter = core::iter::once(2usize);
    let result = bool::read_words(&mut iter);
    assert!(result.is_err());

    let mut iter = core::iter::once(42usize);
    let result = bool::read_words(&mut iter);
    assert!(result.is_err());
}

#[test]
fn test_architecture_specific_behavior() {
    #[cfg(target_pointer_width = "32")]
    {
        assert_eq!(0u64.word_len(), 2);
        assert_eq!(U256::ZERO.word_len(), 8);
    }

    #[cfg(target_pointer_width = "64")]
    {
        assert_eq!(0u64.word_len(), 1);
        assert_eq!(U256::ZERO.word_len(), 4);
    }
}

#[test]
fn test_word_len_matches_serialized_len() {
    assert_eq!(().word_len(), ().to_word_vec().len());
    assert_eq!(true.word_len(), true.to_word_vec().len());
    assert_eq!(255u8.word_len(), 255u8.to_word_vec().len());
    assert_eq!(0u32.word_len(), 0u32.to_word_vec().len());
    assert_eq!(0u64.word_len(), 0u64.to_word_vec().len());

    let u256_val = U256::ZERO;
    assert_eq!(u256_val.word_len(), u256_val.to_word_vec().len());

    let b160_val = B160::ZERO;
    assert_eq!(b160_val.word_len(), b160_val.to_word_vec().len());

    let tuple_val = (0u32, 0u64);
    assert_eq!(WordSerializable::word_len(&tuple_val), tuple_val.to_word_vec().len());

    let array_val = [0u32; 3];
    assert_eq!(WordSerializable::word_len(&array_val), array_val.to_word_vec().len());
}

#[test]
fn test_word_serializable_to_vec_matches_word_len() {
    let value =
        U256::from_str("0x123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF")
            .unwrap();
    let new_words = WordSerializable::to_word_vec(&value);
    assert_eq!(new_words.len(), WordSerializable::word_len(&value));
}

#[test]
fn test_word_deserializable_roundtrip() {
    let value = B160::from_str("0x1234567890123456789012345678901234567890").unwrap();
    let serialized = WordSerializable::to_word_vec(&value);
    let mut iter = serialized.into_iter();

    let roundtrip = <B160 as WordDeserializable>::read_words(&mut iter).unwrap();
    assert_eq!(roundtrip, value);
}

#[test]
fn test_vec_word_roundtrip() {
    let value = vec![1u32, 2u32, 3u32, 4u32];
    let serialized = WordSerializable::to_word_vec(&value);
    let mut iter = serialized.into_iter();

    let roundtrip = <Vec<u32> as WordDeserializable>::read_words(&mut iter).unwrap();
    assert_eq!(roundtrip, value);
}

#[test]
fn test_vec_of_tuples_word_roundtrip() {
    let value = vec![(1u32, 2u64), (3u32, 4u64), (5u32, 6u64)];
    let serialized = WordSerializable::to_word_vec(&value);
    let mut iter = serialized.into_iter();

    let roundtrip = <Vec<(u32, u64)> as WordDeserializable>::read_words(&mut iter).unwrap();
    assert_eq!(roundtrip, value);
}
