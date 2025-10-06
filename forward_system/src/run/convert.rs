use alloy::consensus::{Header, Sealed};
use alloy::primitives::Log;
use basic_bootloader::bootloader::block_header::BlockHeader;
use ruint::aliases::B160;
use zk_ee::common_structs::GenericEventContent;
use zk_ee::system::metadata::{BlockHashes, BlockMetadataFromOracle};
use zk_ee::types_config::EthereumIOTypesConfig;
use zksync_os_interface::error::InvalidTransaction;
use zksync_os_interface::types::{BlockContext, L2ToL1Log};

pub trait FromInterface<T> {
    fn from_interface(value: T) -> Self;
}

pub trait IntoInterface<T> {
    fn into_interface(self) -> T;
}

impl FromInterface<BlockContext> for BlockMetadataFromOracle {
    fn from_interface(value: BlockContext) -> Self {
        BlockMetadataFromOracle {
            chain_id: value.chain_id,
            block_number: value.block_number,
            block_hashes: BlockHashes(value.block_hashes.0),
            timestamp: value.timestamp,
            eip1559_basefee: value.eip1559_basefee,
            pubdata_price: value.pubdata_price,
            native_price: value.native_price,
            coinbase: B160::from_be_bytes(value.coinbase.0 .0),
            gas_limit: value.gas_limit,
            pubdata_limit: value.pubdata_limit,
            mix_hash: value.mix_hash,
        }
    }
}

impl IntoInterface<InvalidTransaction>
    for basic_bootloader::bootloader::errors::InvalidTransaction
{
    fn into_interface(self) -> InvalidTransaction {
        match self {
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
            basic_bootloader::bootloader::errors::InvalidTransaction::ReceivedInsufficientFees { received, required } => { InvalidTransaction::ReceivedInsufficientFees { received, required } }
            basic_bootloader::bootloader::errors::InvalidTransaction::InvalidMagic => { InvalidTransaction::InvalidMagic }
            basic_bootloader::bootloader::errors::InvalidTransaction::InvalidReturndataLength => { InvalidTransaction::InvalidReturndataLength }
            basic_bootloader::bootloader::errors::InvalidTransaction::OutOfGasDuringValidation => { InvalidTransaction::OutOfGasDuringValidation }
            basic_bootloader::bootloader::errors::InvalidTransaction::OutOfNativeResourcesDuringValidation => { InvalidTransaction::OutOfNativeResourcesDuringValidation }
            basic_bootloader::bootloader::errors::InvalidTransaction::NonceUsedAlready => { InvalidTransaction::NonceUsedAlready }
            basic_bootloader::bootloader::errors::InvalidTransaction::NonceNotIncreased => { InvalidTransaction::NonceNotIncreased }
            basic_bootloader::bootloader::errors::InvalidTransaction::BlockGasLimitReached => { InvalidTransaction::BlockGasLimitReached }
            basic_bootloader::bootloader::errors::InvalidTransaction::BlockNativeLimitReached => { InvalidTransaction::BlockNativeLimitReached }
            basic_bootloader::bootloader::errors::InvalidTransaction::BlockPubdataLimitReached => { InvalidTransaction::BlockPubdataLimitReached }
            basic_bootloader::bootloader::errors::InvalidTransaction::BlockL2ToL1LogsLimitReached => { InvalidTransaction::BlockL2ToL1LogsLimitReached }
        }
    }
}

impl IntoInterface<Log> for &GenericEventContent<4, EthereumIOTypesConfig> {
    fn into_interface(self) -> Log {
        Log::new(
            self.address.to_be_bytes().into(),
            self.topics.iter().map(|t| t.as_u8_array().into()).collect(),
            self.data.as_slice().to_vec().into(),
        )
        .unwrap()
    }
}

impl IntoInterface<L2ToL1Log> for zk_ee::common_structs::L2ToL1Log {
    fn into_interface(self) -> L2ToL1Log {
        L2ToL1Log {
            l2_shard_id: self.l2_shard_id,
            is_service: self.is_service,
            tx_number_in_block: self.tx_number_in_block,
            sender: self.sender.to_be_bytes().into(),
            key: self.key.as_u8_array().into(),
            value: self.value.as_u8_array().into(),
        }
    }
}

impl IntoInterface<Sealed<Header>> for BlockHeader {
    fn into_interface(self) -> Sealed<Header> {
        let hash = self.hash();
        let header = Header {
            parent_hash: self.parent_hash.as_u8_array().into(),
            ommers_hash: self.ommers_hash.as_u8_array().into(),
            beneficiary: self.beneficiary.to_be_bytes().into(),
            state_root: self.state_root.as_u8_array().into(),
            transactions_root: self.transactions_root.as_u8_array().into(),
            receipts_root: self.receipts_root.as_u8_array().into(),
            logs_bloom: self.logs_bloom.into(),
            difficulty: self.difficulty,
            number: self.number,
            gas_limit: self.gas_limit,
            gas_used: self.gas_used,
            timestamp: self.timestamp,
            extra_data: self.extra_data.to_vec().into(),
            mix_hash: self.mix_hash.as_u8_array().into(),
            nonce: self.nonce.into(),
            base_fee_per_gas: Some(self.base_fee_per_gas),
            withdrawals_root: None,
            blob_gas_used: None,
            excess_blob_gas: None,
            parent_beacon_block_root: None,
            requests_hash: None,
        };
        Sealed::new_unchecked(header, hash.into())
    }
}
