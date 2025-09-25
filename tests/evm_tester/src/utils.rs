//!
//! The evm tester utils.
//!

pub(crate) fn address_to_b160(value: alloy::primitives::Address) -> ruint::aliases::B160 {
    ruint::aliases::B160::from_be_bytes(value.0 .0)
}
