// Includes code adapted from https://github.com/bluealloy/revm/blob/fb80087996dfbd6c74eaf308538cfa707ecb763c/crates/context/interface/src/result.rs

use crate::run::result_keeper::ForwardRunningResultKeeper;
use crate::run::TxResultCallback;
use alloy::consensus::{Header, Sealed};
use alloy::primitives::{Address, Log};
pub use basic_bootloader::bootloader::block_header::BlockHeader;
use ruint::aliases::B160;
use std::collections::HashMap;
use zk_ee::common_structs::{
    derive_flat_storage_key, GenericEventContent, GenericLogContent, PreimageType,
};
use zk_ee::system::errors::internal::InternalError;
use zk_ee::utils::Bytes32;
use zksync_os_interface::error::{AAMethod, InvalidTransaction};
use zksync_os_interface::types::{AccountDiff, ExecutionOutput, ExecutionResult, StorageWrite};

// Use interface type as the direct place-in, can be changed in the future.
pub use zksync_os_interface::types::TxOutput;

// Use interface type as the direct place-in, can be changed in the future.
use basic_system::system_implementation::flat_storage_model::{
    AccountProperties, ACCOUNT_PROPERTIES_STORAGE_ADDRESS,
};
use zk_ee::types_config::EthereumIOTypesConfig;
pub use zksync_os_interface::types::BlockOutput;
use zksync_os_interface::types::{L2ToL1Log, L2ToL1LogWithPreimage};

