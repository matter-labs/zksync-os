//! Storage of L2->L1 logs.
//! There are two kinds of such logs:
//! - user messages (sent via l1 messenger system hook).
//! - l1 -> l2 txs logs, to prove execution result on l1.
use crate::system::IOResultKeeper;
use crate::{
    memory::stack_trait::{StackCtor, StackCtorConst},
    system::errors::SystemError,
    types_config::{EthereumIOTypesConfig, SystemIOTypesConfig},
    utils::{Bytes32, UsizeAlignedByteBox},
};
use alloc::alloc::Global;
use core::alloc::Allocator;
use crypto::MiniDigest;
use ruint::aliases::B160;
use u256::U256;

use super::history_list::HistoryList;

const L2_TO_L1_LOG_SERIALIZE_SIZE: usize = 88;

///
/// L2 to l1 log structure, used for merkle tree leaves.
/// This structure holds both kinds of logs (user messages
/// and l1 -> l2 tx logs).
///
#[derive(Default, Debug, Clone)]
pub struct L2ToL1Log {
    ///
    /// Shard id.
    /// Deprecated, kept for compatibility, always set to 0.
    ///
    l2_shard_id: u8,
    ///
    /// Boolean flag.
    /// Deprecated, kept for compatibility, always set to `true`.
    ///
    is_service: bool,
    ///
    /// The L2 transaction number in a block, in which the log was sent
    ///
    tx_number_in_block: u16,
    ///
    /// The L2 address which sent the log.
    /// For user messages set to `L1Messenger` system hook address,
    /// for l1 -> l2 txs logs - `BootloaderFormalAddress`.
    ///
    sender: B160,
    ///
    /// The 32 bytes of information that was sent in the log.
    /// For user messages used to save message sender address(padded),
    /// for l1 -> l2 txs logs - transaction hash.
    ///
    key: Bytes32,
    ///
    /// The 32 bytes of information that was sent in the log.
    /// For user messages used to save message hash.
    /// for l1 -> l2 txs logs - success flag(padded).
    ///
    value: Bytes32,
}

///
/// Message/log content to be saved in the storage.
///
#[derive(Clone, Debug)]
pub struct GenericLogContent<IOTypes: SystemIOTypesConfig, A: Allocator = Global> {
    pub tx_number: u32,
    pub data: GenericLogContentData<UsizeAlignedByteBox<A>, Bytes32, IOTypes::Address>,
}

///
/// Data stored for a message/log.
/// Generic over data, hash and address type to represent both
/// the data and references to it.
///
#[derive(Clone, Debug)]
pub enum GenericLogContentData<DATA, HASH, ADDRESS> {
    UserMsg(UserMsgData<DATA, HASH, ADDRESS>),
    L1TxLog(L1TxLog<HASH>),
}
///
/// Data stored for a user message.
/// Generic over data, hash and address type to represent both
/// the data and references to it.
///
#[derive(Clone, Debug)]
pub struct UserMsgData<DATA, HASH, ADDRESS> {
    pub address: ADDRESS,
    pub data: DATA,
    pub data_hash: HASH,
}

///
/// Data stored for an l1->l2 tx log.
///
#[derive(Clone, Debug)]
pub struct L1TxLog<HASH> {
    pub tx_hash: HASH,
    pub success: bool,
}

/// Log content reference to be returned from the storage
///
#[derive(Clone, Debug)]
pub struct GenericLogContentWithTxRef<'a, IOTypes: SystemIOTypesConfig> {
    pub tx_number: u32,
    pub data: GenericLogContentData<&'a [u8], &'a Bytes32, &'a IOTypes::Address>,
}

impl<IOTypes: SystemIOTypesConfig, A: Allocator> GenericLogContent<IOTypes, A> {
    fn to_ref<'a>(&'a self) -> GenericLogContentWithTxRef<'a, IOTypes> {
        let data = match &self.data {
            GenericLogContentData::L1TxLog(l) => GenericLogContentData::L1TxLog(L1TxLog {
                tx_hash: &l.tx_hash,
                success: l.success,
            }),
            GenericLogContentData::UserMsg(m) => GenericLogContentData::UserMsg(UserMsgData {
                address: &m.address,
                data: m.data.as_slice(),
                data_hash: &m.data_hash,
            }),
        };
        GenericLogContentWithTxRef {
            tx_number: self.tx_number,
            data,
        }
    }

    pub fn from_ref<'a>(r: GenericLogContentWithTxRef<'a, IOTypes>, allocator: A) -> Self {
        let data = match r.data {
            GenericLogContentData::L1TxLog(l) => GenericLogContentData::L1TxLog(L1TxLog {
                tx_hash: *l.tx_hash,
                success: l.success,
            }),
            GenericLogContentData::UserMsg(m) => GenericLogContentData::UserMsg(UserMsgData {
                address: m.address.clone(),
                data: UsizeAlignedByteBox::from_slice_in(m.data, allocator),
                data_hash: *m.data_hash,
            }),
        };
        GenericLogContent {
            tx_number: r.tx_number,
            data,
        }
    }
}

