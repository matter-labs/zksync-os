use super::*;
use crate::impls::u256_to_usize_checked;
use alloc::vec::Vec;
use ruint::aliases::U256;

#[derive(Clone, Copy)]
pub struct Slice<'a, T: 'a> {
    _marker: core::marker::PhantomData<&'a T>,
}

impl<'a, T: SelectorCodable + 'a> SelectorCodable for Slice<'a, T> {
    const CANONICAL_IDENT: &'static str = "[]";

    fn append_to_selector(
        buffer: &mut [u8],
        offset: &mut usize,
        is_first: &mut bool,
    ) -> Result<(), ()> {
        match T::append_to_selector(buffer, offset, is_first) {
            Ok(_) => {}
            Err(_) => {
                return Err(());
            }
        }

        match append_ascii_str(buffer, offset, Self::CANONICAL_IDENT) {
            Ok(_) => {}
            Err(_) => {
                return Err(());
            }
        }

        Ok(())
    }
}

impl<T: SelectorCodable, A: Allocator> SelectorCodable for Vec<T, A> {
    const CANONICAL_IDENT: &'static str = "[]";

    fn append_to_selector(
        buffer: &mut [u8],
        offset: &mut usize,
        is_first: &mut bool,
    ) -> Result<(), ()> {
        match T::append_to_selector(buffer, offset, is_first) {
            Ok(_) => {}
            Err(_) => {
                return Err(());
            }
        }

        match append_ascii_str(buffer, offset, Self::CANONICAL_IDENT) {
            Ok(_) => {}
            Err(_) => {
                return Err(());
            }
        }

        Ok(())
    }
}

impl<'a, T: SolidityCodable + 'a> SolidityCodable for Slice<'a, T> {
    type ReflectionRef<'b>
        = SliceRef<'b, T>
    where
        Self: 'b;
    type ReflectionRefMut<'b>
        = SliceRefMut<'b, T>
    where
        Self: 'b;

    const IS_DYNAMIC: bool = true;
}

impl<T: SolidityCodable, A: Allocator> SolidityCodable for Vec<T, A> {
    type ReflectionRef<'b>
        = SliceRef<'b, T>
    where
        Self: 'b;
    type ReflectionRefMut<'b>
        = SliceRefMut<'b, T>
    where
        Self: 'b;

    const IS_DYNAMIC: bool = true;
}

#[derive(Clone, Copy)]
pub struct SliceRef<'a, T: SolidityCodable + 'a> {
    slice_source: &'a [u8],
    num_elements: usize,
    _marker: core::marker::PhantomData<T>,
}

pub struct SliceRefMut<'a, T: SolidityCodable + 'a> {
    slice_source: &'a mut [u8],
    num_elements: usize,
    _marker: core::marker::PhantomData<T>,
}

impl<'a, T: SolidityCodable + 'a> SolidityCodableReflectionRef<'a> for SliceRef<'a, T> {
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
        let (_, body) = local_tail.split_at_checked(32).ok_or(())?;
        // here it's actually different from all other structure types, and
        // we should continue into preliminary parsing of all other elements
        let mut local_head_offset = 0;
        for i in 0..len {
            debug_assert_eq!(local_head_offset, i * T::HEAD_SIZE);
            let _ = T::ReflectionRef::parse(body, &mut local_head_offset)?;
        }
        debug_assert_eq!(local_head_offset, len * T::HEAD_SIZE);
        let new = Self {
            slice_source: body,
            num_elements: len,
            _marker: core::marker::PhantomData,
        };
        *head_offset += 32;

        Ok(new)
    }
}

impl<'a, T: SolidityCodable + 'a> SolidityCodableReflectionRefMut<'a> for SliceRefMut<'a, T> {
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
        let (_, body) = local_tail.split_at_mut_checked(32).ok_or(())?;
        // here it's actually different from all other structure types, and
        // we should continue into preliminary parsing of all other elements
        let mut local_head_offset = 0;
        for i in 0..len {
            debug_assert_eq!(local_head_offset, i * T::HEAD_SIZE);
            let _ = T::ReflectionRefMut::parse_mut(body, &mut local_head_offset)?;
        }
        debug_assert_eq!(local_head_offset, len * T::HEAD_SIZE);
        let new = Self {
            slice_source: body,
            num_elements: len,
            _marker: core::marker::PhantomData,
        };
        *head_offset += 32;

