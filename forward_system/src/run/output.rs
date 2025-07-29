// Includes code adapted from https://github.com/bluealloy/revm/blob/fb80087996dfbd6c74eaf308538cfa707ecb763c/crates/context/interface/src/result.rs

use crate::run::result_keeper::ForwardRunningResultKeeper;
use crate::run::TxResultCallback;
use arrayvec::ArrayVec;
pub use basic_bootloader::bootloader::block_header::BlockHeader;
use basic_bootloader::bootloader::errors::InvalidTransaction;
use ruint::aliases::B160;
use zk_ee::common_structs::GenericLogContent;
use zk_ee::common_structs::{
    derive_flat_storage_key, GenericEventContent, L2ToL1Log, PreimageType,
};
use zk_ee::kv_markers::MAX_EVENT_TOPICS;
use zk_ee::system::errors::internal::InternalError;
use zk_ee::types_config::EthereumIOTypesConfig;
use zk_ee::utils::Bytes32;

#[derive(Debug, Clone)]
// Output not observed for now, we allow dead code temporarily
#[allow(dead_code)]
pub enum ExecutionOutput {
    Call(Vec<u8>),
    Create(Vec<u8>, B160),
}

#[derive(Debug, Clone)]
// Output not observed for now, we allow dead code temporarily
#[allow(dead_code)]
pub enum ExecutionResult {
    /// Transaction executed successfully
    Success(ExecutionOutput),
    /// Transaction reverted
    Revert(Vec<u8>),
}

///
/// Transaction output in case of successful validation.
/// This structure includes data to create receipts and update state.
///
#[derive(Debug, Clone)]
// Output not observed for now, we allow dead code temporarily
#[allow(dead_code)]
pub struct TxOutput {
    /// Transaction execution step result
    pub execution_result: ExecutionResult,
    /// Total gas used, including all the steps(validation, execution, postOp call)
    pub gas_used: u64,
    /// Amount of refunded gas
    pub gas_refunded: u64,

    pub computational_native_used: u64,

    pub pubdata_used: u64,
    /// Deployed contract address
    /// - `Some(address)` for the deployment transaction
    /// - `None` otherwise
    pub contract_address: Option<B160>,
    /// Total logs list emitted during all the steps(validation, execution, postOp call)
    pub logs: Vec<Log>,
    /// Total l2 to l1 logs list emitted during all the steps(validation, execution, postOp call)
    pub l2_to_l1_logs: Vec<L2ToL1LogWithPreimage>,
    /// Deduplicated storage writes happened during tx processing(validation, execution, postOp call)
    /// TODO: now this field empty as we return writes on the blocks level, but eventually should be moved here
    pub storage_writes: Vec<StorageWrite>,
}

#[derive(Debug, Clone)]
pub struct L2ToL1LogWithPreimage {
    pub log: L2ToL1Log,
    pub preimage: Option<Vec<u8>>,
}

impl From<&GenericLogContent<EthereumIOTypesConfig>> for L2ToL1LogWithPreimage {
    fn from(value: &GenericLogContent<EthereumIOTypesConfig>) -> Self {
        use zk_ee::common_structs::GenericLogContentData;
        use zk_ee::common_structs::UserMsgData;
        let preimage = match &value.data {
            GenericLogContentData::UserMsg(UserMsgData { data, .. }) => {
                Some(data.as_slice().to_vec())
            }
            GenericLogContentData::L1TxLog(_) => None,
        };
        let log = value.into();
        Self { log, preimage }
    }
}

impl TxOutput {
    pub fn is_success(&self) -> bool {
        matches!(self.execution_result, ExecutionResult::Success(_))
    }

    pub fn as_returned_bytes(&self) -> &[u8] {
        match &self.execution_result {
            ExecutionResult::Success(o) => match o {
                ExecutionOutput::Call(vec) => vec,
                ExecutionOutput::Create(vec, _) => vec,
            },
            ExecutionResult::Revert(vec) => vec,
        }
    }
}

pub type TxResult = Result<TxOutput, InvalidTransaction>;

#[derive(Debug, Clone)]
pub struct Log {
    pub address: B160,
    pub topics: ArrayVec<Bytes32, MAX_EVENT_TOPICS>,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct StorageWrite {
    // TODO: maybe we should provide an index as well for efficiency?
    pub key: Bytes32,
    pub value: Bytes32,
    // Additional information (account & account key).
    // hash of them is equal to the key below.
    // We export them for now, to make integration with existing systems (like anvil-zksync) easier.
    // In the future, we might want to remove these for performance reasons.
    pub account: B160,
    pub account_key: Bytes32,
}

#[derive(Debug, Clone)]
pub struct BlockOutput {
    pub header: BlockHeader,
    pub tx_results: Vec<TxResult>,
    // TODO: will be returned per tx later
    pub storage_writes: Vec<StorageWrite>,
    pub published_preimages: Vec<(Bytes32, Vec<u8>, PreimageType)>,
    pub pubdata: Vec<u8>,
}

impl From<&GenericEventContent<MAX_EVENT_TOPICS, EthereumIOTypesConfig>> for Log {
    fn from(value: &GenericEventContent<MAX_EVENT_TOPICS, EthereumIOTypesConfig>) -> Self {
        Self {
            address: value.address,
            topics: value.topics.clone(),
            data: value.data.as_slice().to_vec(),
        }
    }
}

impl From<(B160, Bytes32, Bytes32)> for StorageWrite {
    fn from(value: (B160, Bytes32, Bytes32)) -> Self {
        let flat_key = derive_flat_storage_key(&value.0, &value.1);
        Self {
            key: flat_key,
            value: value.2,
            account: value.0,
            account_key: value.1,
        }
    }
}

impl<TR: TxResultCallback> From<ForwardRunningResultKeeper<TR>> for BlockOutput {
    fn from(value: ForwardRunningResultKeeper<TR>) -> Self {
        let ForwardRunningResultKeeper {
            block_header,
            events,
            logs,
            storage_writes,
            tx_results,
            new_preimages,
            pubdata,
            ..
        } = value;

        let tx_results = tx_results
            .into_iter()
            .enumerate()
            .map(|(tx_number, result)| {
                result.map(|output| {
                    let execution_result = if output.status {
                        ExecutionResult::Success(if output.contract_address.is_some() {
                            ExecutionOutput::Create(output.output, output.contract_address.unwrap())
                        } else {
                            ExecutionOutput::Call(output.output)
                        })
                    } else {
                        ExecutionResult::Revert(output.output)
                    };
                    TxOutput {
                        gas_used: output.gas_used,
                        gas_refunded: output.gas_refunded,

                        computational_native_used: output.computational_native_used,

                        pubdata_used: output.pubdata_used,
                        contract_address: output.contract_address,
                        logs: events
                            .iter()
                            .filter_map(|e| {
                                if e.tx_number == tx_number as u32 {
                                    Some(e.into())
                                } else {
                                    None
                                }
                            })
                            .collect(),
                        l2_to_l1_logs: logs
                            .iter()
                            .filter_map(|m| {
                                if m.tx_number == tx_number as u32 {
                                    Some(m.into())
                                } else {
                                    None
                                }
                            })
                            .collect(),
                        execution_result,
                        storage_writes: vec![],
                    }
                })
            })
            .collect();

        let storage_writes = storage_writes.into_iter().map(|s| s.into()).collect();

        Self {
            header: block_header.unwrap(),
            tx_results,
            storage_writes,
            published_preimages: new_preimages,
            pubdata,
        }
    }
}

#[allow(dead_code)]
pub type BatchResult = Result<BlockOutput, InternalError>;
