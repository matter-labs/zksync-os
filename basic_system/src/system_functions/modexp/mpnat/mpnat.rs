// Adapted from https://github.com/aurora-is-near/aurora-engine

use super::{arith::*, U256};

extern crate alloc;
use alloc::vec::Vec;
use zk_ee::system::logger::Logger;
use zk_ee::system_io_oracle::{ArithmeticsParam, IOOracle};
use core::alloc::Allocator;
use core::cell::LazyCell;
use core::marker::PhantomData;
use zk_ee::system_io_oracle::Arithmetics;

use alloc::boxed::Box;

pub(crate) static U256_ZERO: U256 = U256::ZERO;

static mut Q_U8_SCRATCH: *mut () = core::ptr::null_mut();
static mut R_U8_SCRATCH: *mut () = core::ptr::null_mut();
static mut R_U256_SCRATCH: *mut () = core::ptr::null_mut();
static mut RCHECK_U256_SCRATCH: *mut () = core::ptr::null_mut();
static mut D_U256_SCRATCH: *mut () = core::ptr::null_mut();
static mut Q_U256_SCRATCH: *mut () = core::ptr::null_mut();

struct OpaqueRef<'a, T, A: Allocator + Clone> {
    alloc: A,
    ptr: &'a mut *mut (),
    phantom: PhantomData<&'a mut T>,
}

impl<'a, T, A: Allocator + Clone> OpaqueRef<'a, T, A> {
    fn access(r: &'a mut *mut (), alloc: A) -> Self {
        Self { ptr: r, alloc, phantom: PhantomData }
    }

    fn as_mut(&mut self) -> Option<&mut T> {
        unsafe { self.ptr.cast::<T>().as_mut() }
    }

    fn set<L: Logger>(&mut self, value: T, logger: &mut L) {
        self.drop_if_needed(logger);

        let boxed = Box::new_in(value, self.alloc.clone());

        let (ptr, _) = Box::into_raw_with_allocator(boxed);

        *self.ptr = ptr as *mut ();
    }

    fn drop_if_needed<L: Logger>(&mut self, logger: &mut L) {
        if self.ptr.is_null() == false {
            unsafe { Box::from_raw_in(self.ptr.cast::<T>(), self.alloc.clone()) };
            *self.ptr = core::ptr::null_mut();
        }
    }
}

impl<'a, T, A: Allocator + Clone> OpaqueRef<'a, Vec<T, A>, A> {
    fn prepared<L: Logger>(&mut self, required_cap: usize, cap_factor: usize, logger: &mut L) -> &mut Vec<T, A> {
        let alloc = self.alloc.clone();

        match self.as_mut() {
            Some(x) if x.capacity() < required_cap => {
                *x = Vec::<T, A>::with_capacity_in(required_cap * cap_factor, alloc);
            },
            None => {
                self.set(Vec::<T, A>::with_capacity_in(required_cap * cap_factor, self.alloc.clone()), logger);
            },
            _ => {}
        };

        self.as_mut().unwrap()
    }
}

/// Multi-precision natural number, represented in base `Word::MAX + 1 = 2^WORD_BITS`.
/// The digits are stored in little-endian order, i.e. digits[0] is the least
/// significant digit.
#[derive(Debug)]
pub struct MPNatU256<A: Allocator + Clone> {
    pub digits: Vec<U256, A>,
}

impl<A: Allocator + Clone> MPNatU256<A> {

    pub fn from_big_endian(bytes: &[u8], allocator: A) -> Self {
        if bytes.is_empty() {
            let vec = Vec::with_capacity_in(0, allocator.clone());
            return Self {
                digits: vec,
            };
        }
        // Remainder on division by WORD_BYTES
        let r = bytes.len() & (U256::BYTES - 1);
        let n_digits = if r == 0 {
            bytes.len() / U256::BYTES
        } else {
            // Need an extra digit for the remainder
            (bytes.len() / U256::BYTES) + 1
        };

        let mut digits = Vec::with_capacity_in(n_digits, allocator.clone());
        digits.resize_with(n_digits, || U256::ZERO);

        // buffer to hold Word-sized slices of the input bytes
        let mut buf = [0u8; U256::BYTES];
        let mut i = n_digits - 1;
        if r != 0 {
            buf[(U256::BYTES - r)..].copy_from_slice(&bytes[0..r]);
            digits[i] = U256::from_be_bytes(&buf);
            if i == 0 {
                // Special case where there is just one digit
                return Self { digits };
            }
            i -= 1;
        }
        let mut j = r;
        loop {
            let next_j = j + U256::BYTES;
            buf.copy_from_slice(&bytes[j..next_j]);
            digits[i] = U256::from_be_bytes(&buf);
            if i == 0 {
                break;
            } else {
                i -= 1;
                j = next_j;
            }
        }
        // throw away leading zeros
        while digits.len() > 1 && digits[digits.len() - 1] == U256::ZERO {
            digits.pop();
        }
        Self { digits }
    }

