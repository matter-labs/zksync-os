use errors::subsystem::Subsystem;

use super::*;
pub mod base_system_functions;
pub mod call_modifiers;
pub mod constants;
pub mod errors;
mod execution_environment;
mod io;
pub mod logger;
pub mod metadata;
pub mod resources;
mod result_keeper;
pub mod tracer;

pub use self::base_system_functions::*;
pub use self::call_modifiers::*;
pub use self::constants::*;
pub use self::execution_environment::*;
pub use self::io::*;
pub use self::logger::NullLogger;

pub use self::resources::*;
pub use self::result_keeper::*;

pub const MAX_GLOBAL_CALLS_STACK_DEPTH: usize = 1024; // even though we do not have to formally limit it,
                                                      // for all practical purposes (63/64) ^ 1024 is 10^-7, and it's unlikely that one can create any new frame
                                                      // with such remaining resources

use core::alloc::Allocator;
use core::fmt::Write;

use self::{
    errors::{internal::InternalError, system::SystemError},
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

    /// Common system functions implementation(ecrecover, keccak256, ecadd, etc).
    type SystemFunctions: SystemFunctions<Self::Resources>;
    type SystemFunctionsExt: SystemFunctionsExt<Self::Resources>;

    type Logger: Logger + Default;

    // These are just shorthands. They are completely defined by the above types.
    type IOTypes: SystemIOTypesConfig;
    type Resources: Resources + Default;
    type Allocator: Allocator + Clone + Default;
}
pub trait EthereumLikeTypes: SystemTypes<IOTypes = EthereumIOTypesConfig> {}

pub struct System<S: SystemTypes> {
    pub io: S::IO,
    metadata: Metadata<S::IOTypes>,
    allocator: S::Allocator,
}

pub struct SystemFrameSnapshot<S: SystemTypes> {
    io: <S::IO as IOSubsystem>::StateSnapshot,
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

    pub fn get_mix_hash(&self) -> ruint::aliases::U256 {
        #[cfg(feature = "prevrandao")]
        {
            self.metadata.block_level_metadata.mix_hash
        }

        #[cfg(not(feature = "prevrandao"))]
        {
            ruint::aliases::U256::ONE
        }
    }

