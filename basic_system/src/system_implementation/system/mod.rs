//! Implementation of the system interface.
use crate::system_implementation::caches::storage_access_policy::StorageAccessPolicy;
use crate::system_implementation::flat_storage_model::*;
use core::alloc::Allocator;
use errors::system::SystemError;
use evm_interpreter::gas_constants::COLD_SLOAD_COST;
use evm_interpreter::gas_constants::SSTORE_RESET_EXTRA;
use evm_interpreter::gas_constants::SSTORE_SET_EXTRA;
use evm_interpreter::gas_constants::WARM_STORAGE_READ_COST;
use ruint::aliases::U256;
use zk_ee::common_structs::history_map::CacheSnapshotId;
use zk_ee::common_structs::WarmStorageKey;
use zk_ee::execution_environment_type::ExecutionEnvironmentType;
use zk_ee::utils::Bytes32;
use zk_ee::{
    memory::stack_trait::StackFactory,
    oracle::IOOracle,
    system::{errors::internal::InternalError, logger::Logger, Resources, MAX_EVENT_TOPICS, *},
};

pub mod interop_roots;
mod io_subsystem;

pub use self::io_subsystem::*;

#[derive(Clone, Copy, Debug, Default)]
pub struct EthereumLikeStorageAccessCostModel;

impl<R: Resources> StorageAccessPolicy<R, Bytes32> for EthereumLikeStorageAccessCostModel {
    fn charge_warm_storage_read(
        &self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut R,
    ) -> Result<(), SystemError> {
        let gas = match ee_type {
            ExecutionEnvironmentType::NoEE => 0,
            ExecutionEnvironmentType::EVM => WARM_STORAGE_READ_COST,
        };
        resources.charge_legacy_gas_and_native(
            gas,
            crate::system_implementation::flat_storage_model::cost_constants::WARM_STORAGE_READ_NATIVE_COST,
        )
    }

    fn charge_cold_storage_read_extra(
        &self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut R,
        is_new_slot: bool,
        is_warm_access: bool,
    ) -> Result<(), SystemError> {
        let gas = match ee_type {
            ExecutionEnvironmentType::NoEE => 0,
            ExecutionEnvironmentType::EVM => {
                if is_warm_access {
                    0
                } else {
                    COLD_SLOAD_COST - WARM_STORAGE_READ_COST
                }
            }
        };
        let native = if is_new_slot {
            crate::system_implementation::flat_storage_model::cost_constants::COLD_NEW_STORAGE_READ_NATIVE_COST
        } else {
            crate::system_implementation::flat_storage_model::cost_constants::COLD_EXISTING_STORAGE_READ_NATIVE_COST
        };
        resources.charge_legacy_gas_and_native(gas, native)
    }

    fn charge_storage_write_extra(
        &self,
        ee_type: ExecutionEnvironmentType,
        initial_value: &Bytes32,
        current_value: &Bytes32,
        new_value: &Bytes32,
        resources: &mut R,
        is_warm_access: bool,
        is_cold_write_charged: bool,
        is_new_slot: bool,
    ) -> Result<(), SystemError> {
        let gas = match ee_type {
            ExecutionEnvironmentType::NoEE => 0,
            ExecutionEnvironmentType::EVM => {
                let total_cost = if new_value == current_value {
                    0
                } else if current_value == initial_value {
                    if initial_value.is_zero() {
                        // we do not purge slots, so we use another indicator here
                        SSTORE_SET_EXTRA
                    } else {
                        SSTORE_RESET_EXTRA
                    }
                } else {
                    0
                };

                // In EVM spec there's a discrepancy for cold read and cold write costs. Cold
                // writes add another 100 from thin air.
                // Uses access warmness (EIP-2929): warm after any SLOAD or SSTORE.
                if is_warm_access == false {
                    total_cost + 100
                } else {
                    total_cost
                }
            }
        };
        // A write has two independent native components:
        // 1. Updating the in-memory slot data (`addr_data`). This happens on
        //    every write — no-op, warm, and cold alike — so
        //    `WARM_STORAGE_WRITE_EXTRA_NATIVE_COST` is always charged. (Mirrors
        //    the account path, which always charges `WARM_ACCOUNT_CACHE_WRITE_EXTRA`.)
        // 2. The merkle-path work of a cold write, charged on top of (1) only for
        //    the first (cold) write to the slot this tx. A warm write (cold extra
        //    already charged this tx) or a no-op write pays only (1).
        use crate::system_implementation::flat_storage_model::cost_constants;
        let merkle_extra = if new_value == current_value || is_cold_write_charged {
            0
        } else if is_new_slot {
            cost_constants::COLD_NEW_STORAGE_WRITE_EXTRA_NATIVE_COST
        } else {
            cost_constants::COLD_EXISTING_STORAGE_WRITE_EXTRA_NATIVE_COST
        };
        resources.charge_legacy_gas_and_native(
            gas,
            cost_constants::WARM_STORAGE_WRITE_EXTRA_NATIVE_COST + merkle_extra,
        )
    }

    /// Refund some resources if needed
    #[allow(unused_variables)]
    fn refund_for_storage_write(
        &self,
        ee_type: ExecutionEnvironmentType,
        value_at_tx_start: &Bytes32,
        current_value: &Bytes32,
        new_value: &Bytes32,
        resources: &mut R,
        refund_counter: &mut R,
    ) -> Result<(), SystemError> {
        if ee_type == ExecutionEnvironmentType::EVM {
            // EVM specific refunds calculation
            {
                if current_value != new_value {
                    if current_value == value_at_tx_start {
                        if !value_at_tx_start.is_zero() && new_value.is_zero() {
                            refund_counter.add_legacy_gas(4800);
                        }
                    } else {
                        if !value_at_tx_start.is_zero() {
                            if current_value.is_zero() {
                                refund_counter.charge_legacy_gas(4800)?;
                            } else if new_value.is_zero() {
                                refund_counter.add_legacy_gas(4800);
                            }
                        }
                        if new_value == value_at_tx_start {
                            if value_at_tx_start.is_zero() {
                                refund_counter.add_legacy_gas(20000 - 100);
                            } else {
                                refund_counter.add_legacy_gas(5000 - 2100 - 100);
                            }
                        }
                    }
                }

                Ok(())
            }
        } else {
            Ok(())
        }
    }
}
