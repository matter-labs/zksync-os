//! Storage of L2->L1 logs.
//! There are three kinds of such logs:
//! - user messages (sent via l1 messenger system hook).
//! - l1 -> l2 txs logs, to prove execution result on l1.
//! - interop commitment tree (IMT) leaves (reported via the interop
//!   commitment leaf system hook), to make the IMT reconstructible from DA.
use super::history_list::HistoryList;
use super::merkle_tree::merkle_root_in_place;
use crate::internal_error;
use crate::system::errors::internal::InternalError;
use crate::system::IOResultKeeper;
use crate::utils::write_bytes::WriteBytes;
use crate::{
    memory::stack_trait::StackFactory,
    system::errors::system::SystemError,
    types_config::{EthereumIOTypesConfig, SystemIOTypesConfig},
    utils::{Bytes32, UsizeAlignedByteBox},
};
use alloc::alloc::Global;
use arrayvec::ArrayVec;
use core::alloc::Allocator;
use crypto::MiniDigest;
use ruint::aliases::B160;
use ruint::aliases::U256;

pub const L2_TO_L1_LOG_SERIALIZE_SIZE: usize = 88;
// Taken from the size of the Merkle tree.
pub const MAX_NUMBER_OF_LOGS: u64 = 16_384;

/// Height of the L2->L1 logs Merkle tree (capacity `2^14 == 16_384` leaves).
pub const L2_TO_L1_LOG_TREE_HEIGHT: usize = 14;

