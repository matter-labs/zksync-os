#![no_main]

use arbitrary::Arbitrary;
use basic_system::system_functions::u256_advice::verify_mulmod_hint;
use libfuzzer_sys::fuzz_target;
use u256::U256;

#[derive(Arbitrary, Debug, Clone, Copy)]
enum ValueKind {
    Random,
    Zero,
    One,
    Max,
    PowerOfTwo,
    Small,
}

#[derive(Arbitrary, Debug, Clone, Copy)]
enum BadHintKind {
    Random,
    OffByOneQLo,
    OffByOneR,
    SwapQLoAndR,
}

#[derive(Arbitrary, Debug)]
struct Input {
    a_kind: ValueKind,
    b_kind: ValueKind,
    mod_kind: ValueKind,
    bad_hint_kind: BadHintKind,
    a_seed: [u64; 4],
    b_seed: [u64; 4],
    mod_seed: [u64; 4],
    bad_seed_lo: [u64; 4],
    bad_seed_hi: [u64; 4],
    bad_seed_r: [u64; 4],
    power_shift: u8,
}

fn shape_value(seed: [u64; 4], kind: ValueKind, power_shift: u8) -> [u64; 4] {
    match kind {
        ValueKind::Random => seed,
        ValueKind::Zero => [0; 4],
        ValueKind::One => [1, 0, 0, 0],
        ValueKind::Max => [u64::MAX; 4],
        ValueKind::PowerOfTwo => {
            let bit = (power_shift as usize) % 256;
            let limb = bit / 64;
            let shift = bit % 64;
            let mut v = [0u64; 4];
            v[limb] = 1u64 << shift;
            v
        }
        ValueKind::Small => {
            let small = (seed[0] % 255) + 1;
            [small, 0, 0, 0]
        }
    }
}

fn wrapping_inc(limbs: [u64; 4]) -> [u64; 4] {
    let mut v = limbs;
    let (val, overflow) = v[0].overflowing_add(1);
    v[0] = val;
    if overflow {
        v[1] = v[1].wrapping_add(1);
    }
    v
}

fn fuzz(input: Input) {
    let a_raw = shape_value(input.a_seed, input.a_kind, input.power_shift);
    let b_raw = shape_value(input.b_seed, input.b_kind, input.power_shift);
    let mod_raw = shape_value(input.mod_seed, input.mod_kind, input.power_shift);

    if mod_raw == [0; 4] {
        return;
    }

    let mut product = [0u64; 8];
    let overflow = ruint::algorithms::addmul(&mut product, &a_raw, &b_raw);
    debug_assert!(!overflow);
    let mut m = mod_raw;
    ruint::algorithms::div(&mut product, &mut m);

    let q_lo_limbs: [u64; 4] = product[..4].try_into().unwrap();
    let q_hi_limbs: [u64; 4] = product[4..].try_into().unwrap();
    let r_limbs = m;

    let a = U256::from_limbs(a_raw);
    let b = U256::from_limbs(b_raw);
    let modulus = U256::from_limbs(mod_raw);

    // Positive: correct hint must pass
    assert!(verify_mulmod_hint(&a, &b, &modulus, q_lo_limbs, q_hi_limbs, r_limbs));

    // Compare against software path
    let mut sw_a = U256::from_limbs(a_raw);
    let mut sw_b = U256::from_limbs(b_raw);
    let mut sw_m = U256::from_limbs(mod_raw);
    U256::mul_mod(&mut sw_a, &mut sw_b, &mut sw_m);
    assert_eq!(*sw_m.as_limbs(), r_limbs, "remainder mismatch");

    // Construct bad hints
    let (bad_q_lo, bad_q_hi, bad_r) = match input.bad_hint_kind {
        BadHintKind::Random => (input.bad_seed_lo, input.bad_seed_hi, input.bad_seed_r),
        BadHintKind::OffByOneQLo => (wrapping_inc(q_lo_limbs), q_hi_limbs, r_limbs),
        BadHintKind::OffByOneR => (q_lo_limbs, q_hi_limbs, wrapping_inc(r_limbs)),
        BadHintKind::SwapQLoAndR => (r_limbs, q_hi_limbs, q_lo_limbs),
    };

    // Negative: bad q_lo only
    if bad_q_lo != q_lo_limbs {
        assert!(
            !verify_mulmod_hint(&a, &b, &modulus, bad_q_lo, q_hi_limbs, r_limbs),
            "verification accepted bad q_lo"
        );
    }

    // Negative: bad q_hi only
    if bad_q_hi != q_hi_limbs {
        assert!(
            !verify_mulmod_hint(&a, &b, &modulus, q_lo_limbs, bad_q_hi, r_limbs),
            "verification accepted bad q_hi"
        );
    }

    // Negative: bad remainder only
    if bad_r != r_limbs {
        assert!(
            !verify_mulmod_hint(&a, &b, &modulus, q_lo_limbs, q_hi_limbs, bad_r),
            "verification accepted bad remainder"
        );
    }

    // Negative: all bad
    if bad_q_lo != q_lo_limbs || bad_q_hi != q_hi_limbs || bad_r != r_limbs {
        assert!(
            !verify_mulmod_hint(&a, &b, &modulus, bad_q_lo, bad_q_hi, bad_r),
            "verification accepted bad hint"
        );
    }
}

fuzz_target!(|input: Input| {
    fuzz(input);
});
