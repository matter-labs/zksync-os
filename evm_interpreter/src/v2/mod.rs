//! Second-generation EVM interpreter.
//!
//! Same observable behavior as [`crate::Interpreter`] (gas and native accounting, exit
//! reasons, tracer events on the host), restructured around what the proving target pays for:
//!
//! - The frame is split into [`HotFrameParts`] and [`ColdFrameParts`]. The hot part is what
//!   every instruction touches (instruction pointer, stack, resources); for a run of the loop
//!   it is loaded into [`hot::Hot`], a struct of plain values whose address never escapes, so
//!   the compiler keeps it in registers. The cold part (addresses, calldata, heap, code,
//!   pending requests) is reached through a pointer.
//! - Every instruction is a method of `Hot`. Frequent ones are `#[inline(always)]` and become
//!   arms of the loop. Instructions that talk to the system, grow the heap, or are long are
//!   `#[inline(never)]` and run on a copy of the hot state ([`hot::Hot::outlined`]): passing
//!   `&mut Hot` to a real call would put the hot state back in memory for the whole loop.
//! - The dispatch is a loop over a jump table: on the proving machine a cycle is an
//!   instruction and there is no branch predictor, so threaded (tail-call) dispatch has no
//!   advantage over a loop and would pay a prologue per instruction. The gain of the threaded
//!   designs, hot state in registers, is what `Hot` gives the loop.
//! - Tracer hooks, the instruction counter and opcode logging are compiled out on `riscv32`;
//!   the `Env` of a run has no tracer field there at all.

use core::ops::Range;

use u256::U256;
use zk_ee::common_structs::system_hooks::HooksStorage;
use zk_ee::memory::slice_vec::SliceVec;
use zk_ee::system::errors::root_cause::{GetRootCause, RootCause};
use zk_ee::system::errors::runtime::RuntimeError;
use zk_ee::system::errors::system::SystemError;
use zk_ee::system::evm::{EvmError, EvmFrameInterface, EvmStackInterface};
use zk_ee::system::tracer::Tracer;
use zk_ee::system::{ErgsResource, EthereumLikeTypes, Resource, Resources, System, SystemTypes};
use zk_ee::types_config::SystemIOTypesConfig;

use crate::errors::EvmSubsystemError;
use crate::evm_stack::EvmStack;
use crate::interpreter::EVMCallRequest;
use crate::{BytecodePreprocessingData, ExitCode, PendingOsRequest};

/// A tracer call: compiled out on the proving target, where the tracer is always the no-op one.
macro_rules! trace {
    ($env:expr, |$t:ident| $call:expr) => {
        #[cfg(not(target_arch = "riscv32"))]
        {
            let $t = &mut *$env.tracer;
            $call;
        }
    };
}

mod dispatch;
mod ee;
pub(crate) mod hot;
mod ops_cold;
mod ops_hot;

pub(crate) use hot::Hot;

/// The part of the frame every instruction touches. Between runs of the loop it lives here;
/// during a run it lives in [`Hot`], which borrows this holder for the whole run.
pub struct HotFrameParts<S: EthereumLikeTypes> {
    /// Instruction pointer.
    pub instruction_pointer: usize,
    /// Stack.
    pub stack: EvmStack<S::Allocator>,
    /// Resources of the frame.
    pub resources: S::Resources,
}

/// The part of the frame that is read through a pointer, by the instructions that need it.
pub struct ColdFrameParts<'a, S: EthereumLikeTypes> {
    /// Caller address
    pub caller: <S::IOTypes as SystemIOTypesConfig>::Address,
    /// Address of the executing contract
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
    /// The words of the jumpdest bitmap in `bytecode_preprocessing`, so a jump reads the bitmap
    /// without going through the borrowed/owned distinction
    jumpdest_words: *const usize,
    /// Call value
    pub call_value: U256,
    /// Is interpreter call static.
    pub is_static: bool,
    /// Is interpreter call executing construction code.
    pub is_constructor: bool,
    /// Gas already paid for the heap
    pub gas_paid_for_heap_growth: u64,
    /// Indicating that EE is waiting for the result of some operation from the OS. `continue_after_preemption` will panic if this is None
    pub pending_os_request: Option<PendingOsRequest<S>>,
    /// The call or deployment request of the instruction that stopped the last run with
    /// `ExitCode::ExternalCall`
    pub(crate) pending_call: Option<EVMCallRequest<S>>,
}

