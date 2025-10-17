use crate::internal_error;
use crate::oracle::usize_serialization::{UsizeDeserializable, UsizeSerializable};
use crate::system::errors::internal::InternalError;

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ExecutionEnvironmentType {
    NoEE = 0,
    EVM = 1,
}

impl ExecutionEnvironmentType {
    pub const NO_EE_BYTE: u8 = Self::NoEE as u8;
    pub const EVM_EE_BYTE: u8 = Self::EVM as u8;

    pub fn u8_value_ref(&self) -> &'static u8 {
        match self {
            Self::NoEE => &Self::NO_EE_BYTE,
            Self::EVM => &Self::EVM_EE_BYTE,
        }
    }

    pub fn parse_ee_version_byte(byte: u8) -> Result<Self, InternalError> {
        match byte {
            Self::NO_EE_BYTE => Ok(Self::NoEE),
            Self::EVM_EE_BYTE => Ok(Self::EVM),
            _ => Err(internal_error!("Unknown EE type")),
        }
    }
}

impl UsizeSerializable for ExecutionEnvironmentType {
    const USIZE_LEN: usize = <u8 as UsizeSerializable>::USIZE_LEN;

    fn iter(&self) -> impl ExactSizeIterator<Item = usize> {
        UsizeSerializable::iter(self.u8_value_ref())
    }
}

impl UsizeDeserializable for ExecutionEnvironmentType {
    const USIZE_LEN: usize = <Self as UsizeSerializable>::USIZE_LEN;

    fn from_iter(src: &mut impl ExactSizeIterator<Item = usize>) -> Result<Self, InternalError> {
        let discr = <u8 as UsizeDeserializable>::from_iter(src)?;

        match discr {
            Self::NO_EE_BYTE => Ok(Self::NoEE),
            Self::EVM_EE_BYTE => Ok(Self::EVM),
            _ => Err(internal_error!("Unknown EE type")),
        }
    }
}
