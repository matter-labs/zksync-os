//! Serialization and deserialization helpers for keys and values for storage.
pub mod kv_impls;

use arrayvec::ArrayVec;
use core::iter::Chain;
use core::marker::PhantomData;
use core::mem::MaybeUninit;
use std::alloc::Allocator;
use ruint::aliases::U256;

use super::system::errors::internal::InternalError;
use super::types_config::SystemIOTypesConfig;

bitflags::bitflags! {
    /// Represents a set of flags.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct EmptyBitflags: u32 {}
}

pub trait ReadonlyKVMarker: 'static {
    const CAN_BE_COLD_AND_WARM_READ: bool = true;

    type Key: UsizeSerializable;
    type Value: UsizeDeserializable;
    type AccessStatsBitmask: bitflags::Flags<Bits = u32> = EmptyBitflags;
}

pub trait ReadWriteKVMarker: ReadonlyKVMarker
where
    Self::Value: UsizeSerializable,
{
    const CAN_BE_COLD_AND_WARM_WRITE: bool = true;
}

pub trait UsizeSerializable {
    const USIZE_LEN: usize;

    fn iter(&self) -> impl ExactSizeIterator<Item = usize>;
}

pub trait UsizeSerializableDynamic {
    fn iter(&self) -> impl ExactSizeIterator<Item = usize>;
}

pub trait UsizeDeserializable: Sized {
    const USIZE_LEN: usize;

    fn from_iter(src: &mut impl ExactSizeIterator<Item = usize>) -> Result<Self, InternalError>;

    ///
    /// # Safety
    ///
    /// The correct layout of the serialization is enforced by the `from_iter`
    /// implementation, as long as the data in the external storage is correctly populated. It is a
    /// UB to read from any location that wasn't populated by this type before.
    ///
    unsafe fn init_from_iter(
        this: &mut MaybeUninit<Self>,
        src: &mut impl ExactSizeIterator<Item = usize>,
    ) -> Result<(), InternalError> {
        let new = UsizeDeserializable::from_iter(src)?;
        this.write(new);

        Ok(())
    }
}

pub trait UsizeDeserializableDynamic<A: Allocator + Clone>: Sized {
    fn from_iter(src: &mut impl ExactSizeIterator<Item = usize>, alloc: A) -> Result<Self, InternalError>;
}

impl <T: UsizeDeserializable, A: Allocator + Clone> UsizeDeserializableDynamic<A> for alloc::vec::Vec<T, A> {
    fn from_iter(src: &mut impl ExactSizeIterator<Item = usize>, alloc: A) -> Result<Self, InternalError> {
        let len: u32 = UsizeDeserializable::from_iter(src)?;

        let mut res = Vec::with_capacity_in(len as usize, alloc.clone());
        for _ in 0..len {
            res.push(T::from_iter(src)?);
        }

        Ok(res)
    }
}

pub struct UsizeSerializableArrayIterator<'a, T: UsizeSerializable> {
    iter: Box<dyn Iterator<Item = usize>  + 'a>,
    len: usize,
    _marker: PhantomData<T>
}

impl<'a, T: UsizeSerializable> UsizeSerializableArrayIterator<'a, T> {
    pub fn from(input: &'a [T]) -> Self {
        let prefix: Vec<_> = (input.len() as u64).iter().collect();
        let prefix_len = prefix.len();

        let input_iter = input.iter().flat_map(|x| x.iter());
        let input_iter_len = input.len() * T::USIZE_LEN;
        
        Self {
            iter: Box::new(core::iter::once(prefix).flatten().chain(input_iter)),
            len: prefix_len + input_iter_len,
            _marker: Default::default()
        }
    }
}

impl<'a, T: UsizeSerializable> Iterator for UsizeSerializableArrayIterator<'a, T> {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        self.len -= 1;
        self.iter.next()
    }
}

impl<'a, T: UsizeSerializable> ExactSizeIterator for UsizeSerializableArrayIterator<'a, T> {
    fn len(&self) -> usize {
        self.len
    }
}

// helper structs for most of the cases

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StorageAddress<IOTypes: SystemIOTypesConfig> {
    pub address: IOTypes::Address,
    pub key: IOTypes::StorageKey,
}

impl<IOTypes: SystemIOTypesConfig> UsizeSerializable for StorageAddress<IOTypes> {
    const USIZE_LEN: usize = <IOTypes::Address as UsizeSerializable>::USIZE_LEN
        + <IOTypes::StorageKey as UsizeSerializable>::USIZE_LEN;

    fn iter(&self) -> impl ExactSizeIterator<Item = usize> {
        ExactSizeChain::new(
            UsizeSerializable::iter(&self.address),
            UsizeSerializable::iter(&self.key),
        )
    }
}

impl<IOTypes: SystemIOTypesConfig> UsizeDeserializable for StorageAddress<IOTypes> {
    const USIZE_LEN: usize = <Self as UsizeSerializable>::USIZE_LEN;

    fn from_iter(src: &mut impl ExactSizeIterator<Item = usize>) -> Result<Self, InternalError> {
        let address = UsizeDeserializable::from_iter(src)?;
        let key = UsizeDeserializable::from_iter(src)?;

        let new = Self { address, key };

        Ok(new)
    }
}

pub const MAX_EVENT_TOPICS: usize = 4;

pub struct EventFullKey<const N: usize, IOTypes: SystemIOTypesConfig> {
    pub address: IOTypes::Address,
    pub topics: ArrayVec<IOTypes::EventKey, N>,
}

impl<const N: usize, IOTypes: SystemIOTypesConfig> UsizeSerializable for EventFullKey<N, IOTypes> {
    const USIZE_LEN: usize =
        <IOTypes::Address as UsizeSerializable>::USIZE_LEN + IOTypes::EventKey::USIZE_LEN * N;

