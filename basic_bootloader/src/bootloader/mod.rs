use alloc::vec::Vec;
use constants::{MAX_TX_LEN_WORDS, TX_OFFSET_WORDS};
use errors::{BootloaderSubsystemError, InvalidTransaction};
use result_keeper::ResultKeeperExt;
use ruint::aliases::*;
use system_hooks::addresses_constants::BOOTLOADER_FORMAL_ADDRESS;
use zk_ee::common_structs::MAX_NUMBER_OF_LOGS;
use zk_ee::execution_environment_type::ExecutionEnvironmentType;
use zk_ee::memory::slice_vec::SliceVec;
use zk_ee::system::tracer::Tracer;
use zk_ee::system::{EthereumLikeTypes, System, SystemTypes};

pub mod run_single_interaction;
pub mod runner;
pub mod supported_ees;

mod account_models;
mod gas_helpers;
mod paymaster_helper;
mod process_transaction;
pub mod transaction;

pub mod block_header;
pub mod config;
pub mod constants;
pub mod errors;
pub mod result_keeper;
mod rlp;

use alloc::boxed::Box;
use core::alloc::Allocator;
use core::fmt::Write;
use core::mem::MaybeUninit;
use crypto::sha3::Keccak256;
use crypto::MiniDigest;
use zk_ee::{internal_error, oracle::*};

use crate::bootloader::account_models::{ExecutionOutput, ExecutionResult, TxProcessingResult};
use crate::bootloader::block_header::BlockHeader;
use crate::bootloader::config::BasicBootloaderExecutionConfig;
use crate::bootloader::constants::TX_OFFSET;
use crate::bootloader::errors::TxError;
use crate::bootloader::result_keeper::*;
use crate::bootloader::runner::RunnerMemoryBuffers;
use system_hooks::HooksStorage;
use zk_ee::system::*;
use zk_ee::utils::*;

pub(crate) const EVM_EE_BYTE: u8 = ExecutionEnvironmentType::EVM_EE_BYTE;
pub const DEBUG_OUTPUT: bool = false;

pub struct BasicBootloader<S: EthereumLikeTypes> {
    _marker: core::marker::PhantomData<S>,
}

struct TxDataBuffer<A: Allocator> {
    buffer: Vec<u32, A>,
}

impl<A: Allocator> TxDataBuffer<A> {
    fn new(allocator: A) -> Self {
        let mut buffer: Vec<u32, A> =
            Vec::with_capacity_in(TX_OFFSET_WORDS + MAX_TX_LEN_WORDS, allocator);
        buffer.resize(TX_OFFSET_WORDS, 0u32);

        Self { buffer }
    }

    #[allow(clippy::wrong_self_convention)]
    fn into_writable<'a>(&'a mut self) -> TxDataBufferWriter<'a> {
        self.buffer.resize(TX_OFFSET_WORDS, 0u32);
        let capacity = self.buffer.spare_capacity_mut();

        TxDataBufferWriter {
            capacity,
            offset: 0,
        }
    }

    fn as_tx_buffer<'a>(&'a mut self, next_tx_data_len_bytes: usize) -> &'a mut [u8] {
        let word_len = TX_OFFSET_WORDS
            + next_tx_data_len_bytes.next_multiple_of(core::mem::size_of::<u32>())
                / core::mem::size_of::<u32>();
        assert!(self.buffer.capacity() >= word_len);
        unsafe {
            self.buffer.set_len(word_len);
            core::slice::from_raw_parts_mut(
                self.buffer.as_mut_ptr().cast(),
                TX_OFFSET + next_tx_data_len_bytes,
            )
        }
    }
}

struct TxDataBufferWriter<'a> {
    capacity: &'a mut [MaybeUninit<u32>],
    offset: usize,
}

impl<'a> UsizeWriteable for TxDataBufferWriter<'a> {
    unsafe fn write_usize(&mut self, value: usize) {
        #[cfg(target_pointer_width = "32")]
        {
            if self.offset >= self.capacity.len() {
                panic!();
            }
            self.capacity[self.offset].write(value as u32);
            self.offset += 1;
        }

        #[cfg(target_pointer_width = "64")]
        {
            if self.offset + 1 >= self.capacity.len() {
                panic!();
            }
            self.capacity[self.offset].write(value as u32);
            self.capacity[self.offset + 1].write((value >> 32) as u32);
            self.offset += 2;
        }

        #[cfg(not(any(target_pointer_width = "32", target_pointer_width = "64")))]
        {
            compile_error!("unsupported arch")
        }
    }
}

