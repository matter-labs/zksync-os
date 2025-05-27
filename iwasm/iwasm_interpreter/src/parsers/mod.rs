use core::fmt::Debug;

use crate::types::*;

pub mod runtime;
pub mod verification_time;

// a trait to define a parser of base and smaller types, like ValueType, etc
pub trait IWasmBaseSourceParser: Clone + Debug {
    type Error;

    type StartMark: Clone + Copy + Debug;
    fn get_start_mark(&self) -> Self::StartMark;
    /// # Safety
    /// caller must ensure that they are from the same allocation
    unsafe fn absolute_offset(&self, start: Self::StartMark) -> u32;

    fn read_slice<'a>(&mut self, len: u32) -> Result<&'a [u8], Self::Error>
    where
        Self: 'a;
    fn read_byte(&mut self) -> Result<u8, Self::Error>;
    fn parse_value_type(&mut self) -> Result<ValueType, Self::Error>;
    fn parse_block_type(&mut self) -> Result<BlockType, Self::Error>;
    fn parse_value_type_vec<T: ValueTypeVec>(&mut self) -> Result<T, Self::Error>;
    fn parse_u32_fixed(&mut self) -> Result<u32, Self::Error>;
    fn parse_leb_u32(&mut self) -> Result<u32, Self::Error>;
    fn parse_leb_s32(&mut self) -> Result<i32, Self::Error>;
    fn parse_leb_u64(&mut self) -> Result<u64, Self::Error>;
    fn parse_leb_s64(&mut self) -> Result<i64, Self::Error>;
    fn parse_leb_s33(&mut self) -> Result<i64, Self::Error>;
    fn parse_function_type_ref<T: ValueTypeVec>(&mut self) -> Result<FunctionType<T>, Self::Error>;
    fn remaining_len(&self) -> usize;
    fn peek_end(&self) -> Option<u8>;

    #[inline]
    fn is_empty(&self) -> bool {
        self.remaining_len() == 0
    }
    fn skip_bytes(&mut self, num_bytes_to_skip: usize) -> Result<(), Self::Error>;
    fn create_subparser(&mut self, source_len: usize) -> Result<Self, Self::Error>;
}

pub trait IWasmParser<B: IWasmBaseSourceParser>: Clone + Debug {
    type Error = B::Error;

    fn inner_ref(&self) -> &B;
    fn inner(&mut self) -> &mut B;
    fn remaining_len(&self) -> usize;

    #[inline]
    fn is_empty(&self) -> bool {
        self.remaining_len() == 0
    }

    fn skip_bytes(&mut self, num_bytes_to_skip: usize) -> Result<(), Self::Error>;
    fn create_subparser(&mut self, source_len: usize) -> Result<Self, Self::Error>;

    // we have corresponding types for all structures that we need to parse
    fn parse_section_data(&mut self) -> Result<(SectionType, usize), Self::Error>;
    fn parse_function_type<T: ValueTypeVec>(&mut self) -> Result<FunctionType<T>, Self::Error>;
    fn parse_type_section_element<T: ValueTypeVec>(
        &mut self,
    ) -> Result<FunctionType<T>, Self::Error>;

    fn parse_limit(&mut self) -> Result<Limits, Self::Error>;
    fn parse_memory_limit(&mut self) -> Result<MemoryLimits, Self::Error>;
    fn parse_global_type(&mut self) -> Result<GlobalType, Self::Error>;
    fn parse_global_decl(
        &mut self,
        num_imported_globals: u32,
        func_defs: &[FunctionDef],
        globals: &[GlobalType],
    ) -> Result<GlobalDecl, Self::Error>;
    fn parse_import_type<'a>(&mut self) -> Result<PartialImportRecord<'a>, Self::Error>
    where
        Self: 'a;
    fn parse_blocktype(&mut self) -> Result<BlockType, Self::Error>;
    fn parse_constant_expression(
        &mut self,
        declared_functions: u16,
        imported_globals: u32,
    ) -> Result<ConstantExpression, Self::Error>;
    fn parse_export_type<'a>(&mut self) -> Result<ExportRecord<'a>, Self::Error>
    where
        Self: 'a;
    fn parse_local_decl(&mut self) -> Result<LocalDecl, Self::Error>;
    fn parse_i32_constant_expression(&mut self) -> Result<u32, Self::Error>;

    fn skip_blocktype(&mut self) {
        let mut tmp_self = self.clone();
        let Ok(first_byte) = tmp_self.inner().read_byte() else {
            panic!()
        };
        match first_byte {
            0x40 => {
                *self = tmp_self;
                return;
            }
            0x6f | 0x70 | 0x7b | 0x7c | 0x7d | 0x7e | 0x7f => {
                *self = tmp_self;
                return;
            }
            _ => {}
        }

        // otherwise go multivalue
        let Ok(_type_idx) = self.inner().parse_leb_s33() else {
            panic!()
        };
    }
}
