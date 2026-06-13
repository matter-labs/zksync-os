use super::*;
use crypto::MiniDigest;
use ruint::algorithms::{add_nx1, inv_mod};

pub const MILLER_RABIN_BASES: [u32; 3] = [2, 7, 61];

#[derive(Clone, Copy, Debug)]
pub struct HashToPrimeData {
    pub first_step_n: u32,
    pub pocklington_witness_1: PocklingtonStepData<64, 1>,
    pub pocklington_witness_2: PocklingtonStepData<128, 2>,
    pub pocklington_witness_3: PocklingtonStepData<256, 4>,
    pub pocklington_witness_4: PocklingtonStepData<384, 6>,
    pub final_prime: Uint<384, 6>,
}

#[derive(Clone, Copy, Debug)]
pub struct PocklingtonStepData<const BITS: usize, const LIMBS: usize> {
    pub n: u32,
    pub witness: u32,
    pub inverse_witness: Uint<BITS, LIMBS>,
}

#[inline(always)]
fn mont_mul_u32(a: u32, b: u32, modulus: u32, mont_inv: u32) -> u32 {
    // we do not need multiprecision
    let t = (a as u64) * (b as u64);
    let m = (t as u32).wrapping_mul(mont_inv);
    let (t, of) = t.overflowing_add((modulus as u64) * (m as u64));
    let mut result = (t >> 32) | (of as u64) << 32;
    if result >= modulus as u64 {
        result -= modulus as u64;
    }

    result as u32
}

#[inline(always)]
fn mont_square_u32(a: u32, modulus: u32, mont_inv: u32) -> u32 {
    mont_mul_u32(a, a, modulus, mont_inv)
}

#[doc(hidden)]
pub fn mac_with_carry(a: u64, b: u64, c: u64, carry: &mut u64) -> u64 {
    let tmp = (a as u128) + (b as u128 * c as u128) + (*carry as u128);
    *carry = (tmp >> 64) as u64;
    tmp as u64
}

#[inline]
pub fn bigint_mont_mul<const BITS: usize, const LIMBS: usize>(
    a: &Uint<BITS, LIMBS>,
    b: &Uint<BITS, LIMBS>,
    modulus: &Uint<BITS, LIMBS>,
    mont_inv: u64,
) -> Uint<BITS, LIMBS>
where
    [(); BITS * 2]:,
    [(); LIMBS * 2]:,
{
    let mut tmp = a.widening_mul::<BITS, LIMBS, { BITS * 2 }, { LIMBS * 2 }>(*b);
    let result = unsafe {
        for i in 0..LIMBS {
            let limb = *tmp.as_limbs().get_unchecked(i);
            let m = limb.wrapping_mul(mont_inv);
            let mut carry = 0u64;
            // perform (high||low) += modulus * m
            for j in 0..LIMBS {
                let dst_idx = i + j;
                let a = *tmp.as_limbs().get_unchecked(dst_idx);
                let b = *modulus.as_limbs().get_unchecked(j);
                let c = m;
                let result = mac_with_carry(a, b, c, &mut carry);
                *tmp.as_limbs_mut().get_unchecked_mut(dst_idx) = result;
            }
            if i + LIMBS < LIMBS * 2 {
                let _ = add_nx1(
                    tmp.as_limbs_mut()
                        .get_unchecked_mut((i + LIMBS)..(LIMBS * 2)),
                    carry,
                );
            }
        }

        let mut result = Uint::<BITS, LIMBS>::ZERO;
        result
            .as_limbs_mut()
            .copy_from_slice(&tmp.as_limbs()[LIMBS..]);

        result
    };

    let (reduced, uf) = result.overflowing_sub(*modulus);
    if uf {
        result
    } else {
        reduced
    }
}

#[inline]
pub fn bigint_mont_square<const BITS: usize, const LIMBS: usize>(
    a: &Uint<BITS, LIMBS>,
    modulus: &Uint<BITS, LIMBS>,
    mont_inv: u64,
) -> Uint<BITS, LIMBS>
where
    [(); BITS * 2]:,
    [(); LIMBS * 2]:,
{
    bigint_mont_mul(a, &*a, modulus, mont_inv)
}

pub fn bigint_from_le_bytes<const BITS: usize, const LIMBS: usize>(
    source: &[u8],
) -> Uint<BITS, LIMBS> {
    debug_assert_eq!(source.len() / 8, LIMBS);
    Uint::<BITS, LIMBS>::from_le_slice(source)
}

