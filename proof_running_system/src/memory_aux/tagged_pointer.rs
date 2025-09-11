use super::*;
use core::{cell::Cell, ptr::NonNull};

#[inline(always)]
pub(crate) fn tag_pointer<T>(ptr: NonNull<T>, tag: usize) -> NonNull<()> {
    debug_assert!(tag < core::mem::align_of::<T>());
    debug_assert!(ptr.addr().get().is_multiple_of(core::mem::align_of::<T>()));
    let raw_addr = ptr.addr().get();
    let tagged_addr = raw_addr | tag;
    unsafe { NonNull::new_unchecked(core::ptr::with_exposed_provenance_mut(tagged_addr)) }
}

#[inline(always)]
pub(crate) fn strip_tag<T>(ptr: NonNull<()>) -> (NonNull<T>, usize) {
    debug_assert!(core::mem::align_of::<T>() > 1);
    let raw_addr = ptr.addr().get();
    let untagged_ptr = raw_addr & !(core::mem::align_of::<T>() - 1);
    let tag = raw_addr & (core::mem::align_of::<T>() - 1);
    unsafe {
        (
            NonNull::new_unchecked(core::ptr::with_exposed_provenance_mut(untagged_ptr)),
            tag,
        )
    }
}

#[repr(transparent)]
pub struct TaggedPointer<A: TaggedPointerCompatible<1>, B: TaggedPointerCompatible<1>> {
    pointer: NonNull<()>,
    _marker: core::marker::PhantomData<(A, B)>,
}

pub const TAGGED_POINTER_SIZE: usize = const { core::mem::size_of::<NonNull<()>>() };

impl<A: TaggedPointerCompatible<1>, B: TaggedPointerCompatible<1>> core::fmt::Debug
    for TaggedPointer<A, B>
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut t = f.debug_struct("TaggedPointer");
        t.field("pointer", &self.pointer.as_ptr());
        if self.is_a() {
            t.field("A", &true);
            t.field("B", &false);
        } else {
            t.field("A", &false);
            t.field("B", &true);
        }

        t.finish()
    }
}

impl<A: TaggedPointerCompatible<1>, B: TaggedPointerCompatible<1>> Clone for TaggedPointer<A, B> {
    #[allow(clippy::non_canonical_clone_impl)]
    fn clone(&self) -> Self {
        Self {
            pointer: self.pointer,
            _marker: core::marker::PhantomData,
        }
    }
}

impl<A: TaggedPointerCompatible<1>, B: TaggedPointerCompatible<1>> Copy for TaggedPointer<A, B> {}

impl<A: TaggedPointerCompatible<1>, B: TaggedPointerCompatible<1>> TaggedPointer<A, B>
// where Assert<{compatible_with_tagged_pointer_2::<A, B>()}>: IsTrue
{
    pub const TAG_A: usize = 0x00;
    pub const TAG_B: usize = 0x01;

    pub fn from_a(inner: NonNull<A>) -> Self {
        // tag is 0 for A
        let tagged = tag_pointer(inner, Self::TAG_A);
        Self {
            pointer: tagged,
            _marker: core::marker::PhantomData,
        }
    }

    pub fn from_b(inner: NonNull<B>) -> Self {
        // tag is 1 for b
        let tagged = tag_pointer(inner, Self::TAG_B);
        Self {
            pointer: tagged,
            _marker: core::marker::PhantomData,
        }
    }

    pub fn is_a(&self) -> bool {
        let (_, tag) = strip_tag::<A>(self.pointer);
        tag == Self::TAG_A
    }

    pub fn as_a_ref(&self) -> Option<&A> {
        let (ptr, tag) = strip_tag::<A>(self.pointer);
        if tag == Self::TAG_A {
            unsafe { Some(ptr.as_ref()) }
        } else {
            None
        }
    }

    #[allow(clippy::mut_from_ref)]
    pub fn as_b_ref(&self) -> Option<&B> {
        let (ptr, tag) = strip_tag::<B>(self.pointer);
        if tag == Self::TAG_B {
            unsafe { Some(ptr.as_ref()) }
        } else {
            None
        }
    }

    #[allow(clippy::mut_from_ref)]
    pub fn as_a_mut(&self) -> Option<&mut A> {
        let (mut ptr, tag) = strip_tag::<A>(self.pointer);
        if tag == Self::TAG_A {
            unsafe { Some(ptr.as_mut()) }
        } else {
            None
        }
    }

    #[allow(clippy::mut_from_ref)]
    pub fn as_b_mut(&self) -> Option<&mut B> {
        let (mut ptr, tag) = strip_tag::<B>(self.pointer);
        if tag == Self::TAG_B {
            unsafe { Some(ptr.as_mut()) }
        } else {
            None
        }
    }

    pub fn as_a(self) -> Option<NonNull<A>> {
        let (ptr, tag) = strip_tag::<A>(self.pointer);
        if tag == Self::TAG_A {
            Some(ptr)
        } else {
            None
        }
    }

    pub fn as_b(self) -> Option<NonNull<B>> {
        let (ptr, tag) = strip_tag::<B>(self.pointer);
        if tag == Self::TAG_B {
            Some(ptr)
        } else {
            None
        }
    }
}

#[repr(C)]
pub struct RcTaggedWrapper<T: TaggedPointerCompatible<1>> {
    pub(crate) strong_count: Cell<usize>,
    pub(crate) weak_count: Cell<usize>,
    pub(crate) value: T,
}

impl<T: TaggedPointerCompatible<1>> TaggedPointerCompatible<1> for RcTaggedWrapper<T> {}
