/// Params for U256 div_rem oracle query (pointer-based, like modexp).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct U256DivRemAdviceParamsGeneric<W> {
    pub dividend_ptr: W,
    pub divisor_ptr: W,
}

pub type U256DivRemAdviceParams = U256DivRemAdviceParamsGeneric<u32>;
pub type U256DivRemAdviceParams64 = U256DivRemAdviceParamsGeneric<u64>;

/// Params for U256 wide div_rem oracle query (512-bit dividend, 256-bit divisor).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct U256WideDivRemAdviceParamsGeneric<W> {
    pub dividend_lo_ptr: W,
    pub dividend_hi_ptr: W,
    pub divisor_ptr: W,
}

pub type U256WideDivRemAdviceParams = U256WideDivRemAdviceParamsGeneric<u32>;
pub type U256WideDivRemAdviceParams64 = U256WideDivRemAdviceParamsGeneric<u64>;