/// Empty-subtree hashes for the L2->L1 logs tree, where entry `i` is the root
/// of an empty subtree of height `i`. The empty leaf is `keccak256([0; 88])`
/// (the hash of a zeroed log), and each level doubles up:
/// `entry[i] = keccak256(entry[i - 1] || entry[i - 1])`.
///
/// These are the input expected by
/// [`merkle_root_in_place`](crate::common_structs::merkle_root_in_place) for the
/// logs tree; the recurrence is checked against this table in the `merkle_tree`
/// unit tests.
pub const L2_TO_L1_LOG_EMPTY_SUBTREE_HASHES: [[u8; 32]; L2_TO_L1_LOG_TREE_HEIGHT + 1] = [
    [
        0x72, 0xab, 0xee, 0x45, 0xb5, 0x9e, 0x34, 0x4a, 0xf8, 0xa6, 0xe5, 0x20, 0x24, 0x1c, 0x47,
        0x44, 0xaf, 0xf2, 0x6e, 0xd4, 0x11, 0xf4, 0xc4, 0xb0, 0x0f, 0x8a, 0xf0, 0x9a, 0xda, 0xda,
        0x43, 0xba,
    ],
    [
        0xc3, 0xd0, 0x3e, 0xeb, 0xfd, 0x83, 0x04, 0x99, 0x91, 0xea, 0x3d, 0x3e, 0x35, 0x8b, 0x67,
        0x12, 0xe7, 0xaa, 0x2e, 0x2e, 0x63, 0xdc, 0x2d, 0x4b, 0x43, 0x89, 0x87, 0xce, 0xc2, 0x8a,
        0xc8, 0xd0,
    ],
    [
        0xe3, 0x69, 0x7c, 0x7f, 0x33, 0xc3, 0x1a, 0x9b, 0x0f, 0x0a, 0xeb, 0x85, 0x42, 0x28, 0x7d,
        0x0d, 0x21, 0xe8, 0xc4, 0xcf, 0x82, 0x16, 0x3d, 0x0c, 0x44, 0xc7, 0xa9, 0x8a, 0xa1, 0x1a,
        0xa1, 0x11,
    ],
    [
        0x19, 0x9c, 0xc5, 0x81, 0x25, 0x43, 0xdd, 0xce, 0xed, 0xdd, 0x0f, 0xc8, 0x28, 0x07, 0x64,
        0x6a, 0x48, 0x99, 0x44, 0x42, 0x40, 0xdb, 0x2c, 0x0d, 0x2f, 0x20, 0xc3, 0xcc, 0xeb, 0x5f,
        0x51, 0xfa,
    ],
    [
        0xe4, 0x73, 0x3f, 0x28, 0x1f, 0x18, 0xba, 0x3e, 0xa8, 0x77, 0x5d, 0xd6, 0x2d, 0x2f, 0xcd,
        0x84, 0x01, 0x1c, 0x8c, 0x93, 0x8f, 0x16, 0xea, 0x57, 0x90, 0xfd, 0x29, 0xa0, 0x3b, 0xf8,
        0xdb, 0x89,
    ],
    [
        0x17, 0x98, 0xa1, 0xfd, 0x9c, 0x8f, 0xbb, 0x81, 0x8c, 0x98, 0xcf, 0xf1, 0x90, 0xda, 0xa7,
        0xcc, 0x10, 0xb6, 0xe5, 0xac, 0x97, 0x16, 0xb4, 0xa2, 0x64, 0x9f, 0x7c, 0x2e, 0xbc, 0xef,
        0x22, 0x72,
    ],
    [
        0x66, 0xd7, 0xc5, 0x98, 0x3a, 0xfe, 0x44, 0xcf, 0x15, 0xea, 0x8c, 0xf5, 0x65, 0xb3, 0x4c,
        0x6c, 0x31, 0xff, 0x0c, 0xb4, 0xdd, 0x74, 0x45, 0x24, 0xf7, 0x84, 0x2b, 0x94, 0x2d, 0x08,
        0x77, 0x0d,
    ],
    [
        0xb0, 0x4e, 0x5e, 0xe3, 0x49, 0x08, 0x69, 0x85, 0xf7, 0x4b, 0x73, 0x97, 0x1c, 0xe9, 0xdf,
        0xe7, 0x6b, 0xbe, 0xd9, 0x5c, 0x84, 0x90, 0x6c, 0x5d, 0xff, 0xd9, 0x65, 0x04, 0xe1, 0xe5,
        0x39, 0x6c,
    ],
    [
        0xac, 0x50, 0x6e, 0xcb, 0x54, 0x65, 0x65, 0x9b, 0x3a, 0x92, 0x71, 0x43, 0xf6, 0xd7, 0x24,
        0xf9, 0x1d, 0x8d, 0x9c, 0x4b, 0xdb, 0x24, 0x63, 0xae, 0xe1, 0x11, 0xd9, 0xaa, 0x86, 0x98,
        0x74, 0xdb,
    ],
    [
        0x12, 0x4b, 0x05, 0xec, 0x27, 0x2c, 0xec, 0xd7, 0x53, 0x8f, 0xda, 0xfe, 0x53, 0xb6, 0x62,
        0x8d, 0x31, 0x18, 0x8f, 0xfb, 0x6f, 0x34, 0x51, 0x39, 0xaa, 0xc3, 0xc3, 0xc1, 0xfd, 0x2e,
        0x47, 0x0f,
    ],
    [
        0xc3, 0xbe, 0x9c, 0xbd, 0x19, 0x30, 0x4d, 0x84, 0xcc, 0xa3, 0xd0, 0x45, 0xe0, 0x6b, 0x8d,
        0xb3, 0xac, 0xd6, 0x8c, 0x30, 0x4f, 0xc9, 0xcd, 0x4c, 0xbf, 0xfe, 0x6d, 0x18, 0x03, 0x6c,
        0xb1, 0x3f,
    ],
    [
        0xfe, 0xf7, 0xbd, 0x9f, 0x88, 0x98, 0x11, 0xe5, 0x9e, 0x40, 0x76, 0xa0, 0x17, 0x40, 0x87,
        0x13, 0x5f, 0x08, 0x01, 0x77, 0x30, 0x27, 0x63, 0x01, 0x9a, 0xda, 0xf5, 0x31, 0x25, 0x7e,
        0x3a, 0x87,
    ],
    [
        0xa7, 0x07, 0xd1, 0xc6, 0x2d, 0x8b, 0xe6, 0x99, 0xd3, 0x4c, 0xb7, 0x48, 0x04, 0xfd, 0xd7,
        0xb4, 0xc5, 0x68, 0xb6, 0xc1, 0xa8, 0x21, 0x06, 0x6f, 0x12, 0x6c, 0x68, 0x0d, 0x4b, 0x83,
        0xe0, 0x0b,
    ],
    [
        0xf6, 0xe0, 0x93, 0x07, 0x0e, 0x03, 0x89, 0xd2, 0xe5, 0x29, 0xd6, 0x0f, 0xad, 0xb8, 0x55,
        0xfd, 0xde, 0xd5, 0x49, 0x76, 0xec, 0x50, 0xac, 0x70, 0x9e, 0x3a, 0x36, 0xce, 0xaa, 0x64,
        0xc2, 0x91,
    ],
    [
        0x37, 0x5a, 0x5b, 0xf9, 0x09, 0xcb, 0x02, 0x14, 0x3e, 0x36, 0x95, 0xca, 0x65, 0x8e, 0x06,
        0x41, 0xe7, 0x39, 0xaa, 0x59, 0x0f, 0x00, 0x04, 0xdb, 0xa9, 0x35, 0x72, 0xc4, 0x4c, 0xdb,
        0x9d, 0x2d,
    ],
];