// SAFETY: `jumpdest_words` points into `bytecode_preprocessing`, owned by (or borrowed for
// `'a` by) the same struct, so it adds no sharing beyond what the other fields carry; the
// bounds are those the auto traits would derive from them.
unsafe impl<'a, S: EthereumLikeTypes> Send for ColdFrameParts<'a, S>
where
    S::Allocator: Send,
    S::Resources: Send,
    <S::IOTypes as SystemIOTypesConfig>::Address: Send,
{
}

// SAFETY: as for `Send`
unsafe impl<'a, S: EthereumLikeTypes> Sync for ColdFrameParts<'a, S>
where
    S::Allocator: Sync,
    S::Resources: Sync,
    <S::IOTypes as SystemIOTypesConfig>::Address: Sync,
{
}

/// A frame of the EVM
pub struct Interpreter<'a, S: EthereumLikeTypes> {
    pub hot: HotFrameParts<S>,
    pub cold: ColdFrameParts<'a, S>,
    /// Why the last run stopped: stored by the instruction that stopped it
    pub exit_code: Option<ExitCode>,
}

/// Everything an instruction may need besides the frame itself
pub(crate) struct Env<'a, S: EthereumLikeTypes, T: Tracer<S>> {
    pub system: &'a mut System<S>,
    pub hooks: &'a mut HooksStorage<S, S::Allocator>,
    #[cfg(not(target_arch = "riscv32"))]
    pub tracer: &'a mut T,
    /// Instructions executed by the run, for the log
    #[cfg(not(target_arch = "riscv32"))]
    pub cycles: usize,
    #[cfg(target_arch = "riscv32")]
    _tracer: core::marker::PhantomData<&'a mut T>,
}

impl<'a, S: EthereumLikeTypes, T: Tracer<S>> Env<'a, S, T> {
    #[inline(always)]
    pub(crate) fn new(
        system: &'a mut System<S>,
        hooks: &'a mut HooksStorage<S, S::Allocator>,
        tracer: &'a mut T,
    ) -> Self {
        #[cfg(target_arch = "riscv32")]
        let _ = tracer;
        Self {
            system,
            hooks,
            #[cfg(not(target_arch = "riscv32"))]
            tracer,
            #[cfg(not(target_arch = "riscv32"))]
            cycles: 0,
            #[cfg(target_arch = "riscv32")]
            _tracer: core::marker::PhantomData,
        }
    }
}

impl<'a, S: EthereumLikeTypes> ColdFrameParts<'a, S> {
    /// Installs the code of the frame together with its jumpdest artifacts
    #[inline(always)]
    pub(crate) fn set_code(
        &mut self,
        code: &'a [u8],
        preprocessing: BytecodePreprocessingData<'a, S::Allocator>,
    ) {
        self.bytecode = code;
        self.bytecode_preprocessing = preprocessing;
        self.jumpdest_words = self.bytecode_preprocessing.bitmap_words().as_ptr();
    }

    /// Is `offset` a valid jump destination of the code of the frame
    #[inline(always)]
    pub(crate) fn is_valid_jumpdest(&self, offset: usize) -> bool {
        const BITS: usize = usize::BITS as usize;
        offset < self.bytecode_preprocessing.original_bytecode_len && {
            // SAFETY: the bitmap has at least `original_bytecode_len` bits (`artifacts_byte_len`)
            let word = unsafe { *self.jumpdest_words.add(offset / BITS) };
            (word >> (offset % BITS)) & 1 != 0
        }
    }

    #[inline(always)]
    pub(crate) const fn is_static_frame(&self) -> bool {
        self.is_static
    }

