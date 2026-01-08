use zk_ee::execution_environment_type::ExecutionEnvironmentType;

use zk_ee::system::{errors::system::SystemError, Resources};

/// EE-specific IO charging.
pub trait StorageAccessPolicy<R: Resources, V>: 'static + Sized {
    /// Charge for a warm read (already in cache).
    fn charge_warm_storage_read(
        &self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut R,
    ) -> Result<(), SystemError>;

    /// Charge the extra cost of reading a key
    /// not present in the cache. This cost is added
    /// to the cost of a warm read.
    fn charge_cold_storage_read_extra(
        &self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut R,
        is_new_slot: bool,
    ) -> Result<(), SystemError>;

    /// Charge the additional cost of performing a write.
    /// This cost is added to the cost of reading.
    /// We assume writing is always at least as expensive
    /// as reading.
    fn charge_storage_write_extra(
        &self,
        ee_type: ExecutionEnvironmentType,
        initial_value: &V,
        current_value: &V,
        new_value: &V,
        resources: &mut R,
        is_warm_write: bool,
        is_new_slot: bool,
    ) -> Result<(), SystemError>;

    /// Refund some resources if needed
    fn refund_for_storage_write(
        &self,
        ee_type: ExecutionEnvironmentType,
        value_at_tx_start: &V,
        current_value: &V,
        new_value: &V,
        resources: &mut R,
        refund_counter: &mut R,
    ) -> Result<(), SystemError>;
}
