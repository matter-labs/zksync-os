use crate::system::errors::internal::InternalError;
use crate::{internal_error, kv_markers::*};

#[repr(u32)]
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum TransactionNature {
    Intrinsic = 0,
    Request,
    Enforced,
    EnforcedWithOutputNeeded,
    SystemUpgrade,
}

const _: () = const {
    assert!(core::mem::size_of::<TransactionNature>() == core::mem::size_of::<u32>());
    assert!(core::mem::align_of::<TransactionNature>() == core::mem::align_of::<u32>());
};

impl UsizeSerializable for TransactionNature {
    const USIZE_LEN: usize = <u32 as UsizeSerializable>::USIZE_LEN;

    fn iter(&self) -> impl ExactSizeIterator<Item = usize> {
        <u32 as UsizeSerializable>::iter(unsafe {
            core::mem::transmute::<&TransactionNature, &u32>(self)
        })
    }
}

impl UsizeDeserializable for TransactionNature {
    const USIZE_LEN: usize = <Self as UsizeSerializable>::USIZE_LEN;

    fn from_iter(src: &mut impl ExactSizeIterator<Item = usize>) -> Result<Self, InternalError> {
        let discr = <u32 as UsizeDeserializable>::from_iter(src)?;
        match discr {
            a if a == const { TransactionNature::Intrinsic as u32 } => {
                Ok(TransactionNature::Intrinsic)
            }
            a if a == const { TransactionNature::Request as u32 } => Ok(TransactionNature::Request),
            a if a == const { TransactionNature::Enforced as u32 } => {
                Ok(TransactionNature::Enforced)
            }
            a if a == const { TransactionNature::EnforcedWithOutputNeeded as u32 } => {
                Ok(TransactionNature::EnforcedWithOutputNeeded)
            }
            a if a == const { TransactionNature::SystemUpgrade as u32 } => {
                Ok(TransactionNature::SystemUpgrade)
            }
            _ => Err(internal_error!("invalid encoding for transaction nature")),
        }
    }
}
