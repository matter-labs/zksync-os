// We want to avoid copies as much as we can, as it's sensitive for RISC-V 32 bit arch,
// and make a special stack implementation.

use crate::ExitCode;
use crate::STACK_SIZE;
use alloc::boxed::Box;
use core::{alloc::Allocator, mem::MaybeUninit};
use u256::U256;

pub struct EVMStack<A: Allocator> {
    buffer: Box<[MaybeUninit<U256>; STACK_SIZE], A>,
    // our length both indicates how many elements are there, and
    // at least how many of them are initialized
    len: usize,
}

impl<A: Allocator> EVMStack<A> {
    pub(crate) fn new_in(allocator: A) -> Self {
        Self {
            buffer: Box::new_in([const { MaybeUninit::uninit() }; STACK_SIZE], allocator),
            len: 0,
        }
    }

    pub(crate) fn iter(&'_ self) -> impl Iterator<Item = &'_ U256> {
        unsafe {
            self.buffer[..self.len]
                .iter()
                .map(|el| el.assume_init_ref())
        }
    }

    // this is kind-of overoptimization, but all push/pop ops are unrolled
    // for ABI optimizations

    #[inline(always)]
    pub(crate) fn stack_swap(&mut self, n: usize) -> Result<(), ExitCode> {
        let src_offset = if self.len == 0 {
            return Err(ExitCode::StackUnderflow);
        } else {
            self.len - 1
        };
        let dst_offset = if n > src_offset {
            return Err(ExitCode::StackUnderflow);
        } else {
            src_offset - n
        };
        unsafe {
            core::mem::swap(
                self.buffer
                    .as_mut_ptr()
                    .add(src_offset)
                    .as_mut_unchecked()
                    .assume_init_mut(),
                self.buffer
                    .as_mut_ptr()
                    .add(dst_offset)
                    .as_mut_unchecked()
                    .assume_init_mut(),
            );
        }

        Ok(())
    }

    #[inline(always)]
    pub(crate) fn stack_dup(&mut self, n: usize) -> Result<(), ExitCode> {
        if self.len == STACK_SIZE {
            return Err(ExitCode::StackOverflow);
        }
        let offset = if n > self.len {
            return Err(ExitCode::StackUnderflow);
        } else {
            self.len - n
        };
        unsafe {
            let src_ref = self
                .buffer
                .as_ptr()
                .add(offset)
                .as_ref_unchecked()
                .assume_init_ref();
            let dst_ref_mut = self.buffer.as_mut_ptr().add(self.len).as_mut_unchecked();
            U256::write_into_ptr(dst_ref_mut.as_mut_ptr(), src_ref);
        }
        self.len += 1;

        Ok(())
    }

    #[inline(always)]
    pub(crate) fn stack_reduce_one(&mut self) -> Result<(), ExitCode> {
        if self.len == 0 {
            Err(ExitCode::StackUnderflow)
        } else {
            self.len -= 1;

            Ok(())
        }
    }

    #[inline(always)]
    pub fn pop_1(&'_ mut self) -> Result<&'_ U256, ExitCode> {
        unsafe {
            if self.len < 1 {
                return Err(ExitCode::StackUnderflow);
            }
            let offset = self.len - 1;
            let p0 = self.buffer.get_unchecked(offset).assume_init_ref();
            self.len = offset;

            Ok(p0)
        }
    }

    #[inline(always)]
    pub fn pop_2(&'_ mut self) -> Result<(&'_ U256, &'_ U256), ExitCode> {
        unsafe {
            if self.len < 2 {
                return Err(ExitCode::StackUnderflow);
            }
            let mut offset = self.len - 1;
            let p0 = self.buffer.get_unchecked(offset).assume_init_ref();
            offset -= 1;
            let p1 = self.buffer.get_unchecked(offset).assume_init_ref();
            self.len = offset;

            Ok((p0, p1))
        }
    }

    #[inline(always)]
    pub fn pop_3(&'_ mut self) -> Result<(&'_ U256, &'_ U256, &'_ U256), ExitCode> {
        unsafe {
            if self.len < 3 {
                return Err(ExitCode::StackUnderflow);
            }
            let mut offset = self.len - 1;
            let p0 = self.buffer.get_unchecked(offset).assume_init_ref();
            offset -= 1;
            let p1 = self.buffer.get_unchecked(offset).assume_init_ref();
            offset -= 1;
            let p2 = self.buffer.get_unchecked(offset).assume_init_ref();

            self.len = offset;

            Ok((p0, p1, p2))
        }
    }

    #[inline(always)]
    pub fn pop_4(&'_ mut self) -> Result<(&'_ U256, &'_ U256, &'_ U256, &'_ U256), ExitCode> {
        unsafe {
            if self.len < 4 {
                return Err(ExitCode::StackUnderflow);
            }
            let mut offset = self.len - 1;
            let p0 = self.buffer.get_unchecked(offset).assume_init_ref();
            offset -= 1;
            let p1 = self.buffer.get_unchecked(offset).assume_init_ref();
            offset -= 1;
            let p2 = self.buffer.get_unchecked(offset).assume_init_ref();
            offset -= 1;
            let p3 = self.buffer.get_unchecked(offset).assume_init_ref();

            self.len = offset;

            Ok((p0, p1, p2, p3))
        }
    }

    #[inline(always)]
    pub fn peek_mut(&'_ mut self) -> Result<&'_ mut U256, ExitCode> {
        unsafe {
            if self.len < 1 {
                return Err(ExitCode::StackUnderflow);
            }
            let offset = self.len - 1;
            let peeked = self.buffer.get_unchecked_mut(offset).assume_init_mut();

            Ok(peeked)
        }
    }

    #[inline(always)]
    pub fn pop_1_and_peek_mut(&'_ mut self) -> Result<(&'_ U256, &'_ mut U256), ExitCode> {
        unsafe {
            if self.len < 2 {
                return Err(ExitCode::StackUnderflow);
            }
            let mut offset = self.len - 1;
            let p0 = self
                .buffer
                .as_ptr()
                .add(offset)
                .as_ref_unchecked()
                .assume_init_ref();
            self.len = offset;

            offset -= 1;
            let peeked = self
                .buffer
                .as_mut_ptr()
                .add(offset)
                .as_mut_unchecked()
                .assume_init_mut();

            Ok(((p0), peeked))
        }
    }

    #[inline(always)]
    pub fn pop_2_and_peek_mut(
        &'_ mut self,
    ) -> Result<((&'_ U256, &'_ U256), &'_ mut U256), ExitCode> {
        unsafe {
            if self.len < 3 {
                return Err(ExitCode::StackUnderflow);
            }
            let mut offset = self.len - 1;
            let p0 = self
                .buffer
                .as_ptr()
                .add(offset)
                .as_ref_unchecked()
                .assume_init_ref();
            offset -= 1;
            let p1 = self
                .buffer
                .as_ptr()
                .add(offset)
                .as_ref_unchecked()
                .assume_init_ref();
            self.len = offset;

            offset -= 1;
            let peeked = self
                .buffer
                .as_mut_ptr()
                .add(offset)
                .as_mut_unchecked()
                .assume_init_mut();

            Ok(((p0, p1), peeked))
        }
    }

    #[inline(always)]
    pub fn pop_1_mut_and_peek(&'_ mut self) -> Result<(&'_ mut U256, &'_ mut U256), ExitCode> {
        unsafe {
            if self.len < 2 {
                return Err(ExitCode::StackUnderflow);
            }
            let mut offset = self.len - 1;
            let p0 = self
                .buffer
                .as_mut_ptr()
                .add(offset)
                .as_mut_unchecked()
                .assume_init_mut();
            self.len = offset;

            offset -= 1;
            let peeked = self
                .buffer
                .as_mut_ptr()
                .add(offset)
                .as_mut_unchecked()
                .assume_init_mut();

            Ok((p0, peeked))
        }
    }

    #[inline(always)]
    pub fn pop_2_mut_and_peek(
        &'_ mut self,
    ) -> Result<((&'_ mut U256, &'_ mut U256), &'_ mut U256), ExitCode> {
        unsafe {
            if self.len < 2 {
                return Err(ExitCode::StackUnderflow);
            }
            let mut offset = self.len - 1;
            let p0 = self
                .buffer
                .as_mut_ptr()
                .add(offset)
                .as_mut_unchecked()
                .assume_init_mut();

            offset -= 1;
            let p1 = self
                .buffer
                .as_mut_ptr()
                .add(offset)
                .as_mut_unchecked()
                .assume_init_mut();
            self.len = offset;

            offset -= 1;
            let peeked = self
                .buffer
                .as_mut_ptr()
                .add(offset)
                .as_mut_unchecked()
                .assume_init_mut();

            Ok(((p0, p1), peeked))
        }
    }

    #[inline(always)]
    pub fn push_unchecked(&mut self, value: &U256) {
        unsafe {
            let dst_ref_mut = self.buffer.as_mut_ptr().add(self.len).as_mut_unchecked();
            U256::write_into_ptr(dst_ref_mut.as_mut_ptr(), value);
        }
        self.len += 1;
    }

    #[inline(always)]
    pub fn push_zero(&mut self) -> Result<(), ExitCode> {
        let new_len = self.len + 1;
        if new_len >= STACK_SIZE {
            return Err(ExitCode::StackOverflow);
        }
        unsafe {
            let dst_ref_mut = self.buffer.as_mut_ptr().add(self.len).as_mut_unchecked();
            U256::write_zero_into_ptr(dst_ref_mut.as_mut_ptr());
            self.len += 1;
        }

        Ok(())
    }

    #[inline(always)]
    pub fn push_one(&mut self) -> Result<(), ExitCode> {
        let new_len = self.len + 1;
        if new_len >= STACK_SIZE {
            return Err(ExitCode::StackOverflow);
        }
        unsafe {
            let dst_ref_mut = self.buffer.as_mut_ptr().add(self.len).as_mut_unchecked();
            U256::write_one_into_ptr(dst_ref_mut.as_mut_ptr());
            self.len += 1;
        }

        Ok(())
    }

    #[inline(always)]
    pub fn push_1(&mut self, value: &U256) -> Result<(), ExitCode> {
        let new_len = self.len + 1;
        if new_len >= STACK_SIZE {
            return Err(ExitCode::StackOverflow);
        }
        unsafe {
            let dst_ref_mut = self.buffer.as_mut_ptr().add(self.len).as_mut_unchecked();
            U256::write_into_ptr(dst_ref_mut.as_mut_ptr(), value);
            self.len += 1;
        }

        Ok(())
    }

    #[inline(always)]
    pub fn push_2(&mut self, val0: &U256, val1: &U256) -> Result<(), ExitCode> {
        let new_len = self.len + 2;
        if new_len >= STACK_SIZE {
            return Err(ExitCode::StackOverflow);
        }
        unsafe {
            let dst_ref_mut = self.buffer.as_mut_ptr().add(self.len).as_mut_unchecked();
            U256::write_into_ptr(dst_ref_mut.as_mut_ptr(), val0);
            self.len += 1;

            let dst_ref_mut = self.buffer.as_mut_ptr().add(self.len).as_mut_unchecked();
            U256::write_into_ptr(dst_ref_mut.as_mut_ptr(), val1);
            self.len += 1;
        }

        Ok(())
    }
}