pub type TxResult = Result<TxOutput, InvalidTransaction>;

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
                                        Some(convert_log(e))
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
                    .map_err(convert_error)
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
            header: convert_header(block_header.unwrap()),
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
                let flat_key = derive_flat_storage_key(address, key);
                result.push(AccountDiff {
                    address: Address::from_slice(&flat_key.as_u8_array_ref()[12..]),
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

fn convert_header(value: BlockHeader) -> Sealed<Header> {
    let hash = value.hash();
    let header = Header {
        parent_hash: value.parent_hash.as_u8_array().into(),
        ommers_hash: value.ommers_hash.as_u8_array().into(),
        beneficiary: value.beneficiary.to_be_bytes().into(),
        state_root: value.state_root.as_u8_array().into(),
        transactions_root: value.transactions_root.as_u8_array().into(),
        receipts_root: value.receipts_root.as_u8_array().into(),
        logs_bloom: value.logs_bloom.into(),
        difficulty: value.difficulty,
        number: value.number,
        gas_limit: value.gas_limit,
        gas_used: value.gas_used,
        timestamp: value.timestamp,
        extra_data: value.extra_data.to_vec().into(),
        mix_hash: value.mix_hash.as_u8_array().into(),
        nonce: value.nonce.into(),
        base_fee_per_gas: Some(value.base_fee_per_gas),
        withdrawals_root: None,
        blob_gas_used: None,
        excess_blob_gas: None,
        parent_beacon_block_root: None,
        requests_hash: None,
    };
    Sealed::new_unchecked(header, hash.into())
}

fn convert_log(value: &GenericEventContent<4, EthereumIOTypesConfig>) -> Log {
    Log::new(
        value.address.to_be_bytes().into(),
        value
            .topics
            .iter()
            .map(|t| t.as_u8_array().into())
            .collect(),
        value.data.as_slice().to_vec().into(),
    )
    .unwrap()
}

fn convert_l2_to_l1_log(value: zk_ee::common_structs::L2ToL1Log) -> L2ToL1Log {
    L2ToL1Log {
        l2_shard_id: value.l2_shard_id,
        is_service: value.is_service,
        tx_number_in_block: value.tx_number_in_block,
        sender: value.sender.to_be_bytes().into(),
        key: value.key.as_u8_array().into(),
        value: value.value.as_u8_array().into(),
    }
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
    let log = convert_l2_to_l1_log(value.into());
    L2ToL1LogWithPreimage { log, preimage }
}

pub(crate) fn convert_error(
    value: basic_bootloader::bootloader::errors::InvalidTransaction,
) -> InvalidTransaction {
    match value {
        basic_bootloader::bootloader::errors::InvalidTransaction::InvalidEncoding => { InvalidTransaction::InvalidEncoding }
        basic_bootloader::bootloader::errors::InvalidTransaction::InvalidStructure => { InvalidTransaction::InvalidStructure }
        basic_bootloader::bootloader::errors::InvalidTransaction::PriorityFeeGreaterThanMaxFee => { InvalidTransaction::PriorityFeeGreaterThanMaxFee }
        basic_bootloader::bootloader::errors::InvalidTransaction::BaseFeeGreaterThanMaxFee => { InvalidTransaction::BaseFeeGreaterThanMaxFee }
        basic_bootloader::bootloader::errors::InvalidTransaction::GasPriceLessThanBasefee => { InvalidTransaction::GasPriceLessThanBasefee }
        basic_bootloader::bootloader::errors::InvalidTransaction::CallerGasLimitMoreThanBlock => { InvalidTransaction::CallerGasLimitMoreThanBlock }
        basic_bootloader::bootloader::errors::InvalidTransaction::CallGasCostMoreThanGasLimit => { InvalidTransaction::CallGasCostMoreThanGasLimit }
        basic_bootloader::bootloader::errors::InvalidTransaction::RejectCallerWithCode => { InvalidTransaction::RejectCallerWithCode }
        basic_bootloader::bootloader::errors::InvalidTransaction::LackOfFundForMaxFee { fee, balance } => { InvalidTransaction::LackOfFundForMaxFee { fee, balance } }
        basic_bootloader::bootloader::errors::InvalidTransaction::OverflowPaymentInTransaction => { InvalidTransaction::OverflowPaymentInTransaction }
        basic_bootloader::bootloader::errors::InvalidTransaction::NonceOverflowInTransaction => { InvalidTransaction::NonceOverflowInTransaction }
        basic_bootloader::bootloader::errors::InvalidTransaction::NonceTooHigh { tx, state } => { InvalidTransaction::NonceTooHigh { tx, state } }
        basic_bootloader::bootloader::errors::InvalidTransaction::NonceTooLow { tx, state } => { InvalidTransaction::NonceTooLow { tx, state } }
        basic_bootloader::bootloader::errors::InvalidTransaction::MalleableSignature => { InvalidTransaction::MalleableSignature }
        basic_bootloader::bootloader::errors::InvalidTransaction::IncorrectFrom { tx, recovered } => { InvalidTransaction::IncorrectFrom { tx: tx.to_be_bytes().into(), recovered: recovered.to_be_bytes().into() } }
        basic_bootloader::bootloader::errors::InvalidTransaction::CreateInitCodeSizeLimit => { InvalidTransaction::CreateInitCodeSizeLimit }
        basic_bootloader::bootloader::errors::InvalidTransaction::InvalidChainId => { InvalidTransaction::InvalidChainId }
        basic_bootloader::bootloader::errors::InvalidTransaction::AccessListNotSupported => { InvalidTransaction::AccessListNotSupported }
        basic_bootloader::bootloader::errors::InvalidTransaction::GasPerPubdataTooHigh => { InvalidTransaction::GasPerPubdataTooHigh }
        basic_bootloader::bootloader::errors::InvalidTransaction::BlockGasLimitTooHigh => { InvalidTransaction::BlockGasLimitTooHigh }
        basic_bootloader::bootloader::errors::InvalidTransaction::UpgradeTxNotFirst => { InvalidTransaction::UpgradeTxNotFirst }
        basic_bootloader::bootloader::errors::InvalidTransaction::Revert { method, output } => { InvalidTransaction::Revert { method: match method {
            basic_bootloader::bootloader::errors::AAMethod::AccountValidate => { AAMethod::AccountValidate}
            basic_bootloader::bootloader::errors::AAMethod::AccountPayForTransaction => { AAMethod::AccountPayForTransaction}
            basic_bootloader::bootloader::errors::AAMethod::AccountPrePaymaster => {AAMethod::AccountPrePaymaster}
            basic_bootloader::bootloader::errors::AAMethod::PaymasterValidateAndPay => {AAMethod::PaymasterValidateAndPay}
        }, output } }
        basic_bootloader::bootloader::errors::InvalidTransaction::ReceivedInsufficientFees { received, required } => { InvalidTransaction::ReceivedInsufficientFees { received, required } }
        basic_bootloader::bootloader::errors::InvalidTransaction::InvalidMagic => { InvalidTransaction::InvalidMagic }
        basic_bootloader::bootloader::errors::InvalidTransaction::InvalidReturndataLength => { InvalidTransaction::InvalidReturndataLength }
        basic_bootloader::bootloader::errors::InvalidTransaction::OutOfGasDuringValidation => { InvalidTransaction::OutOfGasDuringValidation }
        basic_bootloader::bootloader::errors::InvalidTransaction::OutOfNativeResourcesDuringValidation => { InvalidTransaction::OutOfNativeResourcesDuringValidation }
        basic_bootloader::bootloader::errors::InvalidTransaction::NonceUsedAlready => { InvalidTransaction::NonceUsedAlready }
        basic_bootloader::bootloader::errors::InvalidTransaction::NonceNotIncreased => { InvalidTransaction::NonceNotIncreased }
        basic_bootloader::bootloader::errors::InvalidTransaction::PaymasterReturnDataTooShort => { InvalidTransaction::PaymasterReturnDataTooShort }
        basic_bootloader::bootloader::errors::InvalidTransaction::PaymasterInvalidMagic => { InvalidTransaction::PaymasterInvalidMagic }
        basic_bootloader::bootloader::errors::InvalidTransaction::PaymasterContextInvalid => { InvalidTransaction::PaymasterContextInvalid }
        basic_bootloader::bootloader::errors::InvalidTransaction::PaymasterContextOffsetTooLong => { InvalidTransaction::PaymasterContextOffsetTooLong }
        basic_bootloader::bootloader::errors::InvalidTransaction::BlockGasLimitReached => { InvalidTransaction::BlockGasLimitReached }
        basic_bootloader::bootloader::errors::InvalidTransaction::BlockNativeLimitReached => { InvalidTransaction::BlockNativeLimitReached }
        basic_bootloader::bootloader::errors::InvalidTransaction::BlockPubdataLimitReached => { InvalidTransaction::BlockPubdataLimitReached }
        basic_bootloader::bootloader::errors::InvalidTransaction::BlockL2ToL1LogsLimitReached => { InvalidTransaction::BlockL2ToL1LogsLimitReached }
    }
}

#[allow(dead_code)]
pub type BatchResult = Result<BlockOutput, InternalError>;
