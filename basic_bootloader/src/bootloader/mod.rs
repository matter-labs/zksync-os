use errors::{BootloaderSubsystemError, InvalidTransaction};
use result_keeper::ResultKeeperExt;
use ruint::aliases::*;
use zk_ee::common_structs::system_hooks::HooksStorage;
use zk_ee::common_structs::MAX_NUMBER_OF_LOGS;
use zk_ee::execution_environment_type::ExecutionEnvironmentType;
use zk_ee::system::errors::internal::InternalError;
use zk_ee::system::tracer::Tracer;
use zk_ee::system::{EthereumLikeTypes, IOSubsystemExt, System, SystemTypes};

pub mod block_flow;
pub mod run_single_interaction;
pub mod runner;
pub mod supported_ees;

pub mod transaction;
pub mod transaction_flow;

pub mod block_header;
pub mod config;
pub mod constants;
pub mod errors;
pub mod result_keeper;
mod rlp;
pub mod stf;

use crate::bootloader::block_flow::{
    MetadataInitOp, PostSystemInitOp, PostTxLoopOp, PreTxLoopOp, TxLoopOp,
};
use crate::bootloader::block_header::BlockHeader;
use crate::bootloader::config::BasicBootloaderExecutionConfig;
use crate::bootloader::errors::TxError;
use crate::bootloader::result_keeper::*;
use crate::bootloader::runner::RunnerMemoryBuffers;
use crate::bootloader::stf::EthereumLikeBasicSTF;
use crate::bootloader::transaction_flow::{BasicTransactionFlow, ExecutionOutput, ExecutionResult};
use alloc::boxed::Box;
use core::fmt::Write;

pub const MAX_HEAP_BUFFER_SIZE: usize = 1 << 27; // 128 MB
pub const MAX_RETURN_BUFFER_SIZE: usize = 1 << 28; // 256 MB

pub(crate) const EVM_EE_BYTE: u8 = ExecutionEnvironmentType::EVM_EE_BYTE;
pub const DEBUG_OUTPUT: bool = false;

/// Generic bootloader implementation using composable block execution flow.
///
/// This bootloader uses the State Transition Function (STF) trait to compose
/// different execution phases (metadata init, system init, transaction loop, finalization)
/// into a complete block execution pipeline.
pub struct BasicBootloader<S: EthereumLikeTypes, F: BasicTransactionFlow<S>>
where
    S::IO: IOSubsystemExt,
{
    _marker: core::marker::PhantomData<(S, F)>,
}

impl<S: EthereumLikeBasicSTF, F: BasicTransactionFlow<S>> BasicBootloader<S, F>
where
    S::IO: IOSubsystemExt,
{
    /// Runs the transactions that it loads from the oracle.
    /// This code runs both in sequencer (then it uses ForwardOracle - that stores data in local variables)
    /// and in prover (where oracle uses CRS registers to communicate).
    pub fn run_prepared<Config: BasicBootloaderExecutionConfig>(
        mut oracle: <S::IO as IOSubsystemExt>::IOOracle,
        result_keeper: &mut impl ResultKeeperExt<S::IOTypes, BlockHeader = S::BlockHeader>,
        tracer: &mut impl Tracer<S>,
    ) -> Result<<S::IO as IOSubsystemExt>::FinalData, BootloaderSubsystemError>
    where
        S::IO: IOSubsystemExt,
    {
        cycle_marker::start!("process_block");
        // initialize the system
        cycle_marker::start!("system_init");

        let metadata = <S::MetadataOp as MetadataInitOp<S>>::metadata_op::<Config>(
            &mut oracle,
            S::Allocator::default(),
        )?;

        // we will model initial calldata buffer as just another "heap"
        let mut system: System<S> = System::init_from_metadata_and_oracle(metadata, oracle)?;
        let mut system_functions = HooksStorage::new_in(system.get_allocator());

        <S::PostSystemInitOp as PostSystemInitOp<S>>::post_init_op::<Config>(
            &mut system,
            &mut system_functions,
        )?;

        let mut heaps = Box::new_uninit_slice_in(MAX_HEAP_BUFFER_SIZE, system.get_allocator());
        let mut return_data =
            Box::new_uninit_slice_in(MAX_RETURN_BUFFER_SIZE, system.get_allocator());

        let memories = RunnerMemoryBuffers {
            heaps: &mut heaps,
            return_data: &mut return_data,
        };

        cycle_marker::end!("system_init");

        // Pre-op
        let mut block_data_keeper =
            <S::PreTxLoopOp as PreTxLoopOp<S>>::pre_op(&mut system, result_keeper);

        // TX loop
        <S::TxLoopOp as TxLoopOp<S>>::loop_op::<Config>(
            &mut system,
            &mut system_functions,
            memories,
            &mut block_data_keeper,
            result_keeper,
            tracer,
        )?;

        // whatever the non-persistent data was there, it's now gone

        // Post-op

        let res =
            <S::PostTxLoopOp as PostTxLoopOp<S>>::post_op(system, block_data_keeper, result_keeper);
        cycle_marker::end!("process_block");
        #[allow(clippy::let_and_return)]
        res
    }
}
