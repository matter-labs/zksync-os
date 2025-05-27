pub mod tagged_pointer;

pub use self::tagged_pointer::*;

pub const fn compatible_with_tagged_pointer_2<A, B>() -> bool {
    core::mem::align_of::<A>() > 1 && core::mem::align_of::<B>() > 1
}

pub const fn compatible_with_pointer_tag_bitlength<T: Sized>(bits: usize) -> bool {
    core::mem::align_of::<T>().trailing_zeros() >= bits as u32
}

pub const fn fits_into_page<T: Sized>(num_elements: usize) -> bool {
    core::mem::size_of::<T>() * num_elements <= PAGE_SIZE
}

// ideally we could use compile-time constant to ensure everything works,
// but not yet...

pub trait TaggedPointerCompatible<const TAG_BITS: usize>: Sized {}

// impl<T: Sized> TaggedPointerCompatible<1> for T where Assert::<{core::mem::align_of::<T>() > 1}>: IsTrue {

// }

pub const PAGE_SIZE: usize = 4096;

#[repr(align(4096))]
#[derive(Default, Debug)]
pub struct PageAligner;
