use crate::internal_error;
use crate::oracle::usize_serialization::{WordDeserializable, WordSerializable, WordSink};
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

impl WordSerializable for ExecutionEnvironmentType {
    fn word_len(&self) -> usize {
        <u8 as WordSerializable>::word_len(self.u8_value_ref())
    }

    fn write_words(&self, out: &mut impl WordSink) {
        <u8 as WordSerializable>::write_words(self.u8_value_ref(), out);
    }
}

impl WordDeserializable for ExecutionEnvironmentType {
    fn read_words(src: &mut impl ExactSizeIterator<Item = usize>) -> Result<Self, InternalError> {
        let discr = <u8 as WordDeserializable>::read_words(src)?;

        match discr {
            Self::NO_EE_BYTE => Ok(Self::NoEE),
            Self::EVM_EE_BYTE => Ok(Self::EVM),
            _ => Err(internal_error!("Unknown EE type")),
        }
    }
}
