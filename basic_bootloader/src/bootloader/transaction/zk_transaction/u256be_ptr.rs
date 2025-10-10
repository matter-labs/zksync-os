use ruint::aliases::{B160, U256};

///
/// Reference to a BE encoded U256.
/// Can be read into U256 value by copying.
///
/// Contains few efficient(without copying) utility methods.
///
#[derive(Clone, Copy)]
pub struct U256BEPtr<'a> {
    pub encoding: &'a [u8; U256::BYTES],
}

impl<'a> U256BEPtr<'a> {
    ///
    /// Create self from a slice.
    ///
    #[allow(clippy::result_unit_err)]
    pub fn try_from_slice(slice: &'a [u8]) -> Result<(Self, &'a [u8]), ()> {
        let (encoding, slice) = slice.split_at_checked(U256::BYTES).ok_or(())?;
        let new = U256BEPtr {
            encoding: encoding.try_into().unwrap(),
        };

        Ok((new, slice))
    }

    ///
    /// Read into U256 value.
    ///
    pub fn read(&self) -> U256 {
        U256::from_be_bytes(*self.encoding)
    }

    ///
    /// Validates that value can be safely read as an address(higher 12 bytes are zero).
    ///
    pub fn validate_address(&self) -> Result<B160, ()> {
        for byte in 0..12 {
            if self.encoding[byte] != 0 {
                return Err(());
            }
        }
        let value =
            B160::from_be_bytes::<{ B160::BYTES }>(self.encoding[12..32].try_into().unwrap());
        Ok(value)
    }

    ///
    /// Validates that value can be safely read as single u8
    ///
    pub fn validate_u8(&self) -> Result<u8, ()> {
        for byte in 0..31 {
            if self.encoding[byte] != 0 {
                return Err(());
            }
        }
        Ok(self.encoding[31])
    }

    ///
    /// Validates that value can be safely read as an u32 (higher 28 bytes are zero).
    ///
    pub fn validate_u32(&self) -> Result<u32, ()> {
        for byte in 0..28 {
            if self.encoding[byte] != 0 {
                return Err(());
            }
        }
        let value = u32::from_be_bytes(self.encoding[28..32].try_into().unwrap());
        Ok(value)
    }

    ///
    /// Validates that value can be safely read as an u64 (higher 24 bytes are zero).
    ///
    pub fn validate_u64(&self) -> Result<u64, ()> {
        for byte in 0..24 {
            if self.encoding[byte] != 0 {
                return Err(());
            }
        }
        let value = u64::from_be_bytes(self.encoding[24..32].try_into().unwrap());
        Ok(value)
    }

    ///
    /// Validates that value can be safely read as an u128 (higher 16 bytes are zero).
    ///
    pub fn validate_u128(&self) -> Result<u128, ()> {
        for byte in 0..16 {
            if self.encoding[byte] != 0 {
                return Err(());
            }
        }
        let value = u128::from_be_bytes(self.encoding[16..32].try_into().unwrap());
        Ok(value)
    }
}
