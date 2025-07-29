#![cfg_attr(not(feature = "testing"), no_std)]
#![feature(allocator_api)]
#![feature(iter_advance_by)]
#![allow(incomplete_features)]
#![feature(generic_const_exprs)]
#![feature(vec_push_within_capacity)]
#![feature(slice_swap_unchecked)]
#![feature(ptr_as_ref_unchecked)]
#![allow(clippy::new_without_default)]
#![allow(clippy::needless_lifetimes)]
#![allow(clippy::needless_borrow)]
#![allow(clippy::needless_borrows_for_generic_args)]
#![allow(clippy::bool_comparison)]

extern crate alloc;

// unfortunately Reth is written in a way that requires a huge rewrite to abstract away
// not just some database access for storage/accounts, but also all the memory and stack.
// Eventually we plan to try to include this abstraction back into Reth itself

use core::alloc::Allocator;
use core::ops::Range;
use either::Either;

use errors::EvmSubsystemError;
use evm_stack::EvmStack;
use gas::Gas;
use ruint::aliases::U256;
use zk_ee::execution_environment_type::ExecutionEnvironmentType;
use zk_ee::memory::slice_vec::SliceVec;
use zk_ee::system::errors::root_cause::{GetRootCause, RootCause};
use zk_ee::system::errors::runtime::RuntimeError;
use zk_ee::system::errors::{internal::InternalError, system::SystemError};
use zk_ee::system::evm::{EvmFrameInterface, EvmStackInterface};
use zk_ee::system::{EthereumLikeTypes, Resource, Resources, System, SystemTypes};

use alloc::vec::Vec;
use zk_ee::utils::*;
use zk_ee::{internal_error, types_config::*};

mod ee_trait_impl;
pub mod errors;
mod evm_stack;
pub mod gas;
pub mod gas_constants;
pub mod i256;
pub mod instructions;
pub mod interpreter;
pub mod native_resource_constants;
pub mod opcodes;
pub mod u256;
pub mod utils;

pub(crate) const THIS_EE_TYPE: ExecutionEnvironmentType = ExecutionEnvironmentType::EVM;

/// No artifacts cached
pub const DEFAULT_CODE_VERSION_BYTE: u8 = 0u8;

/// Artifacts cached
pub const ARTIFACTS_CACHING_CODE_VERSION_BYTE: u8 = 1u8;

// this is the interpreter that can be found in Reth itself, modified for purposes of having abstract view
// on memory and resources
pub struct Interpreter<'a, S: SystemTypes> {
    /// Instruction pointer.
    pub instruction_pointer: usize,
    /// Implementation of gas accounting on top of system resources.
    pub gas: Gas<S>,
    /// Stack.
    pub stack: EvmStack<S::Allocator>,
    /// Caller address
    pub caller: <S::IOTypes as SystemIOTypesConfig>::Address,
    /// Contract information and invoking data
    pub address: <S::IOTypes as SystemIOTypesConfig>::Address,
    /// calldata
    pub calldata: &'a [u8],
    /// returndata is available from here if it exists
    pub returndata: &'a [u8],
    /// Heap that belongs to this interpreter, can be resided
    pub heap: SliceVec<'a, u8>,
    /// returndata location serves to save range information at various points
    pub returndata_location: Range<usize>,
    /// Bytecode
    pub bytecode: &'a [u8],
    /// Preprocessing result
    pub bytecode_preprocessing: BytecodePreprocessingData<'a, S::Allocator>,
    /// Call value
    pub call_value: U256,
    /// Is interpreter call static.
    pub is_static: bool,
    /// Is interpreter call executing construction code.
    pub is_constructor: bool,
}

impl<'ee, S: EthereumLikeTypes> EvmFrameInterface<S> for Interpreter<'ee, S> {
    fn instruction_pointer(&self) -> usize {
        self.instruction_pointer
    }

    fn resources(&self) -> &<S as SystemTypes>::Resources {
        &self.gas.resources
    }

    fn stack(&self) -> &impl EvmStackInterface {
        &self.stack
    }

    fn caller(&self) -> <<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::Address {
        self.caller
    }

    fn address(&self) -> <<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::Address {
        self.address
    }

