use crate::test::test_structure::TransactionSection;
use alloy::primitives::*;
use serde::Deserializer;
use serde::{de::IgnoredAny, Deserialize};

#[derive(Debug, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct HeaderSection {
    pub parent_hash: Option<B256>,
    pub uncle_hash: Option<B256>,
    pub coinbase: Address,
    pub state_root: Option<B256>,
    pub transactions_trie: Option<B256>,
    pub receipt_trie: Option<B256>,
    pub bloom: Option<IgnoredAny>,
    pub difficulty: Option<U256>,
    pub number: U256,
    pub gas_limit: U256,
    pub gas_used: U256,
    pub timestamp: U256,
    pub extra_data: Option<String>,
    pub mix_hash: Option<U256>,
    pub nonce: Option<U256>,
    pub base_fee_per_gas: Option<U256>,
    pub withdrawals_root: Option<B256>,
    pub blob_gas_used: Option<U256>,
    pub excess_blob_gas: Option<U256>,
    pub parent_beacon_block_root: Option<B256>,
    pub hash: B256,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BlockSection {
    pub block_header: HeaderSection,
    pub transactions: Vec<TransactionSection>,
    pub expect_exception: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct RlpDecoded {
    rlp_decoded: BlockSection,
    #[serde(rename = "expectException")]
    expect_exception: Option<String>,
}

// Sometimes the block is wrapped in rlp_decoded...
#[derive(Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum BlockElem {
    Wrapped(RlpDecoded),
    Plain(BlockSection),
}

pub fn blocks_from_plain_or_wrapped<'de, D>(de: D) -> Result<Vec<BlockSection>, D::Error>
where
    D: Deserializer<'de>,
{
    let elems = <Vec<BlockElem>>::deserialize(de)?;
    Ok(elems
        .into_iter()
        .map(|e| match e {
            BlockElem::Plain(b) => b,
            BlockElem::Wrapped(RlpDecoded {
                rlp_decoded,
                expect_exception,
            }) => {
                if rlp_decoded.expect_exception.is_none() {
                    BlockSection {
                        expect_exception,
                        block_header: rlp_decoded.block_header,
                        transactions: rlp_decoded.transactions,
                    }
                } else {
                    rlp_decoded
                }
            }
        })
        .collect())
}
