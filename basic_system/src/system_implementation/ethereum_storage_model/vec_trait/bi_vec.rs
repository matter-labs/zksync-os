// Quasi-vector implementation that descends into same sized allocated chunks

use alloc::boxed::Box;
use core::{alloc::Allocator, mem::MaybeUninit};

// Backing capacity will not implement any notable traits itself. It is also dynamic, so whoever uses it
// will be able to decide on allocation strategy
struct CapacityChunk<T: Sized, A: Allocator> {
    capacity: Box<[MaybeUninit<T>], A>,
    filled: usize,
}

impl<T: Sized, A: Allocator> core::fmt::Debug for CapacityChunk<T, A>
where
    T: core::fmt::Debug,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("CapacityChunk")
            .field("filled", &self.filled)
            .field("content", &unsafe { self.filled_slice() })
            .finish()
    }
}

impl<T: Sized, A: Allocator> CapacityChunk<T, A> {
    fn with_capacity_in(capacity: usize, allocator: A) -> Self {
        // it's hacky ensure that this structure is itself "small". Allocators are usually "small",
        // but we will use generous 4 usizes for it
        let _self_size_is_small =
            const { core::mem::size_of::<Self>() <= 8 * core::mem::size_of::<usize>() };
        debug_assert!(_self_size_is_small);

        let capacity = Box::new_uninit_slice_in(capacity, allocator);

        Self {
            capacity,
            filled: 0,
        }
    }

    const fn capacity_for_backing_size(backing_size: usize) -> usize {
        let inner_size = core::mem::size_of::<T>();
        backing_size / inner_size
    }

    const unsafe fn filled_slice(&self) -> &[T] {
        core::slice::from_raw_parts(self.capacity.as_ptr().cast::<T>(), self.filled)
    }

    const unsafe fn filled_slice_mut(&mut self) -> &mut [T] {
        core::slice::from_raw_parts_mut(self.capacity.as_mut_ptr().cast::<T>(), self.filled)
    }

    unsafe fn get_unchecked(&self, index: usize) -> &T {
        debug_assert!(index < self.filled);
        self.capacity.get_unchecked(index).assume_init_ref()
    }

    unsafe fn get_unchecked_mut(&mut self, index: usize) -> &mut T {
        debug_assert!(index < self.filled);
        self.capacity.get_unchecked_mut(index).assume_init_mut()
    }

    unsafe fn push_back_unchecked(&mut self, el: T) {
        debug_assert!(self.capacity() > self.filled);
        self.capacity.get_unchecked_mut(self.filled).write(el);
        self.filled += 1;
    }

    unsafe fn clear(&mut self) {
        // drop
        core::ptr::drop_in_place(self.filled_slice_mut() as *mut [T]);
        self.filled = 0;
    }

    fn allocator(&self) -> &A {
        Box::allocator(&self.capacity)
    }

    unsafe fn pop(&mut self) -> T {
        debug_assert!(self.filled > 0);
        self.filled -= 1;
        self.capacity
            .get_unchecked_mut(self.filled)
            .assume_init_read()
    }

    fn is_full(&self) -> bool {
        self.filled == self.capacity.len()
    }

    const fn is_empty(&self) -> bool {
        self.filled == 0
    }

    const fn capacity(&self) -> usize {
        self.capacity.len()
    }
}

impl<T: Sized, A: Allocator> Drop for CapacityChunk<T, A> {
    fn drop(&mut self) {
        // NOTE: drop internal content, and then we will deallocate backing capacity as usual
        unsafe { self.clear() };
    }
}

#[inline(never)]
#[cold]
#[track_caller]
fn bivec_push_panic(total_len: usize) -> ! {
    panic!("BiVec: preallocated capacity exceeded, current length is {total_len}");
}

#[inline(never)]
#[cold]
#[track_caller]
fn index_out_of_bounds(requested: usize, len: usize) -> ! {
    panic!("BiVec: index out of bounds. Length is {len}, but requested index is {requested}");
}

// Invariants:
// - all elements in the list except for the last are full
pub struct BiVec<T: Sized, A: Allocator> {
    capacity: CapacityChunk<CapacityChunk<T, A>, A>,
    len: usize,
}

