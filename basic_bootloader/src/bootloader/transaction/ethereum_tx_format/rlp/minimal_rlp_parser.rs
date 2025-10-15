// rlp.rs

use core::marker::PhantomData;
use ruint::aliases::{B160, U256};

use crate::bootloader::errors::InvalidTransaction;

/// Minimal, zero-copy RLP cursor.
#[derive(Clone, Copy, Debug)]
pub struct Rlp<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Rlp<'a> {
    /// Construct a cursor over bytes.
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, pos: 0 }
    }

    /// True iff the cursor consumed the entire buffer.
    pub fn is_empty(&self) -> bool {
        self.pos == self.bytes.len()
    }

    /// Save the current offset for later byte-slice recovery.
    pub fn mark(&self) -> usize {
        self.pos
    }

    /// Return the exact bytes consumed since `mark()`.
    pub fn consumed_since(&self, mark: usize) -> &'a [u8] {
        &self.bytes[mark..self.pos]
    }

    /// Return the remaining unconsumed bytes.
    pub fn remaining(&self) -> &'a [u8] {
        &self.bytes[self.pos..]
    }

    fn take_exact(&mut self, n: usize) -> Result<&'a [u8], InvalidTransaction> {
        let end = self
            .pos
            .checked_add(n)
            .ok_or(InvalidTransaction::InvalidStructure)?;
        if end > self.bytes.len() {
            return Err(InvalidTransaction::InvalidStructure);
        }
        let out = &self.bytes[self.pos..end];
        self.pos = end;
        Ok(out)
    }

    fn take1(&mut self) -> Result<u8, InvalidTransaction> {
        if self.pos >= self.bytes.len() {
            return Err(InvalidTransaction::InvalidStructure);
        }
        let b = self.bytes[self.pos];
        self.pos += 1;
        Ok(b)
    }

    fn be_u32(s: &[u8]) -> Result<usize, InvalidTransaction> {
        if s.len() > 4 {
            return Err(InvalidTransaction::InvalidStructure);
        }
        let mut v: u32 = 0;
        for &b in s {
            v = (v << 8) | b as u32;
        }
        Ok(v as usize)
    }

    /// Decode an RLP string and return its payload bytes (no header).
    pub fn bytes(&mut self) -> Result<&'a [u8], InvalidTransaction> {
        let m = self.take1()?;
        if m < 0x80 {
            Ok(&self.bytes[self.pos - 1..self.pos])
        } else if m <= 0xb7 {
            let len = (m - 0x80) as usize;
            if len == 1 && self.bytes[self.pos] < 0x80 {
                // non-canonical single byte
                return Err(InvalidTransaction::InvalidStructure);
            }
            self.take_exact(len)
        } else if m < 0xc0 {
            let ll = (m - 0xb7) as usize;
            // we make some reasonable bound here - max u32 length
            if ll > 4 {
                return Err(InvalidTransaction::InvalidStructure);
            }
            let len = Self::be_u32(self.take_exact(ll)?)?;
            if len < 56 {
                // non-canonical long length
                return Err(InvalidTransaction::InvalidStructure);
            }
            self.take_exact(len)
        } else {
            Err(InvalidTransaction::InvalidStructure)
        }
    }

    /// Enter a list and return a sub-cursor limited to the list payload bytes.
    pub fn list(&mut self) -> Result<Rlp<'a>, InvalidTransaction> {
        let m = self.take1()?;
        if m < 0xc0 {
            return Err(InvalidTransaction::InvalidStructure);
        }
        let len = if m <= 0xf7 {
            (m - 0xc0) as usize
        } else {
            let ll = (m - 0xf7) as usize;
            // we make some reasonable bound here - max u32 length
            if ll > 4 {
                return Err(InvalidTransaction::InvalidStructure);
            }
            let len = Self::be_u32(self.take_exact(ll)?)?;
            if len < 56 {
                // non-canonical long length
                return Err(InvalidTransaction::InvalidStructure);
            }
            len
        };
        let content = self.take_exact(len)?;
        Ok(Rlp::new(content))
    }

    /// Decode u8: empty string -> 0, one byte -> that value, otherwise error.
    pub fn u8(&mut self) -> Result<u8, InvalidTransaction> {
        let s = self.bytes()?;
        match s.len() {
            0 => Ok(0),
            1 => Ok(s[0]),
            _ => Err(InvalidTransaction::InvalidStructure),
        }
    }

    /// Decode bool: 0 -> false, 1 -> true, otherwise error.
    pub fn bool(&mut self) -> Result<bool, InvalidTransaction> {
        let v = self.u8()?;
        match v {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(InvalidTransaction::InvalidStructure),
        }
    }

    /// Decode u64 in big-endian, allowing 0..=8 bytes.
    pub fn u64(&mut self) -> Result<u64, InvalidTransaction> {
        let s = self.bytes()?;
        if s.len() > 8 {
            return Err(InvalidTransaction::InvalidStructure);
        }
        let mut buf = [0u8; 8];
        buf[8 - s.len()..].copy_from_slice(s);
        Ok(u64::from_be_bytes(buf))
    }

    /// Decode U256 from a big-endian byte string.
    pub fn u256(&mut self) -> Result<U256, InvalidTransaction> {
        U256::try_from_be_slice(self.bytes()?).ok_or(InvalidTransaction::InvalidStructure)
    }
}