    pub fn from_little_endian_into<L: Logger>(bytes: &[u8], out: &mut Vec<U256, A>, logger: &mut L, allocator: A) {
        if bytes.is_empty() {
            out.clear();
            return;
        }

        // Remainder on division by WORD_BYTES
        let r = bytes.len() & (U256::BYTES - 1);
        let (n_digits, full_digits) = if r == 0 {
            let ws = bytes.len() / U256::BYTES;
            (ws, ws)
        } else {
            // Need an extra digit for the remainder
            let ws = bytes.len() / U256::BYTES;
            (ws + 1, ws)
        };

        if out.capacity() < n_digits {
            *out = Vec::with_capacity_in(n_digits, allocator.clone());
        }

        logger.write_fmt(format_args!("le cap {}", out.capacity()));
        logger.write_fmt(format_args!("in len {}", bytes.len()));

        let mut digits = out;

        // buffer to hold Word-sized slices of the input bytes
        let mut buf = [0u8; U256::BYTES];

        let mut i_b = 0;
        let mut i_w = 0;

        logger.write_str("le 0");

        loop {
            if i_w == full_digits { break; }
            let next = i_b + U256::BYTES;
            buf.copy_from_slice(&bytes[i_b..next]);

            digits.push(U256::from_le_bytes(&buf));

            i_w += 1;
            i_b = next;
        }

        logger.write_str("le 1");

        if r != 0 {
            buf[..r].copy_from_slice(&bytes[i_b..]);
            buf[i_w + r..].iter_mut().for_each(|x| *x = 0);

            digits.push(U256::from_le_bytes(&buf));
        }
        logger.write_str("le 2");
    }

    pub fn from_little_endian<L: Logger>(bytes: &[u8], logger: &mut L, allocator: A) -> Self {
        if bytes.is_empty() {
            let vec = Vec::with_capacity_in(0, allocator.clone());
            return Self {
                digits: vec,
            };
        }

        // Remainder on division by WORD_BYTES
        let r = bytes.len() & (U256::BYTES - 1);
        let (n_digits, full_digits) = if r == 0 {
            let ws = bytes.len() / U256::BYTES;
            (ws, ws)
        } else {
            // Need an extra digit for the remainder
            let ws = bytes.len() / U256::BYTES;
            (ws + 1, ws)
        };

        let mut digits = Vec::with_capacity_in(n_digits, allocator.clone());

        // buffer to hold Word-sized slices of the input bytes
        let mut buf = [0u8; U256::BYTES];

        let mut i_b = 0;
        let mut i_w = 0;

        loop {
            if i_w == full_digits { break; }
            let next = i_b + U256::BYTES;
            buf.copy_from_slice(&bytes[i_b..next]);

            digits.push(U256::from_le_bytes(&buf));

            i_w += 1;
            i_b = next;
        }

        if r != 0 {
            buf[..r].copy_from_slice(&bytes[i_b..]);
            buf[i_w + r..].iter_mut().for_each(|x| *x = 0);

            digits.push(U256::from_le_bytes(&buf));
        }

        Self { digits }
    }

    pub fn eq(&self, rhs: &Self) -> bool {
        let (a, b) = match self.digits.len() < rhs.digits.len() {
            true => (self, rhs),
            false => (rhs, self),
        };

        a.digits.iter()
            .chain(core::iter::repeat(&U256::ZERO))
            .zip(b.digits.iter())
            .all(|(x, y)| x == y)
    }
    
    pub fn eq_digits(lhs: &[U256], rhs: &[U256]) -> bool {
        let (a, b) = match lhs.len() < rhs.len() {
            true => (lhs, rhs),
            false => (rhs, lhs),
        };

        a.iter()
            .chain(core::iter::repeat(&U256::ZERO))
            .zip(b.iter())
            .all(|(x, y)| x == y)
    }

    pub fn sub(&self, rhs: &Self, out: &mut Self) -> bool {
        let mut carry = false;

        let min = core::cmp::min(self.digits.len(), rhs.digits.len());
        let max = core::cmp::max(self.digits.len(), rhs.digits.len());

        for i in 0..min  {
            out.digits[i] = self.digits[i].clone();

            let uf1 = out.digits[i].overflowing_sub_assign(&rhs.digits[i]);

            let uf2 = if carry { out.digits[i].overflowing_sub_assign(&U256::ONE) } else { false };

            carry = uf1 | uf2;
        }

        for i in min..max {
            out.digits[i] = self.digits.get(i).unwrap_or(&U256::ZERO).clone();

            let uf1 = out.digits[i].overflowing_sub_assign(&rhs.digits.get(i).unwrap_or(&U256::ZERO));

            let uf2 = if carry { out.digits[i].overflowing_sub_assign(&U256::ONE) } else { false };

            carry = uf1 | uf2;
        }

        carry
    }

