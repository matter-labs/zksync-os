use ruint::aliases::{B160, U256};
use zk_ee::{
    system::{logger::Logger, EthereumLikeTypes, MemorySubsystem},
    utils::u256_to_b160,
};

use crate::Vec;
use crate::{utils::assume, ExitCode, STACK_SIZE};

pub struct EvmStack<S: EthereumLikeTypes> {
    data: Vec<U256, <S::Memory as MemorySubsystem>::Allocator>,
}

impl<S: EthereumLikeTypes> EvmStack<S> {
    #[inline(always)]
    pub fn new(alloc: <S::Memory as MemorySubsystem>::Allocator) -> Self {
        Self {
            data: Vec::with_capacity_in(STACK_SIZE, alloc),
        }
    }

    #[inline(always)]
    pub(crate) fn raw_push_within_capacity(&mut self, value: U256) -> Result<(), U256> {
        self.data.push_within_capacity(value)
    }

    #[inline(always)]
    pub(crate) fn pop_addresses<const N: usize>(&mut self) -> Result<[B160; N], ExitCode> {
        let len = self.data.len();
        if len < N {
            return Err(ExitCode::StackUnderflow);
        }
        unsafe {
            let values =
                core::array::from_fn(|_| u256_to_b160(self.data.pop().unwrap_unchecked()));

            Ok(values)
        }
    }

    #[inline(always)]
    pub(crate) fn push_values<const N: usize>(
        &mut self,
        values: &[U256; N],
    ) -> Result<(), ExitCode> {
        if self.data.len() + N > STACK_SIZE {
            return Err(ExitCode::StackOverflow);
        }
        unsafe {
            assume(self.data.capacity() == STACK_SIZE);
        }
        self.data.extend_from_slice(values);
        Ok(())
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
    pub(crate) fn top(&mut self) -> Result<&mut U256, ExitCode> {
        let len = self.data.len();
        if len < 1 {
            return Err(ExitCode::StackUnderflow);
        }
        unsafe {
            Ok(self.top_unsafe())
        }
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

    #[allow(dead_code)]
    pub(crate) fn debug_print(&self, mut logger: impl Logger) {
        for el in self.data.iter() {
            let _ = logger.write_fmt(format_args!("{:?}\n", el));
        }
    }
}
