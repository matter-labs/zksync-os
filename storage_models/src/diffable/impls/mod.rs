use super::*;

pub mod transient_storage_value;
pub mod warm_storage_key;
pub mod warm_storage_value;

#[derive(Clone, Debug)]
pub enum RollbackType<V: Rollbackable> {
    Read(V::ReadOrTouchRollbackInformation),
    Write(V::WriteRollbackInformation),
}
