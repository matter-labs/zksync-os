use super::*;
use crate::run::convert_alloy::FromAlloy;
use crate::run::NextTxResponse;
use crate::run::TxSource;
use basic_bootloader::bootloader::transaction::TxEncodingFormat;
use ruint::aliases::B160;
use zk_ee::oracle::memory_io::host::write_dynamic_bytes;
use zk_ee::oracle::query_ids::TX_FROM_QUERY_ID;
use zk_ee::oracle::query_ids::{
    NEXT_TX_SIZE_QUERY_ID, TX_DATA_WORDS_QUERY_ID, TX_ENCODING_FORMAT_QUERY_ID,
};

/// This processor handles four types of queries:
/// 1. NEXT_TX_SIZE_QUERY_ID - Returns the size of the next transaction
/// 2. TX_DATA_WORDS_QUERY_ID - Returns the actual transaction data
/// 3. TX_ENCODING_FORMAT_QUERY_ID - Returns the encoding format of the
///    current transaction.
/// 4. TX_FROM_QUERY_ID - Returns the originator address of the current
///    transaction. Only to be called for RLP encoded transactions.
#[cfg_attr(feature = "testing", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
pub struct TxDataResponder<TS: TxSource> {
    pub tx_source: TS,
    /// Cached next transaction data, populated after size query
    pub next_tx: Option<Vec<u8>>,
    /// Cached next transaction format, populated after size query
    /// Note: we use different fields for next_tx and next_tx_format
    /// so that they don't have to be consumed at the same time.
    pub next_tx_format: Option<TxEncodingFormat>,
    /// Cached next transaction from, populated after size query
    /// (if present)
    pub next_tx_from: Option<B160>,
}

impl<TS: TxSource> TxDataResponder<TS> {
    const SUPPORTED_QUERY_IDS: &[u32] = &[
        NEXT_TX_SIZE_QUERY_ID,
        TX_DATA_WORDS_QUERY_ID,
        TX_ENCODING_FORMAT_QUERY_ID,
        TX_FROM_QUERY_ID,
    ];
}

impl<TS: TxSource> OracleQueryProcessor for TxDataResponder<TS> {
    fn supported_memory_query_ids(&self) -> Vec<u32> {
        Self::SUPPORTED_QUERY_IDS.to_vec()
    }

    fn process_memory_query(
        &mut self,
        query_id: u32,
        _input_word: usize,
        _memory: &dyn QuerierMemory,
        mode: RunMode,
        native_run_responses: &mut Vec<u32>,
        guest_run_responses: &mut Vec<u32>,
    ) {
        assert!(Self::SUPPORTED_QUERY_IDS.contains(&query_id));

        let response = match query_id {
            NEXT_TX_SIZE_QUERY_ID => {
                let len = match &self.next_tx {
                    Some(next_tx) => next_tx.len(),
                    None => {
                        match self.tx_source.get_next_tx() {
                            NextTxResponse::SealBlock => 0,
                            NextTxResponse::Tx(EncodedTx::Abi(next_tx)) => {
                                let next_tx_len = next_tx.len();
                                // `0` interpreted as seal batch
                                assert_ne!(next_tx_len, 0);
                                self.next_tx = Some(next_tx);
                                self.next_tx_format = Some(TxEncodingFormat::Abi);
                                self.next_tx_from = None;
                                next_tx_len
                            }
                            NextTxResponse::Tx(EncodedTx::Rlp(next_tx, from)) => {
                                let next_tx_len = next_tx.len();
                                // `0` interpreted as seal batch
                                assert_ne!(next_tx_len, 0);
                                self.next_tx = Some(next_tx);
                                self.next_tx_format = Some(TxEncodingFormat::Rlp);
                                self.next_tx_from = Some(B160::from_alloy(from));
                                next_tx_len
                            }
                        }
                    }
                } as u32;

                memory_response(&len)
            }
            TX_DATA_WORDS_QUERY_ID => {
                let Some(tx) = self.next_tx.take() else {
                    panic!(
                        "trying to read next tx content before size query or after seal response"
                    );
                };

                let mut response = Vec::new();
                write_dynamic_bytes(&tx, &mut response);
                response
            }
            TX_ENCODING_FORMAT_QUERY_ID => {
                let Some(format) = self.next_tx_format.take() else {
                    panic!(
                        "trying to read next tx format before size query or after seal response"
                    );
                };

                memory_response(&format)
            }
            TX_FROM_QUERY_ID => {
                let Some(from) = self.next_tx_from.take() else {
                    panic!(
                        "trying to read next tx from before size query, after seal response or for a zk transaction"
                    );
                };
                memory_response(&from)
            }
            _ => unreachable!(),
        };
        respond_to_every_target(mode, response, native_run_responses, guest_run_responses);
    }
}
