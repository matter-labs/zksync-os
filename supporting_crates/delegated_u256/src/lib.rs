#![cfg_attr(not(test), no_std)]

mod arithmetic;
mod copy;
mod delegation;
mod utils;

#[allow(clippy::derived_hash_with_manual_eq)]
#[derive(Hash, Default)]
#[repr(align(32))]
pub struct DelegatedU256([u64; 4]);

impl serde::Serialize for DelegatedU256 {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut be_bytes = [0u8; 32];
        for (i, limb) in self.0.iter().enumerate() {
            be_bytes[24 - i * 8..32 - i * 8].copy_from_slice(&limb.to_be_bytes());
        }
        serializer.serialize_bytes(&be_bytes)
    }
}

impl<'de> serde::Deserialize<'de> for DelegatedU256 {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl serde::de::Visitor<'_> for Visitor {
            type Value = DelegatedU256;
            fn expecting(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
                f.write_str("32 bytes in big-endian")
            }
            fn visit_bytes<E: serde::de::Error>(self, v: &[u8]) -> Result<DelegatedU256, E> {
                if v.len() != 32 {
                    return Err(E::invalid_length(v.len(), &self));
                }
                let mut limbs = [0u64; 4];
                for (i, limb) in limbs.iter_mut().enumerate() {
                    let start = 24 - i * 8;
                    let mut bytes = [0u8; 8];
                    bytes.copy_from_slice(&v[start..start + 8]);
                    *limb = u64::from_be_bytes(bytes);
                }
                Ok(DelegatedU256(limbs))
            }
        }
        deserializer.deserialize_bytes(Visitor)
    }
}

pub use arithmetic::*;
pub use copy::*;
pub use delegation::*;

pub fn init() {
    arithmetic::init();
}