/// Trait for list-encoded structures that want both entry points:
///   - decode_list_from: parse and continue
///   - decode_list_full: parse and require full consumption
///
/// The implementation must assume the cursor is positioned at the list header
/// and decode the list body fields inside.
pub trait RlpListDecode<'a>: Sized {
    /// Decode just the body from a sub-cursor already restricted to the list payload.
    fn decode_list_body(r: &mut Rlp<'a>) -> Result<Self, InvalidTransaction>;

    /// Strip the list header, decode the body, and return the value.
    fn decode_list_from(r: &mut Rlp<'a>) -> Result<Self, InvalidTransaction> {
        let mut inner = r.list()?;
        let v = Self::decode_list_body(&mut inner)?;
        if !inner.is_empty() {
            return Err(InvalidTransaction::InvalidStructure);
        }
        Ok(v)
    }

    /// Parse from a standalone buffer and require full consumption.
    fn decode_list_full(bytes: &'a [u8]) -> Result<Self, InvalidTransaction> {
        let mut r = Rlp::new(bytes);
        let v = Self::decode_list_from(&mut r)?;
        if !r.is_empty() {
            return Err(InvalidTransaction::InvalidStructure);
        }
        Ok(v)
    }
}

// Traits for items that can be decoded from RLP, either as individual items or inside lists.

pub trait RlpItemDecode<'a>: Sized {
    // Decode one item starting at the current cursor position
    fn decode_from_item(r: &mut Rlp<'a>) -> Result<Self, InvalidTransaction>;
}

// Trivially, any RlpListDecode can also be decoded as an item (a list item)
impl<'a, T: RlpListDecode<'a>> RlpItemDecode<'a> for T {
    fn decode_from_item(r: &mut Rlp<'a>) -> Result<Self, InvalidTransaction> {
        T::decode_list_from(r)
    }
}

// Implement RlpItemDecode for u8 for testing purposes
impl<'a> RlpItemDecode<'a> for u8 {
    fn decode_from_item(r: &mut Rlp<'a>) -> Result<Self, InvalidTransaction> {
        r.u8()
    }
}