///
/// L2 to l1 log structure, used for merkle tree leaves.
/// This structure holds all kinds of logs (user messages,
/// l1 -> l2 tx logs and interop commitment tree leaves).
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
    /// for l1 -> l2 txs logs - `BootloaderFormalAddress`,
    /// for interop commitment tree leaves - `L2InteropCommitmentTree` address.
    ///
    pub sender: B160,
    ///
    /// The 32 bytes of information that was sent in the log.
    /// For user messages used to save message sender address(padded),
    /// for l1 -> l2 txs logs - transaction hash,
    /// for interop commitment tree leaves - zero.
    ///
    pub key: Bytes32,
    ///
    /// The 32 bytes of information that was sent in the log.
    /// For user messages used to save message hash,
    /// for l1 -> l2 txs logs - success flag(padded),
    /// for interop commitment tree leaves - leaf hash.
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
    InteropCommitmentLeaf(InteropCommitmentLeafLog<HASH>),
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

///
/// Data stored for an interop commitment tree (IMT) leaf log.
/// Reported by the `L2InteropCommitmentTree` system contract on every
/// leaf insertion so that the leaves are always publishable to DA.
///
#[derive(Clone, Debug)]
pub struct InteropCommitmentLeafLog<HASH> {
    pub leaf_hash: HASH,
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
            GenericLogContentData::InteropCommitmentLeaf(l) => {
                GenericLogContentData::InteropCommitmentLeaf(InteropCommitmentLeafLog {
                    leaf_hash: &l.leaf_hash,
                })
            }
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
            GenericLogContentData::InteropCommitmentLeaf(l) => {
                GenericLogContentData::InteropCommitmentLeaf(InteropCommitmentLeafLog {
                    leaf_hash: *l.leaf_hash,
                })
            }
        };
        GenericLogContent {
            tx_number: r.tx_number,
            data,
        }
    }
}

#[allow(type_alias_bounds)]
pub type LogContent<A: Allocator = Global> = GenericLogContent<EthereumIOTypesConfig, A>;

pub struct LogsStorage<SF: StackFactory<M>, const M: usize, A: Allocator + Clone = Global> {
    list: HistoryList<LogContent<A>, u32, SF, M, A>,
    pubdata_used_by_committed_logs: u32,
    /// Number of logs already committed to prior transactions, snapshotted at
    /// [`begin_new_tx`](Self::begin_new_tx). Used to charge only the mandatory
    /// log-record pubdata in Validium mode (see [`calculate_log_records_pubdata_used_by_tx`]).
    logs_count_by_committed_logs: u32,
    _marker: core::marker::PhantomData<A>,
}

