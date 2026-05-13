#![no_main]

use arbitrary::Arbitrary;
use basic_system::system_functions::u256_advice::verify_div_rem_hint;
use libfuzzer_sys::fuzz_target;
use u256::U256;

#[derive(Arbitrary, Debug, Clone, Copy)]
enum ValueKind {
    Random,
    Zero,
    One,
    Max,
    PowerOfTwo,
    SmallDivisor,
}

#[derive(Arbitrary, Debug, Clone, Copy)]
enum BadHintKind {
    Random,
    OffByOnePlus,
    OffByOneMinus,
}

#[derive(Arbitrary, Debug)]
struct Input {
    dividend_kind: ValueKind,
    divisor_kind: ValueKind,
    bad_hint_kind: BadHintKind,
    dividend_seed: [u64; 4],
    divisor_seed: [u64; 4],
    bad_seed: [u64; 4],
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
        ValueKind::SmallDivisor => {
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

fn wrapping_dec(limbs: [u64; 4]) -> [u64; 4] {
    let mut v = limbs;
    let (val, overflow) = v[0].overflowing_sub(1);
    v[0] = val;
    if overflow {
        v[1] = v[1].wrapping_sub(1);
    }
    v
}

fn fuzz(input: Input) {
    let dividend_raw = shape_value(input.dividend_seed, input.dividend_kind, input.power_shift);
    let divisor_raw = shape_value(input.divisor_seed, input.divisor_kind, input.power_shift);

    if divisor_raw == [0; 4] {
        return;
    }

    let mut q_limbs = dividend_raw;
    let mut r_limbs = divisor_raw;
    ruint::algorithms::div(&mut q_limbs, &mut r_limbs);

    let dividend = U256::from_limbs(dividend_raw);
    let divisor = U256::from_limbs(divisor_raw);

    // Positive: correct hint must pass
    let remainder = verify_div_rem_hint(&dividend, &divisor, q_limbs).expect("valid hint rejected");
    assert_eq!(*remainder.as_limbs(), r_limbs, "remainder mismatch vs ruint");

    // Compare against software path
    let mut sw_dividend = U256::from_limbs(dividend_raw);
    let mut sw_divisor = U256::from_limbs(divisor_raw);
    U256::div_rem(&mut sw_dividend, &mut sw_divisor);
    assert_eq!(*sw_dividend.as_limbs(), q_limbs, "quotient mismatch");
    assert_eq!(*sw_divisor.as_limbs(), r_limbs, "remainder mismatch");

    // Negative: bad quotient must be rejected
    let bad_q = match input.bad_hint_kind {
        BadHintKind::Random => input.bad_seed,
        BadHintKind::OffByOnePlus => wrapping_inc(q_limbs),
        BadHintKind::OffByOneMinus => wrapping_dec(q_limbs),
    };

    if bad_q != q_limbs {
        assert!(
            verify_div_rem_hint(&dividend, &divisor, bad_q).is_none(),
            "verification accepted bad quotient"
        );
    }
}

fuzz_target!(|input: Input| {
    fuzz(input);
});
