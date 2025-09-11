//! Storage of L2->L1 logs.
//! There are two kinds of such logs:
//! - user messages (sent via l1 messenger system hook).
//! - l1 -> l2 txs logs, to prove execution result on l1.
use super::history_list::HistoryList;
use crate::internal_error;
use crate::system::errors::internal::InternalError;
use crate::system::IOResultKeeper;
use crate::{
    memory::stack_trait::StackCtor,
    system::errors::system::SystemError,
    types_config::{EthereumIOTypesConfig, SystemIOTypesConfig},
    utils::{Bytes32, UsizeAlignedByteBox},
};
use alloc::alloc::Global;
use core::alloc::Allocator;
use crypto::sha3::Keccak256;
use crypto::MiniDigest;
use ruint::aliases::B160;
use ruint::aliases::U256;

pub const L2_TO_L1_LOG_SERIALIZE_SIZE: usize = 88;
// Taken from the size of the Merkle tree.
pub const MAX_NUMBER_OF_LOGS: u64 = 16_384;

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
    pub l2_shard_id: u8,
    ///
    /// Boolean flag.
    /// Deprecated, kept for compatibility, always set to `true`.
    ///
    pub is_service: bool,
    ///
    /// The L2 transaction number in a block, in which the log was sent
    ///
    pub tx_number_in_block: u16,
    ///
    /// The L2 address which sent the log.
    /// For user messages set to `L1Messenger` system hook address,
    /// for l1 -> l2 txs logs - `BootloaderFormalAddress`.
    ///
    pub sender: B160,
    ///
    /// The 32 bytes of information that was sent in the log.
    /// For user messages used to save message sender address(padded),
    /// for l1 -> l2 txs logs - transaction hash.
    ///
    pub key: Bytes32,
    ///
    /// The 32 bytes of information that was sent in the log.
    /// For user messages used to save message hash.
    /// for l1 -> l2 txs logs - success flag(padded).
    ///
    pub value: Bytes32,
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
                address: *m.address,
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

pub struct LogsStorage<SC: StackCtor<N>, const N: usize, A: Allocator + Clone = Global> {
    pub list: HistoryList<LogContent<A>, u32, SC, N, A>,
    pubdata_used_by_committed_logs: u32,
    _marker: core::marker::PhantomData<A>,
}

impl<SC: StackCtor<N>, const N: usize, A: Allocator + Clone + Default> LogsStorage<SC, N, A> {
    pub fn new_from_parts(allocator: A) -> Self {
        Self {
            list: HistoryList::new(allocator),
            pubdata_used_by_committed_logs: 0,
            _marker: core::marker::PhantomData,
        }
    }

    pub fn begin_new_tx(&mut self) {
        self.pubdata_used_by_committed_logs = self.list.top().map_or(0, |(_, m)| *m);
    }

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
        // TODO: double check that we should have 4 here
        let total_pubdata = 4 + data.len() + L2_TO_L1_LOG_SERIALIZE_SIZE;
        let total_pubdata = total_pubdata as u32;

        let total_pubdata = self
            .list
            .top()
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
            .top()
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

    pub fn len(&self) -> u64 {
        self.list.len() as u64
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
        &'_ self,
    ) -> impl ExactSizeIterator<Item = GenericLogContentWithTxRef<'_, EthereumIOTypesConfig>> + Clone
    {
        self.list.iter().map(|message| message.to_ref())
    }

    pub fn calculate_pubdata_used_by_tx(&self) -> Result<u32, InternalError> {
        let total_pubdata_used = self.list.top().map_or(0, |(_, m)| *m);

        if total_pubdata_used < self.pubdata_used_by_committed_logs {
            Err(internal_error!(
                "Pubdata used by logs unexpectedly decreased"
            ))
        } else {
            Ok(total_pubdata_used - self.pubdata_used_by_committed_logs)
        }
    }

