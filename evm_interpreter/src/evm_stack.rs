// We want to avoid copies as much as we can, as it's sensitive for RISC-V 32 bit arch,
// and make a special stack implementation.

use crate::ExitCode;
use crate::STACK_SIZE;
use alloc::boxed::Box;
use core::{alloc::Allocator, mem::MaybeUninit};
use ruint::aliases::U256;
use zk_ee::system::evm::EvmStackInterface;
use zk_ee::system::logger::Logger;
use zksync_os_evm_errors::EvmError;

pub struct EvmStack<A: Allocator> {
    buffer: Box<[MaybeUninit<U256>; STACK_SIZE], A>,
    // our length both indicates how many elements are there, and
    // at least how many of them are initialized
    len: usize,
}

impl<A: Allocator> EvmStack<A> {
    pub(crate) fn new_in(allocator: A) -> Self {
        Self {
            buffer: Box::new_in([const { MaybeUninit::uninit() }; STACK_SIZE], allocator),
            len: 0,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn print_stack_top(&self, logger: &mut impl Logger) {
        unsafe {
            if let Some(el) =
                core::slice::from_raw_parts(self.buffer.as_ptr().cast::<U256>(), self.len).last()
            {
                let _ = logger.write_fmt(format_args!("Stack top = 0x{el:x}\n"));
            } else {
                let _ = logger.write_str("Stack top = empty\n");
            }
        }
    }

    #[allow(dead_code)]
    pub(crate) fn print_stack_content(&self, logger: &mut impl Logger) {
        unsafe {
            let _ = logger.write_fmt(format_args!("DEPTH MAX\n"));
            for el in core::slice::from_raw_parts(self.buffer.as_ptr().cast::<U256>(), self.len)
                .iter()
                .rev()
            {
                let _ = logger.write_fmt(format_args!("{el:x}\n"));
            }
            let _ = logger.write_fmt(format_args!("DEPTH 0\n"));
        }
    }

    // this is kind-of overoptimization, but all push/pop ops are unrolled
    // for ABI optimizations

    #[inline(always)]
    pub(crate) fn swap(&mut self, n: usize) -> Result<(), ExitCode> {
        let src_offset = if self.len == 0 {
            return Err(EvmError::StackUnderflow.into());
        } else {
            self.len - 1
        };
        let dst_offset = if n > src_offset {
            return Err(EvmError::StackUnderflow.into());
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
    pub(crate) fn dup(&mut self, n: usize) -> Result<(), ExitCode> {
        if self.len == STACK_SIZE {
            return Err(EvmError::StackOverflow.into());
        }
        let offset = if n > self.len {
            return Err(EvmError::StackUnderflow.into());
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
            dst_ref_mut.as_mut_ptr().write(*src_ref);
        }
        self.len += 1;

        Ok(())
    }

    #[inline(always)]
    pub(crate) fn pop_and_ignore(&mut self) -> Result<(), ExitCode> {
        if self.len == 0 {
            Err(EvmError::StackUnderflow.into())
        } else {
            self.len -= 1;

            Ok(())
        }
    }

    #[inline(always)]
    pub fn pop_1(&'_ mut self) -> Result<&'_ U256, ExitCode> {
        unsafe {
            if self.len < 1 {
                return Err(EvmError::StackUnderflow.into());
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
                return Err(EvmError::StackUnderflow.into());
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
                return Err(EvmError::StackUnderflow.into());
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
                return Err(EvmError::StackUnderflow.into());
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
    pub fn top_mut(&'_ mut self) -> Result<&'_ mut U256, ExitCode> {
        unsafe {
            if self.len < 1 {
                return Err(EvmError::StackUnderflow.into());
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
                return Err(EvmError::StackUnderflow.into());
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
                return Err(EvmError::StackUnderflow.into());
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
                return Err(EvmError::StackUnderflow.into());
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
    pub fn push_zero(&mut self) -> Result<(), ExitCode> {
        if self.len == STACK_SIZE {
            return Err(EvmError::StackOverflow.into());
        }
        unsafe {
            let dst_ref_mut = self.buffer.as_mut_ptr().add(self.len).as_mut_unchecked();
            dst_ref_mut.as_mut_ptr().write(U256::ZERO);
            self.len += 1;
        }

        Ok(())
    }

    #[inline(always)]
    pub fn push_one(&mut self) -> Result<(), ExitCode> {
        if self.len == STACK_SIZE {
            return Err(EvmError::StackOverflow.into());
        }
        unsafe {
            let dst_ref_mut = self.buffer.as_mut_ptr().add(self.len).as_mut_unchecked();
            dst_ref_mut.as_mut_ptr().write(U256::ONE);
            self.len += 1;
        }

        Ok(())
    }

    #[inline(always)]
    pub fn push(&mut self, value: &U256) -> Result<(), ExitCode> {
        if self.len == STACK_SIZE {
            return Err(EvmError::StackOverflow.into());
        }
        unsafe {
            let dst_ref_mut = self.buffer.as_mut_ptr().add(self.len).as_mut_unchecked();
            dst_ref_mut.as_mut_ptr().write(*value);
            self.len += 1;
        }

        Ok(())
    }
}

impl<A: Allocator> EvmStackInterface for EvmStack<A> {
    fn to_slice(&self) -> &[U256] {
        unsafe { core::slice::from_raw_parts(self.buffer.as_ptr().cast::<U256>(), self.len) }
    }

    fn len(&self) -> usize {
        self.len
    }

    fn peek_n(&self, index: usize) -> Result<&U256, EvmError> {
        unsafe {
            if self.len < index + 1 {
                return Err(EvmError::StackUnderflow);
            }
            let offset = self.len - (index + 1);
            let p0 = self
                .buffer
                .as_ptr()
                .add(offset)
                .as_ref()
                .expect("Should not be null")
                .assume_init_ref();

            Ok(p0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::EvmStack;
    use crate::STACK_SIZE;
    use ruint::aliases::U256;
    use std::alloc::Global;
    use zksync_os_evm_errors::EvmError;

    #[test]
    fn push_then_pop_works() {
        let mut stack = EvmStack::new_in(Global);

        stack.push(&U256::ONE).expect("Should push");
        let res = stack.pop_1().expect("Should pop");

        assert_eq!(*res, U256::ONE);
    }

    #[test]
    fn push_can_not_overflow() {
        let mut stack = EvmStack::new_in(Global);

        for _ in 0..STACK_SIZE {
            stack.push(&U256::ONE).expect("Should push");
        }

        assert_eq!(stack.push(&U256::ONE), Err(EvmError::StackOverflow.into()));
    }

    #[test]
    fn push0_can_not_overflow() {
        let mut stack = EvmStack::new_in(Global);

        for _ in 0..STACK_SIZE {
            stack.push_zero().expect("Should push");
        }

        assert_eq!(stack.push_zero(), Err(EvmError::StackOverflow.into()));
    }

    #[test]
    fn push_one_can_not_overflow() {
        let mut stack = EvmStack::new_in(Global);

        for _ in 0..STACK_SIZE {
            stack.push_one().expect("Should push");
        }

        assert_eq!(stack.push_one(), Err(EvmError::StackOverflow.into()));
    }

    #[test]
    fn pop_can_not_underflow() {
        let mut stack = EvmStack::new_in(Global);

        assert_eq!(stack.pop_1(), Err(EvmError::StackUnderflow.into()));
        assert_eq!(stack.pop_2(), Err(EvmError::StackUnderflow.into()));
        assert_eq!(stack.pop_3(), Err(EvmError::StackUnderflow.into()));
        assert_eq!(stack.pop_4(), Err(EvmError::StackUnderflow.into()));

        stack.push_one().expect("Should push");

        assert_eq!(stack.pop_2(), Err(EvmError::StackUnderflow.into()));
        assert_eq!(stack.pop_3(), Err(EvmError::StackUnderflow.into()));
        assert_eq!(stack.pop_4(), Err(EvmError::StackUnderflow.into()));

        stack.push_one().expect("Should push");

        assert_eq!(stack.pop_3(), Err(EvmError::StackUnderflow.into()));
        assert_eq!(stack.pop_4(), Err(EvmError::StackUnderflow.into()));

        stack.push_one().expect("Should push");

        assert_eq!(stack.pop_4(), Err(EvmError::StackUnderflow.into()));
    }

    #[test]
    fn pop_and_peek_can_not_underflow() {
        let mut stack = EvmStack::new_in(Global);

        stack.push_one().expect("Should push");

        assert_eq!(
            stack.pop_1_and_peek_mut(),
            Err(EvmError::StackUnderflow.into())
        );
        assert_eq!(
            stack.pop_1_mut_and_peek(),
            Err(EvmError::StackUnderflow.into())
        );
        assert_eq!(
            stack.pop_2_and_peek_mut(),
            Err(EvmError::StackUnderflow.into())
        );

        stack.push_one().expect("Should push");

        assert_eq!(
            stack.pop_2_and_peek_mut(),
            Err(EvmError::StackUnderflow.into())
        );
    }

    #[test]
    fn top_mut_can_not_underflow() {
        let mut stack = EvmStack::new_in(Global);

        assert_eq!(stack.top_mut(), Err(EvmError::StackUnderflow.into()));
    }

    #[test]
    fn swap() {
        let mut stack = EvmStack::new_in(Global);

        assert_eq!(stack.swap(1), Err(EvmError::StackUnderflow.into()));

        stack.push_one().expect("Should push");

        assert_eq!(stack.swap(1), Err(EvmError::StackUnderflow.into()));

        stack.push_zero().expect("Should push");
        stack.swap(1).expect("Should swap");

        let (p0, p1) = stack.pop_2().expect("Should pop");

        assert_eq!(*p0, U256::ONE);
        assert_eq!(*p1, U256::ZERO);
    }

    #[test]
    fn dup() {
        let mut stack = EvmStack::new_in(Global);

        assert_eq!(stack.dup(1), Err(EvmError::StackUnderflow.into()));

        stack.push_one().expect("Should push");
        stack.dup(1).expect("Should dup");

        let (p0, p1) = stack.pop_2().expect("Should pop");

        assert_eq!(*p0, U256::ONE);
        assert_eq!(*p1, U256::ONE);

        stack.push_one().expect("Should push");

        for _ in 0..STACK_SIZE - 1 {
            stack.dup(1).expect("Should dup");
        }

        assert_eq!(stack.dup(1), Err(EvmError::StackOverflow.into()));
    }
}
