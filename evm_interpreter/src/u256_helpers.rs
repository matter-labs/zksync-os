use u256::U256;

pub(crate) fn log2floor(value: &U256) -> u64 {
    assert!(!value.is_zero());
    let bit_len = value.bit_len();
    (bit_len as u64) - 1
}
