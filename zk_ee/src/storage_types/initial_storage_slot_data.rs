use crate::types_config::SystemIOTypesConfig;

#[derive(Clone, Copy, Debug, Default)]
pub struct InitialStorageSlotData<IOTypes: SystemIOTypesConfig> {
    // We need to know what was a value of the storage slot,
    // and whether it existed in the state or has to be created
    // (so additional information is needed to reconstruct creation location).
    pub is_new_storage_slot: bool,
    pub initial_value: IOTypes::StorageValue,
}

impl<IOTypes: SystemIOTypesConfig> crate::oracle::word_layout::WordLayout
    for InitialStorageSlotData<IOTypes>
where
    IOTypes::StorageValue: crate::oracle::word_layout::WordLayout,
{
    const WORD_COUNT: Option<usize> = match (
        <bool as crate::oracle::word_layout::WordLayout>::WORD_COUNT,
        <IOTypes::StorageValue as crate::oracle::word_layout::WordLayout>::WORD_COUNT,
    ) {
        (Some(a), Some(b)) => Some(a + b),
        _ => None,
    };

    fn write_words(&self, w: &mut impl FnMut(u32)) {
        self.is_new_storage_slot.write_words(w);
        self.initial_value.write_words(w);
    }

    fn read_words(r: &mut impl FnMut() -> u32) -> Self {
        Self {
            is_new_storage_slot: crate::oracle::word_layout::WordLayout::read_words(r),
            initial_value: crate::oracle::word_layout::WordLayout::read_words(r),
        }
    }
}
