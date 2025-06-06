use super::*;
pub mod base_system_functions;
pub mod call_modifiers;
pub mod constants;
pub mod errors;
mod execution_environment;
mod io;
pub mod logger;
mod memory;
pub mod metadata;
pub mod resources;
mod result_keeper;

pub use self::base_system_functions::*;
pub use self::call_modifiers::*;
pub use self::constants::*;
pub use self::execution_environment::*;
pub use self::io::*;
pub use self::logger::NullLogger;
pub use self::memory::*;

pub use self::resources::*;
pub use self::result_keeper::*;

pub const MAX_GLOBAL_CALLS_STACK_DEPTH: usize = 1024; // even though we do not have to formally limit it,
                                                      // for all practical purposes (63/64) ^ 1024 is 10^-7, and it's unlikely that one can create any new frame
                                                      // with such remaining resources

use core::alloc::Allocator;
use core::fmt::Write;

use self::{
    errors::{InternalError, SystemError},
    logger::Logger,
    metadata::{BlockMetadataFromOracle, Metadata},
};
use crate::utils::Bytes32;
use crate::{
    execution_environment_type::ExecutionEnvironmentType,
    system_io_oracle::{IOOracle, NewTxContentIterator},
    types_config::{EthereumIOTypesConfig, SystemIOTypesConfig},
    utils::USIZE_SIZE,
};

pub trait SystemTypes {
    /// Handles all side effects and information from the outside world.
    type IO: IOSubsystem<IOTypes = Self::IOTypes, Resources = Self::Resources>;

    type Memory: MemorySubsystem<Allocator = Self::Allocator>;

    /// Common system functions implementation(ecrecover, keccak256, ecadd, etc).
    type SystemFunctions: SystemFunctions<Self::Resources>;

    type Logger: Logger + Default;

    // These are just shorthands. They are completely defined by the above types.
    type IOTypes: SystemIOTypesConfig;
    type Resources: Resources;
    type Allocator: Allocator + Clone + Default;
}
pub trait EthereumLikeTypes: SystemTypes<IOTypes = EthereumIOTypesConfig> {}

pub struct System<S: SystemTypes> {
    pub io: S::IO,
    pub memory: S::Memory,
    metadata: Metadata<S::IOTypes>,
    allocator: S::Allocator,
}

pub struct SystemFrameSnapshot<S: SystemTypes>
where
    S::Memory: MemorySubsystemExt,
{
    io: <S::IO as IOSubsystem>::StateSnapshot,
    memory: <S::Memory as MemorySubsystemExt>::Snapshot,
}

impl<S: SystemTypes> System<S> {
    /// Returns logger for debugging purposes.
    pub fn get_logger(&self) -> S::Logger {
        S::Logger::default()
    }

    pub fn get_allocator(&self) -> S::Allocator {
        self.allocator.clone()
    }

    pub fn get_tx_origin(&self) -> <S::IOTypes as SystemIOTypesConfig>::Address {
        self.metadata.tx_origin
    }

    pub fn get_block_number(&self) -> u64 {
        self.metadata.block_level_metadata.block_number
    }

    pub fn get_blockhash(&self, block_number: u64) -> ruint::aliases::U256 {
        let current_block_number = self.metadata.block_level_metadata.block_number;
        if block_number >= current_block_number
            || block_number < current_block_number.saturating_sub(256)
        {
            // Out of range
            ruint::aliases::U256::ZERO
        } else {
            let index = current_block_number - block_number - 1;
            self.metadata.block_level_metadata.block_hashes.0[index as usize]
        }
    }

    pub fn get_chain_id(&self) -> u64 {
        self.metadata.chain_id
    }

    pub fn get_coinbase(&self) -> ruint::aliases::B160 {
        self.metadata.block_level_metadata.coinbase
    }

    pub fn get_eip1559_basefee(&self) -> ruint::aliases::U256 {
        self.metadata.block_level_metadata.eip1559_basefee
    }

    pub fn get_native_price(&self) -> ruint::aliases::U256 {
        self.metadata.block_level_metadata.native_price
    }

    pub fn get_gas_limit(&self) -> u64 {
        self.metadata.block_level_metadata.gas_limit
    }

    pub fn get_gas_per_pubdata(&self) -> ruint::aliases::U256 {
        self.metadata.block_level_metadata.gas_per_pubdata
    }

    pub fn get_gas_price(&self) -> ruint::aliases::U256 {
        self.metadata.tx_gas_price
    }

    pub fn get_timestamp(&self) -> u64 {
        self.metadata.block_level_metadata.timestamp
    }

    pub fn storage_code_version_for_execution_environment<'a, EE: ExecutionEnvironment<'a, S>>(
        &self,
    ) -> Result<u8, InternalError> {
        // TODO
        Ok(1)
    }

    pub fn set_tx_context(
        &mut self,
        tx_origin: <S::IOTypes as SystemIOTypesConfig>::Address,
        tx_gas_price: ruint::aliases::U256,
    ) {
        self.metadata.tx_origin = tx_origin;
        self.metadata.tx_gas_price = tx_gas_price;
    }

