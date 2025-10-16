use alloy::primitives::{Address, B256, U256};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

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
    pub fn get_deployed_addresses(&self) -> HashSet<Address> {
        let mut deployed = HashSet::new();
        self.collect_deployed_addresses(&mut deployed);
        deployed
    }

    fn collect_deployed_addresses(&self, acc: &mut HashSet<Address>) {
        if matches!(self.call_type.as_deref(), Some("CREATE") | Some("CREATE2"))
            && self.error.is_none()
        {
            if let Some(to) = self.to {
                acc.insert(to);
            }
        }

        if let Some(ref calls) = self.calls {
            for call in calls {
                call.collect_deployed_addresses(acc);
            }
        }
    }

    pub fn has_call_to_unsupported_precompile(&self) -> bool {
        use alloy::hex::FromHex;
        self.to == Some(Address::from_hex("0000000000000000000000000000000000000009").unwrap())
            || self.to
                == Some(Address::from_hex("000000000000000000000000000000000000000a").unwrap())
            || self
                .calls
                .as_ref()
                .is_some_and(|calls| calls.iter().any(|i| i.has_call_to_unsupported_precompile()))
    }
}
