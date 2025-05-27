use zk_ee::utils::Bytes32;

use super::*;

// #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
// #[repr(usize)]
// pub enum AccountProperties {
//     AccountAggregateData,
//     Nonce,
//     DeploymentNonce,
//     ObservableBytecodeHash,
//     ObservableBytecodeLen,
//     BytecodeHash,
//     BytecodeLen,
// }

pub trait SpecialAccountProperty: 'static + Clone + Copy + core::fmt::Debug {
    type Value: 'static + Clone + Copy + core::fmt::Debug;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct AccountAggregateDataHash;

impl SpecialAccountProperty for AccountAggregateDataHash {
    type Value = Bytes32;
}

// TODO: extend when needed

// We call it "cache model" because real work do dump everything into KV-storage
// is somewhere outside of it, but this "cache" is fully responsible for resources management
pub trait StorageCacheModel: Sized {
    type IOTypes: SystemIOTypesConfig;
    type Resources: Resources;
    type StateSnapshot;
    type TxStats;

    fn begin_new_tx(&mut self);
    fn tx_stats(&self) -> Self::TxStats;

    fn start_frame(&mut self) -> Self::StateSnapshot;
    fn finish_frame(&mut self, rollback_handle: Option<&Self::StateSnapshot>);

    fn read(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        key: &<Self::IOTypes as SystemIOTypesConfig>::StorageKey,
        oracle: &mut impl IOOracle,
        // cold_value_oracle_fn: impl FnMut() -> Result<<Self::IOTypes as SystemIOTypesConfig>::StorageValue, InternalError>
    ) -> Result<<Self::IOTypes as SystemIOTypesConfig>::StorageKey, SystemError>;

    // returns old value
    fn write(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        key: &<Self::IOTypes as SystemIOTypesConfig>::StorageKey,
        new_value: &<Self::IOTypes as SystemIOTypesConfig>::StorageValue,
        oracle: &mut impl IOOracle,
        // cold_value_oracle_fn: impl FnMut() -> Result<<Self::IOTypes as SystemIOTypesConfig>::StorageValue, InternalError>
    ) -> Result<<Self::IOTypes as SystemIOTypesConfig>::StorageKey, SystemError>;

    // special access function for account properties. Not all enum options could be supported
    fn read_special_account_property<T: SpecialAccountProperty>(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        oracle: &mut impl IOOracle,
        // cold_value_oracle_fn: impl FnMut() -> Result<<Self::IOTypes as SystemIOTypesConfig>::StorageValue, InternalError>
    ) -> Result<T::Value, SystemError>;

    // special mutation function for account properties. Not all enum options could be supported
    fn write_special_account_property<T: SpecialAccountProperty>(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        new_value: &T::Value,
        oracle: &mut impl IOOracle,
        // cold_value_oracle_fn: impl FnMut() -> Result<<Self::IOTypes as SystemIOTypesConfig>::StorageValue, InternalError>
    ) -> Result<T::Value, SystemError>;
}