impl<'a> SafeUsizeWritable for TxDataBufferWriter<'a> {
    fn try_write(&mut self, value: usize) -> Result<(), ()> {
        #[cfg(target_pointer_width = "32")]
        {
            if self.offset >= self.capacity.len() {
                return Err(());
            }
            self.capacity[self.offset].write(value as u32);
            self.offset += 1;

            Ok(())
        }

        #[cfg(target_pointer_width = "64")]
        {
            if self.offset + 1 >= self.capacity.len() {
                return Err(());
            }
            self.capacity[self.offset].write(value as u32);
            self.capacity[self.offset + 1].write((value >> 32) as u32);
            self.offset += 2;

            Ok(())
        }
    }

    fn len(&self) -> usize {
        if core::mem::size_of::<usize>() == core::mem::size_of::<u32>() {
            self.capacity.len()
        } else if core::mem::size_of::<usize>() == core::mem::size_of::<u64>() {
            self.capacity.len() / 2
        } else {
            unreachable!()
        }
    }
}

impl<S: EthereumLikeTypes> BasicBootloader<S> {
    /// Runs the transactions that it loads from the oracle.
    /// This code runs both in sequencer (then it uses ForwardOracle - that stores data in local variables)
    /// and in prover (where oracle uses CRS registers to communicate).
    pub fn run_prepared<Config: BasicBootloaderExecutionConfig>(
        oracle: <S::IO as IOSubsystemExt>::IOOracle,
        result_keeper: &mut impl ResultKeeperExt,
        tracer: &mut impl Tracer<S>,
    ) -> Result<<S::IO as IOSubsystemExt>::FinalData, BootloaderSubsystemError>
    where
        S::IO: IOSubsystemExt,
    {
        cycle_marker::start!("run_prepared");
        // we will model initial calldata buffer as just another "heap"
        let mut system: System<S> =
            System::init_from_oracle(oracle).expect("system must be able to initialize itself");

        let mut initial_calldata_buffer = TxDataBuffer::new(system.get_allocator());

        pub const MAX_HEAP_BUFFER_SIZE: usize = 1 << 27; // 128 MB
        pub const MAX_RETURN_BUFFER_SIZE: usize = 1 << 28; // 256 MB

        let mut heaps = Box::new_uninit_slice_in(MAX_HEAP_BUFFER_SIZE, system.get_allocator());
        let mut return_data =
            Box::new_uninit_slice_in(MAX_RETURN_BUFFER_SIZE, system.get_allocator());

        let mut memories = RunnerMemoryBuffers {
            heaps: &mut heaps,
            return_data: &mut return_data,
        };

        let mut system_functions = HooksStorage::new_in(system.get_allocator());

        system_functions.add_precompiles();

        #[cfg(not(feature = "disable_system_contracts"))]
        {
            system_functions.add_l1_messenger();
            system_functions.add_l2_base_token();
            system_functions.add_contract_deployer();
        }

        let mut tx_rolling_hash = [0u8; 32];
        let mut l1_to_l2_txs_hasher = crypto::blake2s::Blake2s256::new();

        let mut first_tx = true;
        let mut upgrade_tx_hash = Bytes32::zero();
        let mut block_gas_used = 0;
        let mut block_computational_native_used = 0;
        let mut block_pubdata_used = 0;

        // now we can run every transaction
        while let Some(next_tx_data_len_bytes) = {
            let mut writable = initial_calldata_buffer.into_writable();
            system
                .try_begin_next_tx(&mut writable)
                .expect("TX start call must always succeed")
        } {
            let mut inf_resources = S::Resources::FORMAL_INFINITE;
            system
                .io
                .read_account_properties(
                    ExecutionEnvironmentType::NoEE,
                    &mut inf_resources,
                    &system.get_coinbase(),
                    AccountDataRequest::empty(),
                )
                .expect("must heat coinbase");

            let mut logger: <S as SystemTypes>::Logger = system.get_logger();
            let _ = logger.write_fmt(format_args!("====================================\n"));
            let _ = logger.write_fmt(format_args!("TX execution begins\n"));

            let initial_calldata_buffer =
                initial_calldata_buffer.as_tx_buffer(next_tx_data_len_bytes);

            tracer.begin_tx(initial_calldata_buffer);

            // Take a snapshot in case we need to invalidate the
            // transaction to seal the block.
            // This can happen if any of the block limits (native, gas, pubdata
            // logs) is reached by the current transaction.
            let pre_tx_rollback_handle = system.start_global_frame()?;

            // We will give the full buffer here, and internally we will use parts of it to give forward to EEs
            cycle_marker::start!("process_transaction");

            let tx_result = Self::process_transaction::<Config>(
                initial_calldata_buffer,
                &mut system,
                &mut system_functions,
                memories.reborrow(),
                first_tx,
                tracer,
            );

            cycle_marker::end!("process_transaction");

            tracer.finish_tx();

            match tx_result {
                Err(TxError::Internal(err)) => {
                    let _ = system.get_logger().write_fmt(format_args!(
                        "Tx execution result: Internal error = {err:?}\n",
                    ));
                    // Finish the frame opened before processing the tx
                    system.finish_global_frame(None)?;
                    return Err(err);
                }
                Err(TxError::Validation(err)) => {
                    let _ = system.get_logger().write_fmt(format_args!(
                        "Tx execution result: Validation error = {err:?}\n",
                    ));
                    // Finish the frame opened before processing the tx
                    system.finish_global_frame(None)?;
                    result_keeper.tx_processed(Err(err));
                }
                Ok(tx_processing_result) => {
                    // TODO: debug implementation for ruint types uses global alloc, which panics in ZKsync OS
                    #[cfg(not(target_arch = "riscv32"))]
                    let _ = system.get_logger().write_fmt(format_args!(
                        "Tx execution result = {:?}\n",
                        &tx_processing_result,
                    ));
                    block_gas_used += tx_processing_result.gas_used;
                    block_computational_native_used +=
                        tx_processing_result.computational_native_used;
                    block_pubdata_used += tx_processing_result.pubdata_used;
                    let block_logs_used = system.io.logs_len();

                    // Check if the transaction made the block reach any of the limits
                    // for gas, native, pubdata or logs.
                    if let Err(err) = Self::check_for_block_limits(
                        &mut system,
                        block_gas_used,
                        block_computational_native_used,
                        block_pubdata_used,
                        block_logs_used,
                    ) {
                        // Revert to state before transaction
                        system.finish_global_frame(Some(&pre_tx_rollback_handle))?;
                        result_keeper.tx_processed(Err(err));
                    } else {
                        // Finish the frame opened before processing the tx
                        system.finish_global_frame(None)?;

                        let (status, output, contract_address) = match tx_processing_result.result {
                            ExecutionResult::Success { output } => match output {
                                ExecutionOutput::Call(output) => (true, output, None),
                                ExecutionOutput::Create(output, contract_address) => {
                                    (true, output, Some(contract_address))
                                }
                            },
                            ExecutionResult::Revert { output } => (false, output, None),
                        };
                        result_keeper.tx_processed(Ok(TxProcessingOutput {
                            status,
                            output: &output,
                            contract_address,
                            gas_used: tx_processing_result.gas_used,
                            gas_refunded: tx_processing_result.gas_refunded,
                            computational_native_used: tx_processing_result
                                .computational_native_used,
                            pubdata_used: tx_processing_result.pubdata_used,
                        }));

                        let mut keccak = Keccak256::new();
                        keccak.update(tx_rolling_hash);
                        keccak.update(tx_processing_result.tx_hash.as_u8_ref());
                        tx_rolling_hash = keccak.finalize();

                        if tx_processing_result.is_l1_tx {
                            l1_to_l2_txs_hasher.update(tx_processing_result.tx_hash.as_u8_ref());
                        }

                        if tx_processing_result.is_upgrade_tx {
                            upgrade_tx_hash = tx_processing_result.tx_hash;
                        }
                    }
                }
            }

            first_tx = false;

            // The fee is transferred to the coinbase address before
            // finishing the transaction.
            let coinbase = system.get_coinbase();
            let mut inf_resources = S::Resources::FORMAL_INFINITE;
            let bootloader_balance = system
                .io
                .read_account_properties(
                    ExecutionEnvironmentType::NoEE,
                    &mut inf_resources,
                    &BOOTLOADER_FORMAL_ADDRESS,
                    AccountDataRequest::empty().with_nominal_token_balance(),
                )
                .expect("must read bootloader balance")
                .nominal_token_balance
                .0;
            if !bootloader_balance.is_zero() {
                system
                    .io
                    .transfer_nominal_token_value(
                        ExecutionEnvironmentType::NoEE,
                        &mut inf_resources,
                        &BOOTLOADER_FORMAL_ADDRESS,
                        &coinbase,
                        &bootloader_balance,
                    )
                    .expect("must be able to move funds to coinbase");
            }

            let tx_stats = system.flush_tx();
            let _ = system
                .get_logger()
                .write_fmt(format_args!("Tx stats = {tx_stats:?}\n"));

            let mut logger = system.get_logger();
            let _ = logger.write_fmt(format_args!("TX execution ends\n"));
            let _ = logger.write_fmt(format_args!("====================================\n"));
        }

        let block_number = system.get_block_number();

        let previous_block_hash = if block_number == 0 {
            ruint::aliases::U256::ZERO
        } else {
            system.get_blockhash(block_number - 1)
        };
        let beneficiary = system.get_coinbase();
        let gas_limit = system.get_gas_limit();
        let timestamp = system.get_timestamp();
        let consensus_random = Bytes32::from_u256_be(&system.get_mix_hash());
        let base_fee_per_gas = system.get_eip1559_basefee();
        // TODO: add gas_per_pubdata and native price
        let base_fee_per_gas = base_fee_per_gas
            .try_into()
            .map_err(|_| internal_error!("base_fee_per_gas exceeds max u64"))?;
        let block_header = BlockHeader::new(
            Bytes32::from(previous_block_hash.to_be_bytes::<32>()),
            beneficiary,
            tx_rolling_hash.into(),
            block_number,
            gas_limit,
            block_gas_used,
            timestamp,
            consensus_random,
            base_fee_per_gas,
        );
        let block_hash = Bytes32::from(block_header.hash());
        result_keeper.block_sealed(block_header);

        let l1_to_l2_tx_hash = Bytes32::from(l1_to_l2_txs_hasher.finalize());

        #[cfg(not(target_arch = "riscv32"))]
        cycle_marker::log_marker(
            format!(
                "Spent ergs for [run_prepared]: {}",
                result_keeper.get_gas_used() * evm_interpreter::ERGS_PER_GAS
            )
            .as_str(),
        );

        let _ = system
            .get_logger()
            .write_fmt(format_args!("Bootloader completed\n"));

        let mut logger = system.get_logger();
        let _ = logger.write_fmt(format_args!(
            "Bootloader execution is complete, will proceed with applying changes\n"
        ));

        let r = system.finish(block_hash, l1_to_l2_tx_hash, upgrade_tx_hash, result_keeper);
        cycle_marker::end!("run_prepared");
        #[allow(clippy::let_and_return)]
        Ok(r)
    }

