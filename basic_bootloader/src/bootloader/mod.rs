use alloc::boxed::Box;
use alloc::vec::Vec;
use constants::{MAX_TX_LEN_WORDS, TX_OFFSET_WORDS};
use result_keeper::ResultKeeperExt;
use ruint::aliases::*;
use system_hooks::addresses_constants::{
    BOOTLOADER_FORMAL_ADDRESS, L2_INTEROP_ROOT_STORAGE_ADDRESS,
};
use zk_ee::execution_environment_type::ExecutionEnvironmentType;
use zk_ee::memory::slice_vec::SliceVec;
use zk_ee::system::errors::InternalError;
use zk_ee::system::{EthereumLikeTypes, System, SystemTypes};

pub mod run_single_interaction;
mod runner;
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

use core::alloc::Allocator;
use core::fmt::Write;
use core::mem::MaybeUninit;
use crypto::sha3::Keccak256;
use crypto::MiniDigest;
use zk_ee::oracle::*;

use crate::bootloader::account_models::{ExecutionOutput, ExecutionResult, TxProcessingResult};
use crate::bootloader::block_header::BlockHeader;
use crate::bootloader::config::BasicBootloaderExecutionConfig;
use crate::bootloader::constants::{MAX_CALLSTACK_DEPTH, TX_OFFSET};
use crate::bootloader::errors::TxError;
use crate::bootloader::result_keeper::*;
use system_hooks::HooksStorage;
use zk_ee::system::*;
use zk_ee::utils::*;

