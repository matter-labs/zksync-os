use core::ops::Neg;

use super::WNAF_BITS;

pub(super) trait ToWnaf: Neg<Output = Self> {
    fn bits(&self, offset: usize, count: usize) -> u32;
    fn bits_var(&self, offset: usize, count: usize) -> u32;
}

pub(super) struct Wnaf {
    wnaf: [i32; WNAF_BITS],
    bits: i32,
}

impl Default for Wnaf {
    fn default() -> Self {
        Self {
            wnaf: [0; WNAF_BITS],
            bits: -1,
        }
    }
}

impl Wnaf {
    pub(super) fn new(mut s: impl ToWnaf, window: usize) -> Self {
        debug_assert!((2..=31).contains(&window));

        let mut wnaf: [i32; WNAF_BITS] = [0; WNAF_BITS];
        let mut bits = -1;
        let mut sign = 1;
        let mut carry = 0;
        let mut bit = 0;

        if s.bits(255, 1) > 0 {
            s = -s;
            sign = -1;
        }

        while bit < WNAF_BITS {
            if s.bits(bit, 1) == carry as u32 {
                bit += 1;
                continue;
            }

            let mut now = window;
            if now > WNAF_BITS - bit {
                now = WNAF_BITS - bit;
            }

            let mut word = (s.bits_var(bit, now) as i32) + carry;

            carry = (word >> (window - 1)) & 1;
            word -= carry << window;

            wnaf[bit] = sign * word;
            bits = bit as i32;

            bit += now;
        }

        debug_assert_eq!(carry, 0);
        debug_assert!({
            let mut t = true;
            while bit < 256 {
                t = t && (s.bits(bit, 1) == 0);
                bit += 1;
            }
            t
        });

        bits += 1;
        Self { wnaf, bits }
    }

    pub(super) fn bits(&self) -> i32 {
        self.bits
    }

    pub(super) fn get_digit(&self, i: i32) -> Option<i32> {
        let n = self.wnaf[i as usize];
        if i < self.bits && n != 0 {
            Some(n)
        } else {
            None
        }
    }
}
