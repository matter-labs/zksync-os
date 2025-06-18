use core::hint::assert_unchecked;
use core::iter::Extend;
use core::mem::{ManuallyDrop, MaybeUninit};
use core::ops::{Deref, DerefMut};
use core::ptr;

#[derive(Default)]
pub struct SliceVec<'a, T> {
    memory: &'a mut [MaybeUninit<T>],
    length: usize,
}

impl<'a, T> SliceVec<'a, T> {
    pub fn new(memory: &'a mut [MaybeUninit<T>]) -> Self {
        Self { memory, length: 0 }
    }

    pub fn destruct(self) -> (&'a mut [T], &'a mut [MaybeUninit<T>]) {
        let me = ManuallyDrop::new(self);
        unsafe {
            let memory = core::ptr::read(&me.memory);
            let (initialized, uninitialized) = memory.split_at_mut_unchecked(me.length);
            let initialized = &mut *(initialized as *mut [MaybeUninit<T>] as *mut [T]);
            (initialized, uninitialized)
        }
    }

    /// Returns the current contents as a slice and a new empty `SliceVec` that uses the rest of the backing slice.
    pub fn freeze(&mut self) -> (&mut [T], SliceVec<T>) {
        unsafe {
            let (locked, free) = self.memory.split_at_mut_unchecked(self.length);
            let locked = &mut *(locked as *mut [MaybeUninit<T>] as *mut [T]);
            (locked, SliceVec::new(free))
        }
    }

    /// Drops all contents of the `SliceVec`.
    pub fn clear(&mut self) {
        for x in &mut self.memory[..self.length] {
            unsafe { x.assume_init_drop() };
        }
        self.length = 0;
    }

    pub fn try_push(&mut self, x: T) -> Result<(), ()> {
        self.memory
            .get_mut(self.length)
            .map(|m| {
                m.write(x);
                self.length += 1;
            })
            .ok_or(())
    }

    pub fn pop(&mut self) -> Option<T> {
        if self.length == 0 {
            None
        } else {
            self.length -= 1;
            Some(unsafe { self.memory[self.length].assume_init_read() })
        }
    }

    pub fn top(&mut self) -> Option<&mut T> {
        if self.length == 0 {
            None
        } else {
            Some(unsafe { self.memory[self.length - 1].assume_init_mut() })
        }
    }
}

impl<T: Clone> SliceVec<'_, T> {
    /// Resizes the `SliceVec` to the requested length.
    /// Adds copies of `padding` to the end if the size increases.
    pub fn resize(&mut self, new_length: usize, padding: T) -> Result<(), ()> {
        if new_length >= self.memory.len() {
            return Err(());
        }

        if new_length > self.length {
            for x in &mut self.memory[self.length..new_length] {
                x.write(padding.clone());
            }
        }
        if new_length < self.length {
            unsafe {
                assert_unchecked(self.length <= self.memory.len());
                ptr::drop_in_place(
                    &mut self.memory[new_length..self.length] as *mut [MaybeUninit<T>]
                        as *mut [T],
                );
            }
        }
        self.length = new_length;

        Ok(())
    }
}

impl<T> Deref for SliceVec<'_, T> {
    type Target = [T];
    fn deref(&self) -> &[T] {
        unsafe {
            let initialized_part = self.memory.get_unchecked(..self.length);
            &*(initialized_part as *const [MaybeUninit<T>] as *const [T])
        }
    }
}

impl<T> DerefMut for SliceVec<'_, T> {
    fn deref_mut(&mut self) -> &mut [T] {
        unsafe {
            let initialized_part = self.memory.get_unchecked_mut(..self.length);
            &mut *(initialized_part as *mut [MaybeUninit<T>] as *mut [T])
        }
    }
}

impl<T> Extend<T> for SliceVec<'_, T> {
    fn extend<I>(&mut self, iter: I)
    where
        I: IntoIterator<Item = T>,
    {
        for (m, x) in self.memory[self.length..].iter_mut().zip(iter) {
            m.write(x);
            self.length += 1;
        }
        // If there is not enough memory left, the whole iterator will not be consumed!
    }
}

impl<T> Drop for SliceVec<'_, T> {
    fn drop(&mut self) {
        self.clear();
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn basic() {
        let mut memory = [MaybeUninit::uninit(); 10];
        let mut slice_vec = SliceVec::new(&mut memory);

        slice_vec.extend(0..5);
        assert_eq!(*slice_vec, [0, 1, 2, 3, 4]);
        slice_vec.resize(3, 0).unwrap();
        assert_eq!(*slice_vec, [0, 1, 2]);
        slice_vec.resize(5, 0).unwrap();
        assert_eq!(*slice_vec, [0, 1, 2, 0, 0]);

        let (slice, mut slice_vec) = slice_vec.freeze();
        assert_eq!(slice, &[0, 1, 2, 0, 0]);
        slice_vec.extend(5..10);
        assert_eq!(*slice_vec, [5, 6, 7, 8, 9]);
        slice_vec.clear();
        assert_eq!(*slice_vec, []);
    }

    #[test]
    fn recursion() {
        let mut memory = [MaybeUninit::uninit(); 100];
        let slice_vec = SliceVec::new(&mut memory);
        r(slice_vec, 7, &[8]);
    }

    fn r(mut s: SliceVec<u8>, n: u8, prev: &[u8]) {
        if n > 0 {
            s.resize(1, n).unwrap();

            let (mine, next) = s.freeze();
            r(next, n - 1, &mine);

            let (mine, next) = s.freeze();
            r(next, n - 1, &mine);

            assert_eq!(*s, [n]);
            assert_eq!(prev[0], n + 1);
        }
    }
}
