use serde::{Deserialize, Serialize};
use zk_ee::utils::Bytes32;

#[derive(
    Clone, Debug, PartialEq, Serialize, Deserialize, wincode::SchemaRead, wincode::SchemaWrite,
)]
pub struct DivRemResponse {
    pub quotient: [u64; 4],
}

#[repr(C)]
#[derive(Clone, Debug, Serialize, Deserialize, wincode::SchemaRead, wincode::SchemaWrite)]
pub struct WideDivRemResponse {
    pub quotient_lo: [u64; 4],
    pub quotient_hi: [u64; 4],
}

pub type ModexpLimbs = smallvec::SmallVec<[u64; 64]>;

#[derive(
    Clone, Debug, PartialEq, Serialize, Deserialize, wincode::SchemaRead, wincode::SchemaWrite,
)]
pub struct ModexpResponse {
    pub quotient: ModexpLimbs,
    pub remainder: ModexpLimbs,
}

#[derive(
    Clone, Debug, PartialEq, Serialize, Deserialize, wincode::SchemaRead, wincode::SchemaWrite,
)]
pub struct FieldSqrtResponse {
    pub result: Bytes32,
    pub is_valid: bool,
}

#[repr(C)]
#[derive(Clone, Debug, Serialize, Deserialize, wincode::SchemaRead, wincode::SchemaWrite)]
pub struct FieldInverseResponse {
    pub result: Bytes32,
}

// SAFETY: All these types are repr(C) with LE-native fields, no padding.
unsafe impl zk_ee::oracle::RawWordReadable for DivRemResponse {}
unsafe impl zk_ee::oracle::RawWordReadable for WideDivRemResponse {}