// #[inline(always)]
// fn mont_mul_u64_no_of(a: u64, b: u64, modulus: u64, mont_inv: u64) -> u64 {
//     // we do not need multiprecision
//     let t = (a as u128) * (b as u128);
//     let m = (t as u64).wrapping_mul(mont_inv);
//     // we expect that we will generate 63 bit value, so here there is no overflow over u128 as 2*N < 2^128
//     let t = t.wrapping_add((modulus as u128) * (m as u128));
//     let mut result = (t >> 64) as u64;
//     if result >= modulus {
//         result -= modulus;
//     }

//     result
// }

// #[inline(always)]
// fn mont_square_u64_no_of(a: u64, modulus: u64, mont_inv: u64) -> u64 {
//     mont_mul_u64_no_of(a, a, modulus, mont_inv)
// }

pub fn miller_rabin_u32(candidate: u32) -> bool {
    if candidate & 1 == 0 {
        return false;
    }
    // prime is considered N = 2^s * d + 1;
    let tmp = candidate - 1;
    let s = tmp.trailing_zeros();
    debug_assert!(s > 0);
    let d = tmp >> s;

    // since N is odd, we can compute the Montgomery representation constant
    let mont_r = (1u64 << 32) % (candidate as u64);
    let mont_r2 = (mont_r * mont_r) % (candidate as u64);
    let mont_r = mont_r as u32;
    let mont_r2 = mont_r2 as u32;
    let minus_one = candidate - mont_r;

    let mut inv = 1u32;
    for _ in 0..31 {
        inv = inv.wrapping_mul(inv);
        inv = inv.wrapping_mul(candidate);
    }
    inv = inv.wrapping_neg();

    for a in MILLER_RABIN_BASES.iter().copied() {
        let a = mont_mul_u32(a, mont_r2, candidate, inv);
        // first check that a^d == 1 mod N

        // top bit is set
        let mut result = a;
        let bits = 32 - d.leading_zeros() - 1;
        for i in (0..bits).rev() {
            result = mont_square_u32(result, candidate, inv);
            if d & (1 << i) > 0 {
                result = mont_mul_u32(result, a, candidate, inv);
            }
        }

        if result == mont_r {
            continue;
        }

        // then {a^d)^{2^r} == -1 mod N for 0 < r < s
        if result == minus_one {
            continue;
        }

        for _r in 1..s {
            result = mont_square_u32(result, candidate, inv);
            if result == minus_one {
                continue;
            }
        }

        // no luck
        return false;
    }

    true
}

pub fn create_entropy(source: &[u8]) -> [u8; 64] {
    use crypto::blake2s::Blake2s256;
    assert!(MAX_ENTROPY_BYTES <= 64);
    let mut entropy = [0u8; 64];
    for (idx, dst) in entropy.as_chunks_mut::<32>().0.iter_mut().enumerate() {
        let mut hasher = Blake2s256::new();
        hasher.update(&(idx as u32).to_le_bytes());
        hasher.update(&source);
        dst.copy_from_slice(hasher.finalize().as_slice());
    }

    entropy
}

pub fn write_entropy_le(dst: &mut [u8], src: &mut impl Iterator<Item = u8>, entropy_bits: u32) {
    let take_bytes = entropy_bits.next_multiple_of(8) / 8;
    debug_assert_eq!(dst.len(), take_bytes as usize);
    let mut entropy_bits = entropy_bits;
    for (dst, src) in dst.iter_mut().zip(src) {
        let mut src = src;
        if entropy_bits <= 8 {
            // set top bit, but clear the rest
            let mut tmp = src as u16;
            let clear_bits_mask = (1u16 << entropy_bits) - 1;
            tmp &= clear_bits_mask;
            let top_bit_mask = 1u16 << (entropy_bits - 1);
            tmp |= top_bit_mask;
            src = tmp as u8;
        } else {
            entropy_bits -= 8;
        }
        *dst = src;
    }
}

