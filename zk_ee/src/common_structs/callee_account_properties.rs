use ruint::aliases::{B160, U256};

pub struct TransferInfo {
    pub value: U256,
    pub target: B160,
}

pub struct CalleeAccountProperties<'a> {
    pub next_ee_version: u8,
    pub nonce: u64,
    pub nominal_token_balance: U256,
    pub bytecode: &'a [u8],
    pub code_version: u8,
    pub unpadded_code_len: u32,
    pub artifacts_len: u32,
}
