//! The storage slot cost model of Glamsterdam: the accesses and writes of EIP-8038
//! (`COLD_STORAGE_ACCESS`, `WARM_ACCESS`, `STORAGE_WRITE`, `REFUND_STORAGE_CLEAR`) and the
//! state gas of setting a slot that was zero (EIP-8037, `STORAGE_SET`, refilled when the
//! slot is cleared again in the transaction). The native charges are those of the
//! Ethereum cost model.

use basic_system::system_implementation::caches::storage_access_policy::StorageAccessPolicy;
use basic_system::system_implementation::flat_storage_model::cost_constants;
use evm_interpreter::v3::gas::{
    COLD_STORAGE_ACCESS, REFUND_STORAGE_CLEAR, STORAGE_SET, STORAGE_WRITE, WARM_ACCESS,
};
use zk_ee::execution_environment_type::ExecutionEnvironmentType;
use zk_ee::system::errors::system::SystemError;
use zk_ee::system::StateGasResources;
use zk_ee::utils::Bytes32;

#[derive(Clone, Copy, Debug, Default)]
pub struct GlamsterdamStorageAccessCostModel;

impl<R: StateGasResources> StorageAccessPolicy<R, Bytes32> for GlamsterdamStorageAccessCostModel {
    fn charge_warm_storage_read(
        &self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut R,
    ) -> Result<(), SystemError> {
        let gas = match ee_type {
            ExecutionEnvironmentType::NoEE => 0,
            ExecutionEnvironmentType::EVM => WARM_ACCESS,
        };
        resources.charge_legacy_gas_and_native(gas, cost_constants::WARM_STORAGE_READ_NATIVE_COST)
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
                    COLD_STORAGE_ACCESS - WARM_ACCESS
                }
            }
        };
        let native = if is_new_slot {
            cost_constants::COLD_NEW_STORAGE_READ_NATIVE_COST
        } else {
            cost_constants::COLD_EXISTING_STORAGE_READ_NATIVE_COST
        };
        resources.charge_legacy_gas_and_native(gas, native)
    }

    /// The write (EIP-8038) when the slot moves away from its value at the start of the
    /// transaction, and the state gas of setting a zero slot (EIP-8037); the execution
    /// gas first, so that running out of it leaves the state gas alone
    fn charge_storage_write_extra(
        &self,
        ee_type: ExecutionEnvironmentType,
        initial_value: &Bytes32,
        current_value: &Bytes32,
        new_value: &Bytes32,
        resources: &mut R,
        _is_warm_access: bool,
        is_cold_write_charged: bool,
        is_new_slot: bool,
    ) -> Result<(), SystemError> {
        let moves_away = new_value != current_value && current_value == initial_value;
        let (gas, state_gas) = match ee_type {
            ExecutionEnvironmentType::NoEE => (0, 0),
            ExecutionEnvironmentType::EVM => {
                if moves_away {
                    let state_gas = if initial_value.is_zero() {
                        STORAGE_SET
                    } else {
                        0
                    };
                    (STORAGE_WRITE, state_gas)
                } else {
                    (0, 0)
                }
            }
        };
        // the native work of the write, and of its merkle path for the first write of the
        // slot in the transaction
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
        )?;
        if state_gas != 0 {
            resources.charge_state_gas(state_gas)?;
        }
        Ok(())
    }

    /// The refund rules of EIP-8038 on the refund counter, and the refill of the state gas
    /// of a slot set and cleared again in the transaction (EIP-8037)
    fn refund_for_storage_write(
        &self,
        ee_type: ExecutionEnvironmentType,
        value_at_tx_start: &Bytes32,
        current_value: &Bytes32,
        new_value: &Bytes32,
        resources: &mut R,
        refund_counter: &mut R,
    ) -> Result<(), SystemError> {
        if ee_type != ExecutionEnvironmentType::EVM || current_value == new_value {
            return Ok(());
        }
        let original_set = !value_at_tx_start.is_zero();
        if original_set && !current_value.is_zero() && new_value.is_zero() {
            // a slot set at the start of the transaction is cleared
            refund_counter.add_legacy_gas(REFUND_STORAGE_CLEAR);
        }
        if original_set && current_value.is_zero() {
            // the clearing refunded before is undone
            refund_counter.charge_legacy_gas(REFUND_STORAGE_CLEAR)?;
        }
        if new_value == value_at_tx_start {
            // restored: the write that moved it away is refunded
            refund_counter.add_legacy_gas(STORAGE_WRITE);
            if value_at_tx_start.is_zero() {
                // set then cleared: the state it would have created is refilled
                resources.credit_state_gas(STORAGE_SET);
            }
        }
        Ok(())
    }
}
