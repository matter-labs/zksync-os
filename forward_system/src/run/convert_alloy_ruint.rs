pub trait IntoAlloy<T> {
    fn into_alloy(self) -> T;
}

impl IntoAlloy<alloy::primitives::Address> for ruint::aliases::B160 {
    fn into_alloy(self) -> alloy::primitives::Address {
        alloy::primitives::Address::from(self.to_be_bytes())
    }
}

pub trait IntoRuint<T> {
    fn into_ruint(self) -> T;
}

impl IntoRuint<ruint::aliases::B160> for alloy::primitives::Address {
    fn into_ruint(self) -> ruint::aliases::B160 {
        ruint::aliases::B160::from_be_bytes(self.0.into())
    }
}
