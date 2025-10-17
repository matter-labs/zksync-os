use core::str::FromStr;

use super::*;
use ruint::aliases::{B160, U256};

#[test]
fn test_unit_serialization() {
    let unit = ();
    assert_eq!(<() as UsizeSerializable>::USIZE_LEN, 0);
    let mut iter = unit.iter();
    assert_eq!(iter.len(), 0);
    assert_eq!(iter.next(), None);

    // Test deserialization
    let mut empty_iter = core::iter::empty();
    let deserialized = <() as UsizeDeserializable>::from_iter(&mut empty_iter).unwrap();
}

#[test]
fn test_bool_serialization() {
    // Test true
    let val = true;
    let iter = val.iter();
    let collected: Vec<_> = iter.collect();

    let mut iter = collected.into_iter();
    let deserialized = bool::from_iter(&mut iter).unwrap();
    assert_eq!(deserialized, true);

    // Test false
    let val = false;
    let iter = val.iter();
    let collected: Vec<_> = iter.collect();

    let mut iter = collected.into_iter();
    let deserialized = bool::from_iter(&mut iter).unwrap();
    assert_eq!(deserialized, false);
}

#[test]
fn test_u8_serialization() {
    let val = 255u8;
    assert_eq!(
        <u8 as UsizeSerializable>::USIZE_LEN,
        <u64 as UsizeSerializable>::USIZE_LEN
    );

    let iter = val.iter();
    let collected: Vec<_> = iter.collect();

    let mut iter = collected.into_iter();
    let deserialized = u8::from_iter(&mut iter).unwrap();
    assert_eq!(deserialized, 255);
}

#[test]
fn test_u8_overflow_detection() {
    // Create an iterator with a value too large for u8
    let large_value = 256usize;
    let mut iter = core::iter::once(large_value);

    // This should fail since 256 > u8::MAX
    let result = u8::from_iter(&mut iter);
    assert!(result.is_err());
}

#[test]
fn test_u32_serialization() {
    let val = 0x12345678u32;
    assert_eq!(
        <u32 as UsizeSerializable>::USIZE_LEN,
        <u64 as UsizeSerializable>::USIZE_LEN
    );

    let iter = val.iter();
    let collected: Vec<_> = iter.collect();

    let mut iter = collected.into_iter();
    let deserialized = u32::from_iter(&mut iter).unwrap();
    assert_eq!(deserialized, 0x12345678);
}

#[test]
fn test_u32_overflow_detection() {
    // Create an iterator with a value too large for u32
    let large_value = (u32::MAX as u64 + 1) as usize;
    let mut iter = core::iter::once(large_value);

    let result = u32::from_iter(&mut iter);
    assert!(result.is_err());
}

#[test]
fn test_u64_serialization() {
    let val = 0x123456789ABCDEFu64;

    let iter = val.iter();
    let collected: Vec<_> = iter.collect();

    let mut iter = collected.into_iter();
    let deserialized = u64::from_iter(&mut iter).unwrap();
    assert_eq!(deserialized, 0x123456789ABCDEF);
}

#[cfg(target_pointer_width = "64")]
#[test]
fn test_u64_single_word_on_64bit() {
    assert_eq!(<u64 as UsizeSerializable>::USIZE_LEN, 1);

    let val = 0x123456789ABCDEFu64;
    let mut iter = val.iter();
    assert_eq!(iter.len(), 1);
    assert_eq!(iter.next(), Some(0x123456789ABCDEF));
    assert_eq!(iter.next(), None);
}

#[cfg(target_pointer_width = "32")]
#[test]
fn test_u64_two_words_on_32bit() {
    assert_eq!(<u64 as UsizeSerializable>::USIZE_LEN, 2);

    let val = 0x123456789ABCDEFu64;
    let mut iter = val.iter();
    assert_eq!(iter.len(), 2);

    let low = iter.next().unwrap();
    let high = iter.next().unwrap();
    assert_eq!(iter.next(), None);

    // Reconstruct the value to verify correct decomposition
    let reconstructed = ((high as u64) << 32) | (low as u64);
    assert_eq!(reconstructed, 0x123456789ABCDEF);
}

#[test]
fn test_u256_serialization() {
    let val = U256::from_str("0x123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF")
        .unwrap();

    let iter = val.iter();
    let collected: Vec<_> = iter.collect();

    let mut iter = collected.into_iter();
    let deserialized = U256::from_iter(&mut iter).unwrap();
    assert_eq!(deserialized, val);
}

