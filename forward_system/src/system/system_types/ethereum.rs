use super::*;
use basic_bootloader::bootloader::block_flow::ethereum::*;
use basic_system::system_implementation::ethereum_storage_model::{
    vec_trait::VecCtor, EthereumStorageModel,
};

pub struct EthereumStorageSystemTypes<O>(O);

impl<O: IOOracle> SystemTypes for EthereumStorageSystemTypes<O> {
    type IOTypes = EthereumIOTypesConfig;
    type Resources = BaseResources<Native>;
    type IO = FullIO<
        Self::Allocator,
        Self::Resources,
        EthereumLikeStorageAccessCostModel,
        VecStackFactory,
        0,
        O,
        EthereumStorageModel<
            Self::Allocator,
            Self::Resources,
            EthereumLikeStorageAccessCostModel,
            VecStackFactory,
            0,
            false,
        >,
        false,
    >;
    type SystemFunctions = NoStdSystemFunctions;
    type SystemFunctionsExt = NoStdSystemFunctions;
    type Allocator = Global;
    type Logger = Logger;
    type Metadata = EthereumBlockMetadata;
}

impl<O: IOOracle> EthereumLikeTypes for EthereumStorageSystemTypes<O> {}

impl<O: IOOracle> BasicSTF for EthereumStorageSystemTypes<O> {
    type BlockDataKeeper = EthereumBasicTransactionDataKeeper<Global, Global>;
    type BatchDataKeeper = ();
    type BlockHeader = PectraForkHeader;
    type MetadataOp = EthereumMetadataOp;
    type PostSystemInitOp = EthereumPostInitOp;
    type PreTxLoopOp = EthereumPreOp;
    type TxLoopOp = EthereumLoopOp;
    type PostTxLoopOp = EthereumPostOp<VecCtor, false>;
}

impl<O: IOOracle> EthereumLikeBasicSTF for EthereumStorageSystemTypes<O> {}

pub struct EthereumStorageSystemTypesWithPostOps<O>(O);

impl<O: IOOracle> SystemTypes for EthereumStorageSystemTypesWithPostOps<O> {
    type IOTypes = EthereumIOTypesConfig;
    type Resources = BaseResources<Native>;
    type IO = FullIO<
        Self::Allocator,
        Self::Resources,
        EthereumLikeStorageAccessCostModel,
        VecStackFactory,
        0,
        O,
        EthereumStorageModel<
            Self::Allocator,
            Self::Resources,
            EthereumLikeStorageAccessCostModel,
            VecStackFactory,
            0,
            true,
        >,
        true,
    >;
    type SystemFunctions = NoStdSystemFunctions;
    type SystemFunctionsExt = NoStdSystemFunctions;
    type Allocator = Global;
    type Logger = Logger;
    type Metadata = EthereumBlockMetadata;
}

impl<O: IOOracle> EthereumLikeTypes for EthereumStorageSystemTypesWithPostOps<O> {}

impl<O: IOOracle> BasicSTF for EthereumStorageSystemTypesWithPostOps<O> {
    type BlockDataKeeper = EthereumBasicTransactionDataKeeper<Global, Global>;
    type BatchDataKeeper = ();
    type BlockHeader = PectraForkHeader;
    type MetadataOp = EthereumMetadataOp;
    type PostSystemInitOp = EthereumPostInitOp;
    type PreTxLoopOp = EthereumPreOp;
    type TxLoopOp = EthereumLoopOp;
    type PostTxLoopOp = EthereumPostOp<VecCtor, true>;
}

impl<O: IOOracle> EthereumLikeBasicSTF for EthereumStorageSystemTypesWithPostOps<O> {}
