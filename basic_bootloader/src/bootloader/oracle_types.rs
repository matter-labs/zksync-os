use serde::{Deserialize, Serialize};
use zk_ee::utils::Bytes32;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DivRemResponse {
    pub quotient: [u64; 4],
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WideDivRemResponse {
    pub quotient_lo: [u64; 4],
    pub quotient_hi: [u64; 4],
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModexpResponse {
    pub quotient: alloc::vec::Vec<u64>,
    pub remainder: alloc::vec::Vec<u64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FieldSqrtResponse {
    pub result: Bytes32,
    pub is_valid: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FieldInverseResponse {
    pub result: Bytes32,
}
