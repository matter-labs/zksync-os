use super::*;
use crate::impls::u256_to_usize_checked;
use ruint::aliases::U256;

#[derive(Clone, Copy)]
pub struct Array<'a, T: 'a, const N: usize> {
    _marker: core::marker::PhantomData<&'a T>,
}

pub(crate) const fn format_short_integer(
    buffer: &mut [u8],
    offset: &mut usize,
    integer: usize,
) -> Result<(), ()> {
    assert!(integer <= u8::MAX as usize);
    // at most 3 digits
    let mut integer = integer;
    let mut i = 0;
    let mut divisor = 100;
    while i < 3 {
        if integer != 0 {
            let q = integer / divisor;
            let ascii_byte = [(q as u8) + b"0"[0]];
            let char = unsafe { core::str::from_utf8_unchecked(&ascii_byte) };
            match append_ascii_str(buffer, offset, char) {
                Ok(_) => {}
                Err(_) => {
                    return Err(());
                }
            }
        }
        integer %= divisor;
        divisor /= 10;

        i += 1;
    }

    Ok(())
}

impl<'a, T: SelectorCodable + 'a, const N: usize> SelectorCodable for Array<'a, T, N> {
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

        match format_short_integer(buffer, offset, N) {
            Ok(_) => {}
            Err(_) => {
                return Err(());
            }
        }

        Ok(())
    }
}

impl<'a, T: SolidityCodable + 'a, const N: usize> SolidityCodable for Array<'a, T, N> {
    type ReflectionRef<'b>
        = ArrayRef<'b, T, N>
    where
        Self: 'b;
    type ReflectionRefMut<'b>
        = ArrayRefMut<'b, T, N>
    where
        Self: 'b;

    const IS_DYNAMIC: bool = T::IS_DYNAMIC;
    const HEAD_SIZE: usize = T::HEAD_SIZE * N;
}

#[derive(Clone, Copy)]
pub struct ArrayRef<'a, T: SolidityCodable + 'a, const N: usize> {
    source: &'a [u8],
    _marker: core::marker::PhantomData<T>,
}

pub struct ArrayRefMut<'a, T: SolidityCodable + 'a, const N: usize> {
    source: &'a mut [u8],
    _marker: core::marker::PhantomData<T>,
}

impl<'a, T: SolidityCodable + 'a, const N: usize> SolidityCodableReflectionRef<'a>
    for ArrayRef<'a, T, N>
{
    fn parse(source: &'a [u8], head_offset: &mut usize) -> Result<Self, ()> {
        let (_, local_head) = source.split_at_checked(*head_offset).ok_or(())?;
        if local_head.len() < 32 {
            return Err(());
        }
        let array_source = if T::IS_DYNAMIC {
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

            body
        } else {
            todo!()
        };

        let new = Self {
            source: array_source,
            _marker: core::marker::PhantomData,
        };
        *head_offset += 32;

        Ok(new)
    }
}

impl<'a, T: SolidityCodable + 'a, const N: usize> SolidityCodableReflectionRefMut<'a>
    for ArrayRefMut<'a, T, N>
{
    fn parse_mut(source: &'a mut [u8], head_offset: &mut usize) -> Result<Self, ()> {
        let (_, local_head) = source.split_at_mut_checked(*head_offset).ok_or(())?;
        if local_head.len() < 32 {
            return Err(());
        }
        let array_source = if T::IS_DYNAMIC {
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

            body
        } else {
            todo!()
        };

        let new = Self {
            source: array_source,
            _marker: core::marker::PhantomData,
        };
        *head_offset += 32;

        Ok(new)
    }
}

impl<'a, T: SolidityCodable + 'a, const N: usize> SolidityCodableReflectionRefAddressable<'a>
    for ArrayRef<'a, T, N>
{
    type Value = T::ReflectionRef<'a>;

    fn get(&self, index: usize) -> Result<Self::Value, ()> {
        if index >= N {
            return Err(());
        }
        let mut local_head_offset = index * T::HEAD_SIZE;
        let el = T::ReflectionRef::<'a>::parse(self.source, &mut local_head_offset)?;

        Ok(el)
    }

    fn len(&self) -> usize {
        N
    }
}

impl<'a, T: SolidityCodable + 'a, const N: usize> SolidityCodableReflectionRefAddressable<'a>
    for ArrayRefMut<'a, T, N>
{
    type Value = T::ReflectionRef<'a>;

    fn get(&'a self, index: usize) -> Result<Self::Value, ()> {
        if index >= N {
            return Err(());
        }
        let mut local_head_offset = index * T::HEAD_SIZE;
        use core::borrow::Borrow;
        let el = T::ReflectionRef::<'a>::parse(self.source.borrow(), &mut local_head_offset)?;

        Ok(el)
    }

    fn len(&self) -> usize {
        N
    }
}

impl<'a, T: SolidityCodable + 'a, const N: usize> SolidityCodableReflectionRefAddressableMut<'a>
    for ArrayRefMut<'a, T, N>
{
    type ValueMut = T::ReflectionRefMut<'a>;
    fn get_mut(&'a mut self, index: usize) -> Result<Self::ValueMut, ()> {
        if index >= N {
            return Err(());
        }
        let mut local_head_offset = index * T::HEAD_SIZE;
        let el = T::ReflectionRefMut::<'a>::parse_mut(self.source, &mut local_head_offset)?;

        Ok(el)
    }
}

#[cfg(test)]
mod tests {
    use core::str;

    use super::{Array, SelectorCodable};

    struct Test;

    impl SelectorCodable for Test {
        const CANONICAL_IDENT: &'static str = "test";
    }

    #[test]
    fn array_selector_codeable() {
        let mut buffer = [0; 20];
        let mut offset = 0;
        let mut is_first = true;

        Array::<Test, 1>::append_to_selector(&mut buffer, &mut offset, &mut is_first).unwrap();
        assert!(!is_first);
        assert_eq!(offset, 7);
        let s = str::from_utf8(&buffer)
            .unwrap()
            .trim_end_matches(char::from(0));
        assert_eq!(s, "test001");

        Array::<Test, 42>::append_to_selector(&mut buffer, &mut offset, &mut is_first).unwrap();
        assert!(!is_first);
        assert_eq!(offset, 15);
        let s = str::from_utf8(&buffer)
            .unwrap()
            .trim_end_matches(char::from(0));
        assert_eq!(s, "test001,test042");
    }
}
