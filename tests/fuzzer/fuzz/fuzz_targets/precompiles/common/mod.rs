use bytes::Bytes;
use rig::ethers::{abi::Address, types::TransactionRequest};

pub fn get_tx(id: &str, data: &[u8]) -> TransactionRequest {
    let addr = Address::from_slice(hex::decode(id).unwrap().as_slice());

    TransactionRequest::new()
        .to(addr)
        .gas(1 << 27)
        .gas_price(1000)
        .data(Bytes::copy_from_slice(data))
        .nonce(0)
}

#[allow(dead_code)]
pub fn is_zero(data: &[u8]) -> bool {
    data.iter().all(|b| *b == 0)
}