    /// Check if the transaction made the block reach any of the limits
    /// for gas, native, pubdata or logs.
    /// If one such limit is reached, return the corresponding validation
    /// error.
    fn check_for_block_limits(
        system: &mut System<S>,
        gas_used: u64,
        computational_native_used: u64,
        pubdata_used: u64,
        logs_used: u64,
    ) -> Result<(), InvalidTransaction> {
        if cfg!(feature = "resources_for_tester") {
            // EVM tester uses some really high gas limits,
            // so we don't limit the block's native resource.
            Ok(())
        } else {
            let mut logger = system.get_logger();

            if gas_used > system.get_gas_limit() {
                let _ = logger.write_fmt(format_args!(
                    "Block gas limit reached, invalidating transaction\n"
                ));
                Err(InvalidTransaction::BlockGasLimitReached)
            } else if computational_native_used > MAX_NATIVE_COMPUTATIONAL {
                let _ = logger.write_fmt(format_args!(
                    "Block native limit reached, invalidating transaction\n"
                ));
                Err(InvalidTransaction::BlockNativeLimitReached)
            } else if pubdata_used > system.get_pubdata_limit() {
                let _ = logger.write_fmt(format_args!(
                    "Block pubdata limit reached, invalidating transaction\n"
                ));
                Err(InvalidTransaction::BlockPubdataLimitReached)
            } else if logs_used > MAX_NUMBER_OF_LOGS {
                let _ = logger.write_fmt(format_args!(
                    "Block logs limit reached, invalidating transaction\n"
                ));
                Err(InvalidTransaction::BlockL2ToL1LogsLimitReached)
            } else {
                Ok(())
            }
        }
    }
}
