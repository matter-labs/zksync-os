//! Oracle side of the protocol: reading the inputs of queries from the memory of the querier, and encoding
//! responses the way the querier receives them. Mirrors [`QueryInput`] and [`QueryOutput`].

use super::continuous::{continuous_words, ContinuousDeserializable, ContinuousSerializable};
use super::query::{QueryInput, QueryOutput};
use super::MemoryOracle;
use crate::internal_error;
use crate::oracle::usize_serialization::{UsizeDeserializable, UsizeSerializable};
use crate::oracle::IOOracle;
use crate::storage_types::InitialStorageSlotData;
use crate::system::errors::internal::InternalError;
use crate::types_config::SystemIOTypesConfig;
use alloc::collections::VecDeque;
// for the `impl_short_query_io` macro
#[doc(hidden)]
pub use alloc::vec::Vec;
use core::mem::{size_of, MaybeUninit};

/// Read access of the oracle to the memory of the querier.
pub trait QuerierMemory {
    /// Size in bytes of a word, and of an address, of the querier: 4 on the proving target, and
    /// `size_of::<usize>()` for a querier in the same process.
    fn word_size(&self) -> usize;

    /// Reads the `u32` at `address`, which must be 4-byte aligned.
    fn read_u32(&self, address: usize) -> Result<u32, InternalError>;
}

/// The memory of a querier that runs in the same process (forward mode), whose addresses are exposed
/// pointers.
#[derive(Debug)]
pub struct NativeQuerierMemory(());

impl NativeQuerierMemory {
    /// # Safety
    ///
    /// Only for reading the inputs of the queries sent through [`MemoryOracle`](super::MemoryOracle) by
    /// code of this process, while they are being sent: their addresses refer to live values then.
    pub const unsafe fn new() -> Self {
        Self(())
    }
}

impl QuerierMemory for NativeQuerierMemory {
    fn word_size(&self) -> usize {
        size_of::<usize>()
    }

    fn read_u32(&self, address: usize) -> Result<u32, InternalError> {
        if address == 0 || !address.is_multiple_of(size_of::<u32>()) {
            return Err(internal_error!("invalid querier address"));
        }
        // SAFETY: guaranteed by the creator of the view
        Ok(unsafe { core::ptr::with_exposed_provenance::<u32>(address).read() })
    }
}

fn offset_address(address: usize, offset: usize) -> Result<usize, InternalError> {
    address
        .checked_add(offset)
        .ok_or_else(|| internal_error!("querier address overflows"))
}

/// Reads a word of the querier (`QuerierMemory::word_size` bytes), e.g. an element of the address array of
/// a composite input, or a field of a request struct of querier words.
pub fn read_querier_word<M: QuerierMemory + ?Sized>(
    memory: &M,
    address: usize,
) -> Result<usize, InternalError> {
    let low = memory.read_u32(address)?;
    match memory.word_size() {
        4 => Ok(low as usize),
        8 => {
            let high = memory.read_u32(offset_address(address, 4)?)?;
            usize::try_from(u64::from(low) | (u64::from(high) << 32))
                .map_err(|_| internal_error!("querier word does not fit into usize"))
        }
        _ => Err(internal_error!("unsupported querier word size")),
    }
}

/// Reads `num_words` consecutive `u32` words at `address` of the querier.
pub fn read_querier_u32_words<M: QuerierMemory + ?Sized>(
    memory: &M,
    address: usize,
    num_words: usize,
) -> Result<Vec<u32>, InternalError> {
    (0..num_words)
        .map(|i| {
            let offset = i
                .checked_mul(size_of::<u32>())
                .ok_or_else(|| internal_error!("querier address overflows"))?;
            memory.read_u32(offset_address(address, offset)?)
        })
        .collect()
}

/// Reads the value continuous in memory at `address` of the querier, and validates it.
pub fn read_continuous<T: ContinuousDeserializable, M: QuerierMemory + ?Sized>(
    memory: &M,
    address: usize,
) -> Result<T, InternalError> {
    let num_words = const { continuous_words::<T>() };
    let mut value = MaybeUninit::<T>::uninit();
    let dst = value.as_mut_ptr().cast::<u32>();
    for i in 0..num_words {
        let word = memory.read_u32(offset_address(address, i * size_of::<u32>())?)?;
        // SAFETY: `i < num_words`, and `value` consists of `num_words` aligned words
        unsafe { dst.add(i).write(word) };
    }
    // SAFETY: all bytes of `value` were just written, and it is not borrowed elsewhere
    unsafe { T::validate(value.as_mut_ptr())? };
    // SAFETY: validated above
    Ok(unsafe { value.assume_init() })
}

/// Oracle side of [`QueryInput`]: reads an input from the memory of the querier, the way the querier
/// sends it.
pub trait ReadQueryInput: QueryInput + Sized {
    fn read_input<M: QuerierMemory + ?Sized>(
        memory: &M,
        input_word: usize,
    ) -> Result<Self, InternalError>;
}

/// Oracle side of [`QueryOutput`]: encodes an output the way the querier receives it.
pub trait WriteQueryOutput: QueryOutput {
    fn write_output(&self, response: &mut Vec<u32>);
}

