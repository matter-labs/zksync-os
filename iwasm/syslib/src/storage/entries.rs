use crate::types::ints::U256BE;

use super::Storeable;


pub struct Entry<T: Storeable> {
    pub(crate) key_hash: U256BE,
    pub(crate) value: T
}

impl<T: Storeable> Entry<T> {
    pub fn value(&self) -> &T {
        &self.value
    }

    pub fn write_value(&mut self, value: T) {
        self.value = value;
        crate::impl_arch::system::storage::write_s(&self.key_hash, &self.value.to_u256_be(0));
    }
}