    fn iter(&self) -> impl ExactSizeIterator<Item = usize> {
        ExactSizeChainN::<_, _, N>::new(
            ExactSizeChain::new(
                UsizeSerializable::iter(&self.address),
                core::iter::once(self.topics.len()),
            ),
            core::array::from_fn(|i| {
                let topic = self
                    .topics
                    .get(i)
                    .unwrap_or(IOTypes::static_default_event_key());
                Some(UsizeSerializable::iter(topic))
            }),
        )
    }
}

pub struct SignalFullKey<const N: usize, IOTypes: SystemIOTypesConfig> {
    pub address: IOTypes::Address,
    pub topics: ArrayVec<IOTypes::SignalingKey, N>,
}

impl<const N: usize, IOTypes: SystemIOTypesConfig> UsizeSerializable for SignalFullKey<N, IOTypes> {
    const USIZE_LEN: usize =
        <IOTypes::Address as UsizeSerializable>::USIZE_LEN + IOTypes::SignalingKey::USIZE_LEN * N;

    fn iter(&self) -> impl ExactSizeIterator<Item = usize> {
        ExactSizeChainN::<_, _, N>::new(
            ExactSizeChain::new(
                UsizeSerializable::iter(&self.address),
                core::iter::once(self.topics.len()),
            ),
            core::array::from_fn(|i| {
                let topic = self
                    .topics
                    .get(i)
                    .unwrap_or(IOTypes::static_default_signaling_key());
                Some(UsizeSerializable::iter(topic))
            }),
        )
    }
}

#[derive(Clone, Debug)]
pub struct ExactSizeChain<A, B> {
    // These are "fused" with `Option` so we don't need separate state to track which part is
    // already exhausted, and we may also get niche layout for `None`. We don't use the real `Fuse`
    // adapter because its specialization for `FusedIterator` unconditionally descends into the
    // iterator, and that could be expensive to keep revisiting stuff like nested chains. It also
    // hurts compiler performance to add more iterator layers to `Chain`.
    //
    // Only the "first" iterator is actually set `None` when exhausted, depending on whether you
    // iterate forward or backward. If you mix directions, then both sides may be `None`.
    a: Option<A>,
    b: Option<B>,
}
impl<A, B> ExactSizeChain<A, B> {
    pub fn new(a: A, b: B) -> ExactSizeChain<A, B> {
        ExactSizeChain {
            a: Some(a),
            b: Some(b),
        }
    }
}

impl<A, B> Iterator for ExactSizeChain<A, B>
where
    A: ExactSizeIterator,
    B: ExactSizeIterator<Item = A::Item>,
{
    type Item = A::Item;

    #[inline]
    fn next(&mut self) -> Option<A::Item> {
        and_then_or_clear(&mut self.a, Iterator::next).or_else(|| self.b.as_mut()?.next())
    }
}

impl<A, B> ExactSizeIterator for ExactSizeChain<A, B>
where
    A: ExactSizeIterator,
    B: ExactSizeIterator<Item = A::Item>,
{
    fn len(&self) -> usize {
        self.a.as_ref().map(|el| el.len()).unwrap_or(0)
            + self.b.as_ref().map(|el| el.len()).unwrap_or(0)
    }
}

#[inline]
fn and_then_or_clear<T, U>(opt: &mut Option<T>, f: impl FnOnce(&mut T) -> Option<U>) -> Option<U> {
    let x = f(opt.as_mut()?);
    if x.is_none() {
        *opt = None;
    }
    x
}

#[derive(Clone, Debug)]
pub struct ExactSizeChainN<A, B, const N: usize> {
    // These are "fused" with `Option` so we don't need separate state to track which part is
    // already exhausted, and we may also get niche layout for `None`. We don't use the real `Fuse`
    // adapter because its specialization for `FusedIterator` unconditionally descends into the
    // iterator, and that could be expensive to keep revisiting stuff like nested chains. It also
    // hurts compiler performance to add more iterator layers to `Chain`.
    //
    // Only the "first" iterator is actually set `None` when exhausted, depending on whether you
    // iterate forward or backward. If you mix directions, then both sides may be `None`.
    a: Option<A>,
    b: [Option<B>; N],
    b_idx: usize,
}

impl<A, B, const N: usize> ExactSizeChainN<A, B, N> {
    pub fn new(a: A, b: [Option<B>; N]) -> Self {
        assert!(N > 0);
        Self {
            a: Some(a),
            b,
            b_idx: 0,
        }
    }
}

impl<A, B, const N: usize> Iterator for ExactSizeChainN<A, B, N>
where
    A: ExactSizeIterator,
    B: ExactSizeIterator<Item = A::Item>,
{
    type Item = A::Item;

    #[inline]
    fn next(&mut self) -> Option<A::Item> {
        if N == 0 {
            and_then_or_clear(&mut self.a, Iterator::next)
        } else {
            and_then_or_clear(&mut self.a, Iterator::next).or_else(|| {
                while self.b_idx < N {
                    if let Some(next) = self.b[self.b_idx].as_mut().unwrap().next() {
                        return Some(next);
                    } else {
                        self.b[self.b_idx] = None;
                        self.b_idx += 1
                    }
                }

                None
            })
        }
    }
}

impl<A, B, const N: usize> ExactSizeIterator for ExactSizeChainN<A, B, N>
where
    A: ExactSizeIterator,
    B: ExactSizeIterator<Item = A::Item>,
{
    fn len(&self) -> usize {
        let mut result = self.a.as_ref().map(|el| el.len()).unwrap_or(0);
        for el in self.b.iter().skip(self.b_idx) {
            result += el.as_ref().map(|el| el.len()).unwrap_or(0)
        }

        result
    }
}
