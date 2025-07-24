use crate::run::TxResultCallback;
use basic_bootloader::bootloader::block_header::BlockHeader;
use basic_bootloader::bootloader::result_keeper::{ResultKeeperExt, TxProcessingOutput};
use ruint::aliases::B160;
use std::alloc::Global;
use zk_ee::common_structs::{
    GenericEventContent, GenericEventContentWithTxRef, GenericLogContent,
    GenericLogContentWithTxRef, PreimageType,
};
use zk_ee::kv_markers::MAX_EVENT_TOPICS;
use zk_ee::system::IOResultKeeper;
use zk_ee::types_config::EthereumIOTypesConfig;
use zk_ee::utils::{Bytes32, UsizeAlignedByteBox};

#[derive(Debug, Clone)]
pub struct TxProcessingOutputOwned {
    pub status: bool,
    pub output: Vec<u8>,
    pub contract_address: Option<B160>,
    pub gas_used: u64,
    pub gas_refunded: u64,

    pub computational_native_used: u64,

    pub pubdata_used: u64,
}

pub struct ForwardRunningResultKeeper<TR: TxResultCallback> {
    pub block_header: Option<BlockHeader>,
    pub events: Vec<GenericEventContent<MAX_EVENT_TOPICS, EthereumIOTypesConfig>>,
    pub logs: Vec<GenericLogContent<EthereumIOTypesConfig>>,
    pub storage_writes: Vec<(B160, Bytes32, Bytes32)>,
    pub tx_results: Vec<
        Result<TxProcessingOutputOwned, basic_bootloader::bootloader::errors::InvalidTransaction>,
    >,
    pub new_preimages: Vec<(Bytes32, Vec<u8>, PreimageType)>,
    pub pubdata: Vec<u8>,

    pub tx_result_callback: TR,
}

impl<TR: TxResultCallback> ForwardRunningResultKeeper<TR> {
    pub fn new(tx_result_callback: TR) -> Self {
        Self {
            block_header: None,
            events: vec![],
            logs: vec![],
            storage_writes: vec![],
            tx_results: vec![],
            new_preimages: vec![],
            pubdata: vec![],
            tx_result_callback,
        }
    }
}

impl<TR: TxResultCallback> IOResultKeeper<EthereumIOTypesConfig>
    for ForwardRunningResultKeeper<TR>
{
    fn events<'a>(
        &mut self,
        iter: impl Iterator<
            Item = GenericEventContentWithTxRef<'a, { MAX_EVENT_TOPICS }, EthereumIOTypesConfig>,
        >,
    ) {
        self.events = iter
            .map(|e| GenericEventContent {
                tx_number: e.tx_number,
                address: *e.address,
                topics: e.topics.clone(),
                data: UsizeAlignedByteBox::from_slice_in(e.data, Global),
            })
            .collect();
    }

    fn logs<'a>(
        &mut self,
        iter: impl Iterator<Item = GenericLogContentWithTxRef<'a, EthereumIOTypesConfig>>,
    ) {
        self.logs = iter
            .map(|m| GenericLogContent::from_ref(m, Global))
            .collect();
    }

    fn storage_diffs(&mut self, iter: impl Iterator<Item = (B160, Bytes32, Bytes32)>) {
        self.storage_writes = iter.collect();
    }

    fn new_preimages<'a>(
        &mut self,
        iter: impl Iterator<Item = (&'a Bytes32, &'a [u8], PreimageType)>,
    ) {
        self.new_preimages = iter
            .map(|(hash, preimage, preimage_type)| (*hash, preimage.to_vec(), preimage_type))
            .collect();
    }

    fn pubdata(&mut self, value: &[u8]) {
        self.pubdata.extend_from_slice(value);
    }
}

impl<TR: TxResultCallback> ResultKeeperExt for ForwardRunningResultKeeper<TR> {
    fn tx_processed(
        &mut self,
        tx_result: Result<
            TxProcessingOutput,
            basic_bootloader::bootloader::errors::InvalidTransaction,
        >,
    ) {
        let owned_result = tx_result.map(|output| TxProcessingOutputOwned {
            status: output.status,
            output: output.output.to_vec(),
            contract_address: output.contract_address,
            gas_used: output.gas_used,
            gas_refunded: output.gas_refunded,

            computational_native_used: output.computational_native_used,

            pubdata_used: output.pubdata_used,
        });
        self.tx_result_callback.tx_executed(owned_result.clone());
        self.tx_results.push(owned_result);
    }

    fn block_sealed(&mut self, block_header: BlockHeader) {
        self.block_header = Some(block_header);
    }

    fn get_gas_used(&self) -> u64 {
        self.tx_results
            .iter()
            .map(|r| r.as_ref().map_or(0, |r| r.gas_used))
            .sum()
    }
}
