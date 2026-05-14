use u256::U256;

/// Returns the floor of the base-2 logarithm of the given value.
/// Panics if the value is zero.
pub(crate) fn log2floor(value: &U256) -> u64 {
    assert!(!value.is_zero());
    let bit_len = value.bit_len();
    (bit_len as u64) - 1
}
