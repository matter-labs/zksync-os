use ruint::aliases::U256;
use zk_ee::utils::u256_mul_by_word;

pub(crate) fn fake_exponential(prefactor: U256, numerator: &U256, denominator: &U256) -> U256 {
    let mut i = 1;
    let mut output = U256::ZERO;
    let mut numerator_accumulator = prefactor * denominator;
    while numerator_accumulator.is_zero() == false {
        output += &numerator_accumulator;
        numerator_accumulator =
            (numerator_accumulator * numerator) / (u256_mul_by_word(denominator, i).0);
        i += 1;
    }

    output / denominator
}
