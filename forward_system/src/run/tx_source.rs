#[derive(Debug, Clone)]
pub enum NextTxResponse {
    Tx(Vec<u8>),
    SealBatch,
}

pub trait TxSource: 'static {
    fn get_next_tx(&mut self) -> NextTxResponse;
}