    pub fn apply_logs_to_pubdata_and_record_log_hashes<LFN: FnMut(&Bytes32)>(
        &self,
        result_keeper: &mut impl IOResultKeeper<EthereumIOTypesConfig>,
        pubdata_hasher: &mut impl MiniDigest,
        mut log_hasher_fn: Option<LFN>,
    ) {
        let logs_count = (self.list.len() as u32).to_be_bytes();
        pubdata_hasher.update(logs_count);
        result_keeper.pubdata(&logs_count);

        let mut messages_count: u32 = 0;
        // First we encode all the L2L1 log information.

        let mut log_inner_hasher = crypto::sha3::Keccak256::new();
        for log in self.list.iter() {
            if let GenericLogContentData::UserMsg(..) = &log.data {
                messages_count += 1;
            }
            let log = L2ToL1Log::from(log);
            if let Some(log_hasher_fn) = log_hasher_fn.as_mut() {
                (log_hasher_fn)(&log.hash(&mut log_inner_hasher));
            }
            log.add_encoding_to_hasher(pubdata_hasher);
            log.pubdata(result_keeper);
        }

        // Then, we do a second pass to publish messages
        let messages_count = messages_count.to_be_bytes();
        pubdata_hasher.update(messages_count);
        result_keeper.pubdata(&messages_count);
        self.list.iter().for_each(|el| {
            if let GenericLogContentData::UserMsg(UserMsgData { data, .. }) = &el.data {
                let len = (data.len() as u32).to_be_bytes();
                pubdata_hasher.update(len);
                result_keeper.pubdata(&len);
                pubdata_hasher.update(data.as_slice());
                result_keeper.pubdata(data.as_slice());
            }
        })
    }

    pub fn apply_logs_to_pubdata(
        &self,
        results_keeper: &mut impl IOResultKeeper<EthereumIOTypesConfig>,
        pubdata_hasher: &mut impl MiniDigest,
    ) {
        let lfn = |_: &Bytes32| {};
        self.apply_logs_to_pubdata_and_record_log_hashes(
            results_keeper,
            pubdata_hasher,
            Some(lfn).take(), // otherwise type can not be deduced
        )
    }