pub trait RlpFixedItem<'a>: RlpItemDecode<'a> {
    // Total encoded length of the item (header + payload)
    const ENCODING_LEN: usize;
    // Decode from an already sliced encoded item of exactly ENCODING_LEN bytes
    fn decode_from_fixed(encoded: &'a [u8]) -> Result<Self, InvalidTransaction>;
}

// Implementations for common fixed items

impl<'a> RlpItemDecode<'a> for &'a [u8; 32] {
    fn decode_from_item(r: &mut Rlp<'a>) -> Result<Self, InvalidTransaction> {
        let s = r.bytes()?;
        if s.len() == 32 {
            Ok(s.try_into().unwrap())
        } else {
            Err(InvalidTransaction::InvalidStructure)
        }
    }
}
impl<'a> RlpFixedItem<'a> for &'a [u8; 32] {
    const ENCODING_LEN: usize = 1 + 32; // 0xa0 + 32
    fn decode_from_fixed(encoded: &'a [u8]) -> Result<Self, InvalidTransaction> {
        if encoded.len() != 33 || encoded[0] != 0xa0 {
            return Err(InvalidTransaction::InvalidStructure);
        }
        Ok(encoded[1..].try_into().unwrap())
    }
}

impl<'a> RlpItemDecode<'a> for B160 {
    fn decode_from_item(r: &mut Rlp<'a>) -> Result<Self, InvalidTransaction> {
        let s = r.bytes()?;
        if s.len() != 20 {
            return Err(InvalidTransaction::InvalidStructure);
        }
        Ok(B160::from_be_bytes::<{ B160::BYTES }>(
            s.try_into().unwrap(),
        ))
    }
}
impl<'a> RlpFixedItem<'a> for B160 {
    const ENCODING_LEN: usize = 1 + 20; // 0x94 + 20
    fn decode_from_fixed(encoded: &'a [u8]) -> Result<Self, InvalidTransaction> {
        if encoded.len() != 21 || encoded[0] != 0x94 {
            return Err(InvalidTransaction::InvalidStructure);
        }
        Ok(B160::from_be_bytes::<{ B160::BYTES }>(
            encoded[1..].try_into().unwrap(),
        ))
    }
}

// Lists of fixed-length items
// To be used by 4844 txs.
#[derive(Clone, Copy, Debug)]
pub struct FixedList<'a, T: RlpFixedItem<'a>> {
    payload: &'a [u8], // concatenation of encoded items
    pub count: usize,
    _marker: PhantomData<T>,
}

#[derive(Clone, Copy, Debug)]
pub struct FixedListIter<'a, T: RlpFixedItem<'a>> {
    payload: &'a [u8],
    idx: usize,
    count: usize,
    _marker: PhantomData<T>,
}

impl<'a, T: RlpFixedItem<'a>> FixedList<'a, T> {
    // Parse a list header and return a fixed-length view
    pub fn decode_list_from(r: &mut Rlp<'a>) -> Result<Self, InvalidTransaction> {
        let mut inner = r.list()?;
        let all = inner.remaining();
        if all.len() % T::ENCODING_LEN != 0 {
            return Err(InvalidTransaction::InvalidStructure);
        }
        let count = all.len() / T::ENCODING_LEN;
        inner.take_exact(all.len())?; // consume to satisfy caller's emptiness check
        Ok(Self {
            payload: all,
            count,
            _marker: PhantomData,
        })
    }

    pub fn decode_list_full(bytes: &'a [u8]) -> Result<Self, InvalidTransaction> {
        let mut r = Rlp::new(bytes);
        let v = Self::decode_list_from(&mut r)?;
        if !r.is_empty() {
            return Err(InvalidTransaction::InvalidStructure);
        }
        Ok(v)
    }

    pub fn iter(&self) -> FixedListIter<'a, T> {
        FixedListIter {
            payload: self.payload,
            idx: 0,
            count: self.count,
            _marker: PhantomData,
        }
    }
}

