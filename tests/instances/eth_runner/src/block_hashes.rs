use alloy::primitives::{B256, U256};
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BlockInfo {
    pub number: u64,
    pub hash: B256,
}

#[derive(Debug, Deserialize)]
pub struct BlockHashes(pub Vec<BlockInfo>);

impl BlockHashes {
    pub fn into_array(self, block_number: u64) -> [U256; 256] {
        let mut array = [U256::ZERO; 256];
        let mut map = HashMap::<u64, U256>::new();
        self.0.into_iter().for_each(|info| {
            map.insert(info.number, info.hash.into());
        });
        // Add values for most recent 256 block, if present
        for offset in 1..=256 {
            if let Some(hash) = map.get(&(block_number - offset)) {
                array[256 - offset as usize] = *hash;
            }
        }
        array
    }
}
