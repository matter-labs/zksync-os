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

use core::ops::Range;

use ::u256::U256;
use evm_stack::EvmStack;
use gas::Gas;
use zk_ee::execution_environment_type::ExecutionEnvironmentType;
use zk_ee::memory::slice_vec::SliceVec;
use zk_ee::system::errors::{FatalError, InternalError, SystemError};
use zk_ee::system::{EthereumLikeTypes, Resource, System, SystemTypes};

use alloc::vec::Vec;
use zk_ee::types_config::*;
use zk_ee::utils::*;

mod ee_trait_impl;
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

// this is the interpreter that can be found in Reth itself, modified for purposes of having abstract view
// on memory and resources
pub struct Interpreter<'a, S: EthereumLikeTypes> {
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
    pub bytecode_preprocessing: BytecodePreprocessingData<S>,
    /// Call value
    pub call_value: U256,
    /// Is interpreter call static.
    pub is_static: bool,
    /// Is interpreter call executing construction code.
    pub is_constructor: bool,
}

pub const STACK_SIZE: usize = 1024;
pub const MAX_CODE_SIZE: usize = 0x6000;
pub const MAX_INITCODE_SIZE: usize = MAX_CODE_SIZE * 2;
pub const USIZE_SIZE: usize = core::mem::size_of::<usize>();
pub const JUMPDEST_BITMAP_MAX_SIZE: usize = MAX_CODE_SIZE / USIZE_SIZE;
pub const ERGS_PER_GAS: u64 = 256;
pub const ERGS_PER_GAS_U256: U256 = U256::from_limbs([ERGS_PER_GAS, 0, 0, 0]);

pub struct BytecodePreprocessingData<S: SystemTypes> {
    pub original_bytecode_len: usize,
    pub jumpdest_bitmap: BitMap<S>,
}

impl<S: SystemTypes> BytecodePreprocessingData<S> {
    pub(crate) fn empty(system: &mut System<S>) -> Self {
        let jumpdest_bitmap = BitMap::<S>::empty(system);
        let original_bytecode_len = 0;

        Self {
            original_bytecode_len,
            jumpdest_bitmap,
        }
    }

    pub fn is_valid_jumpdest(&self, offset: usize) -> bool {
        if offset >= self.original_bytecode_len {
            false
        } else {
            // we are in range of even the extended bytecode, so we are safe
            unsafe { self.jumpdest_bitmap.get_bit_unchecked(offset) }
        }
    }

    pub fn from_raw_bytecode(
        padded_bytecode: &[u8],
        original_len: u32,
        system: &mut System<S>,
        resources: &mut S::Resources,
    ) -> Result<Self, FatalError> {
        use crate::native_resource_constants::BYTECODE_PREPROCESSING_BYTE_NATIVE_COST;
        use zk_ee::system::{Computational, Resources};
        let native_cost = <S::Resources as Resources>::Native::from_computational(
            BYTECODE_PREPROCESSING_BYTE_NATIVE_COST.saturating_mul(padded_bytecode.len() as u64),
        );
        resources
            .charge(&S::Resources::from_native(native_cost))
            .map_err(|e| match e {
                SystemError::Internal(e) => FatalError::Internal(e),
                SystemError::OutOfErgs => {
                    FatalError::Internal(InternalError("OOE when charging only native"))
                }
                SystemError::OutOfNativeResources => FatalError::OutOfNativeResources,
            })?;
        let jump_map = analyze::<S>(padded_bytecode, system)
            .map_err(|_| InternalError("Could not preprocess bytecode"))?;
        let new = Self {
            original_bytecode_len: original_len as usize,
            jumpdest_bitmap: jump_map,
        };

        Ok(new)
    }
}

pub struct BitMap<S: SystemTypes> {
    inner: Vec<usize, S::Allocator>,
}

impl<S: SystemTypes> BitMap<S> {
    pub(crate) fn empty(system: &mut System<S>) -> Self {
        Self {
            inner: Vec::new_in(system.get_allocator()),
        }
    }

    pub(crate) fn allocate_for_bit_capacity(capacity: usize, system: &mut System<S>) -> Self {
        let usize_capacity =
            capacity.next_multiple_of(usize::BITS as usize) / (usize::BITS as usize);
        let mut storage = Vec::with_capacity_in(usize_capacity, system.get_allocator());
        storage.resize(usize_capacity, 0);

        Self { inner: storage }
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

/// Analyzs bytecode to build a jump map.
fn analyze<S: SystemTypes>(code: &[u8], system: &mut System<S>) -> Result<BitMap<S>, ()> {
    let code_len = code.len();
    let mut jumps = BitMap::<S>::allocate_for_bit_capacity(code_len, system);
    let mut code_iter = code.iter().copied().enumerate();

    use self::opcodes as opcode;

    while let Some((offset, opcode)) = code_iter.next() {
        if opcode::JUMPDEST == opcode {
            // SAFETY: jumps are max length of the code
            unsafe { jumps.set_bit_on_unchecked(offset) }
            // step by 1 is automatic
        } else {
            let push_bytes = opcode.wrapping_sub(opcode::PUSH1);
            if push_bytes < 32 {
                // we just consumed encoding of "PUSH_X" itself, and now we should
                // consume X bytes after
                // step by 1 is automatic, and then we need to skip push_offset + 1
                match code_iter.advance_by((push_bytes + 1) as usize) {
                    Ok(_) => {
                        // nothing, we continue
                    }
                    Err(_advanced_by) => {
                        // actually we are fine, since bytecode is virtually extendable with zero-pad for EVM
                    }
                }
            } else {
                // step by 1 is automatic
            }
        }
    }

    Ok(jumps)
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
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
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
    FatalError(FatalError),
}

impl From<SystemError> for ExitCode {
    fn from(e: SystemError) -> Self {
        match e {
            SystemError::Internal(e) => Self::FatalError(FatalError::Internal(e)),
            SystemError::OutOfNativeResources => Self::FatalError(FatalError::OutOfNativeResources),
            SystemError::OutOfErgs => Self::OutOfGas,
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