    fn calldata(&self) -> &[u8] {
        &self.calldata
    }

    fn return_data(&self) -> &[u8] {
        &self.returndata
    }

    fn heap(&self) -> &[u8] {
        &self.heap
    }

    fn bytecode(&self) -> &[u8] {
        &self.bytecode
    }

    fn call_value(&self) -> &U256 {
        &self.call_value
    }

    fn is_static(&self) -> bool {
        self.is_static
    }

    fn is_constructor(&self) -> bool {
        self.is_constructor
    }
}

pub const STACK_SIZE: usize = 1024;
pub const MAX_CODE_SIZE: usize = 0x6000;
pub const MAX_INITCODE_SIZE: usize = MAX_CODE_SIZE * 2;
pub const ERGS_PER_GAS: u64 = 256;
pub const ERGS_PER_GAS_U256: U256 = U256::from_limbs([ERGS_PER_GAS, 0, 0, 0]);
pub const BYTECODE_ALIGNMENT: usize = core::mem::size_of::<u64>();

#[derive(Debug)]
pub struct BytecodePreprocessingData<'a, A: Allocator> {
    pub original_bytecode_len: usize,
    /// Either a reference to a part of the decommitted bytecode,
    /// or an owned vec created on deployment.
    pub jumpdest_bitmap: either::Either<BitMap<'a>, BitMapOwned<A>>,
}

impl<'a, A: Allocator> BytecodePreprocessingData<'a, A> {
    ///
    /// Creates an empty bitmap, as a slice.
    ///
    #[inline]
    pub fn empty() -> Self {
        Self {
            original_bytecode_len: 0,
            jumpdest_bitmap: Either::Left(BitMap::empty()),
        }
    }

    ///
    /// Determine if an offset is a jumpdest.
    ///
    #[inline]
    pub fn is_valid_jumpdest(&self, off: usize) -> bool {
        match self.jumpdest_bitmap.as_ref() {
            Either::Left(bitmap) => {
                off < self.original_bytecode_len && unsafe { bitmap.get_bit_unchecked(off) }
            }
            Either::Right(bitmap) => {
                off < self.original_bytecode_len && unsafe { bitmap.get_bit_unchecked(off) }
            }
        }
    }

    ///
    /// Parse a decommitted bytecode slice, creating a borrowed
    /// (read-only) jumpdest bitmap.
    ///
    pub fn parse_bytecode(
        bytecode: &'a [u8],
        deployed_len: usize,
        artifacts_len: usize,
    ) -> Result<(&'a [u8], Self), InternalError> {
        let Some(padding) = bytecode
            .len()
            .checked_sub(deployed_len)
            .and_then(|l| l.checked_sub(artifacts_len))
        else {
            return Err(internal_error!("Underflow when computing bytecode padding"));
        };
        let (code, rest) = bytecode.split_at(deployed_len);
        let bitmap_slice = &rest[padding..];

        let preprocessing = Self {
            original_bytecode_len: deployed_len,
            jumpdest_bitmap: Either::Left(BitMap::from_raw(bitmap_slice)),
        };
        Ok((code, preprocessing))
    }

    ///
    /// Create an owned jumpdest-bitmap from deployed code.
    ///
    pub fn create_artifacts<R: Resources>(
        allocator: A,
        deployed_code: &[u8],
        resources: &mut R,
    ) -> Result<Self, SystemError> {
        use crate::native_resource_constants::BYTECODE_PREPROCESSING_BYTE_NATIVE_COST;
        use zk_ee::system::Computational;
        let native_cost = R::Native::from_computational(
            BYTECODE_PREPROCESSING_BYTE_NATIVE_COST.saturating_mul(deployed_code.len() as u64),
        );
        resources
            .charge(&R::from_native(native_cost))
            .map_err(|e| -> SystemError {
                match e {
                    e @ SystemError::LeafDefect(_) => e,
                    SystemError::LeafRuntime(RuntimeError::OutOfErgs(_)) => {
                        SystemError::LeafDefect(internal_error!("OOE when charging only native"))
                    }
                    e @ SystemError::LeafRuntime(RuntimeError::OutOfNativeResources(_)) => e,
                }
            })?;
        Ok(Self::create_artifacts_inner(allocator, deployed_code))
    }

