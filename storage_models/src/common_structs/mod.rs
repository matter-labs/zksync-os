pub mod generic_transient_storage;

mod traits;
pub use self::traits::*;

use crate::diffable::impls::RollbackType;
use crate::diffable::Rollbackable;

#[derive(Clone, Debug)]
pub struct GenericPlainStorageRollbackData<K, V: Rollbackable> {
    pub key: K,
    pub data: RollbackType<V>,
}