    #[inline(always)]
    pub(crate) fn clear_last_returndata(&mut self) {
        self.returndata_location = 0..0;
    }
}

impl<'a, S: EthereumLikeTypes> Interpreter<'a, S> {
    /// Gives resources back to the frame (after a call returned them)
    pub fn reclaim_resources(&mut self, resources: S::Resources) {
        self.hot.resources.reclaim(resources);
    }

    /// Address of a contract deployed by `deployer_address` with `deployer_nonce` (CREATE)
    pub fn derive_address_for_deployment_create(
        resources: &mut S::Resources,
        deployer_address: &<S::IOTypes as SystemIOTypesConfig>::Address,
        deployer_nonce: u64,
    ) -> Result<<S::IOTypes as SystemIOTypesConfig>::Address, EvmSubsystemError> {
        crate::Interpreter::<S>::derive_address_for_deployment_create(
            resources,
            deployer_address,
            deployer_nonce,
        )
    }

    /// Address of a contract deployed with CREATE2
    pub fn derive_address_for_deployment_create2(
        system: &mut System<S>,
        resources: &mut S::Resources,
        salt: &U256,
        deployer_address: &<S::IOTypes as SystemIOTypesConfig>::Address,
        deployment_code: &[u8],
    ) -> Result<<S::IOTypes as SystemIOTypesConfig>::Address, EvmSubsystemError> {
        crate::Interpreter::<S>::derive_address_for_deployment_create2(
            system,
            resources,
            salt,
            deployer_address,
            deployment_code,
        )
    }
}

/// Casts a 256-bit operand to `usize`, through `u32` so that the 64-bit host and the 32-bit
/// proving target accept the same operands.
#[inline(always)]
pub(crate) fn cast_to_usize(src: &U256, error_to_set: ExitCode) -> Result<usize, ExitCode> {
    match src.try_to_u32() {
        Some(value) => Ok(value as usize),
        None => Err(error_to_set),
    }
}

/// Casts a 256-bit value to `u64`: copy opcodes charge their length-dependent cost from this
/// width-independent value, before narrowing it with [`cast_to_usize`].
#[inline(always)]
pub(crate) fn cast_to_u64(src: &U256, error_to_set: ExitCode) -> Result<u64, ExitCode> {
    src.try_to_u64().ok_or(error_to_set)
}

/// Memory offset and length of a range operand; the offset is ignored for an empty range.
#[inline(always)]
pub(crate) fn cast_offset_and_len(
    offset: &U256,
    len: &U256,
    error_to_set: ExitCode,
) -> Result<(usize, usize), ExitCode> {
    if len.is_zero() {
        Ok((0, 0))
    } else {
        let offset = cast_to_usize(offset, error_to_set.clone())?;
        let len = cast_to_usize(len, error_to_set)?;
        Ok((offset, len))
    }
}

/// The exit code of a fatal error
#[cold]
#[inline(never)]
pub(crate) fn fatal_exit(e: impl Into<EvmSubsystemError>) -> ExitCode {
    ExitCode::FatalError(e.into())
}

/// Exit code for an error of a system call made by an instruction
#[inline(always)]
pub(crate) fn system_error_exit(e: SystemError) -> ExitCode {
    match e {
        SystemError::LeafRuntime(RuntimeError::OutOfErgs(_)) => {
            ExitCode::EvmError(EvmError::OutOfGas)
        }
        SystemError::LeafRuntime(RuntimeError::FatalRuntimeError(f)) => ExitCode::FatalRuntime(f),
        SystemError::LeafDefect(e) => fatal_exit(e),
    }
}

/// Exit code for an error of the EVM subsystem itself (deployment, address derivation)
#[inline(always)]
pub(crate) fn subsystem_error_exit(e: EvmSubsystemError) -> ExitCode {
    match e.root_cause() {
        RootCause::Runtime(RuntimeError::OutOfErgs(_)) => ExitCode::EvmError(EvmError::OutOfGas),
        RootCause::Runtime(RuntimeError::FatalRuntimeError(f)) => ExitCode::FatalRuntime(f.clone()),
        _ => fatal_exit(e),
    }
}

