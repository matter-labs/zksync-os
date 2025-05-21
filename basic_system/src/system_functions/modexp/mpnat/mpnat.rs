// Adapted from https://github.com/aurora-is-near/aurora-engine

use super::{arith::*, U256};

extern crate alloc;
use alloc::vec::Vec;
use zk_ee::system::logger::Logger;
use zk_ee::system_io_oracle::{ArithmeticsParam, IOOracle};
use core::alloc::Allocator;
use zk_ee::system_io_oracle::Arithmetics;

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

    pub fn div<O: IOOracle, L: Logger>(&mut self, rhs: &Self, oracle: &mut O, logger: &mut L, allocator: A) -> Self {
        let lhs_len = self.digits.len();
        let lhs_ptr = self.digits.as_mut_ptr();

        let rhs_len = rhs.digits.len();
        let rhs_ptr = rhs.digits.as_ptr();

        let mut arg = ArithmeticsParam {
            op: 0,
            a_ptr: lhs_ptr as usize as u32,
            a_len: lhs_len as usize as u32,
            b_ptr: rhs_ptr as usize as u32,
            b_len: rhs_len as usize as u32,
            c_ptr: 0,
            c_len: 0,
        };

        let ptr = &mut arg as *mut _ as usize as u32;

        let mut it = oracle.create_oracle_access_iterator::<Arithmetics>(ptr).unwrap();

        let q_len = it.next().expect("Quotient length.");
        let r_len = it.next().expect("Remainder length.");

        // The results need to be of the same size as the input.
        let mut q = Vec::with_capacity_in(self.digits.len() * 32, allocator.clone());
        let mut r = Vec::with_capacity_in(self.digits.len() * 32, allocator.clone());

        for _ in 0..q_len {
            let word = it.next().expect("Quotient word.");
            q.extend_from_slice(&word.to_le_bytes());
        }

        for _ in 0..r_len {
            let word = it.next().expect("Remainder word.");
            r.extend_from_slice(&word.to_le_bytes());
        }

        assert!(it.next().is_none(), "Oracle iterator not exhausted.");

        // Force same width as input.
        q.resize(q.capacity(), 0);
        r.resize(r.capacity(), 0);

        let q = MPNatU256::from_little_endian(&q, logger, allocator.clone());
        let r = MPNatU256::from_little_endian(&r, logger, allocator.clone());

        let mut check = Vec::with_capacity_in(q.digits.len() + rhs.digits.len(), allocator.clone());
        // check.extend_from_slice(&r.digits);
        check.resize_with(check.capacity(), || U256::ZERO);

        let mut t = Vec::with_capacity_in(check.len(), allocator.clone());
        t.extend_from_slice(&rhs.digits);
        t.resize_with(t.capacity(), || U256::ZERO);
        let t = MPNatU256 { digits: t };

        // r += q * d
        big_wrapping_mul(logger, &q, &t, &mut check);
        in_place_add(&mut check, &r.digits);


        let check = Self { digits: check };

        assert!(check.eq(self));

        *self = q;
        r
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
        let base = self.div(modulus, oracle, logger, allocator.clone());

        let mut scratch_space = Vec::with_capacity_in(modulus.digits.len() * 2, allocator.clone());
        scratch_space.resize(scratch_space.capacity(), U256::ZERO);

        let mut result = Vec::with_capacity_in(modulus.digits.len() * 2, allocator.clone());
        result.resize(result.capacity(), U256::ZERO);
        let mut result = MPNatU256 { digits: result };

        result.digits[0] = U256::ONE;
        for &b in exp {
            let mut mask: u8 = 1 << 7;
            while mask > 0 {
                big_wrapping_mul(logger, &result, &result, &mut scratch_space);
                result.digits.clone_from_slice(&scratch_space);

                result = result.div(modulus, oracle, logger, allocator.clone());
                scratch_space.fill(U256::ZERO); // zero-out the scratch space

                if b & mask != 0 {
                    big_wrapping_mul(logger, &result, &base, &mut scratch_space);
                    result.digits.clone_from_slice(&scratch_space);

                    result = result.div(modulus, oracle, logger, allocator.clone());
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
