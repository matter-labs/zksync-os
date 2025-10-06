use ruint::aliases::{B160, U256};

#[derive(Clone, Copy, Debug)]
pub struct Parser<'a> {
    pub(crate) slice: &'a [u8],
}

// We only need to define 2 types - slice and list.

pub struct EncodedSlice<'a> {
    pub(crate) data: &'a [u8],
}

fn try_consume_bytes<'a>(src: &mut &'a [u8], len: usize) -> Result<&'a [u8], ()> {
    if src.len() < len {
        return Err(());
    }
    let (data, rest) = src.split_at(len);
    *src = rest;

    Ok(data)
}

fn u32_from_slice_unchecked(src: &[u8]) -> u32 {
    unsafe {
        core::hint::assert_unchecked(src.len() > 0);
        core::hint::assert_unchecked(src.len() <= 4);
    }
    let mut buffer = [0u8; 4];
    buffer[(4 - src.len())..].copy_from_slice(src);

    u32::from_be_bytes(buffer)
}

pub(crate) struct EncodedList<'a> {
    pub(crate) concatenation_data: &'a [u8],
}

impl<'a> Parser<'a> {
    pub unsafe fn pos(&self) -> *const u8 {
        self.slice.as_ptr()
    }

    pub unsafe fn consumed_slice(&self, start: *const u8) -> &'a [u8] {
        unsafe { core::slice::from_ptr_range(start..self.slice.as_ptr()) }
    }

    pub(crate) fn new(slice: &'a [u8]) -> Self {
        Self { slice }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.slice.is_empty()
    }

    pub(crate) fn try_parse_slice(&mut self) -> Result<EncodedSlice<'a>, ()> {
        let marker = try_consume_bytes(&mut self.slice, 1)?;
        let m = marker[0];
        if m < 0x80 {
            Ok(EncodedSlice { data: marker })
        } else if m <= 0x80 + 55 {
            let len = m - 0x80;
            let data = try_consume_bytes(&mut self.slice, len as usize)?;
            Ok(EncodedSlice { data })
        } else if m < 0xc0 {
            // we make some reasonable bound here - max u32 length
            let len_encoding_len = m - 0xb7;
            if len_encoding_len > 4 {
                return Err(());
            }
            let len_bytes = try_consume_bytes(&mut self.slice, len_encoding_len as usize)?;
            let len = u32_from_slice_unchecked(len_bytes);
            let data = try_consume_bytes(&mut self.slice, len as usize)?;
            Ok(EncodedSlice { data })
        } else {
            Err(())
        }
    }

    pub(crate) fn try_parse_list(&mut self) -> Result<EncodedList<'a>, ()> {
        let marker = try_consume_bytes(&mut self.slice, 1)?;
        let m = marker[0];
        if m < 0xc0 {
            // not a slice
            Err(())
        } else if m <= 0xc0 + 55 {
            let len = m - 0xc0;
            let data = try_consume_bytes(&mut self.slice, len as usize)?;
            Ok(EncodedList {
                concatenation_data: data,
            })
        } else {
            // we make some reasonable bound here - max u32 length
            let len_encoding_len = m - 0xf7;
            if len_encoding_len > 4 {
                return Err(());
            }
            let len_bytes = try_consume_bytes(&mut self.slice, len_encoding_len as usize)?;
            let len = u32_from_slice_unchecked(len_bytes);
            let data = try_consume_bytes(&mut self.slice, len as usize)?;
            Ok(EncodedList {
                concatenation_data: data,
            })
        }
    }

    pub(crate) fn try_make_list_subparser(&mut self) -> Result<Self, ()> {
        let list = self.try_parse_list()?;
        Ok(Self::new(list.concatenation_data))
    }
}

pub trait RLPParsableScalar<'a>: Sized {
    fn try_parse(src: EncodedSlice<'a>) -> Result<Self, ()>;
}

pub trait FixedLenScalar<'a>: RLPParsableScalar<'a> {
    const ENCODING_LEN: usize;
}

// we will only do small list of types

impl<'a> RLPParsableScalar<'a> for bool {
    fn try_parse(src: EncodedSlice<'a>) -> Result<Self, ()> {
        let repr: u8 = RLPParsableScalar::try_parse(src)?;
        if repr == 0 {
            Ok(false)
        } else if repr == 1 {
            Ok(true)
        } else {
            Err(())
        }
    }
}