/// The stack of a frame, as seen by tracers
pub struct StackView<'v> {
    base: *const U256,
    depth: usize,
    _borrow: core::marker::PhantomData<&'v U256>,
}

impl<'v> EvmStackInterface for StackView<'v> {
    fn to_slice(&self) -> &[U256] {
        // SAFETY: the first `depth` slots of the stack buffer are initialized
        unsafe { core::slice::from_raw_parts(self.base, self.depth) }
    }

    fn len(&self) -> usize {
        self.depth
    }

    fn peek_n(&self, index: usize) -> Result<&U256, EvmError> {
        if self.depth < index + 1 {
            return Err(EvmError::StackUnderflow);
        }
        // SAFETY: an initialized slot
        Ok(unsafe { &*self.base.add(self.depth - (index + 1)) })
    }
}

/// Read-only view of the frame for tracers, from either form of the hot state
pub struct FrameView<'v, S: EthereumLikeTypes> {
    instruction_pointer: usize,
    stack: StackView<'v>,
    resources: &'v S::Resources,
    cold: &'v ColdFrameParts<'v, S>,
    #[allow(dead_code)]
    system: &'v System<S>,
}

impl<'v, S: EthereumLikeTypes> FrameView<'v, S> {
    /// View during a run
    #[allow(dead_code)]
    pub(crate) fn from_hot(
        hot: &'v Hot<'_, S>,
        cold: &'v ColdFrameParts<'v, S>,
        system: &'v System<S>,
    ) -> Self {
        Self {
            instruction_pointer: hot.ip.addr().wrapping_sub(cold.bytecode.as_ptr().addr()),
            stack: StackView {
                base: hot.stack.sp.wrapping_sub(hot.stack.depth),
                depth: hot.stack.depth,
                _borrow: core::marker::PhantomData,
            },
            resources: &hot.resources,
            cold,
            system,
        }
    }

    /// View between runs
    #[allow(dead_code)]
    pub(crate) fn from_parts(
        hot: &'v HotFrameParts<S>,
        cold: &'v ColdFrameParts<'v, S>,
        system: &'v System<S>,
    ) -> Self {
        Self {
            instruction_pointer: hot.instruction_pointer,
            stack: StackView {
                base: hot.stack.as_ptr(),
                depth: hot.stack.depth(),
                _borrow: core::marker::PhantomData,
            },
            resources: &hot.resources,
            cold,
            system,
        }
    }
}

impl<'v, S: EthereumLikeTypes> EvmFrameInterface<S> for FrameView<'v, S> {
    fn instruction_pointer(&self) -> usize {
        self.instruction_pointer
    }

    fn resources(&self) -> &<S as SystemTypes>::Resources {
        self.resources
    }

    fn stack(&self) -> &impl EvmStackInterface {
        &self.stack
    }

    fn caller(&self) -> <<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::Address {
        self.cold.caller
    }

    fn address(&self) -> <<S as SystemTypes>::IOTypes as SystemIOTypesConfig>::Address {
        self.cold.address
    }

    fn calldata(&self) -> &[u8] {
        self.cold.calldata
    }

    fn return_data(&self) -> &[u8] {
        self.cold.returndata
    }

    fn heap(&self) -> &[u8] {
        &self.cold.heap
    }

    fn bytecode(&self) -> &[u8] {
        self.cold.bytecode
    }

    fn call_value(&self) -> &U256 {
        &self.cold.call_value
    }

    fn is_static(&self) -> bool {
        self.cold.is_static
    }

    fn is_constructor(&self) -> bool {
        self.cold.is_constructor
    }

    fn refund_counter(&self) -> u32 {
        use zk_ee::system::IOSubsystem;
        let refund = self.system.io.get_refund_counter();
        refund.ergs().as_legacy_gas_ceil() as u32
    }
}