impl<'a, T: RlpFixedItem<'a>> Iterator for FixedListIter<'a, T> {
    type Item = Result<T, InvalidTransaction>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.idx >= self.count {
            return None;
        }
        let start = self.idx * T::ENCODING_LEN;
        let end = start + T::ENCODING_LEN;
        self.idx += 1;
        Some(T::decode_from_fixed(&self.payload[start..end]))
    }
}
impl<'a, T: RlpFixedItem<'a>> ExactSizeIterator for FixedListIter<'a, T> {
    fn len(&self) -> usize {
        self.count - self.idx
    }
}

// Lists of homogeneous items (optionally validated)
// Designed to be iterated over instead of parsing fully up-front and storing all items.
#[derive(Clone, Copy, Debug)]
pub struct HomList<'a, T: RlpItemDecode<'a>, const VALIDATE: bool> {
    payload: &'a [u8],
    pub count: Option<usize>, // set when VALIDATE = true
    _marker: PhantomData<T>,
}

#[derive(Clone, Copy, Debug)]
pub struct HomListIter<'a, T: RlpItemDecode<'a>, const VALIDATE: bool> {
    r: Rlp<'a>,
    remaining_ok: bool, // for the non-validated variant to stop after error
    _marker: PhantomData<T>,
}

impl<'a, T: RlpItemDecode<'a>, const VALIDATE: bool> HomList<'a, T, VALIDATE> {
    pub fn decode_list_from(r: &mut Rlp<'a>) -> Result<Self, InvalidTransaction> {
        let mut inner = r.list()?;
        let all = inner.remaining();

        let count = if VALIDATE {
            let mut chk = Rlp::new(all);
            let mut c = 0;
            while !chk.is_empty() {
                T::decode_from_item(&mut chk)?;
                c += 1;
            }
            Some(c)
        } else {
            None
        };

        inner.take_exact(all.len())?;
        Ok(Self {
            payload: all,
            count,
            _marker: PhantomData,
        })
    }

    pub fn decode_list_full(bytes: &'a [u8]) -> Result<Self, InvalidTransaction> {
        let mut r = Rlp::new(bytes);
        let v = Self::decode_list_from(&mut r)?;
        if !r.is_empty() {
            return Err(InvalidTransaction::InvalidStructure);
        }
        Ok(v)
    }

    pub fn iter(&self) -> HomListIter<'a, T, VALIDATE> {
        HomListIter {
            r: Rlp::new(self.payload),
            remaining_ok: true,
            _marker: PhantomData,
        }
    }
}

impl<'a, T: RlpItemDecode<'a>> HomList<'a, T, true> {
    pub fn len(&self) -> usize {
        // Safe to unwrap, always set for VALIDATE = true
        self.count.unwrap()
    }
}