        Ok(new)
    }
}

impl<'a, T: SolidityCodable + 'a> SolidityCodableReflectionRefAddressable<'a> for SliceRef<'a, T> {
    type Value = T::ReflectionRef<'a>;

    fn get(&'a self, index: usize) -> Result<Self::Value, ()> {
        if index >= self.num_elements {
            return Err(());
        }
        let mut local_head_offset = index * T::HEAD_SIZE;
        let el = T::ReflectionRef::<'a>::parse(self.slice_source, &mut local_head_offset)?;

        Ok(el)
    }

    fn len(&self) -> usize {
        self.num_elements
    }
}

impl<'a, T: SolidityCodable + 'a> SolidityCodableReflectionRefAddressable<'a>
    for SliceRefMut<'a, T>
{
    type Value = T::ReflectionRef<'a>;

    fn get(&'a self, index: usize) -> Result<Self::Value, ()> {
        if index >= self.num_elements {
            return Err(());
        }
        let mut local_head_offset = index * T::HEAD_SIZE;
        let el = T::ReflectionRef::<'a>::parse(self.slice_source, &mut local_head_offset)?;

        Ok(el)
    }

    fn len(&self) -> usize {
        self.num_elements
    }
}

impl<'a, T: SolidityCodable + 'a> SolidityCodableReflectionRefAddressableMut<'a>
    for SliceRefMut<'a, T>
{
    type ValueMut = T::ReflectionRefMut<'a>;
    fn get_mut(&'a mut self, index: usize) -> Result<Self::ValueMut, ()> {
        if index >= self.num_elements {
            return Err(());
        }
        let mut local_head_offset = index * T::HEAD_SIZE;
        let el = T::ReflectionRefMut::<'a>::parse_mut(self.slice_source, &mut local_head_offset)?;

        Ok(el)
    }
}

#[cfg(test)]
mod tests {
    use core::str;

    use super::{SelectorCodable, Slice, Vec};

    struct Test;

    impl SelectorCodable for Test {
        const CANONICAL_IDENT: &'static str = "test";
    }

    #[test]
    fn slice_selector_codeable() {
        let mut buffer = [0; 20];
        let mut offset = 0;
        let mut is_first = true;

        Slice::<Test>::append_to_selector(&mut buffer, &mut offset, &mut is_first).unwrap();
        assert!(!is_first);
        assert_eq!(offset, 6);
        let s = str::from_utf8(&buffer)
            .unwrap()
            .trim_end_matches(char::from(0));
        assert_eq!(s, "test[]");

        Slice::<Test>::append_to_selector(&mut buffer, &mut offset, &mut is_first).unwrap();
        assert!(!is_first);
        assert_eq!(offset, 13);
        let s = str::from_utf8(&buffer)
            .unwrap()
            .trim_end_matches(char::from(0));
        assert_eq!(s, "test[],test[]");
    }

    #[test]
    fn vec_selector_codeable() {
        let mut buffer = [0; 20];
        let mut offset = 0;
        let mut is_first = true;

        Vec::<Test>::append_to_selector(&mut buffer, &mut offset, &mut is_first).unwrap();
        assert!(!is_first);
        assert_eq!(offset, 6);
        let s = str::from_utf8(&buffer)
            .unwrap()
            .trim_end_matches(char::from(0));
        assert_eq!(s, "test[]");

        Vec::<Test>::append_to_selector(&mut buffer, &mut offset, &mut is_first).unwrap();
        assert!(!is_first);
        assert_eq!(offset, 13);
        let s = str::from_utf8(&buffer)
            .unwrap()
            .trim_end_matches(char::from(0));
        assert_eq!(s, "test[],test[]");
    }
}
