use core::marker::PhantomData;

use super::*;
use crate::bootloader::block_flow::post_tx_loop_op::PostTxLoopOp;
use basic_system::system_implementation::ethereum_storage_model::vec_trait::VecLikeCtor;
use zk_ee::common_structs::WarmStorageKey;

// TODO: move to fork params
pub const MAX_BLOBS_PER_BLOCK: usize = 9;
pub const TARGET_BLOBS_PER_BLOCK: usize = 6;
pub const TARGET_BLOBS_GAS_PER_BLOCK: u64 = (TARGET_BLOBS_PER_BLOCK as u64) * GAS_PER_BLOB;
pub const VERSIONED_HASH_VERSION_KZG: u8 = 0x01;
pub const GAS_PER_BLOB: u64 = 1 << 17;

#[allow(dead_code)]
pub(crate) const SSZ_BYTES_PER_LENGTH_OFFSET: u32 = 4;

pub struct EthereumMetadataOp;
pub struct EthereumPostInitOp;
pub struct EthereumPreOp;

// VC can later be moved to SystemTypes, but we don't want to
// add it to ZK STF for now.
pub struct EthereumPostOp<VC: VecLikeCtor, const PROOF_ENV: bool> {
    _marker: PhantomData<VC>,
}
pub struct EthereumLoopOp;

mod block_data;
mod block_hashes_cache;
mod block_header;
pub mod eip_2935_historical_block_hash;
pub mod eip_4788_historical_beacon_root;
pub mod eip_6110_deposit_events_parser;
pub mod eip_7002_withdrawal_contract;
pub mod eip_7251_consolidation_contract;
mod hooks;
mod loop_op;
pub mod metadata_op;
pub mod oracle_queries;
mod post_init_op;
mod post_tx_op_proving;
mod post_tx_op_sequencing;
mod pre_tx_loop;
mod rlp_encodings;
mod utils;
pub(crate) mod withdrawals;

pub use self::block_data::*;
pub use self::block_header::PectraForkHeader;
pub use self::metadata_op::EthereumBlockMetadata;

pub use eip_2935_historical_block_hash::HISTORY_STORAGE_ADDRESS;
pub use eip_4788_historical_beacon_root::BEACON_ROOTS_ADDRESS;
pub use eip_6110_deposit_events_parser::DEPOSIT_CONTRACT_ADDRESS;
pub use eip_7002_withdrawal_contract::WITHDRAWAL_REQUEST_PREDEPLOY_ADDRESS;

pub(crate) fn rlp_ordering_and_key_for_index(index: u32) -> (u32, ([u8; 4], usize)) {
    if index == 0 {
        (0x80, ([0x80, 0x00, 0x00, 0x00], 1usize))
    } else if index < 0x80 {
        (index, ([index as u8, 0x00, 0x00, 0x00], 1))
    } else {
        let ordering_key = 0x80 + index;
        if index < 1 << 8 {
            (ordering_key, ([0x80 + 1, index as u8, 0x00, 0x00], 2))
        } else if index < 1 << 16 {
            (
                ordering_key,
                ([0x80 + 2, (index >> 8) as u8, index as u8, 0x00], 3),
            )
        } else {
            unreachable!()
        }
    }
}