pub(crate) const EVM_EE_BYTE: u8 = ExecutionEnvironmentType::EVM_EE_BYTE;
#[allow(dead_code)]
pub(crate) const ERA_VM_EE_BYTE: u8 = ExecutionEnvironmentType::ERA_VM_EE_BYTE;
#[allow(dead_code)]
pub(crate) const IWASM_EE_BYTE: u8 = ExecutionEnvironmentType::IWASM_EE_BYTE;
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
    ) -> Result<<S::IO as IOSubsystemExt>::FinalData, InternalError>
    where
        S::IO: IOSubsystemExt,
        S::Memory: MemorySubsystemExt,
    {
        cycle_marker::start!("run_prepared");
        // we will model initial calldata buffer as just another "heap"
        let mut system: System<S> =
            System::init_from_oracle(oracle).expect("system must be able to initialize itself");

        let mut initial_calldata_buffer = TxDataBuffer::new(system.get_allocator());

        // TODO: extend stack trait to construct it or use a provided function to generate it

        let mut callstack_memory =
            Box::new_uninit_slice_in(MAX_CALLSTACK_DEPTH, system.get_allocator());
        let mut callstack = SliceVec::new(&mut callstack_memory);
        let mut system_functions = HooksStorage::new_in(system.get_allocator());

        system_functions.add_precompiles();
        system_functions.add_l1_messenger();
        system_functions.add_l2_base_token();
        system_functions.add_contract_deployer();

        let bytecode = [
            96, 128, 96, 64, 82, 52, 128, 21, 97, 0, 15, 87, 95, 128, 253, 91, 80, 96, 4, 54, 16,
            97, 0, 52, 87, 95, 53, 96, 224, 28, 128, 99, 119, 207, 209, 113, 20, 97, 0, 56, 87,
            128, 99, 251, 98, 0, 198, 20, 97, 0, 113, 87, 91, 95, 128, 253, 91, 97, 0, 95, 97, 0,
            70, 54, 96, 4, 97, 1, 81, 86, 91, 95, 96, 32, 129, 129, 82, 146, 129, 82, 96, 64, 128,
            130, 32, 144, 147, 82, 144, 129, 82, 32, 84, 129, 86, 91, 96, 64, 81, 144, 129, 82, 96,
            32, 1, 96, 64, 81, 128, 145, 3, 144, 243, 91, 97, 0, 132, 97, 0, 127, 54, 96, 4, 97, 1,
            113, 86, 91, 97, 0, 134, 86, 91, 0, 91, 96, 1, 129, 20, 97, 0, 167, 87, 96, 64, 81, 99,
            47, 89, 189, 13, 96, 224, 27, 129, 82, 96, 4, 1, 96, 64, 81, 128, 145, 3, 144, 253, 91,
            95, 132, 129, 82, 96, 32, 129, 129, 82, 96, 64, 128, 131, 32, 134, 132, 82, 144, 145,
            82, 144, 32, 84, 21, 97, 0, 220, 87, 96, 64, 81, 99, 45, 72, 232, 207, 96, 224, 27,
            129, 82, 96, 4, 1, 96, 64, 81, 128, 145, 3, 144, 253, 91, 129, 129, 95, 129, 129, 16,
            97, 0, 238, 87, 97, 0, 238, 97, 1, 237, 86, 91, 95, 135, 129, 82, 96, 32, 129, 129, 82,
            96, 64, 128, 131, 32, 137, 132, 82, 130, 82, 145, 130, 144, 32, 146, 2, 147, 144, 147,
            1, 53, 144, 85, 80, 81, 131, 144, 133, 144, 127, 107, 69, 27, 132, 34, 99, 110, 69,
            185, 59, 247, 245, 148, 250, 44, 23, 105, 208, 57, 118, 108, 66, 84, 166, 231, 249,
            192, 238, 23, 21, 205, 176, 144, 97, 1, 67, 144, 134, 144, 134, 144, 97, 2, 1, 86, 91,
            96, 64, 81, 128, 145, 3, 144, 163, 80, 80, 80, 80, 86, 91, 95, 128, 96, 64, 131, 133,
            3, 18, 21, 97, 1, 98, 87, 95, 128, 253, 91, 80, 80, 128, 53, 146, 96, 32, 144, 145, 1,
            53, 145, 80, 86, 91, 95, 128, 95, 128, 96, 96, 133, 135, 3, 18, 21, 97, 1, 132, 87, 95,
            128, 253, 91, 132, 53, 147, 80, 96, 32, 133, 1, 53, 146, 80, 96, 64, 133, 1, 53, 103,
            255, 255, 255, 255, 255, 255, 255, 255, 128, 130, 17, 21, 97, 1, 169, 87, 95, 128, 253,
            91, 129, 135, 1, 145, 80, 135, 96, 31, 131, 1, 18, 97, 1, 188, 87, 95, 128, 253, 91,
            129, 53, 129, 129, 17, 21, 97, 1, 202, 87, 95, 128, 253, 91, 136, 96, 32, 130, 96, 5,
            27, 133, 1, 1, 17, 21, 97, 1, 222, 87, 95, 128, 253, 91, 149, 152, 148, 151, 80, 80,
            96, 32, 1, 148, 80, 80, 80, 86, 91, 99, 78, 72, 123, 113, 96, 224, 27, 95, 82, 96, 50,
            96, 4, 82, 96, 36, 95, 253, 91, 96, 32, 128, 130, 82, 129, 1, 130, 144, 82, 95, 96, 1,
            96, 1, 96, 251, 27, 3, 131, 17, 21, 97, 2, 31, 87, 95, 128, 253, 91, 130, 96, 5, 27,
            128, 133, 96, 64, 133, 1, 55, 145, 144, 145, 1, 96, 64, 1, 147, 146, 80, 80, 80, 86,
            254, 162, 100, 105, 112, 102, 115, 88, 34, 18, 32, 89, 183, 57, 111, 109, 60, 111, 239,
            133, 50, 55, 33, 99, 119, 146, 24, 116, 214, 190, 157, 238, 100, 224, 84, 241, 3, 232,
            119, 102, 194, 104, 77, 100, 115, 111, 108, 99, 67, 0, 8, 24, 0, 51,
        ];

        let _ = system.io.deploy_code(
            ExecutionEnvironmentType::EVM,
            &mut S::Resources::FORMAL_INFINITE,
            &L2_INTEROP_ROOT_STORAGE_ADDRESS,
            &bytecode,
            bytecode.len() as u32,
            0,
        );

        let mut tx_rolling_hash = [0u8; 32];
        let mut l1_to_l2_txs_hasher = crypto::blake2s::Blake2s256::new();

        let mut first_tx = true;
        let mut upgrade_tx_hash = Bytes32::zero();
        let mut interop_root_hasher = crypto::sha3::Keccak256::new();

        // Block of code needed for interop.
        // We need to add interop roots to the interop root storage.
        // We do it by calling the addInteropRoot function.
        // The function is defined in the InteropRootStorage contract.
        // The function is called with the chainId, blockOrBatchNumber, and the sides.
        // The sides are the interop roots.
        // The chainId is the chainId of the interop root.
        // The blockOrBatchNumber is the block number of the interop root.
        //
        // We also compute the rolling hash of the interop roots and include it as part of the public input
        system
            .get_interop_roots()
            .iter()
            .filter(|interop_root| interop_root.chain_id != 0 && interop_root.block_number != 0)
            .for_each(|interop_root| {
                let mut data = [0u8; 160];
                data[24..32].copy_from_slice(&interop_root.chain_id.to_be_bytes());
                data[56..64].copy_from_slice(&interop_root.block_number.to_be_bytes());
                data[92..96].copy_from_slice(&32u32.to_be_bytes());
                data[124..128].copy_from_slice(&0u32.to_be_bytes());
                data[128..160].copy_from_slice(&interop_root.root[0].as_u8_ref());
                interop_root_hasher.update(&data);

                data[92..96].copy_from_slice(&96u32.to_be_bytes());

                // fb6200c6: function addInteropRoot(uint256 chainId, uint256 blockOrBatchNumber, bytes32[] calldata sides) external;
                let mut calldata = [0xfb, 0x62, 0x00, 0xc6].to_vec();
                calldata.extend(&data);

                let calldata = unsafe {
                    system.memory.construct_immutable_slice_from_static_slice(
                        core::mem::transmute::<&[u8], &[u8]>(&calldata),
                    )
                };

                let _ = Self::run_single_interaction(
                    &mut system,
                    &mut system_functions,
                    &mut callstack,
                    &calldata,
                    &BOOTLOADER_FORMAL_ADDRESS,
                    &L2_INTEROP_ROOT_STORAGE_ADDRESS,
                    S::Resources::FORMAL_INFINITE,
                    &U256::ZERO,
                    true,
                );
            });

        let interop_root_hash = Bytes32::from(interop_root_hasher.finalize());

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

            // We will give the full buffer here, and internally we will use parts of it to give forward to EEs
            cycle_marker::start!("process_transaction");
            let tx_result = Self::process_transaction::<Config>(
                initial_calldata_buffer,
                &mut system,
                &mut system_functions,
                &mut callstack,
                first_tx,
            );
            cycle_marker::end!("process_transaction");

            match tx_result {
                Err(TxError::Internal(err)) => {
                    let _ = system.get_logger().write_fmt(format_args!(
                        "Tx execution result: Internal error = {:?}\n",
                        err,
                    ));
                    return Err(err);
                }
                Err(TxError::Validation(err)) => {
                    let _ = system.get_logger().write_fmt(format_args!(
                        "Tx execution result: Validation error = {:?}\n",
                        err,
                    ));
                    result_keeper.tx_processed(Err(err));
                }
                Ok(tx_processing_result) => {
                    // TODO: debug implementation for ruint types uses global alloc, which panics in ZKsync OS
                    #[cfg(not(target_arch = "riscv32"))]
                    let _ = system.get_logger().write_fmt(format_args!(
                        "Tx execution result = {:?}\n",
                        &tx_processing_result,
                    ));
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

            let tx_stats = system.flush_tx();
            let _ = system
                .get_logger()
                .write_fmt(format_args!("Tx stats = {:?}\n", tx_stats));

            first_tx = false;

            let mut logger = system.get_logger();
            let _ = logger.write_fmt(format_args!("TX execution ends\n"));
            let _ = logger.write_fmt(format_args!("====================================\n"));
        }

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

        let block_number = system.get_block_number();
        let previous_block_hash = system.get_blockhash(block_number);
        let beneficiary = system.get_coinbase();
        // TODO: Gas limit should be constant
        let gas_limit = system.get_gas_limit();
        // TODO: gas used shouldn't be zero
        let gas_used = 0;
        let timestamp = system.get_timestamp();
        // after consensus should be provided in the block metadata
        let consensus_random = Bytes32::zero();
        let base_fee_per_gas = system.get_eip1559_basefee();
        // TODO: add gas_per_pubdata and native price
        let block_header = BlockHeader::new(
            Bytes32::from(previous_block_hash.to_be_bytes::<32>()),
            beneficiary,
            tx_rolling_hash.into(),
            block_number,
            gas_limit,
            gas_used,
            timestamp,
            consensus_random,
            base_fee_per_gas.try_into().unwrap(),
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

        let r = system.finish(
            block_hash,
            l1_to_l2_tx_hash,
            upgrade_tx_hash,
            interop_root_hash,
            result_keeper,
        );
        cycle_marker::end!("run_prepared");
        #[allow(clippy::let_and_return)]
        Ok(r)
    }
}
