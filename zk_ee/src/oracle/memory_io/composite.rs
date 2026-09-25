//! Proxies for composite inputs and outputs: tuples of two to four values continuous in memory.

use super::continuous::{exposed_address, ContinuousDeserializable, ContinuousSerializable};
use super::MemoryOracle;
use crate::system::errors::internal::InternalError;
use core::mem::MaybeUninit;

/// A composite query input: a tuple of two to four references to [`ContinuousSerializable`] values.
///
/// The oracle gets the address of an array holding the addresses of the elements, in order
/// (`[usize; N]`), and reads each element through its address.
pub trait CompositeSerializable: Copy {
    /// Calls `f` with the address of the array of element addresses. The array lives until `f` returns.
    fn with_address<R>(self, f: impl FnOnce(usize) -> R) -> R;
}

/// A composite query output: a tuple of two to four destinations of [`ContinuousDeserializable`] values.
///
/// The response is the concatenation of the elements: each one is written memcpy-like and validated in
/// turn, and the first invalid element fails the whole output.
pub trait CompositeDeserializable<'a> {
    /// References to the validated elements.
    type Initialized;

    fn init_from<O: MemoryOracle>(self, oracle: &mut O)
        -> Result<Self::Initialized, InternalError>;
}

macro_rules! impl_composite {
    ($($element:ident $index:tt),+) => {
        impl<$($element: ContinuousSerializable),+> CompositeSerializable for ($(&$element,)+) {
            #[inline(always)]
            fn with_address<R>(self, f: impl FnOnce(usize) -> R) -> R {
                let addresses = [$(exposed_address(self.$index)),+];
                f(core::ptr::from_ref(&addresses).expose_provenance())
            }
        }

        impl<'a, $($element: ContinuousDeserializable),+> CompositeDeserializable<'a>
            for ($(&'a mut MaybeUninit<$element>,)+)
        {
            type Initialized = ($(&'a mut $element,)+);

            #[inline(always)]
            fn init_from<O: MemoryOracle>(
                self,
                oracle: &mut O,
            ) -> Result<Self::Initialized, InternalError> {
                // tuple expressions are evaluated left to right, i.e. in the order of the response
                Ok(($(oracle.init(self.$index)?,)+))
            }
        }
    };
}

impl_composite!(A 0, B 1);
impl_composite!(A 0, B 1, C 2);
impl_composite!(A 0, B 1, C 2, D 3);
