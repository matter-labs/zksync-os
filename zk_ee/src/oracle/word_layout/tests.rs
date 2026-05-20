use super::WordLayout;
extern crate alloc;
use alloc::vec;
use alloc::vec::Vec;

fn roundtrip<T: WordLayout + PartialEq + core::fmt::Debug>(val: &T) -> T {
    let mut words = Vec::new();
    val.write_words(&mut |w| words.push(w));
    let mut iter = words.into_iter();
    T::read_words(&mut || iter.next().expect("not enough words"))
}

#[test]
fn primitives() {
    assert_eq!(roundtrip(&true), true);
    assert_eq!(roundtrip(&false), false);
    assert_eq!(roundtrip(&42u8), 42u8);
    assert_eq!(roundtrip(&1234u16), 1234u16);
    assert_eq!(roundtrip(&0xDEADBEEFu32), 0xDEADBEEFu32);
    assert_eq!(roundtrip(&0xDEADBEEF_CAFEBABEu64), 0xDEADBEEF_CAFEBABEu64);
    assert_eq!(roundtrip(&()), ());
}

#[test]
fn byte_arrays() {
    assert_eq!(roundtrip(&[1u8, 2, 3]), [1, 2, 3]);
    assert_eq!(roundtrip(&[1u8, 2, 3, 4]), [1, 2, 3, 4]);
    assert_eq!(roundtrip(&[1u8, 2, 3, 4, 5]), [1, 2, 3, 4, 5]);
    let big: [u8; 32] = core::array::from_fn(|i| i as u8);
    assert_eq!(roundtrip(&big), big);
}

#[test]
fn u64_arrays() {
    assert_eq!(roundtrip(&[1u64, 2, 3, 4]), [1u64, 2, 3, 4]);
}

#[test]
fn u32_arrays() {
    assert_eq!(roundtrip(&[0xAAu32, 0xBB, 0xCC]), [0xAAu32, 0xBB, 0xCC]);
}

#[test]
fn word_counts() {
    assert_eq!(<bool as WordLayout>::WORD_COUNT, Some(1));
    assert_eq!(<u32 as WordLayout>::WORD_COUNT, Some(1));
    assert_eq!(<u64 as WordLayout>::WORD_COUNT, Some(2));
    assert_eq!(<[u8; 3] as WordLayout>::WORD_COUNT, Some(1));
    assert_eq!(<[u8; 4] as WordLayout>::WORD_COUNT, Some(1));
    assert_eq!(<[u8; 5] as WordLayout>::WORD_COUNT, Some(2));
    assert_eq!(<[u8; 32] as WordLayout>::WORD_COUNT, Some(8));
    assert_eq!(<[u8; 48] as WordLayout>::WORD_COUNT, Some(12));
    assert_eq!(<[u64; 4] as WordLayout>::WORD_COUNT, Some(8));
    assert_eq!(<[u32; 3] as WordLayout>::WORD_COUNT, Some(3));
    assert_eq!(<Vec<u8> as WordLayout>::WORD_COUNT, None);
    assert_eq!(<Vec<u64> as WordLayout>::WORD_COUNT, None);
}

#[test]
fn vec_u8_byte_packed() {
    let val: Vec<u8> = vec![1, 2, 3, 4, 5];
    let result = roundtrip(&val);
    assert_eq!(result, val);
    let mut words = Vec::new();
    val.write_words(&mut |w| words.push(w));
    assert_eq!(words.len(), 3); // 1 (length) + 2 (5 bytes packed)
}

#[test]
fn vec_u8_empty() {
    let val: Vec<u8> = vec![];
    let result = roundtrip(&val);
    assert_eq!(result, val);
    let mut words = Vec::new();
    val.write_words(&mut |w| words.push(w));
    assert_eq!(words.len(), 1); // just length word
}

#[test]
fn vec_u64() {
    let val: Vec<u64> = vec![0xAABBCCDD, 0x11223344];
    let result = roundtrip(&val);
    assert_eq!(result, val);
    let mut words = Vec::new();
    val.write_words(&mut |w| words.push(w));
    assert_eq!(words.len(), 5); // 1 (length) + 2*2 (two u64s)
}

#[test]
fn vec_u64_empty() {
    let val: Vec<u64> = vec![];
    let result = roundtrip(&val);
    assert_eq!(result, val);
}

#[test]
fn byte_array_exact_word_boundary() {
    let val: [u8; 8] = [1, 2, 3, 4, 5, 6, 7, 8];
    assert_eq!(<[u8; 8] as WordLayout>::WORD_COUNT, Some(2));
    assert_eq!(roundtrip(&val), val);
}

#[test]
fn u64_max() {
    assert_eq!(roundtrip(&u64::MAX), u64::MAX);
}

#[test]
fn u64_zero() {
    assert_eq!(roundtrip(&0u64), 0u64);
}