    /// Buffer swaps:
    /// `R_U256_SCRATCH` <-> `out`
    /// `Q_U256_SCRATCH` <-> `self`
    pub fn div<O: IOOracle, L: Logger>(&mut self, rhs: &Self, out: &mut Vec<U256, A>, oracle: &mut O, logger: &mut L, allocator: A) {
        let mut arg = {
            let lhs_len = self.digits.len();
            let lhs_ptr = self.digits.as_mut_ptr();

            let rhs_len = rhs.digits.len();
            let rhs_ptr = rhs.digits.as_ptr();

            let arg = ArithmeticsParam {
                op: 0,
                a_ptr: lhs_ptr as usize as u32,
                a_len: lhs_len as usize as u32,
                b_ptr: rhs_ptr as usize as u32,
                b_len: rhs_len as usize as u32,
                c_ptr: 0,
                c_len: 0,
            };

            arg
        };

        let mut it = 
            oracle
            .create_oracle_access_iterator::<Arithmetics>(&raw mut arg as usize as u32)
            .unwrap();

        let q_len = it.next().expect("Quotient length.");
        let r_len = it.next().expect("Remainder length.");


        #[allow(static_mut_refs)]
        let mut q_ref = OpaqueRef::<Vec<U256, A>, A>::access(
            unsafe { &mut Q_U256_SCRATCH },
            allocator.clone());

        #[allow(static_mut_refs)]
        let mut r_ref = OpaqueRef::<Vec<U256, A>, A>::access(
            unsafe { &mut R_U256_SCRATCH },
            allocator.clone());

        let q = q_ref.prepared(self.digits.len(), 1, logger);
        let r = r_ref.prepared(rhs.digits.len(), 2, logger);

        { // Write q
            assert_eq!(0, q_len % 8);
            assert!(q_len < isize::MAX as usize / core::mem::size_of::<U256>());

            let q_ptr = q.as_mut_ptr().cast::<usize>();

            for i in 0..q_len {
                let word = it.next().expect("Quotient word.");
                // Safety: 
                // `q_len` is asserted to be small enough not to cause wrapping.
                // `q` capacity the numerator length at least, thus is large enought to hold the
                // result.
                unsafe { q_ptr.add(i).write(word) };
            }

            unsafe { q.set_len(q_len / 8) };
        }

        { // Write r
            assert_eq!(0, r_len % 8);
            assert!(r_len < isize::MAX as usize / core::mem::size_of::<U256>());

            let r_ptr = r.as_mut_ptr().cast::<usize>();

            for i in 0..r_len {
                let word = it.next().expect("Remainder word.");
                // Safety: 
                // `r_len` is asserted to be small enough not to cause wrapping.
                // `r` capacity the divisor length at least, thus is large enought to hold the
                // result.
                unsafe { r_ptr.add(i).write(word) };
            }

            // Safety:
            // `r_len` is asserted to be small enough not to cause wrapping.
            let r_ptr = unsafe { r_ptr.add(r_len).cast::<U256>() };

            for i in 0 .. rhs.digits.len() - r_len / 8 {
                // Safety:
                // `r_len` is 8 aligned:
                // - The base ptr is for `Vec<U256>`
                // - Added `r_len`, which is asserted to be a multiple of 8.
                // `i` is limited by `rhs.digits.len()`, which is the capacity for `r`.
                // Addition will not overflow, since the resulting pointer lies within `rhs`.
                U256_ZERO.clone_into_unchecked(unsafe { r_ptr.add(i) });
            }

            // Safety:
            // Elements within 0..r_len, r_len .. rhs.digits.len() are init due to two previous
            // `for` loops.
            unsafe { r.set_len(self.digits.len()); }
        }

        assert!(it.next().is_none(), "Oracle iterator not exhausted.");

        { // Check oracle results.
            let mut check_ref = OpaqueRef::<Vec<U256, A>, A>::access(
                #[allow(static_mut_refs)]
                unsafe { &mut RCHECK_U256_SCRATCH },
                allocator.clone());

            let mut check = check_ref.prepared(q.len() + rhs.digits.len(), 2, logger);

            { // Write check
                check.clear();

                let spare = check.spare_capacity_mut();

                for i in 0..r.len() {
                    r[i].clone_into(&mut spare[i]);
                }

                // Safety: elems 0..r.len() were just written.
                unsafe { check.set_len(r.len()) };
            }

            // r += q * d
            // TODO: Instead of cloning `r` into `check`, send `r` inside and use it as carry for
            // initial iteration.
            big_wrapping_mul(logger, &q, &rhs.digits, &mut check);

            assert!(Self::eq_digits(&check, self.digits.as_slice()));
        }

        core::mem::swap(&mut self.digits, q);
        core::mem::swap(out, r);
    }