#[allow(type_alias_bounds)]
pub type LogContent<A: Allocator = Global> = GenericLogContent<EthereumIOTypesConfig, A>;

pub type LogsStorageStackCheck<SCC: const StackCtorConst, A: Allocator> =
    [(); SCC::extra_const_param::<(LogContent<A>, u32), A>()];

pub struct LogsStorage<SC: StackCtor<SCC>, SCC: const StackCtorConst, A: Allocator + Clone = Global>
where
    LogsStorageStackCheck<SCC, A>:,
{
    list: HistoryList<LogContent<A>, u32, SC, SCC, A>,
    _marker: core::marker::PhantomData<A>,
}

impl<SC: StackCtor<SCC>, SCC: const StackCtorConst, A: Allocator + Clone> LogsStorage<SC, SCC, A>
where
    LogsStorageStackCheck<SCC, A>:,
{
    pub fn new_from_parts(allocator: A) -> Self {
        Self {
            list: HistoryList::new(allocator),
            _marker: core::marker::PhantomData,
        }
    }

    pub fn begin_new_tx(&mut self) {}

    #[track_caller]
    pub fn start_frame(&mut self) -> usize {
        self.list.snapshot()
    }

    pub fn push_message(
        &mut self,
        tx_number: u32,
        address: &B160,
        data: UsizeAlignedByteBox<A>,
        data_hash: Bytes32,
    ) -> Result<(), SystemError> {
        // We are publishing message data(4 bytes to encode length) and underlying log
        let total_pubdata = 4 + data.len() + L2_TO_L1_LOG_SERIALIZE_SIZE;
        let total_pubdata = total_pubdata as u32;

        let total_pubdata = self
            .list
            .peek()
            .map_or(total_pubdata, |(_, m)| *m + total_pubdata);

        self.list.push(
            LogContent {
                tx_number,
                data: GenericLogContentData::UserMsg(UserMsgData {
                    address: *address,
                    data,
                    data_hash,
                }),
            },
            total_pubdata,
        );

        Ok(())
    }

    pub fn push_l1_l2_tx_log(
        &mut self,
        tx_number: u32,
        tx_hash: Bytes32,
        success: bool,
    ) -> Result<(), SystemError> {
        let total_pubdata = L2_TO_L1_LOG_SERIALIZE_SIZE;
        let total_pubdata = total_pubdata as u32;

        let total_pubdata = self
            .list
            .peek()
            .map_or(total_pubdata, |(_, m)| *m + total_pubdata);

        self.list.push(
            LogContent {
                tx_number,
                data: GenericLogContentData::L1TxLog(L1TxLog { tx_hash, success }),
            },
            total_pubdata,
        );

        Ok(())
    }

    #[track_caller]
    pub fn finish_frame(&mut self, rollback_handle: Option<usize>) {
        if let Some(x) = rollback_handle {
            self.list.rollback(x);
        }
    }

    pub fn iter_net_diff(&self) -> impl Iterator<Item = &LogContent<A>> {
        self.list.iter()
    }

    pub fn messages_ref_iter(
        &self,
    ) -> impl Iterator<Item = GenericLogContentWithTxRef<EthereumIOTypesConfig>> {
        self.list.iter().map(|message| message.to_ref())
    }

    pub fn apply_l2_to_l1_logs_hashes_to_hasher(&self, hasher: &mut impl MiniDigest) {
        for message in self.list.iter() {
            hasher.update(L2ToL1Log::from(message).hash().as_u8_ref());
        }
    }

    pub fn apply_pubdata(
        &self,
        hasher: &mut impl MiniDigest,
        results_keeper: &mut impl IOResultKeeper<EthereumIOTypesConfig>,
    ) {
        let logs_count = (self.list.len() as u32).to_be_bytes();
        hasher.update(logs_count);
        results_keeper.pubdata(&logs_count);
        let mut messages_count: u32 = 0;
        // First we encode all the L2L1 log information.
        self.list.iter().for_each(|el| {
            if let GenericLogContentData::UserMsg(_) = el.data {
                messages_count += 1;
            }
            let log: L2ToL1Log = el.into();
            log.add_encoding_to_hasher(hasher);
            log.pubdata(results_keeper);
        });
        // Then, we do a second pass to publish messages
        let messages_count = messages_count.to_be_bytes();
        hasher.update(messages_count);
        results_keeper.pubdata(&messages_count);
        self.list.iter().for_each(|el| {
            if let GenericLogContentData::UserMsg(UserMsgData { data, .. }) = &el.data {
                let len = (data.as_slice().len() as u32).to_be_bytes();
                hasher.update(len);
                results_keeper.pubdata(&len);
                hasher.update(data.as_slice());
                results_keeper.pubdata(data.as_slice());
            }
        })
    }
}

