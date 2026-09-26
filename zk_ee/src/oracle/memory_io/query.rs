//! Typed queries: input and output types bound to a query ID.

use super::continuous::{ContinuousDeserializable, ContinuousSerializable};
use super::dynamic::DynamicDestination;
use super::MemoryOracle;
use crate::common_structs::da_commitment_scheme::DACommitmentScheme;
use crate::execution_environment_type::ExecutionEnvironmentType;
use crate::internal_error;
use crate::system::errors::internal::InternalError;
use crate::utils::{UsizeAlignedByteBox, USIZE_SIZE};
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
        let output = Self::Output::receive(oracle, dst);
        finish(oracle, output)
    }

    /// Runs the query, and returns the output as a new value.
    #[inline(always)]
    fn get<O: MemoryOracle>(
        oracle: &mut O,
        input: <Self::Input as QueryInput>::Ref<'_>,
    ) -> Result<Self::Output, InternalError> {
        Self::Input::send(oracle, Self::QUERY_ID, input)?;
        let output = Self::Output::receive_value(oracle);
        finish(oracle, output)
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
        let len = oracle.write_dynamic(dst);
        finish(oracle, len)
    }

    /// Runs the query, and writes the output into a new allocation of at most `max_len` words.
    fn get_boxed<O: MemoryOracle, A: Allocator>(
        oracle: &mut O,
        input: <Self::Input as QueryInput>::Ref<'_>,
        max_len: usize,
        allocator: A,
    ) -> Result<Box<[usize], A>, InternalError> {
        Self::Input::send(oracle, Self::QUERY_ID, input)?;
        let output = oracle.write_dynamic_boxed(max_len, allocator);
        finish(oracle, output)
    }
}

/// Ends the current query, whether its output was received or rejected: the oracle drops the words of
/// a rejected output that were not read, and stays in step with the querier (as the proving target's
/// oracle does when the next query starts).
#[inline(always)]
fn finish<O: MemoryOracle, T>(
    oracle: &mut O,
    output: Result<T, InternalError>,
) -> Result<T, InternalError> {
    let finished = oracle.finish_query();
    let output = output?;
    finished?;
    Ok(output)
}

/// A byte string from two queries with the same input: the first returns its length in bytes, a `u32`
/// (`0` for no bytes, then `None` is returned), and the second the bytes, as a dynamically sized response
/// of whole `usize` words that must cover the length and fit into the length rounded up to whole `u64`
/// words.
pub fn get_bytes_with_length_query<O: MemoryOracle, I: QueryInput, A: Allocator>(
    oracle: &mut O,
    length_query_id: u32,
    body_query_id: u32,
    input: I::Ref<'_>,
    allocator: A,
) -> Result<Option<UsizeAlignedByteBox<A>>, InternalError> {
    I::send(oracle, length_query_id, input)?;
    let num_bytes = oracle.read_short::<u32>();
    let num_bytes = finish(oracle, num_bytes)?;
    if num_bytes == 0 {
        return Ok(None);
    }
    let num_bytes = num_bytes as usize;
    let mut buffer = UsizeAlignedByteBox::preallocated_in(num_bytes, allocator);
    I::send(oracle, body_query_id, input)?;
    let num_words = buffer.init_words(|words| oracle.write_dynamic(words));
    let num_words = finish(oracle, num_words)?;
    if num_words * USIZE_SIZE < num_bytes {
        return Err(internal_error!(
            "oracle response is shorter than the claimed number of bytes"
        ));
    }

    Ok(Some(buffer))
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

/// Implements [`QueryInput`] and [`QueryOutput`], and their oracle side ([`ReadQueryInput`] and
/// [`WriteQueryOutput`]), for types that implement both [`ShortSerializable`] and
/// [`ShortDeserializable`]: such a value is passed by value, and received as a single word.
///
/// [`ReadQueryInput`]: super::host::ReadQueryInput
/// [`WriteQueryOutput`]: super::host::WriteQueryOutput
/// [`ShortSerializable`]: super::ShortSerializable
/// [`ShortDeserializable`]: super::ShortDeserializable
#[macro_export]
macro_rules! impl_short_query_io {
    ($($t:ty),+ $(,)?) => {$(
        impl $crate::oracle::memory_io::QueryInput for $t {
            type Ref<'a> = $t;

            #[inline(always)]
            fn send<O: $crate::oracle::memory_io::MemoryOracle>(
                oracle: &mut O,
                query_id: u32,
                input: Self::Ref<'_>,
            ) -> Result<(), $crate::system::errors::internal::InternalError> {
                oracle.send_short(query_id, input)
            }
        }

        impl $crate::oracle::memory_io::QueryOutput for $t {
            type Destination<'a> = ();
            type Initialized<'a> = $t;

            #[inline(always)]
            fn receive<'a, O: $crate::oracle::memory_io::MemoryOracle>(
                oracle: &mut O,
                _dst: Self::Destination<'a>,
            ) -> Result<Self::Initialized<'a>, $crate::system::errors::internal::InternalError> {
                oracle.read_short()
            }

            #[inline(always)]
            fn receive_value<O: $crate::oracle::memory_io::MemoryOracle>(
                oracle: &mut O,
            ) -> Result<Self, $crate::system::errors::internal::InternalError> {
                oracle.read_short()
            }
        }

        impl $crate::oracle::memory_io::host::ReadQueryInput for $t {
            fn read_input<M: $crate::oracle::memory_io::host::QuerierMemory + ?Sized>(
                _memory: &M,
                input_word: usize,
            ) -> Result<Self, $crate::system::errors::internal::InternalError> {
                let word = u32::try_from(input_word).map_err(|_| {
                    $crate::internal_error!("short input word does not fit into u32")
                })?;
                <$t as $crate::oracle::memory_io::ShortDeserializable>::from_short_word(word)
            }
        }

        impl $crate::oracle::memory_io::host::WriteQueryOutput for $t {
            fn write_output(&self, response: &mut $crate::oracle::memory_io::host::Vec<u32>) {
                response.push($crate::oracle::memory_io::ShortSerializable::to_short_word(*self));
            }
        }
    )+};
}

crate::impl_short_query_io!(
    bool,
    u8,
    u16,
    u32,
    ExecutionEnvironmentType,
    DACommitmentScheme
);

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