    pub fn trim(&mut self) {
        let cnt = self.digits.iter().rev().take_while(|x| **x == U256::ZERO).count();

        let cnt = match cnt == self.digits.len() {
            false => cnt,
            true => cnt - 1, // Keep at least one word.
        };

        self.digits.resize_with(self.digits.len() - cnt, || { unreachable!() });
    }

    pub fn modpow<O: IOOracle, L: Logger>(
        &mut self,
        exp: &[u8],
        modulus: &Self,
        oracle: &mut O,
        logger: &mut L,
        allocator: A
    ) -> Self {
        // Initial reduction

        // Work width is double of modulus.
        let mut scratch_space = Vec::with_capacity_in(modulus.digits.len() * 2, allocator.clone());
        scratch_space.resize(scratch_space.capacity(), U256::ZERO);

        // Div swaps self and scratch buffers.
        // Widths after:
        // self: same.
        // scratch: len m, cap 2m.
        self.div(modulus, &mut scratch_space, oracle, logger, allocator.clone());

        let base = scratch_space.clone();

        scratch_space.fill(U256::ZERO); // zero-out the scratch space
        scratch_space.resize_with(modulus.digits.len() * 2, || U256::ZERO.clone());

        let mut result = Vec::with_capacity_in(modulus.digits.len() * 2, allocator.clone());
        result.resize_with(result.capacity(), || U256_ZERO.clone());
        let mut result = MPNatU256 { digits: result };

        result.digits[0] = U256::ONE;
        for &b in exp {
            let mut mask: u8 = 1 << 7;
            while mask > 0 {

                // result width: 2m
                // scratch width: 2m
                big_wrapping_mul(logger, &result.digits, &result.digits, &mut scratch_space);
                result.digits.clone_from_slice(&scratch_space);

                // `scratch_space` isn't used inside `div`, an is only used as the result
                // dst, so not need to zero it out. For the next call `div` will write over the
                // contents of the buffer.
                // At this point result and scratch are both 2m wide. Both are swapped by buffers of
                // the same size.
                result.div(modulus, &mut scratch_space, oracle, logger, allocator.clone());

                // Here, result and scratch buffers are swapped.
                core::mem::swap(&mut result.digits, &mut scratch_space);
                // This makes it so the result, scrach, and `div` internal buffers, q and r, are
                // rotated around.

                scratch_space.fill(U256::ZERO); // zero-out the scratch space

                if b & mask != 0 {
                    big_wrapping_mul(logger, &result.digits, &base, &mut scratch_space);
                    result.digits.clone_from_slice(&scratch_space);

                    result.div(modulus, &mut scratch_space, oracle, logger, allocator.clone());
                    core::mem::swap(&mut result.digits, &mut scratch_space);
                    scratch_space.fill(U256::ZERO); // zero-out the scratch space
                }

                mask >>= 1;
            }
        }

        let mut scratch_space = MPNatU256 { digits: scratch_space };

        assert_eq!(false, modulus.sub(&result, &mut scratch_space));

        result
    }

    pub fn to_big_endian(&self, allocator: A) -> Vec<u8, A> {
        if self.digits.iter().all(|x| x == &U256::ZERO) {
            let mut r = Vec::with_capacity_in(1, allocator.clone());
            r.push(0);

            return r;
        }

        // Safety: unwrap is safe since `self.digits` is always non-empty.
        let most_sig_bytes: [u8; U256::BYTES] = self.digits.last().unwrap().to_be_bytes();
        // The most significant digit may not need 4 bytes.
        // Only include as many bytes as needed in the output.
        let be_initial_bytes = {
            let mut tmp: &[u8] = &most_sig_bytes;
            while !tmp.is_empty() && tmp[0] == 0 {
                tmp = &tmp[1..];
            }
            tmp
        };

        let mut result = Vec::with_capacity_in((self.digits.len() - 1) * U256::BYTES + be_initial_bytes.len(), allocator);
        result.resize(result.capacity(), 0);
        result[0..be_initial_bytes.len()].copy_from_slice(be_initial_bytes);
        for (i, d) in self.digits.iter().take(self.digits.len() - 1).enumerate() {
            let bytes = d.to_be_bytes();
            let j = result.len() - U256::BYTES * i;
            result[(j - U256::BYTES)..j].copy_from_slice(&bytes);
        }
        result
    }
}
