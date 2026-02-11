/// Generic conversion trait for converting values into Alloy primitives.
pub trait IntoAlloy<T> {
    /// Performs conversion into an Alloy type.
    #[must_use]
    fn into_alloy(self) -> T;
}

/// Generic conversion trait for converting values into ruint primitives.
pub trait IntoRuint<T> {
    /// Performs conversion into a ruint type.
    #[must_use]
    fn into_ruint(self) -> T;
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

impl IntoRuint<ruint::aliases::B160> for alloy::primitives::Address {
    #[inline]
    fn into_ruint(self) -> ruint::aliases::B160 {
        ruint::aliases::B160::from_be_bytes(self.0.into())
    }
}

impl IntoRuint<ruint::aliases::B160> for &alloy::primitives::Address {
    #[inline]
    fn into_ruint(self) -> ruint::aliases::B160 {
        ruint::aliases::B160::from_be_bytes(self.0.into())
    }
}
