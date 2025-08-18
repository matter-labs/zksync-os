/// Internal reasons of possible EVM failures. Declared in a separate module for reuse in tracers.
/// Copied from Geth and modified: https://github.com/ethereum/go-ethereum/blob/3ff99ae52c420477020ae957a61c5c216ac7e7f5/core/vm/errors.go
///
/// Note: errors marked as *Call-specific* can be returned during call (call/constructor) before any opcode execution.
/// Technically, internal VM frame doesn't exist in that moment (see `on_call_error` in Tracer trait)
#[repr(u8)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvmError {
    /// Revert caused by REVERT opcode
    Revert,
    OutOfGas,
    /// Invalid JUMP opcode destination
    InvalidJump,
    /// Attempt to access returndata with invalid index (out of bounds)
    ReturnDataOutOfBounds,
    /// Unknown opcode
    InvalidOpcode(u8),
    StackUnderflow,
    StackOverflow,
    CallNotAllowedInsideStatic,
    StateChangeDuringStaticCall,
    /// Hit memory limit (offset > u32::MAX - 31), out of gas as result
    MemoryLimitOOG,
    /// Invalid operand (e.g. failed to cast), out of gas as result
    InvalidOperandOOG,
    /// Failed to pay gas for code deployment
    CodeStoreOutOfGas,
    /// *Call-specific*, callstack height exceeds allowed limit
    CallTooDeep,
    /// *Call-specific*, insufficient balance for transfer
    InsufficientBalance,
    /// *Call-specific*, attempt to deploy contract on already occupied address
    CreateCollision,
    /// *Call-specific*, caller nonce overflowed during deployment
    NonceOverflow,
    CreateContractSizeLimit,
    CreateInitcodeSizeLimit,
    CreateContractStartingWithEF,
}
