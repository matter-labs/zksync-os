use crate::types_config::SystemIOTypesConfig;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StorageAddress<IOTypes: SystemIOTypesConfig> {
    pub address: IOTypes::Address,
    pub key: IOTypes::StorageKey,
}

impl<IOTypes: SystemIOTypesConfig> crate::oracle::word_layout::WordLayout
    for StorageAddress<IOTypes>
where
    IOTypes::Address: crate::oracle::word_layout::WordLayout,
    IOTypes::StorageKey: crate::oracle::word_layout::WordLayout,
{
    const WORD_COUNT: Option<usize> = match (
        <IOTypes::Address as crate::oracle::word_layout::WordLayout>::WORD_COUNT,
        <IOTypes::StorageKey as crate::oracle::word_layout::WordLayout>::WORD_COUNT,
    ) {
        (Some(a), Some(b)) => Some(a + b),
        _ => None,
    };

    fn write_words(&self, w: &mut impl FnMut(u32)) {
        self.address.write_words(w);
        self.key.write_words(w);
    }

    fn read_words(r: &mut impl FnMut() -> u32) -> Self {
        Self {
            address: crate::oracle::word_layout::WordLayout::read_words(r),
            key: crate::oracle::word_layout::WordLayout::read_words(r),
        }
    }
}
