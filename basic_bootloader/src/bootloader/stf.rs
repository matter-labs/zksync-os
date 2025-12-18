use crate::bootloader::block_flow::{
    MetadataInitOp, PostSystemInitOp, PostTxLoopOp, PreTxLoopOp, TxLoopOp,
};

use super::*;

/// State Transition Function (STF) trait that defines block execution flow.
pub trait BasicSTF: Sized + SystemTypes
where
    <Self as SystemTypes>::IO: IOSubsystemExt,
{
    /// Data structure for tracking block-level state during transactions processing
    type BlockDataKeeper;
    /// Block header format for this STF
    type BlockHeader: 'static + Sized;
    /// Implementation for initializing block metadata
    type MetadataOp: MetadataInitOp<Self>;
    /// Implementation for post-system initialization (precompiles, contracts)
    type PostSystemInitOp: PostSystemInitOp<Self>;
    /// Implementation for pre-transaction-loop setup
    type PreTxLoopOp: PreTxLoopOp<Self, PreTxLoopResult = Self::BlockDataKeeper>;
    /// Implementation for the main transaction processing loop
    type TxLoopOp: TxLoopOp<Self, BlockDataKeeper = Self::BlockDataKeeper>;
    /// Implementation for post-transaction loop operations
    type PostTxLoopOp: PostTxLoopOp<
        Self,
        BlockDataKeeper = Self::BlockDataKeeper,
        BlockHeader = Self::BlockHeader,
    >;
}

pub trait EthereumLikeBasicSTF: BasicSTF
where
    Self: EthereumLikeTypes,
    <Self as SystemTypes>::IO: IOSubsystemExt,
{
}
