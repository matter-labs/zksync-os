#![cfg_attr(not(feature = "testing"), no_std)]
#![feature(allocator_api)]
#![allow(incomplete_features)]
#![allow(clippy::new_without_default)]
#![allow(clippy::needless_lifetimes)]
#![allow(clippy::needless_borrow)]
#![allow(clippy::needless_borrows_for_generic_args)]
#![allow(clippy::bool_comparison)]
#![cfg_attr(
    any(feature = "error_origins", not(target_arch = "riscv32")),
    allow(clippy::result_large_err)
)]
#![cfg_attr(
    any(feature = "error_origins", not(target_arch = "riscv32")),
    allow(clippy::large_enum_variant)
)]

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
use gas_constants::{SHA3, SHA3WORD};
use u256::U256;
use zk_ee::execution_environment_type::ExecutionEnvironmentType;
use zk_ee::memory::slice_vec::SliceVec;
use zk_ee::system::errors::root_cause::{GetRootCause, RootCause};
use zk_ee::system::errors::runtime::{FatalRuntimeError, RuntimeError};
use zk_ee::system::errors::{internal::InternalError, system::SystemError};
use zk_ee::system::evm::{EvmFrameInterface, EvmStackInterface};
use zk_ee::system::{ErgsResource, EthereumLikeTypes, Resource, Resources, System, SystemTypes};

use alloc::vec::Vec;
use zk_ee::utils::*;
use zk_ee::{internal_error, types_config::*};

use zk_ee::system::evm::EvmError;

mod code_with_sentinel;
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
pub mod precompile_addresses;
pub mod u256_helpers;
pub mod utils;
pub mod v2;

pub(crate) use code_with_sentinel::CodeWithSentinel;

pub(crate) const THIS_EE_TYPE: ExecutionEnvironmentType = ExecutionEnvironmentType::EVM;

/// No artifacts cached
pub const DEFAULT_CODE_VERSION_BYTE: u8 = 0u8;

/// Artifacts cached
pub const ARTIFACTS_CACHING_CODE_VERSION_BYTE: u8 = 1u8;

/// Artifacts are not a part of the deployed bytecode (as for `DEFAULT_CODE_VERSION_BYTE`), but the
/// storage model computed them when it loaded the code, and keeps them next to it for the rest
/// of the block. Same layout as for `ARTIFACTS_CACHING_CODE_VERSION_BYTE`, but every frame is
/// charged as if it computed them, so resources do not depend on who computes the artifacts.
pub const ARTIFACTS_FROM_CODE_CACHE_CODE_VERSION_BYTE: u8 = 2u8;

/// An internal flag used to indicate that EE is waiting for the result of some preemption from OS (call or create request).
/// Is public for testing purposes.
pub enum PendingOsRequest<S: SystemTypes> {
    Call,
    Create(<S::IOTypes as SystemIOTypesConfig>::Address),
}

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
    /// Heap that belongs to this interpreter, can be resized
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
    /// Indicating that EE is waiting for the result of some operation from the OS. `continue_after_preemption` will panic if this is None
    pub pending_os_request: Option<PendingOsRequest<S>>,
    /// Why the last `run` stopped: stored by the instruction that stopped it, so the loop
    /// only passes a word around
    pub exit_code: Option<ExitCode>,
}

/// Wrapper to provide external access to EVM frame state
pub struct InterpreterExternal<'ee, S: EthereumLikeTypes> {
    interpreter: &'ee Interpreter<'ee, S>,
    #[allow(dead_code)]
    system: &'ee System<S>,
}

impl<'ee, S: EthereumLikeTypes> InterpreterExternal<'ee, S> {
    pub fn new_from(interpreter: &'ee Interpreter<'ee, S>, system: &'ee System<S>) -> Self {
        Self {
            interpreter,
            system,
        }
    }
}

impl<'ee, S: EthereumLikeTypes> EvmFrameInterface<S> for InterpreterExternal<'ee, S> {
    fn instruction_pointer(&self) -> usize {
        self.interpreter.instruction_pointer
    }

    fn resources(&self) -> &<S as SystemTypes>::Resources {
        &self.interpreter.gas.resources
    }

    fn stack(&self) -> &impl EvmStackInterface {
        &self.interpreter.stack
    }

