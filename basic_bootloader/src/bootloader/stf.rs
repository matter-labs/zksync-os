use crate::bootloader::block_flow::{
    MetadataInitOp, PostSystemInitOp, PostTxLoopOp, PreTxLoopOp, TxLoopOp,
};

use super::*;

pub trait BasicSTF: Sized + SystemTypes
where
    <Self as SystemTypes>::IO: IOSubsystemExt,
{
    type BlockDataKeeper;
    type BlockHeader: 'static + Sized;
    type MetadataOp: MetadataInitOp<Self>;
    type PostSystemInitOp: PostSystemInitOp<Self>;
    type PreTxLoopOp: PreTxLoopOp<Self, PreTxLoopResult = Self::BlockDataKeeper>;
    type TxLoopOp: TxLoopOp<Self, BlockData = Self::BlockDataKeeper>;
    type PostTxLoopOp: PostTxLoopOp<
        Self,
        BlockData = Self::BlockDataKeeper,
        BlockHeader = Self::BlockHeader,
    >;
}

pub trait EthereumLikeBasicSTF: BasicSTF
where
    Self: EthereumLikeTypes,
    <Self as SystemTypes>::IO: IOSubsystemExt,
{
}
