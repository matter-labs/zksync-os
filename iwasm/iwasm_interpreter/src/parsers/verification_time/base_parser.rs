use crate::parsers::IWasmBaseSourceParser;
use crate::types::*;

#[derive(Clone, Copy, Debug)]
pub struct VerificationTimeSource<'src> {
    pub(crate) inner: &'src [u8],
}

use crate::leb128::LEB128;

impl VerificationTimeSource<'_> {
    fn advance_unchecked(&mut self, by: usize) {
        let (_b, rest) = unsafe { self.inner.split_at_unchecked(by) };
        self.inner = rest;
    }
}

impl IWasmBaseSourceParser for VerificationTimeSource<'_> {
    type Error = ();

    type StartMark = *const u8;
    fn get_start_mark(&self) -> Self::StartMark {
        self.inner.as_ptr()
    }
    unsafe fn absolute_offset(&self, start: Self::StartMark) -> u32 {
        self.inner.as_ptr().offset_from(start) as usize as u32
    }

    fn read_slice<'a>(&mut self, len: u32) -> Result<&'a [u8], Self::Error>
    where
        Self: 'a,
    {
        if self.inner.len() < len as usize {
            return Err(());
        }
        let (b, rest) = unsafe { self.inner.split_at_unchecked(len as usize) };
        self.inner = rest;

        Ok(b)
    }

    fn read_byte(&mut self) -> Result<u8, Self::Error> {
        if self.inner.is_empty() {
            return Err(());
        }
        let (b, rest) = unsafe { self.inner.split_at_unchecked(1) };
        self.inner = rest;

        Ok(b[0])
    }

    fn parse_value_type(&mut self) -> Result<ValueType, Self::Error> {
        let byte = self.read_byte()?;
        let value_type = ValueType::from_byte(byte);
        if value_type != ValueType::Unsupported && value_type != ValueType::FormalUnknown {
            Ok(value_type)
        } else {
            Err(())
        }
    }

    fn parse_block_type(&mut self) -> Result<BlockType, Self::Error> {
        let mut tmp_self = *self;
        let first_byte = tmp_self.read_byte()?;
        match first_byte {
            0x40 => {
                self.inner = tmp_self.inner;
                let blocktype = BlockType::Empty;
                return Ok(blocktype);
            }
            0x6f | 0x70 | 0x7b | 0x7c | 0x7d | 0x7e | 0x7f => {
                let value_type = self.parse_value_type()?;
                return Ok(BlockType::ValueType(value_type));
            }
            _ => {}
        }

        // otherwise go multivalue
        let type_idx = self.parse_leb_s33()?;
        if type_idx < 0 {
            return Err(());
        }

        if type_idx > u32::MAX as i64 {
            return Err(());
        }

        Ok(BlockType::TypeIdx(type_idx as u32))
    }

    fn parse_value_type_vec<T: ValueTypeVec>(&mut self) -> Result<T, Self::Error> {
        let result = T::parse_from_source(self)?;

        Ok(result)
    }

    fn parse_function_type_ref<T: ValueTypeVec>(&mut self) -> Result<FunctionType<T>, Self::Error> {
        let inputs = self.parse_value_type_vec()?;
        let outputs = self.parse_value_type_vec()?;

        Ok(FunctionType { inputs, outputs })
    }

    fn parse_u32_fixed(&mut self) -> Result<u32, Self::Error> {
        if self.inner.len() < 4 {
            return Err(());
        }
        let (b, rest) = unsafe { self.inner.split_at_unchecked(4) };
        self.inner = rest;

        let mut buff = [0u8; 4];
        buff.copy_from_slice(b);

        Ok(u32::from_le_bytes(buff))
    }

    fn parse_leb_u32(&mut self) -> Result<u32, Self::Error> {
        match LEB128::consume_decode_u32(self.inner) {
            Ok((value, consumed)) => {
                self.advance_unchecked(consumed);
                Ok(value)
            }
            Err(consumed) => {
                self.advance_unchecked(consumed);
                Err(())
            }
        }
    }
    fn parse_leb_s32(&mut self) -> Result<i32, Self::Error> {
        match LEB128::consume_decode_s32(self.inner) {
            Ok((value, consumed)) => {
                self.advance_unchecked(consumed);
                Ok(value)
            }
            Err(consumed) => {
                self.advance_unchecked(consumed);
                Err(())
            }
        }
    }
    fn parse_leb_u64(&mut self) -> Result<u64, Self::Error> {
        match LEB128::consume_decode_u64(self.inner) {
            Ok((value, consumed)) => {
                self.advance_unchecked(consumed);
                Ok(value)
            }
            Err(consumed) => {
                self.advance_unchecked(consumed);
                Err(())
            }
        }
    }
    fn parse_leb_s64(&mut self) -> Result<i64, Self::Error> {
        match LEB128::consume_decode_s64(self.inner) {
            Ok((value, consumed)) => {
                self.advance_unchecked(consumed);
                Ok(value)
            }
            Err(consumed) => {
                self.advance_unchecked(consumed);
                Err(())
            }
        }
    }
    fn parse_leb_s33(&mut self) -> Result<i64, Self::Error> {
        match LEB128::consume_decode_s33(self.inner) {
            Ok((value, consumed)) => {
                self.advance_unchecked(consumed);
                Ok(value)
            }
            Err(consumed) => {
                self.advance_unchecked(consumed);
                Err(())
            }
        }
    }

    #[inline(always)]
    fn remaining_len(&self) -> usize {
        self.inner.len()
    }

    fn skip_bytes(&mut self, num_bytes_to_skip: usize) -> Result<(), Self::Error> {
        if self.inner.len() < num_bytes_to_skip {
            return Err(());
        }
        self.advance_unchecked(num_bytes_to_skip);

        Ok(())
    }

    fn create_subparser(&mut self, source_len: usize) -> Result<Self, Self::Error> {
        if self.inner.len() < source_len {
            return Err(());
        }
        let (b, rest) = unsafe { self.inner.split_at_unchecked(source_len) };
        self.inner = rest;

        Ok(Self { inner: b })
    }

    #[inline(always)]
    fn peek_end(&self) -> Option<u8> {
        self.inner.last().copied()
    }
}