    pub fn net_pubdata_used(&self) -> u64 {
        self.io.net_pubdata_used()
    }
}

impl<S: SystemTypes> System<S>
where
    S::IO: IOSubsystemExt,
    S::Memory: MemorySubsystemExt,
{
    /// Starts a new "global" frame(with separate memory frame).
    /// Returns the snapshot which the system can rollback to on finishing the frame.
    #[track_caller]
    pub fn start_global_frame(&mut self) -> Result<SystemFrameSnapshot<S>, InternalError> {
        let mut logger = self.get_logger();
        let _ = logger.write_fmt(format_args!("Start global frame\n"));
        let io = self.io.start_io_frame()?;
        let memory = self.memory.start_memory_frame();

        Ok(SystemFrameSnapshot { io, memory })
    }

    /// Finishes a global frame, reverts I/O writes in case of revert.
    /// If `rollback_handle` is provided, will revert to the requested snapshot.
    #[track_caller]
    pub fn finish_global_frame(
        &mut self,
        rollback_handle: Option<&SystemFrameSnapshot<S>>,
    ) -> Result<(), InternalError> {
        let mut logger = self.get_logger();
        let _ = logger.write_fmt(format_args!(
            "Finish global frame, revert = {}\n",
            rollback_handle.is_some()
        ));

        // revert IO if needed, and copy memory
        self.io.finish_io_frame(rollback_handle.map(|x| &x.io))?;
        self.memory
            .finish_memory_frame(rollback_handle.map(|x| x.memory.clone()));

        Ok(())
    }

    /// Finishes current transaction executions, returns execution stats.
    pub fn flush_tx(&mut self) -> Result<u32, InternalError> {
        self.io.finish_tx()?;

        self.memory.assert_no_frames_opened();

        Ok(0)
    }

    pub fn init_from_oracle(
        mut oracle: <S::IO as IOSubsystemExt>::IOOracle,
    ) -> Result<Self, InternalError> {
        // get metadata for block
        let block_level_metadata: BlockMetadataFromOracle = oracle.get_block_level_metadata();
        let io = S::IO::init_from_oracle(oracle)?;
        let memory = <S::Memory as MemorySubsystemExt>::new(S::Allocator::default());

        let metadata = Metadata {
            // For now, we're getting the chain id from the block level metadata.
            // in the future, we might want to do a separate call to oracle for that.
            chain_id: block_level_metadata.chain_id,
            tx_origin: Default::default(),
            tx_gas_price: Default::default(),
            block_level_metadata,
        };
        let system = Self {
            io,
            memory,
            metadata,
            allocator: S::Allocator::default(),
        };

        Ok(system)
    }

    pub fn try_begin_next_tx(
        &mut self,
        tx_write_iter: &mut impl crate::oracle::SafeUsizeWritable,
    ) -> Result<Option<usize>, ()> {
        let next_tx_len_bytes = match self.io.oracle().try_begin_next_tx() {
            None => return Ok(None),
            Some(size) => size.get() as usize,
        };
        let next_tx_len_usize_words = next_tx_len_bytes.next_multiple_of(USIZE_SIZE) / USIZE_SIZE;
        if tx_write_iter.len() < next_tx_len_usize_words {
            return Err(());
        }
        let tx_iterator = self
            .io
            .oracle()
            .create_oracle_access_iterator::<NewTxContentIterator>(())
            .expect("must create iterator for the content");
        if tx_iterator.len() != next_tx_len_usize_words {
            return Err(());
        }
        for word in tx_iterator {
            unsafe {
                tx_write_iter.write_usize(word);
            }
        }

        self.io.begin_next_tx();
        self.memory.begin_next_tx();

        Ok(Some(next_tx_len_bytes))
    }

    pub fn deploy_bytecode(
        &mut self,
        for_ee: ExecutionEnvironmentType,
        resources: &mut S::Resources,
        at_address: &<S::IOTypes as SystemIOTypesConfig>::Address,
        bytecode: OSImmutableSlice<S>,
        bytecode_len: u32,
        artifacts_len: u32,
    ) -> Result<OSImmutableSlice<S>, SystemError> {
        // IO is fully responsible to to deploy
        // and at the end we just need to remap slice
        let bytecode = self.io.deploy_code(
            for_ee,
            resources,
            at_address,
            &bytecode,
            bytecode_len,
            artifacts_len,
        )?;
        let bytecode = unsafe {
            self.memory
                .construct_immutable_slice_from_static_slice(bytecode)
        };

        Ok(bytecode)
    }

    /// Finish system execution.
    pub fn finish(
        self,
        block_hash: Bytes32,
        l1_to_l2_txs_hash: Bytes32,
        upgrade_tx_hash: Bytes32,
        result_keeper: &mut impl IOResultKeeper<S::IOTypes>,
    ) -> <S::IO as IOSubsystemExt>::FinalData {
        let logger = self.get_logger();
        self.io.finish(
            self.metadata.block_level_metadata,
            block_hash,
            l1_to_l2_txs_hash,
            upgrade_tx_hash,
            result_keeper,
            logger,
        )
    }
}