    pub fn get_blockhash(&self, block_number: u64) -> ruint::aliases::U256 {
        let current_block_number = self.metadata.block_level_metadata.block_number;
        if block_number >= current_block_number
            || block_number < current_block_number.saturating_sub(256)
        {
            // Out of range
            ruint::aliases::U256::ZERO
        } else {
            let index = 256 - (current_block_number - block_number);
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

    pub fn get_pubdata_limit(&self) -> u64 {
        self.metadata.block_level_metadata.pubdata_limit
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

    pub fn storage_code_version_for_execution_environment<
        'a,
        Es: Subsystem,
        EE: ExecutionEnvironment<'a, S, Es>,
    >(
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

    pub fn net_pubdata_used(&self) -> Result<u64, InternalError> {
        self.io.net_pubdata_used()
    }
}

impl<S: SystemTypes> System<S>
where
    S::IO: IOSubsystemExt,
{
    /// Starts a new "global" frame(with separate memory frame).
    /// Returns the snapshot which the system can rollback to on finishing the frame.
    #[track_caller]
    pub fn start_global_frame(&mut self) -> Result<SystemFrameSnapshot<S>, InternalError> {
        let mut logger = self.get_logger();
        let _ = logger.write_fmt(format_args!("Start global frame\n"));
        let io = self.io.start_io_frame()?;

        Ok(SystemFrameSnapshot { io })
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

        Ok(())
    }

    /// Finishes current transaction execution
    pub fn flush_tx(&mut self) -> Result<(), InternalError> {
        self.io.finish_tx()
    }

    pub fn init_from_oracle(
        mut oracle: <S::IO as IOSubsystemExt>::IOOracle,
    ) -> Result<Self, InternalError> {
        // get metadata for block
        let block_level_metadata: BlockMetadataFromOracle = oracle.get_block_level_metadata();
        let io = S::IO::init_from_oracle(oracle)?;

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
            metadata,
            allocator: S::Allocator::default(),
        };

        Ok(system)
    }

    ///
    /// Get the length of the next transaction from the oracle.
    /// Returns None when there are no more transactions to process.
    /// Returns Some(Err(_)) if there's an encoding error.
    ///
    pub fn try_begin_next_tx(
        &mut self,
        tx_write_iter: &mut impl crate::oracle::SafeUsizeWritable,
    ) -> Option<Result<usize, NextTxSubsystemError>> {
        let next_tx_len_bytes = match self.io.oracle().try_begin_next_tx() {
            None => return None,
            Some(size) => size.get() as usize,
        };
        // Check to avoid usize overflow in 32-bit target.
        // The maximum allowed length is u32::MAX - 3, as it is
        // the last multiple of 4 (u32 byte size). Any value larger than that
        // will overflow u32 in the next_multiple_of(USIZE_SIZE) call.
        if next_tx_len_bytes > u32::MAX as usize - (core::mem::size_of::<u32>() - 1) {
            return Some(Err(interface_error!(
                crate::system::NextTxInterfaceError::TxLengthTooLarge
            )));
        }
        let next_tx_len_usize_words = next_tx_len_bytes.next_multiple_of(USIZE_SIZE) / USIZE_SIZE;
        if tx_write_iter.len() < next_tx_len_usize_words {
            return Some(Err(interface_error!(
                crate::system::NextTxInterfaceError::TxWriteIteratorTooSmall
            )));
        }
        let tx_iterator = self
            .io
            .oracle()
            .create_oracle_access_iterator::<NewTxContentIterator>(())
            .expect("must create iterator for the content");
        if tx_iterator.len() != next_tx_len_usize_words {
            return Some(Err(interface_error!(
                crate::system::NextTxInterfaceError::TxIteratorLengthMismatch
            )));
        }
        for word in tx_iterator {
            unsafe {
                tx_write_iter.write_usize(word);
            }
        }

        self.io.begin_next_tx();

        Some(Ok(next_tx_len_bytes))
    }

    pub fn deploy_bytecode(
        &mut self,
        for_ee: ExecutionEnvironmentType,
        resources: &mut S::Resources,
        at_address: &<S::IOTypes as SystemIOTypesConfig>::Address,
        bytecode: &[u8],
    ) -> Result<
        (
            &'static [u8],
            <S::IOTypes as SystemIOTypesConfig>::BytecodeHashValue,
            u32,
        ),
        SystemError,
    > {
        // IO is fully responsible to to deploy
        // and at the end we just need to remap slice
        let (bytecode, bytecode_hash, observable_bytecode_len) = self
            .io
            .deploy_code(for_ee, resources, at_address, &bytecode)?;

        Ok((bytecode, bytecode_hash, observable_bytecode_len))
    }

    pub fn set_bytecode_details(
        &mut self,
        resources: &mut S::Resources,
        at_address: &<S::IOTypes as SystemIOTypesConfig>::Address,
        ee: ExecutionEnvironmentType,
        bytecode_hash: Bytes32,
        bytecode_len: u32,
        artifacts_len: u32,
        observable_bytecode_hash: Bytes32,
        observable_bytecode_len: u32,
    ) -> Result<(), SystemError> {
        self.io.set_bytecode_details(
            resources,
            at_address,
            ee,
            bytecode_hash,
            bytecode_len,
            artifacts_len,
            observable_bytecode_hash,
            observable_bytecode_len,
        )
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

define_subsystem!(NextTx,
  interface NextTxInterfaceError {
    TxLengthTooLarge,
    TxWriteIteratorTooSmall,
    TxIteratorLengthMismatch,
  }
);