impl ReadQueryInput for () {
    fn read_input<M: QuerierMemory + ?Sized>(
        _memory: &M,
        _input_word: usize,
    ) -> Result<Self, InternalError> {
        Ok(())
    }
}

impl WriteQueryOutput for () {
    fn write_output(&self, _response: &mut Vec<u32>) {}
}

impl<T: ContinuousSerializable + ContinuousDeserializable> ReadQueryInput for T {
    fn read_input<M: QuerierMemory + ?Sized>(
        memory: &M,
        input_word: usize,
    ) -> Result<Self, InternalError> {
        read_continuous(memory, input_word)
    }
}

/// Appends the image of a value with padding bytes, the way the querier receives it memcpy-like:
/// `write_fields` writes every field of the value into a zeroed image (typically through
/// `&raw mut (*image).field`), so that the padding bytes are zeroes.
pub fn write_padded_image<T: ContinuousDeserializable>(
    response: &mut Vec<u32>,
    write_fields: impl FnOnce(*mut T),
) {
    let num_words = const { continuous_words::<T>() };
    let mut image = MaybeUninit::<T>::uninit();
    // zeroed in place: a copy of a `MaybeUninit<T>` would not keep the padding bytes of `T`
    // SAFETY: the image is valid for writes of one `T`
    unsafe { image.as_mut_ptr().write_bytes(0, 1) };
    write_fields(image.as_mut_ptr());
    // SAFETY: `T` is aligned for `u32` and consists of `num_words` words, all initialized: zeroed, then
    // partly overwritten with the fields
    let words = unsafe { core::slice::from_raw_parts(image.as_ptr().cast::<u32>(), num_words) };
    response.extend_from_slice(words);
}

/// Appends the words of a value without padding bytes.
fn write_image<T: ContinuousSerializable>(value: &T, response: &mut Vec<u32>) {
    let num_words = const { continuous_words::<T>() };
    // SAFETY: `T` is aligned for `u32` and has no padding bytes, so it consists of `num_words`
    // initialized words
    let words =
        unsafe { core::slice::from_raw_parts(core::ptr::from_ref(value).cast::<u32>(), num_words) };
    response.extend_from_slice(words);
}

impl<T: ContinuousSerializable + ContinuousDeserializable> WriteQueryOutput for T {
    fn write_output(&self, response: &mut Vec<u32>) {
        write_image(self, response);
    }
}

macro_rules! impl_host_io_for_tuple {
    ($($element:ident $index:tt),+) => {
        impl<$($element: ContinuousSerializable + ContinuousDeserializable),+> ReadQueryInput
            for ($($element,)+)
        {
            fn read_input<M: QuerierMemory + ?Sized>(
                memory: &M,
                input_word: usize,
            ) -> Result<Self, InternalError> {
                let word_size = memory.word_size();
                Ok(($(
                    read_continuous::<$element, M>(
                        memory,
                        read_querier_word(memory, offset_address(input_word, $index * word_size)?)?,
                    )?,
                )+))
            }
        }

        impl<$($element: ContinuousSerializable + ContinuousDeserializable),+> WriteQueryOutput
            for ($($element,)+)
        {
            fn write_output(&self, response: &mut Vec<u32>) {
                $(write_image(&self.$index, response);)+
            }
        }
    };
}

impl_host_io_for_tuple!(A 0, B 1);
impl_host_io_for_tuple!(A 0, B 1, C 2);
impl_host_io_for_tuple!(A 0, B 1, C 2, D 3);

/// The flag byte, zeroed padding and the value: the image the querier validates (see the
/// `ContinuousDeserializable` impl).
impl<IOTypes: SystemIOTypesConfig> WriteQueryOutput for InitialStorageSlotData<IOTypes>
where
    IOTypes::StorageValue: ContinuousSerializable + ContinuousDeserializable,
{
    fn write_output(&self, response: &mut Vec<u32>) {
        let start = response.len();
        response.resize(start + const { continuous_words::<Self>() }, 0);
        let image = response[start..].as_mut_ptr().cast::<u8>();
        let value_words = const { continuous_words::<IOTypes::StorageValue>() };
        // SAFETY: the image has the size of `Self`, the flag and the value are at their offsets in it,
        // and the value has no padding bytes
        unsafe {
            image
                .add(core::mem::offset_of!(Self, is_new_storage_slot))
                .write(u8::from(self.is_new_storage_slot));
            core::ptr::copy_nonoverlapping(
                core::ptr::from_ref(&self.initial_value).cast::<u32>(),
                image
                    .add(core::mem::offset_of!(Self, initial_value))
                    .cast::<u32>(),
                value_words,
            );
        }
    }
}