impl<'a> RLPParsableScalar<'a> for u8 {
    fn try_parse(src: EncodedSlice<'a>) -> Result<Self, ()> {
        if src.data.len() == 0 {
            Ok(0)
        } else if src.data.len() == 1 {
            Ok(src.data[0])
        } else {
            Err(())
        }
    }
}

impl<'a> RLPParsableScalar<'a> for u64 {
    fn try_parse(src: EncodedSlice<'a>) -> Result<Self, ()> {
        if src.data.len() > 8 {
            Err(())
        } else {
            let mut buffer = [0u8; 8];
            buffer[(8 - src.data.len())..].copy_from_slice(src.data);

            Ok(u64::from_be_bytes(buffer))
        }
    }
}

impl<'a> RLPParsableScalar<'a> for &'a [u8] {
    fn try_parse(src: EncodedSlice<'a>) -> Result<Self, ()> {
        Ok(src.data)
    }
}

impl<'a> RLPParsableScalar<'a> for U256 {
    fn try_parse(src: EncodedSlice<'a>) -> Result<Self, ()> {
        U256::try_from_be_slice(src.data).ok_or(())
    }
}

impl<'a> RLPParsableScalar<'a> for B160 {
    fn try_parse(src: EncodedSlice<'a>) -> Result<Self, ()> {
        let inner: [u8; 20] = RLPParsableScalar::try_parse(src)?;
        Ok(B160::from_be_bytes(inner))
    }
}

impl<'a> FixedLenScalar<'a> for B160 {
    const ENCODING_LEN: usize = 1 + 20;
}

impl<'a, const N: usize> RLPParsableScalar<'a> for [u8; N] {
    fn try_parse(src: EncodedSlice<'a>) -> Result<Self, ()> {
        Self::try_from(src.data).map_err(|_| ())
    }
}

impl<'a, const N: usize> FixedLenScalar<'a> for [u8; N] {
    const ENCODING_LEN: usize = N + 1;
}

impl<'a, const N: usize> RLPParsableScalar<'a> for &'a [u8; N] {
    fn try_parse(src: EncodedSlice<'a>) -> Result<Self, ()> {
        Self::try_from(src.data).map_err(|_| ())
    }
}

impl<'a, const N: usize> FixedLenScalar<'a> for &'a [u8; N] {
    const ENCODING_LEN: usize = N + 1;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct RLPZeroInteger;

impl<'a> RLPParsableScalar<'a> for RLPZeroInteger {
    fn try_parse(src: EncodedSlice<'a>) -> Result<Self, ()> {
        if src.data.len() != 1 || src.data[0] != 0 {
            Err(())
        } else {
            Ok(RLPZeroInteger)
        }
    }
}

impl<'a> FixedLenScalar<'a> for RLPZeroInteger {
    const ENCODING_LEN: usize = 1;
}

pub trait RLPParsable<'a>: Sized {
    fn try_parse_slice_in_full(input: &'a [u8]) -> Result<Self, ()> {
        let mut parser = Parser::new(input);
        let new = Self::try_parse(&mut parser)?;
        if parser.is_empty() {
            Ok(new)
        } else {
            Err(())
        }
    }

    fn try_parse(parser: &mut Parser<'a>) -> Result<Self, ()>;
}

impl<'a, T: RLPParsableScalar<'a>> RLPParsable<'a> for T {
    fn try_parse(parser: &mut Parser<'a>) -> Result<Self, ()> {
        let slice = parser.try_parse_slice()?;
        RLPParsableScalar::try_parse(slice)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ListEncapsulated<'a, T: RLPParsable<'a>> {
    pub(crate) inner: T,
    _marker: core::marker::PhantomData<&'a ()>,
}

impl<'a, T: RLPParsable<'a>> ListEncapsulated<'a, T> {
    pub fn into_inner(self) -> T {
        self.inner
    }
}

impl<'a, T: RLPParsable<'a>> RLPParsable<'a> for ListEncapsulated<'a, T> {
    fn try_parse(parser: &mut Parser<'a>) -> Result<Self, ()> {
        let mut list_parser = parser.try_make_list_subparser()?;
        let inner: T = RLPParsable::try_parse(&mut list_parser)?;
        if list_parser.is_empty() == false {
            return Err(());
        }

        let new = Self {
            inner,
            _marker: core::marker::PhantomData,
        };

        Ok(new)
    }
}