impl L2ToL1Log {
    ///
    /// Encode L2 to l1 log using solidity abi packed encoding.
    ///
    pub fn encode(&self) -> [u8; L2_TO_L1_LOG_SERIALIZE_SIZE] {
        let mut buffer = [0u8; L2_TO_L1_LOG_SERIALIZE_SIZE];
        buffer[0..1].copy_from_slice(&[self.l2_shard_id]);
        buffer[1..2].copy_from_slice(&[if self.is_service { 1 } else { 0 }]);
        buffer[2..4].copy_from_slice(&self.tx_number_in_block.to_be_bytes());
        buffer[4..24].copy_from_slice(&self.sender.to_be_bytes::<20>());
        buffer[24..56].copy_from_slice(self.key.as_u8_ref());
        buffer[56..88].copy_from_slice(self.value.as_u8_ref());
        buffer
    }

    ///
    /// Returns keccak hash of the l2 to l1 log solidity abi packed encoding.
    /// In fact, packed abi encoding in this case just equals to concatenation of all the fields big-endian representations.
    ///
    fn hash(&self) -> Bytes32 {
        let mut hasher = crypto::sha3::Keccak256::new();
        self.add_encoding_to_hasher(&mut hasher);
        hasher.finalize().into()
    }

    ///
    /// Adds the packed abi encoding of the log to the hasher.
    ///
    fn add_encoding_to_hasher(&self, hasher: &mut impl MiniDigest) {
        hasher.update([self.l2_shard_id]);
        hasher.update([if self.is_service { 1 } else { 0 }]);
        hasher.update(self.tx_number_in_block.to_be_bytes());
        hasher.update(self.sender.to_be_bytes::<20>());
        hasher.update(self.key.as_u8_ref());
        hasher.update(self.value.as_u8_ref());
    }

    ///
    /// Adds the packed abi encoding of the log to the pubdata.
    ///
    fn pubdata(&self, result_keeper: &mut impl IOResultKeeper<EthereumIOTypesConfig>) {
        result_keeper.pubdata(&[self.l2_shard_id]);
        result_keeper.pubdata(&[if self.is_service { 1 } else { 0 }]);
        result_keeper.pubdata(&self.tx_number_in_block.to_be_bytes());
        result_keeper.pubdata(&self.sender.to_be_bytes::<20>());
        result_keeper.pubdata(self.key.as_u8_ref());
        result_keeper.pubdata(self.value.as_u8_ref());
    }
}

impl<A: Allocator> From<&LogContent<A>> for L2ToL1Log {
    fn from(m: &LogContent<A>) -> Self {
        let (sender, key, value) = match m.data {
            GenericLogContentData::UserMsg(UserMsgData {
                address, data_hash, ..
            }) => (
                // TODO: move into const
                B160::from_limbs([0x8008, 0, 0]),
                address.into(),
                data_hash,
            ),
            GenericLogContentData::L1TxLog(L1TxLog { tx_hash, success }) => {
                let value = if success {
                    &U256::from(1u64)
                } else {
                    &U256::zero()
                };
                (
                    // TODO: move into const
                    B160::from_limbs([0x8001, 0, 0]),
                    tx_hash,
                    Bytes32::from_u256_be(value),
                )
            }
        };
        Self {
            l2_shard_id: 0,
            is_service: true,
            tx_number_in_block: m.tx_number as u16,
            sender,
            key,
            value,
        }
    }
}
