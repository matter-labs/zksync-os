use crate::{
    oracle::{
        usize_serialization::{UsizeDeserializable, UsizeSerializable},
        IOOracle,
    },
    system::errors::internal::InternalError,
};

///
/// Convenience trait to define all expected types under one umbrella.
///
pub trait SimpleOracleQuery: Sized {
    const QUERY_ID: u32;
    type Input: UsizeSerializable + UsizeDeserializable;
    type Output: UsizeDeserializable;

    fn get<O: IOOracle>(
        oracle: &mut O,
        input: &Self::Input,
    ) -> Result<Self::Output, InternalError> {
        oracle.query_serializable(Self::QUERY_ID, input)
    }

    /// # Safety
    /// Callee must have apriori way to assume type equality
    unsafe fn transmute_input_ref_unchecked<'a, T: Sized + 'a>(val: &'a T) -> &'a Self::Input
    where
        Self::Input: 'a,
    {
        core::mem::transmute(val)
    }

    /// # Safety
    /// Callee must have apriori way to assume type equality. Will check type IDs inside just in case
    unsafe fn transmute_input_ref<'a, T: 'static + Sized>(val: &'a T) -> &'a Self::Input
    where
        Self::Input: 'static,
    {
        assert_eq!(
            core::any::TypeId::of::<T>(),
            core::any::TypeId::of::<Self::Input>()
        );
        core::mem::transmute(val)
    }

    // Copy == no Drop for now
    /// # Safety
    /// Callee must have apriori way to assume type equality. Will check type IDs inside just in case
    unsafe fn transmute_input<T: 'static + Sized + Copy>(val: T) -> Self::Input
    where
        Self::Input: 'static,
    {
        assert!(core::mem::needs_drop::<T>() == false);
        assert_eq!(
            core::any::TypeId::of::<T>(),
            core::any::TypeId::of::<Self::Input>()
        );
        core::ptr::read((&val as *const T).cast::<Self::Input>())
    }

    /// # Safety
    /// Callee must have apriori way to assume type equality. Will check type IDs inside just in case
    unsafe fn transmute_output<T: 'static + Sized>(val: Self::Output) -> T
    where
        Self::Output: 'static,
    {
        assert!(core::mem::needs_drop::<Self::Output>() == false);
        assert_eq!(
            core::any::TypeId::of::<T>(),
            core::any::TypeId::of::<Self::Output>()
        );
        core::ptr::read((&val as *const Self::Output).cast::<T>())
    }

    /// # Safety
    /// Callee must have apriori way to assume type equality
    unsafe fn transmute_output_unchecked<T: Sized>(val: Self::Output) -> T {
        assert!(core::mem::needs_drop::<Self::Output>() == false);
        core::ptr::read((&val as *const Self::Output).cast::<T>())
    }
}