    /// Useful to expose for tests.
    pub fn create_artifacts_inner(allocator: A, deployed_code: &[u8]) -> Self {
        let bitmap = analyze(deployed_code, allocator);
        Self {
            original_bytecode_len: deployed_code.len(),
            jumpdest_bitmap: Either::Right(bitmap),
        }
    }

    /// usize words in the underlying bitmap.
    #[inline(always)]
    fn bitmap_words(&self) -> &[usize] {
        match &self.jumpdest_bitmap {
            Either::Left(b) => b.as_words(),
            Either::Right(b) => b.as_words(),
        }
    }

    ///
    /// Returns a byte slice with the contents of the bitmap.
    ///
    pub fn as_slice(&self) -> &[u8] {
        let words = self.bitmap_words();
        let len_bytes = core::mem::size_of_val(words);
        let ptr = words.as_ptr();
        unsafe { core::slice::from_raw_parts(ptr.cast::<u8>(), len_bytes) }
    }
}

///
/// Owned version of the bitmap, represented as a usize vec.
///
#[derive(Debug)]
pub struct BitMapOwned<A: Allocator> {
    inner: Vec<usize, A>,
}

impl<A: Allocator> BitMapOwned<A> {
    /// Allocates a bitmap for a bytecode of length [capacity].
    pub(crate) fn allocate_for_bit_capacity(capacity: usize, allocator: A) -> Self {
        let u64_capacity = capacity.next_multiple_of(u64::BITS as usize) / (u64::BITS as usize);
        let word_capacity = u64_capacity * (u64::BITS as usize / usize::BITS as usize);
        let mut storage = Vec::with_capacity_in(word_capacity, allocator);
        storage.resize(word_capacity, 0);

        Self { inner: storage }
    }

    #[inline(always)]
    pub fn as_words(&self) -> &[usize] {
        &self.inner
    }

    /// # Safety
    /// pos must be within the bounds of the bitmap.
    pub(crate) unsafe fn set_bit_on_unchecked(&mut self, pos: usize) {
        let (word_idx, bit_idx) = (pos / usize::BITS as usize, pos % usize::BITS as usize);
        let dst = unsafe { self.inner.get_unchecked_mut(word_idx) };
        *dst |= 1usize << bit_idx;
    }

    /// # Safety
    /// [pos] must be within the bounds of the bitmap.
    pub(crate) unsafe fn get_bit_unchecked(&self, pos: usize) -> bool {
        let (word_idx, bit_idx) = (pos / (usize::BITS as usize), pos % (usize::BITS as usize));
        unsafe { self.inner.get_unchecked(word_idx) & (1usize << bit_idx) != 0 }
    }
}

/// Analyzes bytecode to build a jump map.
fn analyze<A: Allocator>(code: &[u8], allocator: A) -> BitMapOwned<A> {
    use self::opcodes as opcode;

    let code_len = code.len();
    let mut jumps = BitMapOwned::<A>::allocate_for_bit_capacity(code_len, allocator);

    let mut i = 0;
    while i < code_len {
        let op = code[i];
        if op == opcode::JUMPDEST {
            // SAFETY: `i` is always < code_len
            unsafe { jumps.set_bit_on_unchecked(i) };
            i += 1;
        } else if (opcode::PUSH1..=opcode::PUSH32).contains(&op) {
            i += 1 + (op - opcode::PUSH1 + 1) as usize;
        } else {
            i += 1;
        }
    }

    jumps
}

///
/// Borrowed bitmap, represented as a usize slice.
///
#[derive(Debug)]
pub struct BitMap<'a>(&'a [usize]);

impl<'a> BitMap<'a> {
    pub fn empty() -> Self {
        Self(&[])
    }

    #[inline(always)]
    pub fn as_words(&self) -> &[usize] {
        self.0
    }

