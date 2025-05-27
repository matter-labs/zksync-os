use core::hint::unreachable_unchecked;

use super::base_parser::RuntimeSource;
use crate::parsers::{IWasmBaseSourceParser, IWasmParser};
use crate::types::*;

#[derive(Clone, Copy, Debug)]
pub struct RuntimeParser<'src> {
    inner: RuntimeSource<'src>,
}

impl<'src> RuntimeParser<'src> {
    pub const fn new(bytecode: &'src [u8]) -> Self {
        Self {
            inner: RuntimeSource { inner: bytecode },
        }
    }
}

impl<'src> IWasmParser<RuntimeSource<'src>> for RuntimeParser<'src> {
    fn inner_ref(&self) -> &RuntimeSource<'src> {
        &self.inner
    }

    fn inner(&mut self) -> &mut RuntimeSource<'src> {
        &mut self.inner
    }

    fn remaining_len(&self) -> usize {
        self.inner.remaining_len()
    }

    fn skip_bytes(&mut self, num_bytes_to_skip: usize) -> Result<(), Self::Error> {
        self.inner.skip_bytes(num_bytes_to_skip)
    }

    fn create_subparser(&mut self, source_len: usize) -> Result<Self, Self::Error> {
        let subparser_inner = self.inner.create_subparser(source_len)?;

        Ok(Self {
            inner: subparser_inner,
        })
    }

    fn parse_section_data(&mut self) -> Result<(SectionType, usize), Self::Error> {
        let section_byte = self.inner.read_byte()?;
        let section_type = SectionType::from_byte(section_byte);
        let section_len = self.inner.parse_leb_u32()?;

        Ok((section_type, section_len as usize))
    }

    fn parse_function_type<T: ValueTypeVec>(&mut self) -> Result<FunctionType<T>, Self::Error> {
        let inputs = self.inner().parse_value_type_vec()?;
        let outputs = self.inner().parse_value_type_vec()?;

        Ok(FunctionType { inputs, outputs })
    }

    fn parse_type_section_element<T: ValueTypeVec>(
        &mut self,
    ) -> Result<FunctionType<T>, Self::Error> {
        let _type_type = self.inner.read_byte()?;
        self.parse_function_type()
    }

    fn parse_limit(&mut self) -> Result<Limits, Self::Error> {
        let limit_type = self.inner.read_byte()?;
        match limit_type {
            Limits::MIN_ONLY_ENCODING => {
                let min = self.inner.parse_leb_u32()?;
                Ok(Limits::MinOnly { min })
            }
            Limits::MIN_MAX_ENCODING => {
                let min = self.inner.parse_leb_u32()?;
                let max = self.inner.parse_leb_u32()?;
                Ok(Limits::MinMax { min, max })
            }
            _ => unsafe { unreachable_unchecked() },
        }
    }

    fn parse_memory_limit(&mut self) -> Result<MemoryLimits, Self::Error> {
        let limit_type = self.inner.read_byte()?;
        match limit_type {
            MemoryLimits::MIN_ONLY_ENCODING => {
                let min = self.inner.parse_leb_u32()?;
                Ok(MemoryLimits::MinOnly { min: min as u16 })
            }
            MemoryLimits::MIN_MAX_ENCODING => {
                let min = self.inner.parse_leb_u32()?;
                let max = self.inner.parse_leb_u32()?;
                Ok(MemoryLimits::MinMax {
                    min: min as u16,
                    max: max as u16,
                })
            }
            _ => unsafe { unreachable_unchecked() },
        }
    }
    fn parse_global_type(&mut self) -> Result<GlobalType, Self::Error> {
        let value_type = self.inner.parse_value_type()?;
        let mutability = self.inner.read_byte()?;
        let is_mutable = match mutability {
            0x00 => false,
            0x01 => true,
            _ => unsafe { unreachable_unchecked() },
        };

        Ok(GlobalType {
            value_type,
            is_mutable,
        })
    }

    fn parse_global_decl(
        &mut self,
        num_imported_globals: u32,
        func_defs: &[FunctionDef],
        globals: &[GlobalType],
    ) -> Result<GlobalDecl, Self::Error> {
        let global_type = self.parse_global_type()?;
        let const_expr =
            self.parse_constant_expression(func_defs.len() as u16, num_imported_globals)?;
        match const_expr.simple_value_type_match(global_type.value_type) {
            Ok(matches) => {
                if !matches {
                    unsafe { unreachable_unchecked() }
                }
                if let ConstantExpression::FuncRef(func_idx) = const_expr {
                    if func_idx as usize >= func_defs.len() {
                        unsafe { unreachable_unchecked() }
                    }
                }
            }
            Err(()) => match const_expr {
                ConstantExpression::Global(global_idx) => {
                    // we know that we reference only imported global
                    let other_global = unsafe { globals.get_unchecked(global_idx as usize) };
                    if other_global.value_type != global_type.value_type {
                        unsafe { unreachable_unchecked() }
                    }
                    if other_global.is_mutable {
                        unsafe { unreachable_unchecked() }
                    }
                }
                _ => unsafe { unreachable_unchecked() },
            },
        }

        let result = GlobalDecl {
            global_type,
            value: const_expr,
        };

        Ok(result)
    }

    fn parse_constant_expression(
        &mut self,
        _declared_functions: u16,
        _imported_globals: u32,
    ) -> Result<ConstantExpression, Self::Error> {
        let may_be_opcode = self.inner.read_byte()?;
        let expr = match may_be_opcode {
            0x41 => {
                // i32 const
                ConstantExpression::I32(self.inner.parse_leb_s32()?)
            }
            0x42 => {
                // i64 const
                ConstantExpression::I64(self.inner.parse_leb_s64()?)
            }
            0xd0 => {
                // ref.null
                let _value_type = self.inner.parse_value_type()?;
                ConstantExpression::RefNull
            }
            0xd2 => {
                let func_idx = self.inner.parse_leb_u32()?;
                ConstantExpression::FuncRef(func_idx as u16)
            }
            0x23 => {
                let global_idx = self.inner.parse_leb_u32()?;
                ConstantExpression::Global(global_idx)
            }
            _ => unsafe { unreachable_unchecked() },
        };

        let may_be_opcode = self.inner.read_byte()?;

        if may_be_opcode == 0x0b {
            // end
            Ok(expr)
        } else {
            unsafe { unreachable_unchecked() }
        }
    }

    fn parse_import_type<'a>(&mut self) -> Result<PartialImportRecord<'a>, Self::Error>
    where
        Self: 'a,
    {
        let str_enc_len = self.inner.parse_leb_u32()?;
        let may_be_str = self.inner.read_slice(str_enc_len)?;
        let module = unsafe { core::str::from_utf8_unchecked(may_be_str) };

        let str_enc_len = self.inner.parse_leb_u32()?;
        let may_be_str = self.inner.read_slice(str_enc_len)?;
        let name = unsafe { core::str::from_utf8_unchecked(may_be_str) };

        let import_type = self.inner.read_byte()?;
        let import_type = ImportDescriptionType::from_byte(import_type);

        let import_type = match import_type {
            ImportDescriptionType::Function => {
                let type_idx = self.inner.parse_leb_u32()?;
                let type_idx = type_idx as u16;
                let function_def = FunctionDef {
                    abi_index: type_idx,
                };

                ImportType::Function { def: function_def }
            }
            ImportDescriptionType::Table => {
                let value_type = self.inner.parse_value_type()?;
                let limits = self.parse_limit()?;

                ImportType::Table {
                    table_type: value_type,
                    limits,
                }
            }
            ImportDescriptionType::Memory => {
                let limits = self.parse_memory_limit()?;

                ImportType::Memory { limits }
            }
            ImportDescriptionType::Global => {
                let global = self.parse_global_type()?;

                ImportType::Global {
                    global_type: global,
                }
            }
            ImportDescriptionType::Unsupported => unsafe { unreachable_unchecked() },
        };

        Ok(PartialImportRecord {
            module,
            name,
            import_type,
        })
    }

    fn parse_blocktype(&mut self) -> Result<BlockType, Self::Error> {
        let mut tmp_self = *self;
        let first_byte = tmp_self.inner.read_byte()?;
        match first_byte {
            0x40 => {
                self.inner = tmp_self.inner;
                let blocktype = BlockType::Empty;
                return Ok(blocktype);
            }
            0x6f | 0x70 | 0x7b | 0x7c | 0x7d | 0x7e | 0x7f => {
                let single_value_type = ValueType::from_byte(first_byte);
                self.inner = tmp_self.inner;
                if single_value_type != ValueType::Unsupported {
                    return Ok(BlockType::ValueType(single_value_type));
                } else {
                    unsafe { unreachable_unchecked() }
                }
            }
            _ => {}
        }

        // otherwise go multivalue
        let type_idx = self.inner.parse_leb_s33()?;
        Ok(BlockType::TypeIdx(type_idx as u32))
    }

    fn parse_export_type<'a>(&mut self) -> Result<ExportRecord<'a>, Self::Error>
    where
        Self: 'a,
    {
        let str_enc_len = self.inner.parse_leb_u32()?;
        let may_be_str = self.inner.read_slice(str_enc_len)?;
        let name = unsafe { core::str::from_utf8_unchecked(may_be_str) };
        let export_type = self.inner.read_byte()?;
        let export_type = ExportDescriptionType::from_byte(export_type);
        let index = self.inner.parse_leb_u32()?;
        let index = index as u16;

        Ok(ExportRecord {
            name,
            export_type,
            abstract_index: index,
        })
    }

    fn parse_local_decl(&mut self) -> Result<LocalDecl, Self::Error> {
        let num_elements = self.inner.parse_leb_u32()?;
        let value_type = self.inner.parse_value_type()?;

        let result = LocalDecl {
            elements: num_elements as u16,
            value_type,
        };

        Ok(result)
    }

    fn parse_i32_constant_expression(&mut self) -> Result<u32, Self::Error> {
        let may_be_opcode = self.inner.read_byte()?;
        let expr = match may_be_opcode {
            0x41 => {
                // i32 const
                self.inner.parse_leb_u32()?
            }
            _ => unsafe { unreachable_unchecked() },
        };

        let may_be_opcode = self.inner.read_byte()?;

        if may_be_opcode == 0x0b {
            Ok(expr)
        } else {
            unsafe { unreachable_unchecked() }
        }
    }
}
