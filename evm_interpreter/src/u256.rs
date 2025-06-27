use u256::U256;

// Based on https://github.com/recmo/uint/blob/9bc4c717fbe126dabaa722489a284021f404652f/src/modular.rs#L55
pub fn mul_mod(this: &U256, other: &U256, mut modulus: U256) -> U256 {
    if modulus == U256::zero() {
        return U256::zero();
    }
    // Compute full product.
    // The challenge here is that Rust doesn't allow us to create a
    // `Uint<2 * BITS, _>` for the intermediate result. Otherwise,
    // we could just use a `widening_mul`. So instead we allocate from heap.
    // Alternatively we could use `alloca`, but that is blocked on
    // See <https://github.com/rust-lang/rust/issues/48055>
    let mut product = [0u64; 8];
    let overflow = ruint::algorithms::addmul(&mut product, this.as_limbs(), other.as_limbs());
    debug_assert!(!overflow);

    // Compute modulus using `div_rem`.
    // This stores the remainder in the divisor, `modulus`.
    ruint::algorithms::div(&mut product, modulus.as_limbs_mut());

    modulus
}

pub(crate) fn log2floor(value: &U256) -> u64 {
    assert!(value.is_zero() == false);
    let mut l: u64 = 256;
    for i in 0..4 {
        let i = 3 - i;
        if value.as_limbs()[i] == 0u64 {
            l -= 64;
        } else {
            l -= value.as_limbs()[i].leading_zeros() as u64;
            if l == 0 {
                return l;
            } else {
                return l - 1;
            }
        }
    }
    l
}
