use super::*;
use alloc::vec::Vec;
use core::alloc::Allocator;

use crate::parsers::IWasmBaseSourceParser;

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValueType {
    Unsupported = 0,
    FormalUnknown = 1,
    ExternRef = 0x6f,
    FuncRef = 0x70,
    I32 = 0x7f,
    I64 = 0x7e,
}

impl ValueType {
    pub const EXTERN_REF: u8 = Self::ExternRef as u8;
    pub const FUNC_REF: u8 = Self::FuncRef as u8;
    pub const I32_CONST: u8 = Self::I32 as u8;
    pub const I64_CONST: u8 = Self::I64 as u8;


    pub const fn from_byte(value: u8) -> Self {
        match value {
            ValueType::EXTERN_REF => ValueType::ExternRef,
            ValueType::FUNC_REF => ValueType::FuncRef,
            ValueType::I32_CONST => ValueType::I32,
            ValueType::I64_CONST => ValueType::I64,
            _ => ValueType::Unsupported,
        }
    }

    pub const fn from_byte_unchecked(value: u8) -> Self {
        match value {
            ValueType::EXTERN_REF => ValueType::ExternRef,
            ValueType::FUNC_REF => ValueType::FuncRef,
            ValueType::I32_CONST => ValueType::I32,
            ValueType::I64_CONST => ValueType::I64,
            _ => unsafe { core::hint::unreachable_unchecked() },
        }
    }

    // pub const fn fuzzy_equal(&self, other: &Self) -> bool {
    //     match (self, other) {
    //         (ValueType::FormalUnknown, _) | (_, ValueType::FormalUnknown) => true,
    //         (a, b) => *a as u8 == *b as u8,
    //     }
    // }

    pub const fn is_ref_type(&self) -> bool {
        matches!(self, ValueType::ExternRef | ValueType::FuncRef)
    }
}

pub trait ValueTypeVec: Clone + core::fmt::Debug {
    #[allow(clippy::result_unit_err)]
    fn parse_from_source<S: IWasmBaseSourceParser>(src: &mut S) -> Result<Self, ()>;
    fn as_ref(&self) -> &[ValueType];
}

pub const MAX_NUM_TYPES: usize = 0x40 - 1;

#[allow(clippy::len_without_is_empty)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ValueTypeVecRef<'a> {
    pub types: &'a [ValueType],
}

impl ValueTypeVecRef<'_> {
    pub const fn len(&self) -> usize {
        self.types.len()
    }

    pub const fn empty() -> Self {
        Self { types: &[] }
    }
}

impl ValueTypeVec for ValueTypeVecRef<'_> {
    fn parse_from_source<S: IWasmBaseSourceParser>(src: &mut S) -> Result<Self, ()> {
        let num_inputs = src.parse_leb_u32().map_err(|_| ())?;
        if num_inputs as usize > MAX_NUM_TYPES {
            return Err(());
        }
        let inputs_slice = src.read_slice(num_inputs).map_err(|_| ())?;
        for src in inputs_slice.iter().copied() {
            let value_type = ValueType::from_byte(src);
            if value_type == ValueType::Unsupported || value_type == ValueType::FormalUnknown {
                return Err(());
            }
        }
        let types = unsafe { core::mem::transmute::<&[u8], &[ValueType]>(inputs_slice) };

        let result = Self { types };

        Ok(result)
    }
    fn as_ref(&self) -> &[ValueType] {
        self.types
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValueTypeArray {
    pub types: arrayvec::ArrayVec<ValueType, MAX_NUM_TYPES>,
}

impl ValueTypeArray {
    pub fn from_type_vec<T: ValueTypeVec>(src: &T) -> Self {
        let mut result = arrayvec::ArrayVec::<ValueType, MAX_NUM_TYPES>::new();
        result
            .try_extend_from_slice(src.as_ref())
            .expect("must have enough capacity");

        Self { types: result }
    }
}

impl ValueTypeVec for ValueTypeArray {
    fn parse_from_source<S: IWasmBaseSourceParser>(src: &mut S) -> Result<Self, ()> {
        let num_inputs = src.parse_leb_u32().map_err(|_| ())?;
        if num_inputs as usize > MAX_NUM_TYPES {
            return Err(());
        }
        let inputs_slice = src.read_slice(num_inputs).map_err(|_| ())?;

        for src in inputs_slice.iter().copied() {
            let value_type = ValueType::from_byte(src);
            if value_type == ValueType::Unsupported || value_type == ValueType::FormalUnknown {
                return Err(());
            }
        }
        let mut result = arrayvec::ArrayVec::<ValueType, MAX_NUM_TYPES>::new();
        let types = unsafe { core::mem::transmute::<&[u8], &[ValueType]>(inputs_slice) };
        result
            .try_extend_from_slice(types)
            .expect("must have enough capacity");

        let result = Self { types: result };

        Ok(result)
    }
    fn as_ref(&self) -> &[ValueType] {
        &self.types[..]
    }
}

impl<A: Allocator + Clone + Default> ValueTypeVec for Vec<ValueType, A>
where
    A: core::fmt::Debug,
{
    fn parse_from_source<S: IWasmBaseSourceParser>(src: &mut S) -> Result<Self, ()> {
        let num_inputs = src.parse_leb_u32().map_err(|_| ())?;
        if num_inputs as usize > MAX_NUM_TYPES {
            return Err(());
        }
        let inputs_slice = src.read_slice(num_inputs).map_err(|_| ())?;

        for src in inputs_slice.iter().copied() {
            let value_type = ValueType::from_byte(src);
            if value_type == ValueType::Unsupported || value_type == ValueType::FormalUnknown {
                return Err(());
            }
        }
        let mut result = Vec::with_capacity_in(num_inputs as usize, A::default());
        let types = unsafe { core::mem::transmute::<&[u8], &[ValueType]>(inputs_slice) };
        result.extend_from_slice(types);

        Ok(result)
    }
    fn as_ref(&self) -> &[ValueType] {
        &self[..]
    }
}

#[derive(Clone, Copy, Debug)]
pub struct LocalDecl {
    pub elements: u16,
    pub value_type: ValueType,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GlobalType {
    pub value_type: ValueType,
    pub is_mutable: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GlobalDecl {
    pub global_type: GlobalType,
    pub value: ConstantExpression,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockType {
    Empty,
    ValueType(ValueType),
    TypeIdx(u32),
}

const _: () = const {
    assert!(core::mem::size_of::<ValueType>() == core::mem::size_of::<u8>());
    assert!(core::mem::align_of::<ValueType>() == core::mem::align_of::<u8>());
};
