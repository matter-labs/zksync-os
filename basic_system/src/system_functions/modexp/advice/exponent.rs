//! The exponent walk of modexp, shared by the single-digit and the multi-digit modulus
//! paths, over the modular multiplication a `ModMul` provides.

/// Modular squaring and multiplication of one representation of residues
pub(crate) trait ModMul {
    type Elem;

    /// A placeholder value, only ever assigned over (a cheap value, not a residue)
    fn placeholder(&mut self) -> Self::Elem;
    fn clone_elem(&mut self, x: &Self::Elem) -> Self::Elem;
    fn is_zero(&self, x: &Self::Elem) -> bool;
    /// `x = x^2 mod m`
    fn square_assign(&mut self, x: &mut Self::Elem);
    /// `x = x * y mod m`
    fn mul_assign(&mut self, x: &mut Self::Elem, y: &Self::Elem);
}

/// Exponents at least this long (in bits) are walked in 4-bit windows with a table of 16
/// powers of the base; shorter ones bit by bit. The table costs 14 multiplications and
/// saves about one multiplication per 4 exponent bits.
const WINDOW_MIN_BITS: usize = 64;
const WINDOW_BITS: usize = 4;
const TABLE_SIZE: usize = 1 << WINDOW_BITS;

/// `base^exp mod m` for a big-endian `exp` with no leading zero bytes and at least one set
/// bit, and a non-zero `base`. Returns early once the accumulator is zero (possible for a
/// composite modulus), as it stays zero.
pub(crate) fn exponentiate<M: ModMul>(m: &mut M, base: &M::Elem, exp: &[u8]) -> M::Elem {
    debug_assert!(exp.first().is_some_and(|byte| *byte != 0));
    let bits = exp.len() * 8 - exp[0].leading_zeros() as usize;
    if bits < WINDOW_MIN_BITS {
        exponentiate_binary(m, base, exp)
    } else {
        exponentiate_windowed(m, base, exp)
    }
}

/// Left-to-right binary exponentiation: after the leading set bit, a squaring per bit and
/// a multiplication per set bit
fn exponentiate_binary<M: ModMul>(m: &mut M, base: &M::Elem, exp: &[u8]) -> M::Elem {
    let mut acc = m.clone_elem(base);
    let mut started = false;
    for &byte in exp {
        for i in (0..8).rev() {
            let bit = byte & (1 << i) != 0;
            if !started {
                started = bit;
                continue;
            }
            m.square_assign(&mut acc);
            if bit {
                m.mul_assign(&mut acc, base);
            }
            if m.is_zero(&acc) {
                return acc;
            }
        }
    }
    acc
}

/// Left-to-right fixed-window exponentiation over the nibbles of `exp`: after the leading
/// non-zero nibble, four squarings per nibble and a multiplication by the tabulated power
/// per non-zero nibble
fn exponentiate_windowed<M: ModMul>(m: &mut M, base: &M::Elem, exp: &[u8]) -> M::Elem {
    // table[i] = base^i for i >= 1 (table[0] is unused)
    let mut table: [M::Elem; TABLE_SIZE] = core::array::from_fn(|_| m.placeholder());
    table[1] = m.clone_elem(base);
    for i in 2..TABLE_SIZE {
        let (lower, upper) = table.split_at_mut(i);
        let mut power = m.clone_elem(&lower[i - 1]);
        m.mul_assign(&mut power, base);
        upper[0] = power;
    }

    let mut acc = m.placeholder();
    let mut started = false;
    for &byte in exp {
        for window in [byte >> WINDOW_BITS, byte & (TABLE_SIZE as u8 - 1)] {
            if !started {
                if window != 0 {
                    started = true;
                    acc = m.clone_elem(&table[window as usize]);
                }
                continue;
            }
            for _ in 0..WINDOW_BITS {
                m.square_assign(&mut acc);
            }
            if window != 0 {
                m.mul_assign(&mut acc, &table[window as usize]);
            }
            if m.is_zero(&acc) {
                return acc;
            }
        }
    }
    debug_assert!(started);
    acc
}