// validated iterator yields T directly; non-validated yields Result<T, InvalidTransaction>
impl<'a, T: RlpItemDecode<'a>> Iterator for HomListIter<'a, T, true> {
    type Item = T;
    fn next(&mut self) -> Option<Self::Item> {
        if self.r.is_empty() {
            return None;
        }
        Some(T::decode_from_item(&mut self.r).expect("pre-validated"))
    }
}
impl<'a, T: RlpItemDecode<'a>> Iterator for HomListIter<'a, T, false> {
    type Item = Result<T, InvalidTransaction>;
    fn next(&mut self) -> Option<Self::Item> {
        if !self.remaining_ok || self.r.is_empty() {
            return None;
        }
        match T::decode_from_item(&mut self.r) {
            Ok(v) => Some(Ok(v)),
            Err(_) => {
                self.remaining_ok = false;
                Some(Err(InvalidTransaction::InvalidStructure))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rlp_basic_test() {
        let data = [0x83, 0x64, 0x6f, 0x67]; // "dog" encoded
        let mut rlp = Rlp::new(&data);

        assert_eq!(rlp.pos, 0);
        assert_eq!(rlp.bytes.len(), 4);
        assert!(!rlp.is_empty());

        let mark1 = rlp.mark();
        assert_eq!(mark1, 0);

        let consumed_data = rlp.bytes().unwrap(); // Consume "dog"
        let consumed = rlp.consumed_since(mark1);
        assert_eq!(consumed_data, b"dog");
        assert_eq!(consumed, &data);
        assert!(rlp.is_empty());

        let empty_rlp = Rlp::new(&[]);
        assert!(empty_rlp.is_empty());

        let data = [0x83, 0x64, 0x6f, 0x67, 0x82, 0x63, 0x61, 0x74]; // "dog" + "cat"
        let mut rlp = Rlp::new(&data);

        let _ = rlp.bytes().unwrap(); // Consume "dog"
        let remaining = rlp.remaining();
        assert_eq!(remaining, &[0x82, 0x63, 0x61, 0x74]); // "cat" encoded
    }

    #[test]
    fn test_rlp_bytes_single_byte() {
        // Single byte < 0x80 represents itself
        let mut rlp = Rlp::new(&[0x41]); // 'A'
        let result = rlp.bytes().unwrap();
        assert_eq!(result, &[0x41]);
        assert!(rlp.is_empty());
    }

    #[test]
    fn test_rlp_error_cases() {
        // Truncated data
        let mut rlp = Rlp::new(&[0x83, 0x64, 0x6f]); // Claims 3 bytes but only has 2
        assert!(rlp.bytes().is_err());

        // Invalid length encoding
        let mut rlp = Rlp::new(&[0xbf]); // 0xb7 + 8, but max length encoding is 4 bytes
        assert!(rlp.bytes().is_err());

        // Empty buffer
        let mut rlp = Rlp::new(&[]);
        assert!(rlp.bytes().is_err());
    }

    #[test]
    fn test_rlp_fixed_list_wrong_size() {
        // Test FixedList with payload that doesn't divide evenly
        let mut payload = vec![0x94]; // B160 header
        payload.extend_from_slice(&[0x11; 20]);
        payload.push(0x12); // Extra byte that breaks the pattern

        let mut encoded = vec![0xc0 + payload.len() as u8];
        encoded.extend_from_slice(&payload);

        assert!(FixedList::<B160>::decode_list_full(&encoded).is_err());
    }

    #[test]
    fn test_rlp_hom_list_validated() {
        // Test HomList with validation enabled
        let mut data1 = [0x94; 21]; // Valid B160
        data1[0] = 0x94;
        let mut data2 = [0x94; 21]; // Valid B160
        data2[0] = 0x94;

        let mut payload = Vec::new();
        payload.extend_from_slice(&data1);
        payload.extend_from_slice(&data2);

        let mut encoded = vec![0xc0 + payload.len() as u8];
        encoded.extend_from_slice(&payload);

        let list: HomList<B160, true> = HomList::decode_list_full(&encoded).unwrap();
        assert_eq!(list.len(), 2);

        let items: Vec<_> = list.iter().collect();
        assert_eq!(items.len(), 2);

        // Test HomList validation with invalid data
        let mut payload = vec![0x95]; // Wrong header for B160 (should be 0x94)
        payload.extend_from_slice(&[0x11; 20]);

        let mut encoded = vec![0xc0 + payload.len() as u8];
        encoded.extend_from_slice(&payload);

        // Validation should fail
        assert!(HomList::<B160, true>::decode_list_full(&encoded).is_err());
    }

    #[test]
    fn test_rlp_hom_list_unvalidated() {
        // Test HomList with validation disabled
        let payload = vec![0x01, 0x02, 0x03]; // Just some bytes

        let mut encoded = vec![0xc0 + payload.len() as u8];
        encoded.extend_from_slice(&payload);

        let list: HomList<u8, false> = HomList::decode_list_full(&encoded).unwrap();
        assert!(list.count.is_none()); // No count for unvalidated

        // Iterator should handle errors gracefully
        let items: Vec<_> = list.iter().collect();
        assert_eq!(items.len(), 3);
        assert!(items[0].is_ok());
        assert!(items[1].is_ok());
        assert!(items[2].is_ok());
    }
}
