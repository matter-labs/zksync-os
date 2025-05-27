use alloc::vec::Vec;
use core::alloc::Allocator;

use super::*;

pub trait LocalDeclsVec: Clone + core::fmt::Debug {
    type ConstructorAux;
    fn new_in(capacity: usize, aux: Self::ConstructorAux) -> Self;
    fn push(&mut self, local_decl: LocalDecl);
    fn as_ref(&self) -> &[LocalDecl];
}

impl<A: Allocator + Clone> LocalDeclsVec for Vec<LocalDecl, A> {
    type ConstructorAux = A;

    fn new_in(capacity: usize, aux: Self::ConstructorAux) -> Self {
        Vec::with_capacity_in(capacity, aux)
    }
    fn push(&mut self, local_decl: LocalDecl) {
        Vec::push(self, local_decl);
    }
    fn as_ref(&self) -> &[LocalDecl] {
        &self[..]
    }
}

#[derive(Clone, Debug)]
pub struct FunctionBody<A: Allocator> {
    pub function_def_idx: u32,
    pub instruction_pointer: u32,
    pub end_instruction_pointer: u32,
    // total locals declared in the function itself (not from the ABI)
    pub total_locals: u32,
    pub locals: Vec<LocalDecl, A>,
    pub initial_sidetable_idx: u32,
}

impl<A: Allocator> FunctionBody<A> {
    #[allow(clippy::result_unit_err)]
    pub fn get_input_for_inner_index(&self, index: usize) -> Result<ValueType, ()> {
        // `elements` field encodes max local index for that type, so we just need to find the first one
        // where we fall in range
        for decl in self.locals.iter() {
            if index < decl.elements as usize {
                return Ok(decl.value_type);
            }
        }

        Err(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FunctionType<T: ValueTypeVec> {
    pub inputs: T,
    pub outputs: T,
}

impl FunctionType<ValueTypeArray> {
    pub fn from_other_type<T: ValueTypeVec>(src: &FunctionType<T>) -> Self {
        let inputs = ValueTypeArray::from_type_vec(&src.inputs);
        let outputs = ValueTypeArray::from_type_vec(&src.outputs);

        Self { inputs, outputs }
    }
}

// #[derive(Clone, Copy, Debug, PartialEq, Eq)]
// pub struct FunctionTypeRef<'a> {
//     pub inputs: ValueTypeVecRef<'a>,
//     pub outputs: ValueTypeVecRef<'a>,
// }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FunctionDef {
    pub abi_index: u16,
}

#[derive(Clone, Debug)]
pub struct FunctionName {
    pub name: &'static str,
}

// #[derive(Clone, Debug)]
// pub struct FunctionTypeOwned<A: Allocator> {
//     pub inputs: Vec<ValueType, A>,
//     pub outputs: Vec<ValueType, A>,
// }

// impl<'a, A: Allocator> PartialEq<FunctionTypeRef<'a>> for FunctionTypeOwned<A> {
//     fn eq(&self, other: &FunctionTypeRef<'a>) -> bool {
//         &self.inputs[..] == other.inputs.types && &self.outputs[..] == other.outputs.types
//     }
// }
