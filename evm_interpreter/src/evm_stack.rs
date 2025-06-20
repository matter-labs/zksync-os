use core::alloc::Allocator;
use ruint::aliases::U256;
use zk_ee::system::logger::Logger;

use crate::Vec;
use crate::{utils::assume, ExitCode, STACK_SIZE};

pub struct EvmStack<A: Allocator> {
    data: Vec<U256, A>,
}

impl<A: Allocator> EvmStack<A> {
    #[inline(always)]
    pub fn new_in(alloc: A) -> Self {
        Self {
            data: Vec::with_capacity_in(STACK_SIZE, alloc),
        }
    }

    #[allow(dead_code)]
    pub(crate) fn print_stack_top(&self, logger: &mut impl Logger) {
        if let Some(el) = self.data.last() {
            let _ = logger.write_fmt(format_args!("Stack top = 0x{:x}\n", el));
        } else {
            let _ = logger.write_str("Stack top = empty\n");
        }
    }

    #[allow(dead_code)]
    pub(crate) fn print_stack_content(&self, logger: &mut impl Logger) {
        let _ = logger.write_fmt(format_args!("DEPTH MAX\n"));
        for el in self.data.iter().rev()
        // TODO rev?
        {
            let _ = logger.write_fmt(format_args!("{:x}\n", el));
        }
        let _ = logger.write_fmt(format_args!("DEPTH 0\n"));
    }

    #[inline(always)]
    pub(crate) fn push(&mut self, value: U256) -> Result<(), ExitCode> {
        unsafe {
            assume(self.data.capacity() == STACK_SIZE);
        }
        if self.data.push_within_capacity(value).is_err() {
            return Err(ExitCode::StackOverflow);
        }

        Ok(())
    }

    #[inline(always)]
    pub(crate) fn top_mut(&mut self) -> Result<&mut U256, ExitCode> {
        let len = self.data.len();
        if len < 1 {
            return Err(ExitCode::StackUnderflow);
        }
        unsafe { Ok(self.top_unsafe()) }
    }

    /// The caller is responsible for checking the length of the stack.
    #[inline(always)]
    pub unsafe fn top_unsafe(&mut self) -> &mut U256 {
        let len = self.data.len();
        self.data.get_unchecked_mut(len - 1)
    }

    #[inline(always)]
    pub(crate) fn pop_values<const N: usize>(&mut self) -> Result<[U256; N], ExitCode> {
        let len = self.data.len();
        if len < N {
            return Err(ExitCode::StackUnderflow);
        }
        unsafe {
            let values = core::array::from_fn(|_| self.data.pop().unwrap_unchecked());

            Ok(values)
        }
    }

    #[inline(always)]
    pub(crate) fn pop_values_and_peek<const N: usize>(
        &mut self,
    ) -> Result<([U256; N], &mut U256), ExitCode> {
        let len = self.data.len();
        if len < N + 1 {
            return Err(ExitCode::StackUnderflow);
        }
        unsafe {
            let values = core::array::from_fn(|_| self.data.pop().unwrap_unchecked());
            let idx = self.data.len() - 1;
            Ok((values, self.data.get_unchecked_mut(idx)))
        }
    }

    #[inline(always)]
    pub(crate) fn swap(&mut self, n: usize) -> Result<(), ExitCode> {
        unsafe {
            assume(self.data.capacity() == STACK_SIZE);
        }
        let len = self.data.len();
        let src_offset = if len == 0 {
            return Err(ExitCode::StackUnderflow);
        } else {
            len - 1
        };
        let dst_offset = if n > src_offset {
            return Err(ExitCode::StackUnderflow);
        } else {
            src_offset - n
        };
        unsafe {
            self.data.swap_unchecked(src_offset, dst_offset);
        }

        Ok(())
    }

    #[inline(always)]
    pub(crate) fn dup(&mut self, n: usize) -> Result<(), ExitCode> {
        if self.data.len() == STACK_SIZE {
            return Err(ExitCode::StackOverflow);
        }
        unsafe {
            assume(self.data.capacity() == STACK_SIZE);
        }
        let len = self.data.len();
        let offset = if n > len {
            return Err(ExitCode::StackUnderflow);
        } else {
            len - n
        };

        let value = unsafe { *self.data.get_unchecked(offset) };
        unsafe {
            assume(self.data.len() < self.data.capacity());
        }
        self.data.push(value);

        Ok(())
    }

    #[inline(always)]
    pub(crate) fn reduce_one(&mut self) -> Result<(), ExitCode> {
        unsafe {
            assume(self.data.capacity() == STACK_SIZE);
        }
        let len = self.data.len();
        if len == 0 {
            Err(ExitCode::StackUnderflow)
        } else {
            unsafe {
                self.data.set_len(len - 1);
            }

            Ok(())
        }
    }
}
