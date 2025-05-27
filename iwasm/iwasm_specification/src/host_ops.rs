#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, FromPrimitive, ToPrimitive)]
pub enum LongHostOp {
    OverflowingAdd = 0x01,
    WrappingAdd,
    OverflowingSub,
    WrappingSub,
    FullWidthMul,
    HalfWidthMul,
    Div,
    Rem,
    DivRem,
    Shl,
    Shr,
    SignedOverflowingAdd,
    SignedWrappingAdd,
    SignedOverflowingSub,
    SignedWrappingSub,
    SignedHalfWidthMul,
    ArithmeticShr,
    Compare,
    UnsignedLt,
    UnsignedGt,
    UnsignedLte,
    UnsignedGte,
    SignedLt,
    SignedGt,
    SignedLte,
    SignedGte,
    Eq,
    StorageRead,
    StorageWrite,
    TransientStorageRead,
    TransientStorageWrite,
    MessageWithoutGuaranteedObservability,
    MessageWithGuaranteedObservability,
    ExternalOpaqueNativeCall,
    ExternalOpaqueCall,
    GetHostParameter,
    CalldataReadU256LE,
    ImmutablesRead,
    /// Fill `dst1` with bytes from `op1` ignoring endianness.
    ///  - op_param: 0
    ///  - op1: &[u8; 32]
    ///  - dst1: &mut U256Repr
    IntxFillBytes = 0x1000,
    /// Swap endianness in `dst1`.
    ///  - op_param: 0
    ///  - dst1: &mut U256Repr
    ///
    /// Swap endianness of `op1` and write the result into `dst1`.
    ///  - op_param: 1
    ///  - op1: &U256Repr
    ///  - dst1: &mut U256Repr
    IntxSwapEndianness = 0x1001,
    /// Compare `op1` and `op2` and return the result.
    ///  - op_param: [endianness: u32, 0: u32]
    ///    - endianness: hints which side to start the comparison from.
    ///      - 0: big endian
    ///      - 1: little endian
    ///  - op1: &U256Repr
    ///  - op2: &U256Repr
    ///  - return:
    ///    - 1 when equal
    ///    - 0 when non-equal
    IntxCompare = 0x1010,
    /// Add overflowing `op1` to `op2` and write the result to `dst1`.
    ///  - op_param: [0: u32, 0: u8, endianness: u8, sign: u8, size: u8].
    ///    - endianness:
    ///      - 0: big endian
    ///      - 1: little endian
    ///    - sign:
    ///      - 0: unsigned
    ///      - 1: signed
    ///    - size: IntX size
    ///  - op1: &U256Repr
    ///  - op2: &U256Repr
    ///  - dst1: &mut U256Repr
    ///  - return:
    ///    - 1 when an overflow has occurred
    ///    - 0 otherwise
    IntxOverflowingAdd = 0x1020,
    /// Subtract underflow `op2` from `op1` and write the result to `dst1`.
    ///  - op_param: [0: u32, 0: u8, endianness: u8, sign: u8, size: u8].
    ///    - endianness:
    ///      - 0: big endian
    ///      - 1: little endian
    ///    - sign:
    ///      - 0: unsigned
    ///      - 1: signed
    ///    - size: IntX size
    ///  - op1: &U256Repr
    ///  - op2: &U256Repr
    ///  - dst1: &mut U256Repr
    ///  - return:
    ///    - 1 when an underflow has occurred
    ///    - 0 otherwise
    IntxOverflowingSub = 0x1021,
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, FromPrimitive, ToPrimitive)]
pub enum ShortHostOp {
    ReturnOk = 0x01,
    /// Marks the execution as failed and transaction changes to be reverted. `op1` holds the
    /// pointer to reason string, `op2` holds the string length. Doesn't return.
    ///  - op_param: 0
    ///  - op1: *const str
    ///  - op2: u32
    Revert = 0x02,
    /// Returns the calldata function selector as a little endian u32.
    ///  - op_param: 0
    ///  - return: u32
    CalldataSelector = 0x10,
    /// Return the calldata size excluding the selector.
    ///  - op_param: 0
    ///  - return: u32
    CalldataSize = 0x11,
    /// Reads the calldata into memory at `op1`. `op1` providence must span at least `CalldataSize`
    /// bytes.
    ///  - op_param: 0
    ///  - op1: *mut [u8]
    CalldataReadInto = 0x12,
    /// Read message data defined by `op_param` into memory at `op1`.
    ///  - op_param:
    ///     - 1: msg.from equivalent (highest limb will be zeroed out)
    ///  - op1: *mut U256Repr
    MessageData = 0x20,
    /// Read from storage at key `op1` and return the result.
    ///  - op_param:
    ///    - 0: main storage
    ///    - 1: transient storage
    ///  - op1: &U256Repr
    ///  - op2: &mut U256Repr
    StorageRead = 0x30,
    /// Write `op2` into storage at key `op1`.
    ///  - op_param:
    ///    - 0: main storage
    ///    - 1: transient storage
    ///  - op1: &U256Repr
    ///  - op2: &mut U256Repr
    StorageWrite = 0x31,
    /// Hash the byte slice `op1` of length `op_param` into `op2`.
    ///  - op_param: u64
    ///  - op1: &[u8]
    ///  - op2: &mut U256Repr
    HashKeccak256 = 0x40,

    CalldataReadU8,
    CalldataReadU32,
}
