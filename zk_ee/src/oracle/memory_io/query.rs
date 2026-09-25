//! Typed queries: input and output types bound to a query ID.

use super::continuous::{ContinuousDeserializable, ContinuousSerializable};
use super::dynamic::DynamicDestination;
use super::MemoryOracle;
use crate::execution_environment_type::ExecutionEnvironmentType;
use crate::system::errors::internal::InternalError;
use alloc::boxed::Box;
use core::alloc::Allocator;
use core::mem::MaybeUninit;

/// The input of a query, which is one of:
/// - `()`, for no input;
/// - a [`ShortSerializable`](super::ShortSerializable) type, passed by value;
/// - a [`ContinuousSerializable`] type `T`, passed as `&T`;
/// - a tuple of two to four [`ContinuousSerializable`] types, passed as a tuple of references
///   ([`CompositeSerializable`](super::CompositeSerializable)).
pub trait QueryInput {
    /// How the querier passes the input.
    type Ref<'a>: Copy
    where
        Self: 'a;

    fn send<O: MemoryOracle>(
        oracle: &mut O,
        query_id: u32,
        input: Self::Ref<'_>,
    ) -> Result<(), InternalError>;
}

/// The output of a query, which is one of:
/// - `()`, for no output;
/// - a [`ShortDeserializable`](super::ShortDeserializable) type, returned by value;
/// - a [`ContinuousDeserializable`] type `T`, written into a `&mut MaybeUninit<T>` destination and returned
///   as `&mut T`;
/// - a tuple of two to four [`ContinuousDeserializable`] types, written into a tuple of destinations
///   ([`CompositeDeserializable`](super::CompositeDeserializable)) and returned as a tuple of references.
pub trait QueryOutput: Sized {
    /// Where the querier wants the output written.
    type Destination<'a>
    where
        Self: 'a;

    /// What the querier gets back once the output is validated.
    type Initialized<'a>
    where
        Self: 'a;

    fn receive<'a, O: MemoryOracle>(
        oracle: &mut O,
        dst: Self::Destination<'a>,
    ) -> Result<Self::Initialized<'a>, InternalError>;

    /// Receives the output as a new value.
    fn receive_value<O: MemoryOracle>(oracle: &mut O) -> Result<Self, InternalError>;
}

/// A query with a fixed-size output: the counterpart of
/// [`SimpleOracleQuery`](crate::oracle::simple_oracle_query::SimpleOracleQuery) for the memory-based
/// protocol.
pub trait OracleQuery {
    const QUERY_ID: u32;
    type Input: QueryInput;
    type Output: QueryOutput;

    /// Runs the query, and writes the output into `dst`.
    #[inline(always)]
    fn get_into<'a, O: MemoryOracle>(
        oracle: &mut O,
        input: <Self::Input as QueryInput>::Ref<'_>,
        dst: <Self::Output as QueryOutput>::Destination<'a>,
    ) -> Result<<Self::Output as QueryOutput>::Initialized<'a>, InternalError>
    where
        Self::Output: 'a,
    {
        Self::Input::send(oracle, Self::QUERY_ID, input)?;
        let output = Self::Output::receive(oracle, dst)?;
        oracle.finish_query()?;
        Ok(output)
    }

    /// Runs the query, and returns the output as a new value.
    #[inline(always)]
    fn get<O: MemoryOracle>(
        oracle: &mut O,
        input: <Self::Input as QueryInput>::Ref<'_>,
    ) -> Result<Self::Output, InternalError> {
        Self::Input::send(oracle, Self::QUERY_ID, input)?;
        let output = Self::Output::receive_value(oracle)?;
        oracle.finish_query()?;
        Ok(output)
    }
}

/// A query with a dynamically sized output of `usize` words (see [`DynamicDestination`]).
pub trait DynamicOracleQuery {
    const QUERY_ID: u32;
    type Input: QueryInput;

    /// Runs the query, writes the output into `dst`, and returns the number of `usize` words written.
    #[inline(always)]
    fn get_into<O: MemoryOracle, D: DynamicDestination>(
        oracle: &mut O,
        input: <Self::Input as QueryInput>::Ref<'_>,
        dst: D,
    ) -> Result<usize, InternalError> {
        Self::Input::send(oracle, Self::QUERY_ID, input)?;
        let len = oracle.write_dynamic(dst)?;
        oracle.finish_query()?;
        Ok(len)
    }

    /// Runs the query, and writes the output into a new allocation of at most `max_len` words.
    fn get_boxed<O: MemoryOracle, A: Allocator>(
        oracle: &mut O,
        input: <Self::Input as QueryInput>::Ref<'_>,
        max_len: usize,
        allocator: A,
    ) -> Result<Box<[usize], A>, InternalError> {
        Self::Input::send(oracle, Self::QUERY_ID, input)?;
        let output = oracle.write_dynamic_boxed(max_len, allocator)?;
        oracle.finish_query()?;
        Ok(output)
    }
}

