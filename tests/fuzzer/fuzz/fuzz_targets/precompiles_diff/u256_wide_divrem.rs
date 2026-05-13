#![no_main]

use arbitrary::Arbitrary;
use basic_system::system_functions::u256_advice::verify_wide_div_rem_hint;
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
    OffByOneQHi,
    SwapQLoAndQHi,
}

#[derive(Arbitrary, Debug)]
struct Input {
    dividend_lo_kind: ValueKind,
    dividend_hi_kind: ValueKind,
    divisor_kind: ValueKind,
    bad_hint_kind: BadHintKind,
    dividend_lo_seed: [u64; 4],
    dividend_hi_seed: [u64; 4],
    divisor_seed: [u64; 4],
    bad_seed_lo: [u64; 4],
    bad_seed_hi: [u64; 4],
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
    let div_lo = shape_value(input.dividend_lo_seed, input.dividend_lo_kind, input.power_shift);
    let div_hi = shape_value(input.dividend_hi_seed, input.dividend_hi_kind, input.power_shift);
    let divisor_raw = shape_value(input.divisor_seed, input.divisor_kind, input.power_shift);

    if divisor_raw == [0; 4] {
        return;
    }

    // Compute correct quotient using ruint
    let mut dividend = [0u64; 8];
    dividend[..4].copy_from_slice(&div_lo);
    dividend[4..].copy_from_slice(&div_hi);
    let mut d = divisor_raw;
    ruint::algorithms::div(&mut dividend, &mut d);

    let q_lo_limbs: [u64; 4] = dividend[..4].try_into().unwrap();
    let q_hi_limbs: [u64; 4] = dividend[4..].try_into().unwrap();

    let dividend_lo = U256::from_limbs(div_lo);
    let dividend_hi = U256::from_limbs(div_hi);
    let divisor = U256::from_limbs(divisor_raw);

    // Positive: correct hint must pass
    let remainder =
        verify_wide_div_rem_hint(&dividend_lo, &dividend_hi, &divisor, q_lo_limbs, q_hi_limbs)
            .expect("valid hint rejected");
    assert_eq!(*remainder.as_limbs(), d, "remainder mismatch");

    // Construct bad hints
    let (bad_q_lo, bad_q_hi) = match input.bad_hint_kind {
        BadHintKind::Random => (input.bad_seed_lo, input.bad_seed_hi),
        BadHintKind::OffByOneQLo => (wrapping_inc(q_lo_limbs), q_hi_limbs),
        BadHintKind::OffByOneQHi => (q_lo_limbs, wrapping_inc(q_hi_limbs)),
        BadHintKind::SwapQLoAndQHi => (q_hi_limbs, q_lo_limbs),
    };

    // Negative: bad q_lo only
    if bad_q_lo != q_lo_limbs {
        assert!(
            verify_wide_div_rem_hint(&dividend_lo, &dividend_hi, &divisor, bad_q_lo, q_hi_limbs)
                .is_none(),
            "verification accepted bad q_lo"
        );
    }

    // Negative: bad q_hi only
    if bad_q_hi != q_hi_limbs {
        assert!(
            verify_wide_div_rem_hint(&dividend_lo, &dividend_hi, &divisor, q_lo_limbs, bad_q_hi)
                .is_none(),
            "verification accepted bad q_hi"
        );
    }

    // Negative: both bad
    if bad_q_lo != q_lo_limbs || bad_q_hi != q_hi_limbs {
        assert!(
            verify_wide_div_rem_hint(&dividend_lo, &dividend_hi, &divisor, bad_q_lo, bad_q_hi)
                .is_none(),
            "verification accepted bad hint"
        );
    }
}

fuzz_target!(|input: Input| {
    fuzz(input);
});