impl<T: Sized, A: Allocator> core::fmt::Debug for BiVec<T, A>
where
    T: core::fmt::Debug,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("BiVec")
            .field("len", &self.len)
            .field("content", &unsafe { self.capacity.filled_slice() })
            .finish()
    }
}

impl<T: Sized, A: Allocator> Drop for BiVec<T, A> {
    fn drop(&mut self) {
        // NOTE: we use CapacityChunk, that will drop later on. I'll take all filled elements (that are capacity chunks themselves),
        // and drop them. Each capacity chunk's drop will clear internal filled slices recursively,
        // and then drop allocation (because we assume init effectively).

        // So, this implementation should do nothing!
    }
}

impl<T: Sized, A: Allocator> BiVec<T, A> {
    const INNER_BACKING_SIZE: usize = (1usize << 12) - 64; // ~ page size minus allocator header overhead

    pub fn clear(&mut self) {
        unsafe {
            self.drop_up_to_len(self.len);
        }

        // No deallocation of inner capacities, so only length is zeroed, but not total number of initialized elements
        self.len = 0;
    }

    unsafe fn drop_up_to_len(&mut self, len: usize) {
        // NOTE: we do not want to call "drop" here and will keep inner capacities initialized. Instead we
        // will make mutable slice of all initialized elements, and clear(!) them
        if len == 0 {
            return;
        }
        let inner_capacity =
            const { CapacityChunk::<T, A>::capacity_for_backing_size(Self::INNER_BACKING_SIZE) };
        let outer_len = len.div_ceil(inner_capacity);
        unsafe {
            for el in core::slice::from_raw_parts_mut(
                self.capacity
                    .capacity
                    .as_mut_ptr()
                    .cast::<CapacityChunk<T, A>>(),
                outer_len,
            ) {
                // NOTE: it'll clear, but not de-init. Backing capacity is still available
                el.clear();
            }
        }
        // NOTE: we will NOT set filled to 0, as we do not deinit inner capacities
    }

    unsafe fn get_unchecked(&self, index: usize) -> &T {
        debug_assert!(index < self.len);
        let inner_capacity =
            const { CapacityChunk::<T, A>::capacity_for_backing_size(Self::INNER_BACKING_SIZE) };
        let outer_index = index / inner_capacity;
        let inner_index = index % inner_capacity;

        self.capacity
            .get_unchecked(outer_index)
            .get_unchecked(inner_index)
    }

    unsafe fn get_unchecked_mut(&mut self, index: usize) -> &mut T {
        debug_assert!(index < self.len);
        let inner_capacity =
            const { CapacityChunk::<T, A>::capacity_for_backing_size(Self::INNER_BACKING_SIZE) };
        let outer_index = index / inner_capacity;
        let inner_index = index % inner_capacity;

        self.capacity
            .get_unchecked_mut(outer_index)
            .get_unchecked_mut(inner_index)
    }
}

impl<T: Sized, A: Allocator + Clone> BiVec<T, A> {
    pub fn with_capacity_in(capacity: usize, allocator: A) -> Self {
        let inner_capacity =
            const { CapacityChunk::<T, A>::capacity_for_backing_size(Self::INNER_BACKING_SIZE) };
        assert!(inner_capacity > 0);
        let outer_capacity = capacity.next_multiple_of(inner_capacity) / inner_capacity;
        let capacity = CapacityChunk::with_capacity_in(outer_capacity, allocator);

        Self { capacity, len: 0 }
    }

    unsafe fn push_back_new_inner(&mut self) {
        debug_assert!(self.capacity.is_full() == false);
        let inner_capacity =
            const { CapacityChunk::<T, A>::capacity_for_backing_size(Self::INNER_BACKING_SIZE) };
        let allocator = self.capacity.allocator().clone();
        self.capacity
            .push_back_unchecked(CapacityChunk::with_capacity_in(inner_capacity, allocator));
    }
}

impl<T: Sized, A: Allocator + Clone> cc_traits::Len for BiVec<T, A> {
    fn len(&self) -> usize {
        self.len
    }

    fn is_empty(&self) -> bool {
        debug_assert!(unsafe {
            if self.len != 0 {
                self.capacity.filled_slice().iter().all(|el| el.is_empty())
            } else {
                true
            }
        });

        self.len == 0
    }
}

