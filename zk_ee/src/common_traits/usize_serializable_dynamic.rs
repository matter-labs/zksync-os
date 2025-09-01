//! Serialization/deserialization to/from streams of `usize` elements for objects with dynamic size (e.g. vectors)

use crate::kv_markers::{UsizeDeserializable, UsizeSerializable};
use crate::system::errors::internal::InternalError;
use alloc::alloc::Allocator;
use alloc::boxed::Box;
use alloc::vec::Vec;
use core::marker::PhantomData;

pub trait UsizeDeserializableDynamic<A: Allocator + Clone>: Sized {
    fn from_iter(
        src: &mut impl ExactSizeIterator<Item = usize>,
        alloc: A,
    ) -> Result<Self, InternalError>;
}

pub trait UsizeSerializableDynamic<A: Allocator + Clone> {
    fn iter(&self, alloc: A) -> impl ExactSizeIterator<Item = usize>;
}

impl<T: UsizeDeserializable, A: Allocator + Clone> UsizeDeserializableDynamic<A>
    for alloc::vec::Vec<T, A>
{
    fn from_iter(
        src: &mut impl ExactSizeIterator<Item = usize>,
        alloc: A,
    ) -> Result<Self, InternalError> {
        let len: u32 = UsizeDeserializable::from_iter(src)?;

        let mut res = Vec::with_capacity_in(len as usize, alloc.clone());
        for _ in 0..len {
            res.push(T::from_iter(src)?);
        }

        Ok(res)
    }
}

impl<T: UsizeSerializable, A: Allocator + Clone> UsizeSerializableDynamic<A>
    for alloc::vec::Vec<T, A>
{
    fn iter(&self, alloc: A) -> impl ExactSizeIterator<Item = usize> {
        UsizeSerializableArrayIterator::<T, A>::from(self.as_slice(), alloc)
    }
}

pub struct UsizeSerializableArrayIterator<'a, T: UsizeSerializable, A: Allocator + Clone> {
    iter: Box<dyn Iterator<Item = usize> + 'a, A>,
    len: usize,
    _marker: PhantomData<T>,
}

impl<'a, T: UsizeSerializable, A: Allocator + Clone + 'a> UsizeSerializableArrayIterator<'a, T, A> {
    pub fn from(input: &'a [T], alloc: A) -> Self {
        let mut prefix =
            Vec::with_capacity_in(<u64 as UsizeSerializable>::USIZE_LEN, alloc.clone());
        prefix.extend((input.len() as u64).iter());
        let prefix_len = prefix.len();

        let input_iter = input.iter().flat_map(|x| x.iter());
        let input_iter_len = input.len() * T::USIZE_LEN;

        Self {
            iter: Box::new_in(core::iter::once(prefix).flatten().chain(input_iter), alloc),
            len: prefix_len + input_iter_len,
            _marker: Default::default(),
        }
    }
}

impl<'a, T: UsizeSerializable, A: Allocator + Clone> Iterator
    for UsizeSerializableArrayIterator<'a, T, A>
{
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        self.len -= 1;
        self.iter.next()
    }
}

impl<'a, T: UsizeSerializable, A: Allocator + Clone> ExactSizeIterator
    for UsizeSerializableArrayIterator<'a, T, A>
{
    fn len(&self) -> usize {
        self.len
    }
}