    /// View a byte-slice as a  usize-slice (no copy, no free).
    ///
    /// # Safety
    /// * `slice` length is checked to be a multiple of `u64`.
    /// * Caller guarantees the buffer lives at least `'a`.
    pub fn from_raw(slice: &'a [u8]) -> Self {
        assert_eq!(slice.len() % BYTECODE_ALIGNMENT, 0);
        let words = slice.len() / core::mem::size_of::<usize>();
        let ptr = slice.as_ptr() as *const usize;
        let ws = unsafe { core::slice::from_raw_parts(ptr, words) };
        Self(ws)
    }

    /// # Safety
    /// [pos] must be within the bounds of the bitmap.
    #[inline(always)]
    pub unsafe fn get_bit_unchecked(&self, pos: usize) -> bool {
        let (w, b) = (pos / usize::BITS as usize, pos % usize::BITS as usize);
        self.0.get_unchecked(w) & (1usize << b) != 0
    }
}

/// Result type for most instructions. Here `Err` signals that execution is suspended
/// rather than an error. A custom enum isn't used because those don't get to use `?`.
///
/// Those that perform an external call use [interpreter::Preemption] instead of ExitCode.
pub type InstructionResult = Result<(), ExitCode>;

///
/// Expected exit reasons from the EVM interpreter.
///
#[repr(u8)]
#[derive(Debug, Clone, PartialEq, Eq)]
// #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ExitCode {
    //success codes
    Stop = 0x01,
    Return = 0x02,
    SelfDestruct = 0x03,

    ExternalCall,

    // revert code
    Revert = 0x20, // revert opcode
    CallTooDeep = 0x21,
    OutOfFund = 0x22,

    // error codes
    OutOfGas = 0x50,
    MemoryOOG = 0x51,
    MemoryLimitOOG = 0x52,
    PrecompileOOG = 0x53,
    InvalidOperandOOG = 0x54,
    OpcodeNotFound,
    CallNotAllowedInsideStatic,
    StateChangeDuringStaticCall,
    InvalidFEOpcode,
    InvalidJump,
    NotActivated,
    StackUnderflow,
    StackOverflow,
    OutOfOffset,
    CreateCollision,
    OverflowPayment,
    PrecompileError,
    NonceOverflow,
    /// Create init code size exceeds limit (runtime).
    CreateContractSizeLimit,
    /// Error on created contract that begins with EF
    CreateContractStartingWithEF,
    /// EIP-3860: Limit and meter initcode. Initcode size limit exceeded.
    CreateInitcodeSizeLimit,

    // Fatal external error. Returned by database.
    FatalExternalError,

    // Fatal internal error
    FatalError(EvmSubsystemError),
}

impl From<SystemError> for ExitCode {
    fn from(e: SystemError) -> Self {
        match e {
            SystemError::LeafRuntime(RuntimeError::OutOfErgs(_)) => Self::OutOfGas,
            e => Self::FatalError(e.into()),
        }
    }
}

/// TODO this is a workaround. We need to contain ExitCode better inside EVM
/// interpreter but it requires a bit of untangling.
impl From<EvmSubsystemError> for ExitCode {
    fn from(e: EvmSubsystemError) -> Self {
        if let RootCause::Runtime(RuntimeError::OutOfErgs(_)) = e.root_cause() {
            Self::OutOfGas
        } else {
            Self::FatalError(e)
        }
    }
}

impl From<InternalError> for ExitCode {
    fn from(e: InternalError) -> Self {
        ExitCode::FatalError(e.into())
    }
}

impl ExitCode {
    fn is_error(&self) -> bool {
        matches!(
            self,
            Self::OutOfGas
                | Self::MemoryOOG
                | Self::MemoryLimitOOG
                | Self::PrecompileOOG
                | Self::InvalidOperandOOG
                | Self::OpcodeNotFound
                | Self::CallNotAllowedInsideStatic
                | Self::StateChangeDuringStaticCall
                | Self::InvalidFEOpcode
                | Self::InvalidJump
                | Self::NotActivated
                | Self::StackUnderflow
                | Self::StackOverflow
                | Self::OutOfOffset
                | Self::CreateCollision
                | Self::OverflowPayment
                | Self::PrecompileError
                | Self::NonceOverflow
                | Self::CreateContractSizeLimit
                | Self::CreateContractStartingWithEF
                | Self::CreateInitcodeSizeLimit
        )
    }
}