#[test]
fn test_u256_length() {
    assert_eq!(
        <U256 as UsizeSerializable>::USIZE_LEN,
        <u64 as UsizeSerializable>::USIZE_LEN * 4
    );
}

#[test]
fn test_b160_serialization() {
    let val = B160::from_str("0x1234567890123456789012345678901234567890").unwrap();

    let iter = val.iter();
    let collected: Vec<_> = iter.collect();

    let mut iter = collected.into_iter();
    let deserialized = B160::from_iter(&mut iter).unwrap();
    assert_eq!(deserialized, val);
}

#[test]
fn test_b160_insufficient_data() {
    // Create an iterator with insufficient data
    let mut iter = core::iter::once(42usize);

    let result = B160::from_iter(&mut iter);
    assert!(result.is_err());
}

#[test]
fn test_tuple_serialization() {
    let val = (42u32, 100u64);

    let iter = val.iter();
    let collected: Vec<_> = iter.collect();

    let mut iter = collected.into_iter();
    let deserialized = <(u32, u64)>::from_iter(&mut iter).unwrap();
    assert_eq!(deserialized, (42, 100));
}

#[test]
fn test_tuple_length() {
    assert_eq!(
        <(u32, u64) as UsizeSerializable>::USIZE_LEN,
        <u32 as UsizeSerializable>::USIZE_LEN + <u64 as UsizeSerializable>::USIZE_LEN
    );
}

#[test]
fn test_array_length() {
    assert_eq!(
        <[u32; 5] as UsizeSerializable>::USIZE_LEN,
        <u32 as UsizeSerializable>::USIZE_LEN * 5
    );
}

#[test]
fn test_exact_size_chain_in_serialization() {
    let first = 42u32;
    let second = 100u64;

    let chain = ExactSizeChain::new(first.iter(), second.iter());
    assert_eq!(
        chain.len(),
        <u32 as UsizeSerializable>::USIZE_LEN + <u64 as UsizeSerializable>::USIZE_LEN
    );

    let collected: Vec<_> = chain.collect();
    let mut iter = collected.into_iter();

    // Deserialize as tuple
    let deserialized = <(u32, u64)>::from_iter(&mut iter).unwrap();
    assert_eq!(deserialized, (42, 100));
}

#[test]
fn test_bool_invalid_values() {
    // Test deserialization with invalid bool values
    let mut iter = core::iter::once(2usize); // Not 0 or 1
    let result = bool::from_iter(&mut iter);
    assert!(result.is_err());

    let mut iter = core::iter::once(42usize);
    let result = bool::from_iter(&mut iter);
    assert!(result.is_err());
}

#[test]
fn test_architecture_specific_behavior() {
    // Test that serialization length is consistent with architecture
    #[cfg(target_pointer_width = "32")]
    {
        assert_eq!(<u64 as UsizeSerializable>::USIZE_LEN, 2);
        assert_eq!(<U256 as UsizeSerializable>::USIZE_LEN, 8);
    }

    #[cfg(target_pointer_width = "64")]
    {
        assert_eq!(<u64 as UsizeSerializable>::USIZE_LEN, 1);
        assert_eq!(<U256 as UsizeSerializable>::USIZE_LEN, 4);
    }
}

#[test]
fn test_usize_len_consistency() {
    // Test that USIZE_LEN matches actual iteration length for all types

    assert_eq!(<() as UsizeSerializable>::USIZE_LEN, ().iter().len());
    assert_eq!(<bool as UsizeSerializable>::USIZE_LEN, true.iter().len());
    assert_eq!(<u8 as UsizeSerializable>::USIZE_LEN, 255u8.iter().len());
    assert_eq!(<u32 as UsizeSerializable>::USIZE_LEN, 0u32.iter().len());
    assert_eq!(<u64 as UsizeSerializable>::USIZE_LEN, 0u64.iter().len());

    let u256_val = U256::ZERO;
    assert_eq!(
        <U256 as UsizeSerializable>::USIZE_LEN,
        u256_val.iter().len()
    );

    let b160_val = B160::ZERO;
    assert_eq!(
        <B160 as UsizeSerializable>::USIZE_LEN,
        b160_val.iter().len()
    );

    let tuple_val = (0u32, 0u64);
    assert_eq!(
        <(u32, u64) as UsizeSerializable>::USIZE_LEN,
        tuple_val.iter().len()
    );

    let array_val = [0u32; 3];
    assert_eq!(
        <[u32; 3] as UsizeSerializable>::USIZE_LEN,
        array_val.iter().len()
    );
}
