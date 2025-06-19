use crate::system::Ergs;
use ruint::aliases::B160;
use u256::U256;

pub struct TransferInfo {
    pub value: U256,
    pub target: B160,
}

pub struct CalleeParameters<'a> {
    pub next_ee_version: u8,
    pub bytecode: &'a [u8],
    pub bytecode_len: u32,
    pub artifacts_len: u32,
    pub stipend: Option<Ergs>,
    pub transfer_to_perform: Option<TransferInfo>,
}