    // we use it for tests to generate single block batches
    ///
    /// Calculate l2 logs merkle tree root.
    ///
    pub fn tree_root(&self) -> Bytes32 {
        const TREE_HEIGHT: usize = 14;
        // keccak256([0; L2_TO_L1_LOG_SERIALIZE_SIZE]), keccak256(keccak256([0; L2_TO_L1_LOG_SERIALIZE_SIZE]) & keccak256([0; L2_TO_L1_LOG_SERIALIZE_SIZE])), ...
        //     0x72abee45b59e344af8a6e520241c4744aff26ed411f4c4b00f8af09adada43ba,
        //     0xc3d03eebfd83049991ea3d3e358b6712e7aa2e2e63dc2d4b438987cec28ac8d0,
        //     0xe3697c7f33c31a9b0f0aeb8542287d0d21e8c4cf82163d0c44c7a98aa11aa111,
        //     0x199cc5812543ddceeddd0fc82807646a4899444240db2c0d2f20c3cceb5f51fa,
        //     0xe4733f281f18ba3ea8775dd62d2fcd84011c8c938f16ea5790fd29a03bf8db89,
        //     0x1798a1fd9c8fbb818c98cff190daa7cc10b6e5ac9716b4a2649f7c2ebcef2272,
        //     0x66d7c5983afe44cf15ea8cf565b34c6c31ff0cb4dd744524f7842b942d08770d,
        //     0xb04e5ee349086985f74b73971ce9dfe76bbed95c84906c5dffd96504e1e5396c,
        //     0xac506ecb5465659b3a927143f6d724f91d8d9c4bdb2463aee111d9aa869874db,
        //     0x124b05ec272cecd7538fdafe53b6628d31188ffb6f345139aac3c3c1fd2e470f,
        //     0xc3be9cbd19304d84cca3d045e06b8db3acd68c304fc9cd4cbffe6d18036cb13f,
        //     0xfef7bd9f889811e59e4076a0174087135f080177302763019adaf531257e3a87,
        //     0xa707d1c62d8be699d34cb74804fdd7b4c568b6c1a821066f126c680d4b83e00b,
        //     0xf6e093070e0389d2e529d60fadb855fdded54976ec50ac709e3a36ceaa64c291,
        //     0x375a5bf909cb02143e3695ca658e0641e739aa590f0004dba93572c44cdb9d2d
        const EMPTY_HASHES: [[u8; 32]; 15] = [
            [
                0x72, 0xab, 0xee, 0x45, 0xb5, 0x9e, 0x34, 0x4a, 0xf8, 0xa6, 0xe5, 0x20, 0x24, 0x1c,
                0x47, 0x44, 0xaf, 0xf2, 0x6e, 0xd4, 0x11, 0xf4, 0xc4, 0xb0, 0x0f, 0x8a, 0xf0, 0x9a,
                0xda, 0xda, 0x43, 0xba,
            ],
            [
                0xc3, 0xd0, 0x3e, 0xeb, 0xfd, 0x83, 0x04, 0x99, 0x91, 0xea, 0x3d, 0x3e, 0x35, 0x8b,
                0x67, 0x12, 0xe7, 0xaa, 0x2e, 0x2e, 0x63, 0xdc, 0x2d, 0x4b, 0x43, 0x89, 0x87, 0xce,
                0xc2, 0x8a, 0xc8, 0xd0,
            ],
            [
                0xe3, 0x69, 0x7c, 0x7f, 0x33, 0xc3, 0x1a, 0x9b, 0x0f, 0x0a, 0xeb, 0x85, 0x42, 0x28,
                0x7d, 0x0d, 0x21, 0xe8, 0xc4, 0xcf, 0x82, 0x16, 0x3d, 0x0c, 0x44, 0xc7, 0xa9, 0x8a,
                0xa1, 0x1a, 0xa1, 0x11,
            ],
            [
                0x19, 0x9c, 0xc5, 0x81, 0x25, 0x43, 0xdd, 0xce, 0xed, 0xdd, 0x0f, 0xc8, 0x28, 0x07,
                0x64, 0x6a, 0x48, 0x99, 0x44, 0x42, 0x40, 0xdb, 0x2c, 0x0d, 0x2f, 0x20, 0xc3, 0xcc,
                0xeb, 0x5f, 0x51, 0xfa,
            ],
            [
                0xe4, 0x73, 0x3f, 0x28, 0x1f, 0x18, 0xba, 0x3e, 0xa8, 0x77, 0x5d, 0xd6, 0x2d, 0x2f,
                0xcd, 0x84, 0x01, 0x1c, 0x8c, 0x93, 0x8f, 0x16, 0xea, 0x57, 0x90, 0xfd, 0x29, 0xa0,
                0x3b, 0xf8, 0xdb, 0x89,
            ],
            [
                0x17, 0x98, 0xa1, 0xfd, 0x9c, 0x8f, 0xbb, 0x81, 0x8c, 0x98, 0xcf, 0xf1, 0x90, 0xda,
                0xa7, 0xcc, 0x10, 0xb6, 0xe5, 0xac, 0x97, 0x16, 0xb4, 0xa2, 0x64, 0x9f, 0x7c, 0x2e,
                0xbc, 0xef, 0x22, 0x72,
            ],
            [
                0x66, 0xd7, 0xc5, 0x98, 0x3a, 0xfe, 0x44, 0xcf, 0x15, 0xea, 0x8c, 0xf5, 0x65, 0xb3,
                0x4c, 0x6c, 0x31, 0xff, 0x0c, 0xb4, 0xdd, 0x74, 0x45, 0x24, 0xf7, 0x84, 0x2b, 0x94,
                0x2d, 0x08, 0x77, 0x0d,
            ],
            [
                0xb0, 0x4e, 0x5e, 0xe3, 0x49, 0x08, 0x69, 0x85, 0xf7, 0x4b, 0x73, 0x97, 0x1c, 0xe9,
                0xdf, 0xe7, 0x6b, 0xbe, 0xd9, 0x5c, 0x84, 0x90, 0x6c, 0x5d, 0xff, 0xd9, 0x65, 0x04,
                0xe1, 0xe5, 0x39, 0x6c,
            ],
            [
                0xac, 0x50, 0x6e, 0xcb, 0x54, 0x65, 0x65, 0x9b, 0x3a, 0x92, 0x71, 0x43, 0xf6, 0xd7,
                0x24, 0xf9, 0x1d, 0x8d, 0x9c, 0x4b, 0xdb, 0x24, 0x63, 0xae, 0xe1, 0x11, 0xd9, 0xaa,
                0x86, 0x98, 0x74, 0xdb,
            ],
            [
                0x12, 0x4b, 0x05, 0xec, 0x27, 0x2c, 0xec, 0xd7, 0x53, 0x8f, 0xda, 0xfe, 0x53, 0xb6,
                0x62, 0x8d, 0x31, 0x18, 0x8f, 0xfb, 0x6f, 0x34, 0x51, 0x39, 0xaa, 0xc3, 0xc3, 0xc1,
                0xfd, 0x2e, 0x47, 0x0f,
            ],
            [
                0xc3, 0xbe, 0x9c, 0xbd, 0x19, 0x30, 0x4d, 0x84, 0xcc, 0xa3, 0xd0, 0x45, 0xe0, 0x6b,
                0x8d, 0xb3, 0xac, 0xd6, 0x8c, 0x30, 0x4f, 0xc9, 0xcd, 0x4c, 0xbf, 0xfe, 0x6d, 0x18,
                0x03, 0x6c, 0xb1, 0x3f,
            ],
            [
                0xfe, 0xf7, 0xbd, 0x9f, 0x88, 0x98, 0x11, 0xe5, 0x9e, 0x40, 0x76, 0xa0, 0x17, 0x40,
                0x87, 0x13, 0x5f, 0x08, 0x01, 0x77, 0x30, 0x27, 0x63, 0x01, 0x9a, 0xda, 0xf5, 0x31,
                0x25, 0x7e, 0x3a, 0x87,
            ],
            [
                0xa7, 0x07, 0xd1, 0xc6, 0x2d, 0x8b, 0xe6, 0x99, 0xd3, 0x4c, 0xb7, 0x48, 0x04, 0xfd,
                0xd7, 0xb4, 0xc5, 0x68, 0xb6, 0xc1, 0xa8, 0x21, 0x06, 0x6f, 0x12, 0x6c, 0x68, 0x0d,
                0x4b, 0x83, 0xe0, 0x0b,
            ],
            [
                0xf6, 0xe0, 0x93, 0x07, 0x0e, 0x03, 0x89, 0xd2, 0xe5, 0x29, 0xd6, 0x0f, 0xad, 0xb8,
                0x55, 0xfd, 0xde, 0xd5, 0x49, 0x76, 0xec, 0x50, 0xac, 0x70, 0x9e, 0x3a, 0x36, 0xce,
                0xaa, 0x64, 0xc2, 0x91,
            ],
            [
                0x37, 0x5a, 0x5b, 0xf9, 0x09, 0xcb, 0x02, 0x14, 0x3e, 0x36, 0x95, 0xca, 0x65, 0x8e,
                0x06, 0x41, 0xe7, 0x39, 0xaa, 0x59, 0x0f, 0x00, 0x04, 0xdb, 0xa9, 0x35, 0x72, 0xc4,
                0x4c, 0xdb, 0x9d, 0x2d,
            ],
        ];
        let mut elements = alloc::vec::Vec::with_capacity_in(self.list.len(), A::default());
        let mut log_hasher = crypto::sha3::Keccak256::new();
        self.list.iter().for_each(|el| {
            let log: L2ToL1Log = el.into();
            elements.push(log.hash(&mut log_hasher));
        });
        let mut curr_non_default = self.list.len();
        #[allow(clippy::needless_range_loop)]
        for level in 0..TREE_HEIGHT {
            for i in 0..curr_non_default.div_ceil(2) {
                let mut hasher = crypto::sha3::Keccak256::new();
                hasher.update(elements[i * 2].as_u8_ref());
                if i * 2 + 1 < curr_non_default {
                    hasher.update(elements[i * 2 + 1].as_u8_ref());
                } else {
                    hasher.update(EMPTY_HASHES[level]);
                }
                elements[i] = hasher.finalize().into();
            }
            curr_non_default = curr_non_default.div_ceil(2);
        }
        if curr_non_default != 0 {
            elements[0]
        } else {
            EMPTY_HASHES[14].into()
        }
    }

