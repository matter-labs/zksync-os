use crate::{
    kv_markers::{ExactSizeChain, UsizeDeserializable, UsizeSerializable},
    system::errors::internal::InternalError,
    utils::Bytes32,
};

#[cfg_attr(feature = "testing", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct InteropRoot {
    pub root: Bytes32,
    pub block_or_batch_number: u64,
    pub chain_id: u64,
}

impl UsizeSerializable for InteropRoot {
    const USIZE_LEN: usize = <Bytes32 as UsizeSerializable>::USIZE_LEN
        + <u64 as UsizeSerializable>::USIZE_LEN
        + <u64 as UsizeSerializable>::USIZE_LEN;

    fn iter(&self) -> impl ExactSizeIterator<Item = usize> {
        ExactSizeChain::new(
            ExactSizeChain::new(
                UsizeSerializable::iter(&self.root),
                UsizeSerializable::iter(&self.block_or_batch_number),
            ),
            UsizeSerializable::iter(&self.chain_id),
        )
    }
}

impl UsizeDeserializable for InteropRoot {
    const USIZE_LEN: usize = <Bytes32 as UsizeSerializable>::USIZE_LEN
        + <u64 as UsizeSerializable>::USIZE_LEN
        + <u64 as UsizeSerializable>::USIZE_LEN;

    fn from_iter(src: &mut impl ExactSizeIterator<Item = usize>) -> Result<Self, InternalError> {
        let root = <Bytes32 as UsizeDeserializable>::from_iter(src)?;
        let block_number = <u64 as UsizeDeserializable>::from_iter(src)?;
        let chain_id = <u64 as UsizeDeserializable>::from_iter(src)?;

        let new = Self {
            root,
            block_or_batch_number: block_number,
            chain_id,
        };

        Ok(new)
    }
}