pub fn compute_mont_params<const BITS: usize, const LIMBS: usize>(
    modulus: &Uint<BITS, LIMBS>,
) -> (Uint<BITS, LIMBS>, Uint<BITS, LIMBS>, u64)
where
    [(); BITS * 2]:,
    [(); LIMBS * 2]:,
{
    use ruint::algorithms::*;
    let (_, mut almost_mont_r) = Uint::<BITS, LIMBS>::MAX.div_rem(*modulus);
    let _ = unsafe { add_nx1(almost_mont_r.as_limbs_mut(), 1) };
    debug_assert!(&almost_mont_r != modulus);
    let mont_r = almost_mont_r;

    let mut a = Uint::<{ BITS * 2 }, { LIMBS * 2 }>::MAX.into_limbs();
    let mut b = *modulus;
    unsafe { div(&mut a, b.as_limbs_mut()) };
    let _ = unsafe { add_nx1(b.as_limbs_mut(), 1) };
    debug_assert!(&b != modulus);
    let mont_r2 = b;

    // let minus_one = candidate - mont_r;

    let modulus_low = modulus.as_limbs()[0];

    let mut inv = 1u64;
    for _ in 0..63 {
        inv = inv.wrapping_mul(inv);
        inv = inv.wrapping_mul(modulus_low);
    }
    inv = inv.wrapping_neg();

    (mont_r, mont_r2, inv)
}

pub fn try_pocklington_witness<const BITS: usize, const LIMBS: usize>(
    candidate_factor: &Uint<BITS, LIMBS>,
    previous_prime: &Uint<BITS, LIMBS>,
) -> Option<(PocklingtonStepData<BITS, LIMBS>, Uint<BITS, LIMBS>)>
where
    [(); BITS * 2]:,
    [(); LIMBS * 2]:,
{
    let mut candidate = candidate_factor * previous_prime;
    debug_assert!(candidate.as_limbs()[0] & 1 == 0);
    let first_pow = *previous_prime;
    let second_pow = *candidate_factor;
    let _ = unsafe { add_nx1(candidate.as_limbs_mut(), 1) };

    // check for some a that a^{candidate-1} == 1 mod candidate,
    // and gcd with lower pow of {candidate-1}/previous_prime, so we save the results

    let (mont_r, mont_r2, mont_inv) = compute_mont_params(&candidate);
    let one = Uint::<BITS, LIMBS>::from(1u64);

    let mut result = PocklingtonStepData {
        n: 0,
        witness: 0,
        inverse_witness: Uint::<BITS, LIMBS>::ZERO,
    };

    for maybe_witness in 2..=u32::MAX {
        let Some(intermediate_pow) = little_fermat(
            maybe_witness,
            &candidate,
            &mont_r,
            &mont_r2,
            mont_inv,
            &first_pow,
            &second_pow,
        ) else {
            // definitely not prime
            return None;
        };

        // now we should do GCD

        // go out of montgomery form
        let intermediate_pow = bigint_mont_mul(&intermediate_pow, &one, &candidate, mont_inv);
        if let Some(inverse_witness) = inv_mod(intermediate_pow, candidate) {
            result.witness = maybe_witness;
            // keep in montgomery form
            result.inverse_witness = inverse_witness;
            break;
        } else {
            continue;
        }
    }

    Some((result, candidate))
}

fn mont_pow<const BITS: usize, const LIMBS: usize>(
    a: &Uint<BITS, LIMBS>,
    modulus: &Uint<BITS, LIMBS>,
    mont_inv: u64,
    pow: &Uint<BITS, LIMBS>,
) -> Uint<BITS, LIMBS>
where
    [(); BITS * 2]:,
    [(); LIMBS * 2]:,
{
    // top bit it always set
    let mut result = *a;
    let bits = BITS - pow.leading_zeros() - 1;
    for i in (0..bits).rev() {
        result = bigint_mont_square(&result, modulus, mont_inv);
        if pow.bit(i) {
            result = bigint_mont_mul(&result, a, modulus, mont_inv);
        }
    }

    result
}

pub fn little_fermat<const BITS: usize, const LIMBS: usize>(
    witness_candidate: u32,
    candidate: &Uint<BITS, LIMBS>,
    mont_r: &Uint<BITS, LIMBS>,
    mont_r2: &Uint<BITS, LIMBS>,
    mont_inv: u64,
    first_pow: &Uint<BITS, LIMBS>,
    next_pow: &Uint<BITS, LIMBS>,
) -> Option<Uint<BITS, LIMBS>>
where
    [(); BITS * 2]:,
    [(); LIMBS * 2]:,
{
    // first move witness into montgomery form
    let witness_candidate = Uint::<BITS, LIMBS>::from(witness_candidate);
    let witness_candidate = bigint_mont_mul(&witness_candidate, &mont_r2, &candidate, mont_inv);

    // check for some a that a^{candidate-1} == 1 mod candidate,

    let first_pow_result = mont_pow(&witness_candidate, candidate, mont_inv, first_pow);

    let full_pow_result = mont_pow(&first_pow_result, candidate, mont_inv, next_pow);

    if &full_pow_result != mont_r {
        // definitely not prime
        None
    } else {
        Some(first_pow_result)
    }
}
