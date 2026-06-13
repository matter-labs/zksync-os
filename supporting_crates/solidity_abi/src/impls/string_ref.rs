use super::*;
use crate::impls::u256_to_usize_checked;
use ruint::aliases::U256;

#[derive(Clone, Copy)]
pub struct SolidityString<'a>(pub &'a str);

impl<'a> SelectorCodable for SolidityString<'a> {
    const CANONICAL_IDENT: &'static str = "string";
}

impl<'a> SolidityCodable for SolidityString<'a> {
    type ReflectionRef<'b>
        = SolidityStringRef<'b>
    where
        Self: 'b;
    type ReflectionRefMut<'b>
        = SolidityStringRefMut<'b>
    where
        Self: 'b;

    const IS_DYNAMIC: bool = true;
}

#[derive(Clone, Copy)]
pub struct SolidityStringRef<'a> {
    source: &'a [u8],
}

pub struct SolidityStringRefMut<'a> {
    source: &'a mut [u8],
}

impl<'a> SolidityCodableReflectionRef<'a> for SolidityStringRef<'a> {
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

impl<'a> SolidityCodableReflectionRefMut<'a> for SolidityStringRefMut<'a> {
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

impl<'a> SolidityCodableReflectionRefReadable<'a> for SolidityStringRef<'a> {
    type Value = SolidityString<'a>;

    fn read(&'a self) -> Result<Self::Value, ()> {
        let as_str = core::str::from_utf8(self.source).map_err(|_| ())?;

        Ok(SolidityString(as_str))
    }
}

impl<'a> SolidityCodableReflectionRefReadable<'a> for SolidityStringRefMut<'a> {
    type Value = SolidityString<'a>;

    fn read(&'a self) -> Result<Self::Value, ()> {
        let as_str = core::str::from_utf8(self.source).map_err(|_| ())?;

        Ok(SolidityString(as_str))
    }
}

impl<'a> SolidityCodableReflectionRefWritable<'a> for SolidityStringRefMut<'a> {
    fn write(
        &'a mut self,
        value: &'_ <Self as SolidityCodableReflectionRefReadable<'a>>::Value,
    ) -> Result<(), ()> {
        if value.0.len() != self.source.len() {
            return Err(());
        }

        self.source.copy_from_slice(value.0.as_bytes());

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::SolidityString;
    use crate::impls::SelectorCodable;
    use core::str;

    #[test]
    fn solidity_string_selector_codeable() {
        let mut buffer = [0; 20];
        let mut offset = 0;
        let mut is_first = true;

        SolidityString::append_to_selector(&mut buffer, &mut offset, &mut is_first).unwrap();
        assert!(!is_first);
        assert_eq!(offset, 6);
        let s = str::from_utf8(&buffer)
            .unwrap()
            .trim_end_matches(char::from(0));
        assert_eq!(s, "string");

        SolidityString::append_to_selector(&mut buffer, &mut offset, &mut is_first).unwrap();
        assert!(!is_first);
        assert_eq!(offset, 13);
        let s = str::from_utf8(&buffer)
            .unwrap()
            .trim_end_matches(char::from(0));
        assert_eq!(s, "string,string");
    }
}
