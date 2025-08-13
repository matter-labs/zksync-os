#[repr(u8)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvmError {
    Revert,
    OutOfGas,
    InvalidJump,
    ReturnDataOutOfBounds,
    InvalidOpcode(u8),
    StackUnderflow,
    StackOverflow,
    CallNotAllowedInsideStatic,
    StateChangeDuringStaticCall,
    MemoryLimitOOG,
    InvalidOperandOOG,
    /// Call-specific error
    CodeStoreOutOfGas,
    /// Call-specific error
    CallTooDeep,
    /// Call-specific error
    InsufficientBalance,
    /// Call-specific error
    CreateCollision,
    /// Call-specific error
    NonceOverflow,
    CreateContractSizeLimit,
    CreateInitcodeSizeLimit,
    CreateContractStartingWithEF,
}