    fn caller(&self) -> <<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::Address {
        self.interpreter.caller
    }

    fn address(&self) -> <<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::Address {
        self.interpreter.address
    }

    fn calldata(&self) -> &[u8] {
        &self.interpreter.calldata
    }

    fn return_data(&self) -> &[u8] {
        &self.interpreter.returndata
    }

    fn heap(&self) -> &[u8] {
        &self.interpreter.heap
    }

    fn bytecode(&self) -> &[u8] {
        &self.interpreter.bytecode
    }

    fn call_value(&self) -> &U256 {
        &self.interpreter.call_value
    }

    fn is_static(&self) -> bool {
        self.interpreter.is_static
    }

    fn is_constructor(&self) -> bool {
        self.interpreter.is_constructor
    }

    fn refund_counter(&self) -> u32 {
        use zk_ee::system::IOSubsystem;
        let refund = self.system.io.get_refund_counter();
        refund.ergs().as_legacy_gas_ceil() as u32
    }
}

pub const STACK_SIZE: usize = 1024;
/// Zero bytes a padded code buffer carries after the code: 32 for the immediate of a
/// `PUSH32` at the last byte, one for the `STOP` fetched after it (rounded up to whole
/// words). See `EthereumLikeTypes::CODE_IS_PADDED`.
pub const CODE_PADDING_BYTES: usize = 40;
pub const MAX_CODE_SIZE: usize = 0x6000;
pub const MAX_INITCODE_SIZE: usize = MAX_CODE_SIZE * 2;
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

    /// [`Self::parse_bytecode`] for the v2 interpreter on unpadded code, which needs a zero
    /// byte after the code ([`CodeWithSentinel`]). In the preimage `code || padding || bitmap`
    /// it is the padding, or bitmap byte 0 if the length is a multiple of 8 (nonzero if a
    /// JUMPDEST is among the first 8 bytes), and empty code has none. So the code is borrowed
    /// only if a zero byte follows it inside `bytecode` (never the rest of the allocation,
    /// which the oracle may have written), and copied otherwise.
    #[inline(always)]
    pub(crate) fn parse_bytecode_followed_by_zero(
        allocator: A,
        bytecode: &'a [u8],
        deployed_len: usize,
        artifacts_len: usize,
    ) -> Result<(CodeWithSentinel<'a, A>, Self), InternalError> {
        let (code, preprocessing) = Self::parse_bytecode(bytecode, deployed_len, artifacts_len)?;
        let code = match CodeWithSentinel::borrow_followed_by_zero(bytecode, deployed_len) {
            Some(code) => code,
            None => CodeWithSentinel::padded_copy_cold(allocator, code),
        };
        Ok((code, preprocessing))
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
        Self::charge_for_artifacts(deployed_code.len(), resources)?;
        Ok(Self::create_artifacts_inner(allocator, deployed_code))
    }

    ///
    /// Charge of `create_artifacts`
    ///
    pub fn charge_for_artifacts<R: Resources>(
        deployed_code_len: usize,
        resources: &mut R,
    ) -> Result<(), SystemError> {
        use crate::native_resource_constants::BYTECODE_PREPROCESSING_BYTE_NATIVE_COST;
        use zk_ee::system::Computational;
        let native_cost = R::Native::from_computational(
            BYTECODE_PREPROCESSING_BYTE_NATIVE_COST.saturating_mul(deployed_code_len as u64),
        );
        resources
            .charge(&R::from_native(native_cost))
            .map_err(|e| -> SystemError {
                match e {
                    e @ SystemError::LeafDefect(_) => e,
                    SystemError::LeafRuntime(RuntimeError::OutOfErgs(_)) => {
                        SystemError::LeafDefect(internal_error!("OOE when charging only native"))
                    }
                    e @ SystemError::LeafRuntime(RuntimeError::FatalRuntimeError(_)) => e,
                }
            })
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
    pub(crate) fn bitmap_words(&self) -> &[usize] {
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
    /// Allocates a zeroed bitmap with space for at least `capacity` bits.
    pub fn allocate_for_bit_capacity(capacity: usize, allocator: A) -> Self {
        let u64_capacity = capacity.div_ceil(u64::BITS as usize);
        let word_capacity = u64_capacity * (u64::BITS as usize / usize::BITS as usize);
        let mut storage = Vec::with_capacity_in(word_capacity, allocator);
        storage.resize(word_capacity, 0);

        Self { inner: storage }
    }

    #[inline(always)]
    pub fn as_words(&self) -> &[usize] {
        &self.inner
    }

    /// Returns the bit at `pos`, or `None` if it is outside the bitmap.
    #[inline(always)]
    pub fn get_bit(&self, pos: usize) -> Option<bool> {
        let (word_idx, bit_idx) = (pos / usize::BITS as usize, pos % usize::BITS as usize);
        self.inner
            .get(word_idx)
            .map(|word| word & (1usize << bit_idx) != 0)
    }

    /// Sets the bit at `pos`. Returns `false` if it is outside the bitmap.
    #[inline(always)]
    pub fn set_bit_on(&mut self, pos: usize) -> bool {
        let (word_idx, bit_idx) = (pos / usize::BITS as usize, pos % usize::BITS as usize);
        let Some(word) = self.inner.get_mut(word_idx) else {
            return false;
        };
        *word |= 1usize << bit_idx;
        true
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
    let mut jumps = BitMapOwned::<A>::allocate_for_bit_capacity(code.len(), allocator);
    analyze_into(code, &mut jumps.inner);

    jumps
}

/// Length of the jumpdest artifacts (the bitmap, as bytes) for the code of the given length
pub const fn artifacts_byte_len(code_len: usize) -> usize {
    code_len.div_ceil(u64::BITS as usize) * core::mem::size_of::<u64>()
}

/// Marks valid jump destinations of the `code` in the zeroed `bitmap`, that should be
/// `artifacts_byte_len(code.len())` bytes long.
pub fn analyze_into(code: &[u8], bitmap: &mut [usize]) {
    use self::opcodes as opcode;

    assert!(core::mem::size_of_val(bitmap) >= artifacts_byte_len(code.len()));

    // The loop runs over every opcode of every contract that gets executed, so it walks the code
    // by a pointer: an iteration is a load, the checks for JUMPDEST and PUSH, and the increment.
    let start = code.as_ptr();
    let end = start.wrapping_add(code.len());
    let mut position = start;
    while position < end {
        // SAFETY: `position` is in the bounds of the `code`
        let op = unsafe { position.read() };
        if op == opcode::JUMPDEST {
            let pos = position.addr() - start.addr();
            let (word_idx, bit_idx) = (pos / usize::BITS as usize, pos % usize::BITS as usize);
            // SAFETY: the length of the bitmap is checked above
            unsafe { *bitmap.get_unchecked_mut(word_idx) |= 1usize << bit_idx };
            position = position.wrapping_add(1);
        } else if (opcode::PUSH1..=opcode::PUSH32).contains(&op) {
            // the immediate can go beyond the end of the code
            position = position.wrapping_add(1 + (op - opcode::PUSH1 + 1) as usize);
        } else {
            position = position.wrapping_add(1);
        }
    }
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
    /// # Panics
    /// If the length of `slice` is not a multiple of `u64`, or `slice` is not empty and not
    /// aligned for `usize`.
    pub fn from_raw(slice: &'a [u8]) -> Self {
        assert_eq!(slice.len() % BYTECODE_ALIGNMENT, 0);
        if slice.is_empty() {
            // an empty byte slice can be anywhere, an empty `&[usize]` must be aligned too
            return Self::empty();
        }
        let ptr = slice.as_ptr().cast::<usize>();
        assert!(ptr.is_aligned(), "jumpdest bitmap is not aligned for usize");
        let words = slice.len() / core::mem::size_of::<usize>();
        // SAFETY: `ptr` is aligned, and the `words` words from it are `slice`, borrowed for `'a`
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
/// Kept small (4 bytes on the proving target, where `EvmSubsystemError` is 3) so that every
/// `Result<_, ExitCode>` of the instruction helpers is returned in registers.
///
#[repr(u8)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExitCode {
    //success codes
    Stop = 0x01,
    Return = 0x02,
    SelfDestruct = 0x03,

    ExternalCall,

    // EVM-defined error
    EvmError(EvmError),

    // Fatal runtime error (out of native resources, out of return memory)
    FatalRuntime(FatalRuntimeError),

    // Fatal internal error of the interpreter or a subsystem it called
    FatalError(EvmSubsystemError),
}

#[cfg(target_arch = "riscv32")]
const _: () = {
    // the helpers return `Result<&U256, ExitCode>` in two registers
    assert!(core::mem::size_of::<ExitCode>() <= 4);
};

impl From<EvmError> for ExitCode {
    fn from(e: EvmError) -> Self {
        Self::EvmError(e)
    }
}

impl<'a, S: EthereumLikeTypes> Interpreter<'a, S> {
    /// The exit code of a fatal error
    #[cold]
    #[inline(never)]
    pub(crate) fn fatal(e: impl Into<EvmSubsystemError>) -> ExitCode {
        ExitCode::FatalError(e.into())
    }

    /// Exit code for an error of a system call made by an instruction
    #[inline(always)]
    pub(crate) fn system_error(e: SystemError) -> ExitCode {
        match e {
            SystemError::LeafRuntime(RuntimeError::OutOfErgs(_)) => {
                ExitCode::EvmError(EvmError::OutOfGas)
            }
            SystemError::LeafRuntime(RuntimeError::FatalRuntimeError(f)) => {
                ExitCode::FatalRuntime(f)
            }
            SystemError::LeafDefect(e) => Self::fatal(e),
        }
    }

    /// Exit code for an error of the EVM subsystem itself (deployment, address derivation)
    #[inline(always)]
    pub(crate) fn subsystem_error(e: EvmSubsystemError) -> ExitCode {
        match e.root_cause() {
            RootCause::Runtime(RuntimeError::OutOfErgs(_)) => {
                ExitCode::EvmError(EvmError::OutOfGas)
            }
            RootCause::Runtime(RuntimeError::FatalRuntimeError(f)) => {
                ExitCode::FatalRuntime(f.clone())
            }
            _ => Self::fatal(e),
        }
    }
}

///
/// Gas cost for keccak on a given input size.
///
pub fn keccak256_gas_cost(len: usize) -> u64 {
    let words = len.div_ceil(32);
    SHA3.saturating_add(SHA3WORD.saturating_mul(words as u64))
}

#[cfg(test)]
mod jumpdest_analysis_tests {
    use super::*;

    /// A plain index-based scan, as a reference for the pointer-walking one.
    fn analyze_reference(code: &[u8], bitmap: &mut [usize]) {
        let mut i = 0;
        while i < code.len() {
            let op = code[i];
            if op == opcodes::JUMPDEST {
                bitmap[i / usize::BITS as usize] |= 1usize << (i % usize::BITS as usize);
                i += 1;
            } else if (opcodes::PUSH1..=opcodes::PUSH32).contains(&op) {
                i += 1 + (op - opcodes::PUSH1 + 1) as usize;
            } else {
                i += 1;
            }
        }
    }

    #[test]
    fn scan_matches_reference() {
        // deterministic LCG so the test needs no dependencies
        let mut seed: u64 = 0x9e37_79b9_7f4a_7c15;
        let mut next = || {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (seed >> 33) as u32
        };
        for case in 0..2000usize {
            let len = (next() % 300) as usize;
            let mut buf = alloc::vec![0u8; len + 8];
            for b in buf.iter_mut() {
                // bias towards the interesting opcodes and towards plain runs
                *b = match next() % 8 {
                    0 => opcodes::JUMPDEST,
                    1 | 2 => opcodes::PUSH1 + (next() % 32) as u8,
                    3 => 0x5c + (next() % 4) as u8,
                    _ => (next() % 0x5b) as u8,
                };
            }
            let offset = case % 5;
            let code = &buf[offset..offset + len];
            let words = artifacts_byte_len(code.len()) / core::mem::size_of::<usize>();
            let mut expected = alloc::vec![0usize; words];
            let mut actual = alloc::vec![0usize; words];
            analyze_reference(code, &mut expected);
            analyze_into(code, &mut actual);
            assert_eq!(expected, actual, "case {case}, len {len}, offset {offset}");
        }
    }

    /// Straightforward analysis by indexes
    fn reference(code: &[u8]) -> alloc::vec::Vec<bool> {
        let mut result = alloc::vec![false; code.len()];
        let mut i = 0;
        while i < code.len() {
            let op = code[i];
            if op == opcodes::JUMPDEST {
                result[i] = true;
            }
            i += 1;
            if (opcodes::PUSH1..=opcodes::PUSH32).contains(&op) {
                i += (op - opcodes::PUSH1 + 1) as usize;
            }
        }
        result
    }

    fn check(code: &[u8]) {
        let expected = reference(code);
        let artifacts =
            BytecodePreprocessingData::create_artifacts_inner(alloc::alloc::Global, code);
        assert_eq!(artifacts.as_slice().len(), artifacts_byte_len(code.len()));
        for (i, expected) in expected.iter().enumerate() {
            assert_eq!(artifacts.is_valid_jumpdest(i), *expected, "position {i}");
        }
        assert!(!artifacts.is_valid_jumpdest(code.len()));
        assert!(!artifacts.is_valid_jumpdest(usize::MAX));
    }

    #[test]
    fn jumpdest_in_push_data_and_truncated_push() {
        let jd = opcodes::JUMPDEST;
        check(&[]);
        check(&[jd]);
        // JUMPDEST inside of the immediate is not a destination, the one after it is
        check(&[opcodes::PUSH1, jd, jd]);
        check(&[opcodes::PUSH2, jd, jd, jd, opcodes::PUSH32]);
        // PUSH with the immediate that goes beyond the end of the code
        check(&[jd, opcodes::PUSH32, jd, jd]);
        check(&[opcodes::PUSH32]);
        // boundaries of the words of the bitmap
        let mut code = alloc::vec![0u8; 300];
        for i in [0, 31, 32, 63, 64, 65, 127, 128, 255, 256, 299] {
            code[i] = jd;
        }
        check(&code);
    }

    #[test]
    fn pseudo_random_code() {
        // xorshift, biased to PUSHes and JUMPDESTs
        let mut state = 0x9E3779B97F4A7C15u64;
        let mut next = || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        for _ in 0..200 {
            let len = (next() % 700) as usize;
            let code: alloc::vec::Vec<u8> = (0..len)
                .map(|_| match next() % 4 {
                    0 => opcodes::JUMPDEST,
                    1 => opcodes::PUSH1 + (next() % 32) as u8,
                    _ => next() as u8,
                })
                .collect();
            check(&code);
        }
    }
}

#[cfg(test)]
mod code_sentinel_tests {
    use super::*;
    use crate::code_with_sentinel::tests::code_and_sentinel;
    use alloc::alloc::Global;

    /// The bytes of `words`: a buffer aligned like a preimage (and like the bitmap in it)
    fn bytes_of(words: &mut [u64]) -> &mut [u8] {
        // SAFETY: the bytes of the `u64`s, borrowed from `words` for the lifetime of the result
        unsafe { core::slice::from_raw_parts_mut(words.as_mut_ptr().cast::<u8>(), words.len() * 8) }
    }

    fn check(code: &[u8], pad_override: Option<u8>, expected_copy: bool, check_jumps: bool) {
        let artifacts = BytecodePreprocessingData::create_artifacts_inner(Global, code);
        let artifact_bytes = artifacts.as_slice();
        let padded_len = code.len().div_ceil(BYTECODE_ALIGNMENT) * BYTECODE_ALIGNMENT;
        let total_len = padded_len + artifact_bytes.len();
        let mut words = alloc::vec![0u64; total_len.div_ceil(8)];
        let raw = bytes_of(&mut words);
        raw[..code.len()].copy_from_slice(code);
        raw[padded_len..total_len].copy_from_slice(artifact_bytes);
        if let Some(pad) = pad_override {
            raw[code.len()] = pad;
        }
        let bytecode = &raw[..total_len];
        let (returned, preprocessing) = BytecodePreprocessingData::parse_bytecode_followed_by_zero(
            Global,
            bytecode,
            code.len(),
            artifact_bytes.len(),
        )
        .unwrap();
        assert_eq!(code_and_sentinel(&returned), (code, 0));
        assert_eq!(returned.is_copy(), expected_copy);
        assert_eq!(returned.as_ptr() == bytecode.as_ptr(), !expected_copy);
        assert_eq!(preprocessing.original_bytecode_len, returned.len());
        if check_jumps {
            let reference = BytecodePreprocessingData::create_artifacts_inner(Global, code);
            for off in 0..code.len() {
                assert_eq!(
                    preprocessing.is_valid_jumpdest(off),
                    reference.is_valid_jumpdest(off)
                );
            }
        }
    }

    #[test]
    fn flat_layout_boundaries() {
        check(&[opcodes::JUMPDEST], None, false, true);
        check(&[0x60, 0x2a, 0, 0, 0, 0, 0, 0], None, false, true);
        check(
            &[0x5b, 0x60, 0x2a, 0x60, 0, 0x55, 0x5b, 0x5b],
            None,
            true,
            true,
        );
        // The JUMPDEST byte is PUSH data, so bit zero of the bitmap is clear.
        check(&[0x60, 0x5b, 0, 0, 0, 0, 0, 0], None, false, true);
        check(&[], None, true, true);
        check(&[0x60, 0x01], Some(0xfe), true, true);
    }

    #[test]
    fn exact_code_without_artifacts() {
        let mut words = [0u64; 1];
        let raw = bytes_of(&mut words);
        raw[..3].copy_from_slice(&[0x5b, 0x60, 0x01]);
        let code = &raw[..3];
        // The bitmap is the empty slice after the code, 3 bytes into an aligned buffer.
        let (returned, data) =
            BytecodePreprocessingData::parse_bytecode_followed_by_zero(Global, code, 3, 0).unwrap();
        assert_eq!(code_and_sentinel(&returned), (code, 0));
        assert!(returned.is_copy());
        // No jump check: an artifact-free nonempty code has no bitmap.
        assert!(data.bitmap_words().is_empty());
    }

    #[test]
    fn oracle_tail_is_never_borrowed() {
        let code = [0x60, 0x01, 0x55, 0, 0, 0, 0, 0];
        let mut words = [0xfefefefefefefefeu64; 3];
        let raw = bytes_of(&mut words);
        raw[..8].copy_from_slice(&code);
        // The oracle wrote FE after the committed slice, which is only 8 bytes.
        let (returned, _) =
            BytecodePreprocessingData::parse_bytecode_followed_by_zero(Global, &raw[..8], 8, 0)
                .unwrap();
        assert_eq!(code_and_sentinel(&returned), (&code[..], 0));
        assert_ne!(returned.as_ptr(), raw.as_ptr());
        assert_eq!(raw[8], 0xfe);
        // The byte is in the slice this time, but is nonzero and must still be copied.
        let (returned, _) =
            BytecodePreprocessingData::parse_bytecode_followed_by_zero(Global, &raw[..16], 8, 8)
                .unwrap();
        assert_eq!(code_and_sentinel(&returned), (&code[..], 0));
        assert_ne!(returned.as_ptr(), raw.as_ptr());
    }

    #[test]
    fn randomized_flat_layout() {
        let mut seed = 0x9e37_79b9_7f4a_7c15u64;
        for len in 0..=200 {
            let mut code = alloc::vec![0; len];
            for byte in &mut code {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                *byte = (seed >> 32) as u8;
            }
            let reference = BytecodePreprocessingData::create_artifacts_inner(Global, &code);
            let copied = len % 8 == 0 && (len == 0 || reference.as_slice()[0] != 0);
            check(&code, None, copied, true);
        }
    }

    #[test]
    fn bitmap_from_raw_empty_and_aligned() {
        let mut words = [u64::MAX; 2];
        let raw = bytes_of(&mut words);
        // an empty bitmap anywhere, also at an address no `usize` can have
        assert!(BitMap::from_raw(&raw[3..3]).as_words().is_empty());
        let bitmap = BitMap::from_raw(&raw[8..16]);
        assert_eq!(core::mem::size_of_val(bitmap.as_words()), 8);
        assert!(bitmap.as_words().iter().all(|word| *word == usize::MAX));
    }

    #[test]
    #[should_panic(expected = "not aligned")]
    fn bitmap_from_raw_rejects_misaligned() {
        let mut words = [0u64; 3];
        let raw = bytes_of(&mut words);
        BitMap::from_raw(&raw[1..9]);
    }
}
