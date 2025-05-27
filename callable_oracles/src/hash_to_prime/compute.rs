use super::*;
use ruint::algorithms::add_nx1;

// NOTE: we know that all moduluses that we will generate have top bit unset when represented as u64 words,
// so we choose non-redundant representation

pub fn compute_from_entropy(entropy: &[u8; 64]) -> HashToPrimeData {
    let mut entropy_it = entropy.iter().copied();
    let (step_0_prime, initial_n) = 'outer: {
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
        let mut n = 0u32;
        for _i in 0..(1 << low_bits) {
            n += 1;
            candidate += 1;
            if miller_rabin_u32(candidate) == true {
                // println!("0x{:08x} is prime", candidate);
                break 'outer (candidate, n);
            }
        }

        unreachable!()
    };

    let step_0_prime = S1BigInt::from(step_0_prime as u64);
    let (step_1_data, step_1_prime): (PocklingtonStepData<64, 1>, S1BigInt) =
        pocklington_step(&step_0_prime, &mut entropy_it, 1);
    let (step_2_data, step_2_prime): (PocklingtonStepData<128, 2>, S2BigInt) =
        pocklington_step(&step_1_prime, &mut entropy_it, 2);
    let (step_3_data, step_3_prime): (PocklingtonStepData<256, 4>, S3BigInt) =
        pocklington_step(&step_2_prime, &mut entropy_it, 3);
    let (step_4_data, step_4_prime): (PocklingtonStepData<384, 6>, S4BigInt) =
        pocklington_step(&step_3_prime, &mut entropy_it, 4);

    HashToPrimeData {
        first_step_n: initial_n,
        pocklington_witness_1: step_1_data,
        pocklington_witness_2: step_2_data,
        pocklington_witness_3: step_3_data,
        pocklington_witness_4: step_4_data,
        final_prime: step_4_prime,
    }
}

fn pocklington_step<
    const BITS: usize,
    const LIMBS: usize,
    const PBITS: usize,
    const PLIMBS: usize,
>(
    previous_prime: &Uint<PBITS, PLIMBS>,
    entropy_it: &mut impl Iterator<Item = u8>,
    step_index: usize,
) -> (PocklingtonStepData<BITS, LIMBS>, Uint<BITS, LIMBS>)
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
    let mut candidate = high_part << low_bits;
    // eventually we will use candidate * previous + 1 as a guess for a new prime,
    // so we need candidate * previous to be even
    let mut n = 0;
    for _i in 0..(1 << (low_bits - 1)) {
        if let Some((witness, next_prime)) =
            try_pocklington_witness::<BITS, LIMBS>(&candidate, &previous)
        {
            let mut witness = witness;
            witness.n = n;
            // println!("0x{:x} is prime with witness {:?}", next_prime, witness);
            return (witness, next_prime);
        }

        let _ = unsafe { add_nx1(candidate.as_limbs_mut(), 2) };
        n += 2;
    }

    unreachable!()
}

#[cfg(test)]
mod test {
    use compute::verify::verify_hash_to_prime;

    use super::*;

    #[test]
    fn try_compute_hash_to_prime() {
        let input = vec![0, 1, 2, 3];
        let entropy = create_entropy(&input);
        let result = compute_from_entropy(&entropy);
        dbg!(result);
        assert!(verify_hash_to_prime(&entropy, &result));

        for seed in 0u64..1000u64 {
            let seed = seed.to_le_bytes();
            let entropy = create_entropy(&seed);
            let result = compute_from_entropy(&entropy);
            assert!(verify_hash_to_prime(&entropy, &result));
        }
    }
}
