/// Params for U256 div_rem oracle query (pointer-based, like modexp).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct U256DivRemAdviceParamsGeneric<W> {
    pub dividend_ptr: W,
    pub divisor_ptr: W,
}

pub type U256DivRemAdviceParams = U256DivRemAdviceParamsGeneric<u32>;
pub type U256DivRemAdviceParams64 = U256DivRemAdviceParamsGeneric<u64>;

/// Params for U256 mulmod oracle query (pointer-based, like modexp).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct U256MulmodAdviceParamsGeneric<W> {
    pub a_ptr: W,
    pub b_ptr: W,
    pub modulus_ptr: W,
}

pub type U256MulmodAdviceParams = U256MulmodAdviceParamsGeneric<u32>;
pub type U256MulmodAdviceParams64 = U256MulmodAdviceParamsGeneric<u64>;