    // we use it for tests to generate single block batches
    pub fn l1_txs_commitment(&self) -> (u32, Bytes32) {
        let mut count = 0u32;
        // keccak256([])
        let mut rolling_hash = Bytes32::from([
            0xc5, 0xd2, 0x46, 0x01, 0x86, 0xf7, 0x23, 0x3c, 0x92, 0x7e, 0x7d, 0xb2, 0xdc, 0xc7,
            0x03, 0xc0, 0xe5, 0x00, 0xb6, 0x53, 0xca, 0x82, 0x27, 0x3b, 0x7b, 0xfa, 0xd8, 0x04,
            0x5d, 0x85, 0xa4, 0x70,
        ]);
        for log in self.list.iter() {
            if let GenericLogContentData::L1TxLog(l1_tx) = &log.data {
                count += 1;
                let mut hasher = Keccak256::new();
                hasher.update(rolling_hash.as_u8_ref());
                hasher.update(l1_tx.tx_hash.as_u8_ref());
                rolling_hash = hasher.finalize().into();
            }
        }
        (count, rolling_hash)
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
    pub fn hash(&self, hasher: &mut impl MiniDigest<HashOutput = [u8; 32]>) -> Bytes32 {
        self.add_encoding_to_hasher(hasher);
        Bytes32::from_array(hasher.finalize_reset())
    }

    ///
    /// Adds the packed abi encoding of the log to the hasher.
    ///
    pub fn add_encoding_to_hasher(&self, hasher: &mut impl MiniDigest) {
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
    pub fn pubdata(&self, result_keeper: &mut impl IOResultKeeper<EthereumIOTypesConfig>) {
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
                let data = if success { U256::from(1) } else { U256::ZERO };
                (
                    // TODO: move into const
                    B160::from_limbs([0x8001, 0, 0]),
                    tx_hash,
                    Bytes32::from_u256_be(&data),
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
