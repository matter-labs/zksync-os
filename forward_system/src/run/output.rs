// Includes code adapted from https://github.com/bluealloy/revm/blob/fb80087996dfbd6c74eaf308538cfa707ecb763c/crates/context/interface/src/result.rs

use crate::run::convert::IntoInterface;
use crate::run::result_keeper::ForwardRunningResultKeeper;
use crate::run::TxResultCallback;
use alloy::primitives::Address;
pub use basic_bootloader::bootloader::block_header::BlockHeader;
use ruint::aliases::B160;
use std::collections::HashMap;
use zk_ee::common_structs::{derive_flat_storage_key, GenericLogContent, PreimageType};
use zk_ee::system::errors::internal::InternalError;
use zk_ee::utils::Bytes32;
use zksync_os_interface::error::InvalidTransaction;
use zksync_os_interface::types::{AccountDiff, ExecutionOutput, ExecutionResult, StorageWrite};

// Use interface type as the direct place-in, can be changed in the future.
pub use zksync_os_interface::types::TxOutput;

// Use interface type as the direct place-in, can be changed in the future.
use basic_system::system_implementation::flat_storage_model::{
    AccountProperties, ACCOUNT_PROPERTIES_STORAGE_ADDRESS,
};
use zk_ee::types_config::EthereumIOTypesConfig;
pub use zksync_os_interface::types::BlockOutput;
use zksync_os_interface::types::L2ToL1LogWithPreimage;

pub type TxResult = Result<TxOutput, InvalidTransaction>;

/// Extension trait to create `StorageWrite` from components.
trait StorageWriteExt {
    #[allow(clippy::new_ret_no_self)]
    fn new(address: B160, key: Bytes32, value: Bytes32) -> StorageWrite;
}

impl StorageWriteExt for StorageWrite {
    fn new(address: B160, key: Bytes32, value: Bytes32) -> StorageWrite {
        let flat_key = derive_flat_storage_key(&address, &key);
        StorageWrite {
            key: flat_key.as_u8_array().into(),
            value: value.as_u8_array().into(),
            account: address.to_be_bytes().into(),
            account_key: key.as_u8_array().into(),
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

        let mut block_computaional_native_used = 0;

        let tx_results = tx_results
            .into_iter()
            .enumerate()
            .map(|(tx_number, result)| {
                result
                    .map(|output| {
                        let execution_result = if output.status {
                            ExecutionResult::Success(if output.contract_address.is_some() {
                                ExecutionOutput::Create(
                                    output.output,
                                    output.contract_address.unwrap(),
                                )
                            } else {
                                ExecutionOutput::Call(output.output)
                            })
                        } else {
                            ExecutionResult::Revert(output.output)
                        };
                        block_computaional_native_used += output.computational_native_used;
                        TxOutput {
                            gas_used: output.gas_used,
                            gas_refunded: output.gas_refunded,
                            native_used: output.native_used,
                            computational_native_used: output.computational_native_used,
                            pubdata_used: output.pubdata_used,
                            contract_address: output.contract_address,
                            logs: events
                                .iter()
                                .filter_map(|e| {
                                    if e.tx_number == tx_number as u32 {
                                        Some(e.into_interface())
                                    } else {
                                        None
                                    }
                                })
                                .collect(),
                            l2_to_l1_logs: logs
                                .iter()
                                .filter_map(|m| {
                                    if m.tx_number == tx_number as u32 {
                                        Some(parse_l2_to_l1_log_w_preimages(m))
                                    } else {
                                        None
                                    }
                                })
                                .collect(),
                            execution_result,
                            storage_writes: vec![],
                        }
                    })
                    .map_err(IntoInterface::into_interface)
            })
            .collect();

        let account_diffs = extract_account_diffs(&storage_writes, &new_preimages);
        let storage_writes = storage_writes
            .into_iter()
            .map(|(address, key, value)| StorageWrite::new(address, key, value))
            .collect();
        let published_preimages = new_preimages
            .into_iter()
            .map(|(hash, data, _)| (hash.as_u8_array().into(), data))
            .collect();

        Self {
            header: block_header.unwrap().into_interface(),
            tx_results,
            storage_writes,
            account_diffs,
            published_preimages,
            pubdata,
            computaional_native_used: block_computaional_native_used,
        }
    }
}

/// Extract account diffs from a BlockOutput.
///
/// This method processes the published preimages and storage writes to extract
/// accounts that were updated during block execution.
pub fn extract_account_diffs(
    storage_writes: &[(B160, Bytes32, Bytes32)],
    published_preimages: &[(Bytes32, Vec<u8>, PreimageType)],
) -> Vec<AccountDiff> {
    // First, collect all account properties from published preimages
    let mut account_properties_preimages = HashMap::new();
    for (hash, preimage, preimage_type) in published_preimages {
        match preimage_type {
            PreimageType::Bytecode => {}
            PreimageType::AccountData => {
                account_properties_preimages.insert(
                    *hash,
                    AccountProperties::decode(
                        &preimage
                            .clone()
                            .try_into()
                            .expect("Preimage should be exactly 124 bytes"),
                    ),
                );
            }
        }
    }

    // Then, map storage writes to account addresses
    let mut result = Vec::new();
    for (address, key, value) in storage_writes {
        if address == &ACCOUNT_PROPERTIES_STORAGE_ADDRESS {
            if let Some(properties) = account_properties_preimages.get(value) {
                result.push(AccountDiff {
                    address: Address::from_slice(&key.as_u8_array_ref()[12..]),
                    nonce: properties.nonce,
                    balance: properties.balance,
                    bytecode_hash: properties.bytecode_hash.as_u8_array().into(),
                });
            } else {
                unreachable!();
            }
        }
    }

    result
}

fn parse_l2_to_l1_log_w_preimages(
    value: &GenericLogContent<EthereumIOTypesConfig>,
) -> L2ToL1LogWithPreimage {
    use zk_ee::common_structs::GenericLogContentData;
    use zk_ee::common_structs::UserMsgData;
    let preimage = match &value.data {
        GenericLogContentData::UserMsg(UserMsgData { data, .. }) => Some(data.as_slice().to_vec()),
        GenericLogContentData::L1TxLog(_) => None,
    };
    let log: zk_ee::common_structs::L2ToL1Log = value.into();
    let log = log.into_interface();
    L2ToL1LogWithPreimage { log, preimage }
}

#[allow(dead_code)]
pub type BatchResult = Result<BlockOutput, InternalError>;
