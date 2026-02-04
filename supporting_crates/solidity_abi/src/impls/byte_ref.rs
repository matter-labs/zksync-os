use super::*;
use crate::impls::u256_to_usize_checked;
use alloc::vec::Vec;
use ruint::aliases::U256;

#[derive(Clone, Copy)]
pub struct Bytes<'a>(pub &'a [u8]);

impl<'a> SelectorCodable for Bytes<'a> {
    const CANONICAL_IDENT: &'static str = "bytes";
}

#[derive(Clone)]
pub struct BytesOwned<A: Allocator>(pub Vec<u8, A>);

impl<A: Allocator> SelectorCodable for BytesOwned<A> {
    const CANONICAL_IDENT: &'static str = "bytes";
}

impl<'a> SolidityCodable for Bytes<'a> {
    type ReflectionRef<'b>
        = BytesRef<'b>
    where
        Self: 'b;
    type ReflectionRefMut<'b>
        = BytesRefMut<'b>
    where
        Self: 'b;

    const IS_DYNAMIC: bool = true;
}

impl<A: Allocator> SolidityCodable for BytesOwned<A> {
    type ReflectionRef<'b>
        = BytesRef<'b>
    where
        Self: 'b;
    type ReflectionRefMut<'b>
        = BytesRefMut<'b>
    where
        Self: 'b;

    const IS_DYNAMIC: bool = true;
}

#[derive(Clone, Copy)]
pub struct BytesRef<'a> {
    source: &'a [u8],
}

pub struct BytesRefMut<'a> {
    source: &'a mut [u8],
}

impl<'a> SolidityCodableReflectionRef<'a> for BytesRef<'a> {
    fn parse(source: &'a [u8], head_offset: &mut usize) -> Result<Self, ()> {
        let (_, local_head) = source.split_at_checked(*head_offset).ok_or(())?;
        if local_head.len() < 32 {
            return Err(());
        }
        let tail_offset = local_head.as_chunks::<32>().0.iter().next().unwrap();
        let tail_offset = U256::from_be_bytes(*tail_offset);
        let tail_offset = u256_to_usize_checked(&tail_offset)?;
        let (_, local_tail) = source.split_at_checked(tail_offset).ok_or(())?;
        if local_tail.len() < 32 {
            return Err(());
        }
        let len = local_tail.as_chunks::<32>().0.iter().next().unwrap();
        let len = U256::from_be_bytes(*len);
        let len = u256_to_usize_checked(&len)?;
        let (_, bytes_body) = local_tail.split_at_checked(32).ok_or(())?;
        let padded_len = len.next_multiple_of(32);
        if bytes_body.len() < core::cmp::max(32, padded_len) {
            return Err(());
        }
        let bytes = &bytes_body[..len];
        let new = Self { source: bytes };
        *head_offset += 32;

        Ok(new)
    }
}

impl<'a> SolidityCodableReflectionRefMut<'a> for BytesRefMut<'a> {
    fn parse_mut(source: &'a mut [u8], head_offset: &mut usize) -> Result<Self, ()> {
        let (_, local_head) = source.split_at_mut_checked(*head_offset).ok_or(())?;
        if local_head.len() < 32 {
            return Err(());
        }
        let tail_offset = local_head.as_chunks::<32>().0.iter().next().unwrap();
        let tail_offset = U256::from_be_bytes(*tail_offset);
        let tail_offset = u256_to_usize_checked(&tail_offset)?;
        let (_, local_tail) = source.split_at_mut_checked(tail_offset).ok_or(())?;
        if local_tail.len() < 32 {
            return Err(());
        }
        let len = local_tail.as_chunks::<32>().0.iter().next().unwrap();
        let len = U256::from_be_bytes(*len);
        let len = u256_to_usize_checked(&len)?;
        let (_, bytes_body) = local_tail.split_at_mut_checked(32).ok_or(())?;
        let padded_len = len.next_multiple_of(32);
        if bytes_body.len() < core::cmp::max(32, padded_len) {
            return Err(());
        }
        let bytes = &mut bytes_body[..len];
        let new = Self { source: bytes };
        *head_offset += 32;

        Ok(new)
    }
}

impl<'a> SolidityCodableReflectionRefReadable<'a> for BytesRef<'a> {
    type Value = Bytes<'a>;

    fn read(&'a self) -> Result<Self::Value, ()> {
        Ok(Bytes(self.source))
    }
}

impl<'a> SolidityCodableReflectionRefReadable<'a> for BytesRefMut<'a> {
    type Value = Bytes<'a>;

    fn read(&'a self) -> Result<Self::Value, ()> {
        Ok(Bytes(self.source))
    }
}

impl<'a> SolidityCodableReflectionRefWritable<'a> for BytesRefMut<'a> {
    fn write(
        &'a mut self,
        value: &'_ <Self as SolidityCodableReflectionRefReadable<'a>>::Value,
    ) -> Result<(), ()> {
        if value.0.len() != self.source.len() {
            return Err(());
        }

        self.source.copy_from_slice(&value.0);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use core::str;

    use super::{Bytes, BytesOwned, SelectorCodable};

    #[test]
    fn bytes_selector_codeable() {
        let mut buffer = [0; 5];
        let mut offset = 0;
        let mut is_first = true;
        Bytes::append_to_selector(&mut buffer, &mut offset, &mut is_first).unwrap();
        let s = str::from_utf8(&buffer).unwrap();
        assert_eq!(s, "bytes");
        assert!(!is_first);
        assert_eq!(offset, 5);

        offset = 1;
        assert_eq!(
            Bytes::append_to_selector(&mut buffer, &mut offset, &mut is_first),
            Err(())
        );
        assert_eq!(offset, 2);

        let mut buffer = [0; 8];
        Bytes::append_to_selector(&mut buffer, &mut offset, &mut is_first).unwrap();
        let s = str::from_utf8(&buffer).unwrap();
        assert_eq!(s, "\0\0,bytes");
    }

    #[test]
    fn bytes_owned_selector_codeable() {
        let mut buffer = [0; 5];
        let mut offset = 0;
        let mut is_first = true;
        BytesOwned::<std::alloc::Global>::append_to_selector(
            &mut buffer,
            &mut offset,
            &mut is_first,
        )
        .unwrap();
        let s = str::from_utf8(&buffer).unwrap();
        assert_eq!(s, "bytes");
    }
}
