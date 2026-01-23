mod bi_vec;

pub use self::bi_vec::BiVec;

use core::alloc::Allocator;

// TODO: this module used to be in zk_ee/src/memory.
// As it's untested and unreviewed, we put it in the experimental
// ethereum storage model, to ensure this isn't used for production.

pub trait VecLikeCtor: 'static {
    type Vec<T: Sized, A: Allocator + Clone>: cc_traits::VecMut<T> + cc_traits::WithCapacityIn<A>;

    fn with_capacity_in<T, A: Allocator + Clone>(capacity: usize, alloc: A) -> Self::Vec<T, A>;
    fn purge<T, A: Allocator + Clone>(vec: &mut Self::Vec<T, A>);
}

pub struct VecCtor {}

impl VecLikeCtor for VecCtor {
    type Vec<T: Sized, A: Allocator + Clone> = alloc::vec::Vec<T, A>;

    fn with_capacity_in<T, A: Allocator + Clone>(capacity: usize, alloc: A) -> Self::Vec<T, A> {
        alloc::vec::Vec::<T, A>::with_capacity_in(capacity, alloc)
    }

    fn purge<T, A: Allocator + Clone>(vec: &mut Self::Vec<T, A>) {
        vec.clear();
    }
}

pub struct BiVecCtor {}

impl VecLikeCtor for BiVecCtor {
    type Vec<T: Sized, A: Allocator + Clone> = BiVec<T, A>;

    fn with_capacity_in<T, A: Allocator + Clone>(capacity: usize, alloc: A) -> Self::Vec<T, A> {
        BiVec::<T, A>::with_capacity_in(capacity, alloc)
    }

    fn purge<T, A: Allocator + Clone>(vec: &mut Self::Vec<T, A>) {
        vec.clear();
    }
}
