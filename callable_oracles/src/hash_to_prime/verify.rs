use super::*;
use ruint::algorithms::add_nx1;

pub fn verify_hash_to_prime(entropy: &[u8; 64], certificate: &HashToPrimeData) -> bool {
    let mut entropy_it = entropy.iter().copied();
    let step_0_prime = {
        let (entropy_bits, low_bits) = GENERATION_STEPS[0];
        let take_bytes = entropy_bits.next_multiple_of(8) / 8;
        let mut repr = [0u8; 4];
        write_entropy_le(
            &mut repr[..take_bytes as usize],
            &mut entropy_it,
            entropy_bits,
        );
        let high_part = u32::from_le_bytes(repr);
        let mut candidate = high_part << low_bits;
        if certificate.first_step_n >= 1u32 << low_bits {
            return false;
        }
        candidate += certificate.first_step_n;
        if candidate & 1 != 1 {
            return false;
        }
        if miller_rabin_u32(candidate) != true {
            return false;
        }

        candidate
    };

    let step_0_prime = S1BigInt::from(step_0_prime as u64);
    // println!("0x{:x} is prime", step_0_prime);

    let Ok(step_1_prime) = pocklington_step_verify(
        &step_0_prime,
        &mut entropy_it,
        1,
        &certificate.pocklington_witness_1,
    ) else {
        return false;
    };
    // println!("0x{:x} is prime", step_1_prime);
    let Ok(step_2_prime) = pocklington_step_verify(
        &step_1_prime,
        &mut entropy_it,
        2,
        &certificate.pocklington_witness_2,
    ) else {
        return false;
    };
    // println!("0x{:x} is prime", step_2_prime);
    let Ok(step_3_prime) = pocklington_step_verify(
        &step_2_prime,
        &mut entropy_it,
        3,
        &certificate.pocklington_witness_3,
    ) else {
        return false;
    };
    // println!("0x{:x} is prime", step_3_prime);
    let Ok(step_4_prime) = pocklington_step_verify(
        &step_3_prime,
        &mut entropy_it,
        4,
        &certificate.pocklington_witness_4,
    ) else {
        return false;
    };
    // println!("0x{:x} is prime", step_4_prime);

    if step_4_prime != certificate.final_prime {
        return false;
    }

    true
}

fn pocklington_step_verify<
    const BITS: usize,
    const LIMBS: usize,
    const PBITS: usize,
    const PLIMBS: usize,
>(
    previous_prime: &Uint<PBITS, PLIMBS>,
    entropy_it: &mut impl Iterator<Item = u8>,
    step_index: usize,
    witness: &PocklingtonStepData<BITS, LIMBS>,
) -> Result<Uint<BITS, LIMBS>, ()>
where
    [(); LIMBS * 8]:,
    [(); BITS * 2]:,
    [(); LIMBS * 2]:,
{
    let mut previous = Uint::<BITS, LIMBS>::ZERO;
    unsafe {
        previous.as_limbs_mut()[..previous_prime.as_limbs().len()]
            .copy_from_slice(previous_prime.as_limbs());
    }
    let (entropy_bits, low_bits) = GENERATION_STEPS[step_index];
    let take_bytes = entropy_bits.next_multiple_of(8) / 8;
    let mut repr = [0u8; LIMBS * 8];
    write_entropy_le(&mut repr[..take_bytes as usize], entropy_it, entropy_bits);
    let high_part: Uint<BITS, LIMBS> = bigint_from_le_bytes(&repr);
    let mut candidate_factor = high_part << low_bits;
    if witness.n >= 1u32 << low_bits {
        return Err(());
    }
    candidate_factor += Uint::<BITS, LIMBS>::from(witness.n);
    if candidate_factor.as_limbs()[0] & 1 != 0 {
        return Err(());
    }

    let base = witness.witness;

    let mut candidate = &candidate_factor * previous;
    if candidate.as_limbs()[0] & 1 != 0 {
        return Err(());
    }
    let first_pow = previous;
    let second_pow = candidate_factor;
    let _ = unsafe { add_nx1(candidate.as_limbs_mut(), 1) };

    // check for some a that a^{candidate-1} == 1 mod candidate,
    // and gcd with lower pow of {candidate-1}/previous_prime, so we save the results

    let (mont_r, mont_r2, mont_inv) = compute_mont_params(&candidate);

    let Some(intermediate_pow) = little_fermat(
        base,
        &candidate,
        &mont_r,
        &mont_r2,
        mont_inv,
        &first_pow,
        &second_pow,
    ) else {
        // definitely not prime
        return Err(());
    };

    let maybe_one = bigint_mont_mul(
        &intermediate_pow,
        &witness.inverse_witness,
        &candidate,
        mont_inv,
    );

    if maybe_one != Uint::<BITS, LIMBS>::from(1u64) {
        return Err(());
    }

    Ok(candidate)
}
