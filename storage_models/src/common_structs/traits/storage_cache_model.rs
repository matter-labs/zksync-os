use zk_ee::utils::Bytes32;

use super::{snapshottable_io::SnapshottableIo, *};

pub trait SpecialAccountProperty: 'static + Clone + Copy + core::fmt::Debug {
    type Value: 'static + Clone + Copy + core::fmt::Debug;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct AccountAggregateDataHash;

impl SpecialAccountProperty for AccountAggregateDataHash {
    type Value = Bytes32;
}

// TODO: extend when needed

// We call it "cache model" because real work to dump everything into KV-storage
// is somewhere outside of it, but this "cache" is fully responsible for resources management
pub trait StorageCacheModel: Sized + SnapshottableIo {
    type IOTypes: SystemIOTypesConfig;
    type Resources: Resources;

    fn read(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        key: &<Self::IOTypes as SystemIOTypesConfig>::StorageKey,
        oracle: &mut impl IOOracle,
        // cold_value_oracle_fn: impl FnMut() -> Result<<Self::IOTypes as SystemIOTypesConfig>::StorageValue, InternalError>
    ) -> Result<<Self::IOTypes as SystemIOTypesConfig>::StorageKey, SystemError>;

    fn touch(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut Self::Resources,
        address: &<Self::IOTypes as SystemIOTypesConfig>::Address,
        key: &<Self::IOTypes as SystemIOTypesConfig>::StorageKey,
        oracle: &mut impl IOOracle,
        is_access_list: bool,
    ) -> Result<(), SystemError>;

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
