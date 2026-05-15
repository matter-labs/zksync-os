// We want to avoid copies as much as we can, as it's sensitive for RISC-V 32 bit arch,
// and make a special stack implementation.

use crate::ExitCode;
use crate::STACK_SIZE;
use alloc::boxed::Box;
use core::{alloc::Allocator, mem::MaybeUninit};
use u256::U256;
use zk_ee::logger_log;
use zk_ee::system::evm::EvmError;
use zk_ee::system::evm::EvmStackInterface;
use zk_ee::system::logger::Logger;

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
                logger_log!(logger, "Stack top = 0x{el:x}\n");
            } else {
                let _ = logger.write_str("Stack top = empty\n");
            }
        }
    }

    #[allow(dead_code)]
    pub(crate) fn print_stack_content(&self, logger: &mut impl Logger) {
        unsafe {
            logger_log!(logger, "DEPTH MAX\n");
            for el in core::slice::from_raw_parts(self.buffer.as_ptr().cast::<U256>(), self.len)
                .iter()
                .rev()
            {
                logger_log!(logger, "{el:x}\n");
            }
            logger_log!(logger, "DEPTH 0\n");
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
            // SAFETY: `src_offset` and `dst_offset` are both within the initialized prefix of
            // the stack, so both slots contain valid `U256` values. They are swapped in place
            // without creating or dropping any extra values.
            let src = self.buffer.as_mut_ptr().add(src_offset).cast::<U256>();
            let dst = self.buffer.as_mut_ptr().add(dst_offset).cast::<U256>();
            U256::swap_in_place(src, dst);
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
            U256::write_into_ptr(dst_ref_mut.as_mut_ptr(), src_ref);
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

    /// Pop 2 elements as mutable and peek the next one as mutable.
    /// Returns ((op1_mut, op2_mut), peek_mut) where op1 was on top.
    /// After the call, the stack length is reduced by 2 (the peeked element stays).
    #[inline(always)]
    pub fn pop_2_mut_and_peek(
        &'_ mut self,
    ) -> Result<((&'_ mut U256, &'_ mut U256), &'_ mut U256), ExitCode> {
        unsafe {
            if self.len < 3 {
                return Err(EvmError::StackUnderflow.into());
            }
            // SAFETY: `p0`, `p1`, and `peeked` are derived from stack slots `len - 1`,
            // `len - 2`, and `len - 3` respectively. The `self.len < 3` guard ensures
            // those offsets exist and are pairwise distinct, so the returned mutable
            // references do not alias.
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
    pub fn push_zero(&mut self) -> Result<(), ExitCode> {
        if self.len == STACK_SIZE {
            return Err(EvmError::StackOverflow.into());
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
        if self.len == STACK_SIZE {
            return Err(EvmError::StackOverflow.into());
        }
        unsafe {
            let dst_ref_mut = self.buffer.as_mut_ptr().add(self.len).as_mut_unchecked();
            U256::write_one_into_ptr(dst_ref_mut.as_mut_ptr());
            self.len += 1;
        }

        Ok(())
    }

    /// Push a u64 value as U256 directly into the next stack slot.
    /// Uses a single delegation (zero + limb write) instead of the
    /// two-delegation path of U256::from(u64) + push.
    #[inline(always)]
    pub fn push_u64(&mut self, value: u64) -> Result<(), ExitCode> {
        if self.len == STACK_SIZE {
            return Err(EvmError::StackOverflow.into());
        }
        unsafe {
            let dst = self.buffer.as_mut_ptr().add(self.len).as_mut_unchecked();
            U256::write_u64_into_ptr(dst.as_mut_ptr(), value);
            self.len += 1;
        }

        Ok(())
    }

    /// Push a B160 address as U256 directly into the next stack slot.
    /// Zeros the slot via delegation, then copies the 3 address limbs.
    #[inline(always)]
    pub fn push_b160(&mut self, addr: ruint::aliases::B160) -> Result<(), ExitCode> {
        if self.len == STACK_SIZE {
            return Err(EvmError::StackOverflow.into());
        }
        unsafe {
            let dst = self.buffer.as_mut_ptr().add(self.len).as_mut_unchecked();
            U256::write_zero_into_ptr(dst.as_mut_ptr());
            let slot = &mut *dst.as_mut_ptr().cast::<U256>();
            slot.as_limbs_mut()[0] = addr.as_limbs()[0];
            slot.as_limbs_mut()[1] = addr.as_limbs()[1];
            slot.as_limbs_mut()[2] = addr.as_limbs()[2];
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
            U256::write_into_ptr(dst_ref_mut.as_mut_ptr(), value);
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
    use std::alloc::Global;
    use u256::U256;
    use zk_ee::system::evm::EvmError;

    #[test]
    fn push_then_pop_works() {
        let mut stack = EvmStack::new_in(Global);

        let one = U256::one();
        stack.push(&one).expect("Should push");
        let res = stack.pop_1().expect("Should pop");

        assert_eq!(*res, one);
    }

    #[test]
    fn push_can_not_overflow() {
        let mut stack = EvmStack::new_in(Global);

        let one = U256::one();
        for _ in 0..STACK_SIZE {
            stack.push(&one).expect("Should push");
        }

        assert_eq!(stack.push(&one), Err(EvmError::StackOverflow.into()));
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

        assert_eq!(*p0, U256::one());
        assert_eq!(*p1, U256::zero());
    }

    #[test]
    fn dup() {
        let mut stack = EvmStack::new_in(Global);

        assert_eq!(stack.dup(1), Err(EvmError::StackUnderflow.into()));

        stack.push_one().expect("Should push");
        stack.dup(1).expect("Should dup");

        let (p0, p1) = stack.pop_2().expect("Should pop");

        assert_eq!(*p0, U256::one());
        assert_eq!(*p1, U256::one());

        stack.push_one().expect("Should push");

        for _ in 0..STACK_SIZE - 1 {
            stack.dup(1).expect("Should dup");
        }

        assert_eq!(stack.dup(1), Err(EvmError::StackOverflow.into()));
    }

    #[test]
    fn push_u64_works() {
        let mut stack = EvmStack::new_in(Global);

        stack.push_u64(42).expect("Should push");
        let res = stack.pop_1().expect("Should pop");

        assert_eq!(*res, U256::from_limbs([42, 0, 0, 0]));
    }

    #[test]
    fn push_u64_overflow() {
        let mut stack = EvmStack::new_in(Global);

        for _ in 0..STACK_SIZE {
            stack.push_u64(1).expect("Should push");
        }
        assert_eq!(stack.push_u64(1), Err(EvmError::StackOverflow.into()));
    }

    #[test]
    fn push_u64_max() {
        let mut stack = EvmStack::new_in(Global);

        stack.push_u64(u64::MAX).expect("Should push");
        let res = stack.pop_1().expect("Should pop");

        assert_eq!(*res, U256::from_limbs([u64::MAX, 0, 0, 0]));
    }
}