/// Encodes a dynamically sized response of `bytes`, the way a
/// [`DynamicDestination`](super::DynamicDestination) receives it: the bytes are padded with zeroes to
/// whole `u64` words, so that they are whole `usize` words on every target, and preceded by their number
/// of `u32` words.
pub fn write_dynamic_bytes(bytes: &[u8], response: &mut Vec<u32>) {
    let num_words = bytes.len().next_multiple_of(size_of::<u64>()) / size_of::<u32>();
    response.reserve(1 + num_words);
    response.push(u32::try_from(num_words).expect("oracle response is too long"));
    let (chunks, tail) = bytes.as_chunks::<4>();
    response.extend(chunks.iter().map(|chunk| u32::from_le_bytes(*chunk)));
    if !tail.is_empty() {
        let mut word = [0u8; 4];
        word[..tail.len()].copy_from_slice(tail);
        response.push(u32::from_le_bytes(word));
    }
    let written = bytes.len().div_ceil(size_of::<u32>());
    response.extend(core::iter::repeat_n(0, num_words - written));
}

/// The response to the current memory-based query, for oracles that serve the protocol in process.
#[derive(Debug, Default)]
pub struct ResponseBuffer {
    words: VecDeque<u32>,
}

impl ResponseBuffer {
    /// Starts serving the response to a new query.
    pub fn set(&mut self, words: Vec<u32>) -> Result<(), InternalError> {
        if !self.words.is_empty() {
            self.words.clear();
            return Err(internal_error!(
                "previous oracle response was not consumed in full"
            ));
        }
        self.words = words.into();
        Ok(())
    }

    /// [`MemoryOracle::read_word`]
    pub fn read_word(&mut self) -> Result<u32, InternalError> {
        self.words
            .pop_front()
            .ok_or_else(|| internal_error!("oracle response is shorter than expected"))
    }

    /// [`MemoryOracle::write_words`]
    ///
    /// # Safety
    ///
    /// `dst` must be aligned for `u32` and valid for writes of `num_words` `u32`s.
    pub unsafe fn write_words(
        &mut self,
        dst: *mut u32,
        num_words: usize,
    ) -> Result<(), InternalError> {
        if self.words.len() < num_words {
            return Err(internal_error!("oracle response is shorter than expected"));
        }
        for (i, word) in self.words.drain(..num_words).enumerate() {
            // SAFETY: `i < num_words`, and the caller guarantees `dst` is valid for `num_words` words
            unsafe { dst.add(i).write(word) };
        }
        Ok(())
    }

    /// [`MemoryOracle::finish_query`]
    pub fn finish(&mut self) -> Result<(), InternalError> {
        if self.words.is_empty() {
            Ok(())
        } else {
            self.words.clear();
            Err(internal_error!("oracle response contains excess data"))
        }
    }
}

/// Implements the reading methods of [`MemoryOracle`] with a [`ResponseBuffer`] field, for oracles that
/// serve the protocol in process and only implement [`send_query`](MemoryOracle::send_query), which
/// computes the response and hands it to the buffer.
#[macro_export]
macro_rules! memory_oracle_response_methods {
    ($field:ident) => {
        fn read_word(&mut self) -> Result<u32, $crate::system::errors::internal::InternalError> {
            self.$field.read_word()
        }

        unsafe fn write_words(
            &mut self,
            dst: *mut u32,
            num_words: usize,
        ) -> Result<(), $crate::system::errors::internal::InternalError> {
            // SAFETY: guaranteed by the caller
            unsafe { self.$field.write_words(dst, num_words) }
        }

        fn finish_query(&mut self) -> Result<(), $crate::system::errors::internal::InternalError> {
            self.$field.finish()
        }
    };
}

/// An oracle that answers memory-based queries in process, with a handler that gets the query ID, the
/// input word and the memory of the querier, and returns the response words. For test doubles and simple
/// oracles of this process; it does not serve the iterator-based protocol.
pub struct InProcessMemoryOracle<H> {
    handler: H,
    response: ResponseBuffer,
}

impl<H> InProcessMemoryOracle<H> {
    pub fn new(handler: H) -> Self {
        Self {
            handler,
            response: ResponseBuffer::default(),
        }
    }

    pub fn handler(&self) -> &H {
        &self.handler
    }
}

impl<H: FnMut(u32, usize, &dyn QuerierMemory) -> Vec<u32>> MemoryOracle
    for InProcessMemoryOracle<H>
{
    fn send_query(&mut self, query_id: u32, input_word: usize) -> Result<(), InternalError> {
        // SAFETY: the querier is code of this process, which exposes the memory the input word refers
        // to while it sends the query, i.e. during this call
        let memory = unsafe { NativeQuerierMemory::new() };
        let response = (self.handler)(query_id, input_word, &memory);
        self.response.set(response)
    }

    crate::memory_oracle_response_methods!(response);
}

impl<H: 'static + FnMut(u32, usize, &dyn QuerierMemory) -> Vec<u32>> IOOracle
    for InProcessMemoryOracle<H>
{
    type RawIterator<'a> = core::iter::Empty<usize>;

    fn raw_query<'a, I: UsizeSerializable + UsizeDeserializable>(
        &'a mut self,
        _query_type: u32,
        _input: &I,
    ) -> Result<Self::RawIterator<'a>, InternalError> {
        Err(internal_error!(
            "oracle does not serve the iterator-based protocol"
        ))
    }
}
