/// Generic conversion trait for converting values into Alloy primitives.
pub trait IntoAlloy<T> {
    /// Performs conversion into an Alloy type.
    #[must_use]
    fn into_alloy(self) -> T;
}

/// Generic conversion trait for converting values from Alloy primitives.
pub trait FromAlloy<T> {
    /// Performs conversion from an Alloy type.
    #[must_use]
    fn from_alloy(self) -> T;
}

impl IntoAlloy<alloy::primitives::Address> for ruint::aliases::B160 {
    #[inline]
    fn into_alloy(self) -> alloy::primitives::Address {
        alloy::primitives::Address::from(self.to_be_bytes())
    }
}

impl IntoAlloy<alloy::primitives::Address> for &ruint::aliases::B160 {
    #[inline]
    fn into_alloy(self) -> alloy::primitives::Address {
        alloy::primitives::Address::from(self.to_be_bytes())
    }
}

impl FromAlloy<ruint::aliases::B160> for alloy::primitives::Address {
    #[inline]
    fn from_alloy(self) -> ruint::aliases::B160 {
        ruint::aliases::B160::from_be_bytes(self.0.into())
    }
}

impl FromAlloy<ruint::aliases::B160> for &alloy::primitives::Address {
    #[inline]
    fn from_alloy(self) -> ruint::aliases::B160 {
        ruint::aliases::B160::from_be_bytes(self.0.into())
    }
}

impl IntoAlloy<alloy::primitives::B256> for zk_ee::utils::Bytes32 {
    #[inline]
    fn into_alloy(self) -> alloy::primitives::B256 {
        alloy::primitives::B256::from(self.as_u8_array())
    }
}

impl IntoAlloy<alloy::primitives::B256> for &zk_ee::utils::Bytes32 {
    #[inline]
    fn into_alloy(self) -> alloy::primitives::B256 {
        alloy::primitives::B256::from(self.as_u8_array())
    }
}

impl FromAlloy<zk_ee::utils::Bytes32> for alloy::primitives::B256 {
    #[inline]
    fn from_alloy(self) -> zk_ee::utils::Bytes32 {
        zk_ee::utils::Bytes32::from(self.0)
    }
}

impl FromAlloy<zk_ee::utils::Bytes32> for &alloy::primitives::B256 {
    #[inline]
    fn from_alloy(self) -> zk_ee::utils::Bytes32 {
        zk_ee::utils::Bytes32::from(self.0)
    }
}