impl<T: Sized, A: Allocator + Clone> cc_traits::Collection for BiVec<T, A> {
    type Item = T;
}

impl<T: Sized, A: Allocator + Clone> cc_traits::CollectionRef for BiVec<T, A> {
    type ItemRef<'a>
        = &'a T
    where
        Self: 'a;

    cc_traits::covariant_item_ref!();
}

impl<T: Sized, A: Allocator + Clone> cc_traits::CollectionMut for BiVec<T, A> {
    type ItemMut<'a>
        = &'a mut T
    where
        Self: 'a;

    cc_traits::covariant_item_mut!();
}

impl<T: Sized, A: Allocator + Clone> cc_traits::SimpleCollectionRef for BiVec<T, A> {
    cc_traits::simple_collection_ref!();
}

impl<T: Sized, A: Allocator + Clone> cc_traits::SimpleCollectionMut for BiVec<T, A> {
    cc_traits::simple_collection_mut!();
}

impl<T: Sized, A: Allocator + Clone> cc_traits::Back for BiVec<T, A> {
    fn back(&self) -> Option<Self::ItemRef<'_>> {
        if self.len == 0 {
            None
        } else {
            let index = self.len - 1;
            Some(unsafe { self.get_unchecked(index) })
        }
    }
}

impl<T: Sized, A: Allocator + Clone> cc_traits::BackMut for BiVec<T, A> {
    fn back_mut(&mut self) -> Option<Self::ItemMut<'_>> {
        if self.len == 0 {
            None
        } else {
            let index = self.len - 1;
            Some(unsafe { self.get_unchecked_mut(index) })
        }
    }
}

// Stack is implemented automatically

impl<T: Sized, A: Allocator + Clone> cc_traits::PushBack for BiVec<T, A> {
    type Output = ();

    fn push_back(&mut self, element: Self::Item) -> Self::Output {
        let inner_capacity =
            const { CapacityChunk::<T, A>::capacity_for_backing_size(Self::INNER_BACKING_SIZE) };
        let outer_index = self.len / inner_capacity;
        let inner_index = self.len % inner_capacity;

        // NOTE: as we count in units of inner capacity, then our inner element is always not full
        unsafe {
            if self.capacity.filled > outer_index {
                self.capacity
                    .get_unchecked_mut(outer_index)
                    .push_back_unchecked(element);
                self.len += 1;
            } else {
                // we may need to initialize inner capacity chunk
                if inner_index == 0 {
                    if self.capacity.is_full() {
                        bivec_push_panic(self.len);
                    }
                    self.push_back_new_inner();
                }
                self.capacity
                    .get_unchecked_mut(outer_index)
                    .push_back_unchecked(element);
                self.len += 1;
            }
        }
    }
}

impl<T: Sized, A: Allocator + Clone> cc_traits::PopBack for BiVec<T, A> {
    fn pop_back(&mut self) -> Option<Self::Item> {
        if self.len == 0 {
            None
        } else {
            self.len -= 1;
            let inner_capacity = const {
                CapacityChunk::<T, A>::capacity_for_backing_size(Self::INNER_BACKING_SIZE)
            };
            let outer_index = self.len / inner_capacity;

            Some(unsafe { self.capacity.get_unchecked_mut(outer_index).pop() })
        }
    }
}

// StackMut is implemented automatically

impl<T: Sized, A: Allocator + Clone> core::ops::Index<usize> for BiVec<T, A> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        if index >= self.len {
            index_out_of_bounds(index, self.len);
        }

        unsafe { self.get_unchecked(index) }
    }
}

impl<T: Sized, A: Allocator + Clone> core::ops::IndexMut<usize> for BiVec<T, A> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        if index >= self.len {
            index_out_of_bounds(index, self.len);
        }

        unsafe { self.get_unchecked_mut(index) }
    }
}

// Vec and VecMut are implemented automatically

impl<T: Sized, A: Allocator + Clone> cc_traits::WithCapacityIn<A> for BiVec<T, A> {
    fn with_capacity_in(capacity: usize, allocator: A) -> Self {
        BiVec::<T, A>::with_capacity_in(capacity, allocator)
    }
}
