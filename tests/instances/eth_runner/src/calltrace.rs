use alloy::primitives::{address, Address, B256, U256};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct CallTraceItem {
    pub from: Address,
    pub to: Option<Address>,
    pub value: Option<U256>,
    pub gas: U256,
    pub gas_used: U256,
    #[serde(skip)]
    pub input: (),
    #[serde(skip)]
    pub output: (),
    #[serde(default)]
    pub calls: Option<Vec<CallTraceItem>>,
    #[serde(rename = "type")]
    pub call_type: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TxCallTraces {
    pub result: CallTraceItem,
    pub tx_hash: B256,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CallTrace {
    pub result: Vec<TxCallTraces>,
}

impl CallTraceItem {
    pub fn has_call_to_unsupported_precompile(&self) -> bool {
        self.to == Some(address!("0x0000000000000000000000000000000000000009"))
            || self
                .calls
                .as_ref()
                .is_some_and(|calls| calls.iter().any(|i| i.has_call_to_unsupported_precompile()))
    }
}