impl QueryInput for () {
    type Ref<'a> = ();

    #[inline(always)]
    fn send<O: MemoryOracle>(
        oracle: &mut O,
        query_id: u32,
        _input: Self::Ref<'_>,
    ) -> Result<(), InternalError> {
        oracle.send_query(query_id, 0)
    }
}

impl QueryOutput for () {
    type Destination<'a> = ();
    type Initialized<'a> = ();

    #[inline(always)]
    fn receive<'a, O: MemoryOracle>(
        _oracle: &mut O,
        _dst: Self::Destination<'a>,
    ) -> Result<Self::Initialized<'a>, InternalError> {
        Ok(())
    }

    #[inline(always)]
    fn receive_value<O: MemoryOracle>(_oracle: &mut O) -> Result<Self, InternalError> {
        Ok(())
    }
}

macro_rules! impl_query_io_for_short {
    ($($t:ty),+) => {$(
        impl QueryInput for $t {
            type Ref<'a> = $t;

            #[inline(always)]
            fn send<O: MemoryOracle>(
                oracle: &mut O,
                query_id: u32,
                input: Self::Ref<'_>,
            ) -> Result<(), InternalError> {
                oracle.send_short(query_id, input)
            }
        }

        impl QueryOutput for $t {
            type Destination<'a> = ();
            type Initialized<'a> = $t;

            #[inline(always)]
            fn receive<'a, O: MemoryOracle>(
                oracle: &mut O,
                _dst: Self::Destination<'a>,
            ) -> Result<Self::Initialized<'a>, InternalError> {
                oracle.read_short()
            }

            #[inline(always)]
            fn receive_value<O: MemoryOracle>(oracle: &mut O) -> Result<Self, InternalError> {
                oracle.read_short()
            }
        }
    )+};
}

impl_query_io_for_short!(bool, u8, u16, u32, ExecutionEnvironmentType);

impl<T: ContinuousSerializable> QueryInput for T {
    type Ref<'a>
        = &'a T
    where
        T: 'a;

    #[inline(always)]
    fn send<O: MemoryOracle>(
        oracle: &mut O,
        query_id: u32,
        input: Self::Ref<'_>,
    ) -> Result<(), InternalError> {
        oracle.send_continuous(query_id, input)
    }
}

impl<T: ContinuousDeserializable> QueryOutput for T {
    type Destination<'a>
        = &'a mut MaybeUninit<T>
    where
        T: 'a;
    type Initialized<'a>
        = &'a mut T
    where
        T: 'a;

    #[inline(always)]
    fn receive<'a, O: MemoryOracle>(
        oracle: &mut O,
        dst: Self::Destination<'a>,
    ) -> Result<Self::Initialized<'a>, InternalError> {
        oracle.init(dst)
    }

    #[inline(always)]
    fn receive_value<O: MemoryOracle>(oracle: &mut O) -> Result<Self, InternalError> {
        let mut value = MaybeUninit::uninit();
        oracle.init(&mut value)?;
        // SAFETY: initialized and validated above
        Ok(unsafe { value.assume_init() })
    }
}

macro_rules! impl_query_io_for_tuple {
    ($($element:ident $index:tt),+) => {
        impl<$($element: ContinuousSerializable),+> QueryInput for ($($element,)+) {
            type Ref<'a> = ($(&'a $element,)+) where Self: 'a;

            #[inline(always)]
            fn send<O: MemoryOracle>(
                oracle: &mut O,
                query_id: u32,
                input: Self::Ref<'_>,
            ) -> Result<(), InternalError> {
                oracle.send_composite(query_id, input)
            }
        }

        impl<$($element: ContinuousDeserializable),+> QueryOutput for ($($element,)+) {
            type Destination<'a> = ($(&'a mut MaybeUninit<$element>,)+) where Self: 'a;
            type Initialized<'a> = ($(&'a mut $element,)+) where Self: 'a;

            #[inline(always)]
            fn receive<'a, O: MemoryOracle>(
                oracle: &mut O,
                dst: Self::Destination<'a>,
            ) -> Result<Self::Initialized<'a>, InternalError> {
                oracle.init_composite(dst)
            }

            #[inline(always)]
            fn receive_value<O: MemoryOracle>(oracle: &mut O) -> Result<Self, InternalError> {
                let mut value = MaybeUninit::<Self>::uninit();
                let this = value.as_mut_ptr();
                // SAFETY: the elements are disjoint places inside `value`, and `MaybeUninit<E>` has the
                // layout of `E`
                let dst = unsafe {
                    ($(&mut *(&raw mut (*this).$index).cast::<MaybeUninit<$element>>(),)+)
                };
                oracle.init_composite(dst)?;
                // SAFETY: every element was initialized and validated above
                Ok(unsafe { value.assume_init() })
            }
        }
    };
}

impl_query_io_for_tuple!(A 0, B 1);
impl_query_io_for_tuple!(A 0, B 1, C 2);
impl_query_io_for_tuple!(A 0, B 1, C 2, D 3);
