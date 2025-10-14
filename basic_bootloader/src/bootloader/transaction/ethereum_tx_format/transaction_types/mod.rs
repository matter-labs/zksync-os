pub mod eip_1559_tx;
pub mod eip_2930_tx;
pub mod eip_7702_tx;
pub mod legacy_tx;

// TODO: only with envelope
pub trait EthereumTxType {
    const TX_TYPE: u8;
}
