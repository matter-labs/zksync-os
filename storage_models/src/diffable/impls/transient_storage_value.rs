use super::Rollbackable;
use zk_ee::common_structs::*;
use zk_ee::utils::Bytes32;

#[derive(Clone, Copy, Debug)]
pub struct TransientStorageSlotRollbackData {
    pub previous_value: Bytes32,
}

impl Rollbackable for TransientStorageValue {
    type ReadOrTouchRollbackInformation = ();
    type WriteRollbackInformation = TransientStorageSlotRollbackData;
    type Value = Bytes32;
    type AuxData = ();
    type InitValue = ();

    fn create_initial(_value: Self::InitValue) -> Self {
        Self {
            current_value: Bytes32::ZERO,
            changes_stack_depth: 0,
        }
    }

    fn current_value(&self) -> &Self::Value {
        &self.current_value
    }

    fn touch(
        &'_ mut self,
        _extra_data: &Self::AuxData,
    ) -> (
        &'_ <Self as Rollbackable>::Value,
        Self::ReadOrTouchRollbackInformation,
    ) {
        (&self.current_value, ())
    }
    fn read(
        &'_ mut self,
        _extra_data: &Self::AuxData,
    ) -> (
        &'_ <Self as Rollbackable>::Value,
        Self::ReadOrTouchRollbackInformation,
    ) {
        (&self.current_value, ())
    }

    fn update(
        &mut self,
        update: &Self::Value,
        _extra_data: &Self::AuxData,
    ) -> Self::WriteRollbackInformation {
        let previous_value = self.current_value;

        self.current_value = *update;
        self.changes_stack_depth += 1;

        TransientStorageSlotRollbackData { previous_value }
    }

    fn rollback_read(&mut self, _rollback: &Self::ReadOrTouchRollbackInformation) {
        // nothing
    }

    fn rollback_write(&mut self, rollback: &Self::WriteRollbackInformation) {
        self.current_value = rollback.previous_value;
        self.changes_stack_depth -= 1;
    }

    fn is_used(&self) -> bool {
        unreachable!("Not meant to be used for transient storage")
    }
}
