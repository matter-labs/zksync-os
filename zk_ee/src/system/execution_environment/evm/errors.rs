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
    // TODO EVM EE itself can't catch some of call/deploy related errors
    /// Currently this error is not used
    CodeStoreOutOfGas,
    /// Currently this error is not used
    CallTooDeep,
    /// Currently this error is not used
    InsufficientBalance,
    /// Currently this error is not used
    CreateCollision,
    /// Currently this error is not used
    NonceOverflow,
    CreateContractSizeLimit,
    CreateInitcodeSizeLimit,
    CreateContractStartingWithEF,
}