impl<SF: StackFactory<M>, const M: usize, A: Allocator + Clone + Default> LogsStorage<SF, M, A> {
    pub fn new_from_parts(allocator: A) -> Self {
        Self {
            list: HistoryList::new(allocator),
            pubdata_used_by_committed_logs: 0,
            logs_count_by_committed_logs: 0,
            _marker: core::marker::PhantomData,
        }
    }

    pub fn begin_new_tx(&mut self) {
        self.pubdata_used_by_committed_logs = self.list.top().map_or(0, |(_, m)| *m);
        self.logs_count_by_committed_logs = self.list.len() as u32;
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

    pub fn push_interop_commitment_leaf(
        &mut self,
        tx_number: u32,
        leaf_hash: Bytes32,
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
                data: GenericLogContentData::InteropCommitmentLeaf(InteropCommitmentLeafLog {
                    leaf_hash,
                }),
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

    pub fn apply_l2_to_l1_logs_hashes_to_hasher(&self, hasher: &mut impl MiniDigest) {
        for message in self.list.iter() {
            hasher.update(L2ToL1Log::from(message).hash().as_u8_ref());
        }
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

    ///
    /// Pubdata used by this tx's mandatory L2->L1 log *records* only — the bytes always committed to
    /// DA (`L2_TO_L1_LOG_SERIALIZE_SIZE` per log), excluding the optional message payloads. This is
    /// what a Validium tx is charged for; see [`DAMode`](crate::common_structs::da_commitment_scheme::DAMode)
    /// and `write_pubdata`.
    ///
    pub fn calculate_log_records_pubdata_used_by_tx(&self) -> Result<u32, InternalError> {
        let logs_count = self.list.len() as u32;

        if logs_count < self.logs_count_by_committed_logs {
            Err(internal_error!("Logs count unexpectedly decreased"))
        } else {
            Ok((logs_count - self.logs_count_by_committed_logs)
                * L2_TO_L1_LOG_SERIALIZE_SIZE as u32)
        }
    }

    ///
    /// Encode the l2 -> l1 log records (count + serialized logs) into the pubdata.
    ///
    /// This is the mandatory part of the logs pubdata: it must always be included
    /// in the DA commitment, regardless of the DA commitment scheme.
    ///
    pub fn apply_logs_pubdata<T: WriteBytes + ?Sized>(
        &self,
        dst: &mut T,
        results_keeper: &mut impl IOResultKeeper<EthereumIOTypesConfig>,
    ) {
        let logs_count = (self.list.len() as u32).to_be_bytes();
        dst.write(&logs_count);
        results_keeper.pubdata(&logs_count);
        self.list.iter().for_each(|el| {
            let log: L2ToL1Log = el.into();
            log.write_encoding(dst);
            log.pubdata(results_keeper);
        });
    }

    ///
    /// Encode the user message payloads (count + length-prefixed data) into the
    /// pubdata.
    ///
    /// This is the optional part of the logs pubdata: only the message hashes
    /// (part of the log records) are guaranteed to be committed to DA.
    ///
    pub fn apply_messages_pubdata<T: WriteBytes + ?Sized>(
        &self,
        dst: &mut T,
        results_keeper: &mut impl IOResultKeeper<EthereumIOTypesConfig>,
    ) {
        let mut messages_count: u32 = 0;
        self.list.iter().for_each(|el| {
            if let GenericLogContentData::UserMsg(_) = el.data {
                messages_count += 1;
            }
        });
        let messages_count = messages_count.to_be_bytes();
        dst.write(&messages_count);
        results_keeper.pubdata(&messages_count);
        self.list.iter().for_each(|el| {
            if let GenericLogContentData::UserMsg(UserMsgData { data, .. }) = &el.data {
                let len = (data.as_slice().len() as u32).to_be_bytes();
                dst.write(&len);
                results_keeper.pubdata(&len);
                dst.write(data.as_slice());
                results_keeper.pubdata(data.as_slice());
            }
        })
    }

    pub fn apply_to_array_vec(&self, array_vec: &mut ArrayVec<Bytes32, 16384>) {
        self.list.iter().for_each(|el| {
            let log: L2ToL1Log = el.into();
            array_vec.push(log.hash())
        });
    }

    // we use it for tests to generate single block batches
    ///
    /// Calculate l2 logs merkle tree root.
    ///
    pub fn tree_root(&self) -> Bytes32 {
        let mut elements = alloc::vec::Vec::with_capacity_in(self.list.len(), A::default());
        self.list.iter().for_each(|el| {
            let log: L2ToL1Log = el.into();
            elements.push(log.hash())
        });
        let empty_hashes = L2_TO_L1_LOG_EMPTY_SUBTREE_HASHES.map(Bytes32::from_array);
        merkle_root_in_place::<crypto::sha3::Keccak256>(&mut elements, &empty_hashes)
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
        self.write_encoding(&mut hasher);
        hasher.finalize().into()
    }

    ///
    /// Adds the packed abi encoding of the log to the hasher.
    ///
    fn write_encoding<T: WriteBytes + ?Sized>(&self, dst: &mut T) {
        dst.write(&[self.l2_shard_id]);
        dst.write(&[if self.is_service { 1 } else { 0 }]);
        dst.write(&self.tx_number_in_block.to_be_bytes());
        dst.write(&self.sender.to_be_bytes::<20>());
        dst.write(self.key.as_u8_ref());
        dst.write(self.value.as_u8_ref());
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
                let data = if success { U256::from(1) } else { U256::ZERO };
                (
                    // TODO: move into const
                    B160::from_limbs([0x8001, 0, 0]),
                    tx_hash,
                    Bytes32::from_u256_be(&data),
                )
            }
            GenericLogContentData::InteropCommitmentLeaf(InteropCommitmentLeafLog {
                leaf_hash,
            }) => {
                // L2InteropCommitmentTree system contract address
                (
                    // TODO: move into const
                    B160::from_limbs([0x10012, 0, 0]),
                    Bytes32::ZERO,
                    leaf_hash,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interop_commitment_leaf_log_field_mapping() {
        let leaf_hash = Bytes32::from_byte_fill(0xab);
        let content: LogContent = GenericLogContent {
            tx_number: 7,
            data: GenericLogContentData::InteropCommitmentLeaf(InteropCommitmentLeafLog {
                leaf_hash,
            }),
        };

        let log = L2ToL1Log::from(&content);
        assert_eq!(log.l2_shard_id, 0);
        assert!(log.is_service);
        assert_eq!(log.tx_number_in_block, 7);
        // L2InteropCommitmentTree system contract address
        assert_eq!(log.sender, B160::from_limbs([0x10012, 0, 0]));
        assert_eq!(log.key, Bytes32::ZERO);
        assert_eq!(log.value, leaf_hash);
    }

    #[test]
    fn interop_commitment_leaf_log_encoding_layout() {
        let leaf_hash = Bytes32::from_byte_fill(0xab);
        let content: LogContent = GenericLogContent {
            tx_number: 0x0102,
            data: GenericLogContentData::InteropCommitmentLeaf(InteropCommitmentLeafLog {
                leaf_hash,
            }),
        };

        let encoding = L2ToL1Log::from(&content).encode();
        // Packed layout: [shard(1), is_service(1), tx_number(2), sender(20), key(32), value(32)]
        let mut expected = [0u8; L2_TO_L1_LOG_SERIALIZE_SIZE];
        expected[1] = 1; // is_service
        expected[2..4].copy_from_slice(&[0x01, 0x02]); // tx number, big-endian
        expected[21..24].copy_from_slice(&[0x01, 0x00, 0x12]); // sender 0x10012, low bytes of B160
                                                               // key stays zero
        expected[56..88].copy_from_slice(leaf_hash.as_u8_ref()); // value = leaf hash
        assert_eq!(encoding, expected);
    }
}
