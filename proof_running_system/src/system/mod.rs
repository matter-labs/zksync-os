use crate::io_oracle::CsrBasedIOOracle;
use crate::system::bootloader::BootloaderAllocator;
use alloc::alloc::Allocator;
use basic_bootloader::bootloader::block_flow;
use basic_bootloader::bootloader::block_flow::ZKBasicBlockDataKeeper;
use basic_bootloader::bootloader::block_flow::ZKHeaderPostInitOp;
use basic_bootloader::bootloader::block_flow::ZKHeaderStructurePreTxOp;
use basic_bootloader::bootloader::block_flow::ZKHeaderStructureTxLoop;
use basic_bootloader::bootloader::block_header::BlockHeader;
use basic_bootloader::bootloader::stf::BasicSTF;
use basic_bootloader::bootloader::stf::EthereumLikeBasicSTF;
use basic_bootloader::bootloader::transaction_flow::zk::ZkTransactionFlowOnlyEOA;
use basic_bootloader::bootloader::BasicBootloader;
use basic_system::system_functions::NoStdSystemFunctions;
use basic_system::system_implementation::flat_storage_model::FlatTreeWithAccountsUnderHashesStorageModel;
use basic_system::system_implementation::system::EthereumLikeStorageAccessCostModel;
use basic_system::system_implementation::system::FullIO;
use stack_trait::StackFactory;
use zk_ee::common_structs::skip_list_quasi_vec::ListVec;
use zk_ee::memory::*;
use zk_ee::oracle::IOOracle;
use zk_ee::reference_implementations::BaseResources;
use zk_ee::system::metadata::zk_metadata::ZkMetadata;
use zk_ee::system::{logger::Logger, EthereumLikeTypes, SystemTypes};
use zk_ee::types_config::EthereumIOTypesConfig;

pub mod bootloader;

pub struct LVStackFactory {}

impl StackFactory<32> for LVStackFactory {
    type Stack<T: Sized, const N: usize, A: Allocator + Clone> = ListVec<T, N, A>;

    fn new_in<T, A: Allocator + Clone>(alloc: A) -> Self::Stack<T, 32, A> {
        Self::Stack::<T, 32, A>::new_in(alloc)
    }
}

pub struct ProofRunningSystemTypes<O, L>(O, L);

type Native = zk_ee::reference_implementations::DecreasingNative;

impl<O: IOOracle, L: Logger + Default> SystemTypes for ProofRunningSystemTypes<O, L> {
    type IOTypes = EthereumIOTypesConfig;
    type Resources = BaseResources<Native>;
    type IO = FullIO<
        Self::Allocator,
        Self::Resources,
        EthereumLikeStorageAccessCostModel,
        LVStackFactory,
        32,
        O,
        FlatTreeWithAccountsUnderHashesStorageModel<
            Self::Allocator,
            Self::Resources,
            EthereumLikeStorageAccessCostModel,
            LVStackFactory,
            32,
            true,
        >,
        true,
    >;
    type SystemFunctions = NoStdSystemFunctions;
    type SystemFunctionsExt = NoStdSystemFunctions;
    type Allocator = BootloaderAllocator;
    type Logger = L;
    type Metadata = ZkMetadata;
}

impl<O: IOOracle, L: Logger + Default> EthereumLikeTypes for ProofRunningSystemTypes<O, L> {}

#[cfg(not(any(feature = "multiblock-batch", feature = "state-diffs-pi")))]
impl<O: IOOracle, L: Logger + Default> BasicSTF for ProofRunningSystemTypes<O, L> {
    type BlockDataKeeper = ZKBasicBlockDataKeeper<block_flow::TransactionsRollingKeccakHasher>;
    type BatchDataKeeper = ();
    type BlockHeader = BlockHeader;
    type MetadataOp = ZkMetadata;
    type PostSystemInitOp = ZKHeaderPostInitOp;
    type PreTxLoopOp = ZKHeaderStructurePreTxOp<block_flow::TransactionsRollingKeccakHasher>;
    type TxLoopOp = ZKHeaderStructureTxLoop<block_flow::TransactionsRollingKeccakHasher, ()>;
    type PostTxLoopOp = block_flow::ZKHeaderStructurePostTxOpProvingSingleblockBatch<false>;
}

#[cfg(feature = "multiblock-batch")]
impl<O: IOOracle, L: Logger + Default> BasicSTF for ProofRunningSystemTypes<O, L> {
    type BlockDataKeeper = ZKBasicBlockDataKeeper<block_flow::NopTxHashesAccumulator>;
    type BatchDataKeeper = block_flow::ZKBatchDataKeeper<Self::Allocator, O>;
    type BlockHeader = BlockHeader;
    type MetadataOp = ZkMetadata;
    type PostSystemInitOp = ZKHeaderPostInitOp;
    type PreTxLoopOp = ZKHeaderStructurePreTxOp<block_flow::NopTxHashesAccumulator>;
    type TxLoopOp = ZKHeaderStructureTxLoop<
        block_flow::NopTxHashesAccumulator,
        block_flow::ZKBatchDataKeeper<Self::Allocator, O>,
    >;
    type PostTxLoopOp = block_flow::ZKHeaderStructurePostTxOpProvingMultiblockBatch;
}

#[cfg(feature = "state-diffs-pi")]
impl<O: IOOracle, L: Logger + Default> BasicSTF for ProofRunningSystemTypes<O, L> {
    type BlockDataKeeper = ZKBasicBlockDataKeeper<block_flow::TransactionsRollingKeccakHasher>;
    type BatchDataKeeper = ();
    type BlockHeader = BlockHeader;
    type MetadataOp = ZkMetadata;
    type PostSystemInitOp = ZKHeaderPostInitOp;
    type PreTxLoopOp = ZKHeaderStructurePreTxOp<block_flow::TransactionsRollingKeccakHasher>;
    type TxLoopOp = ZKHeaderStructureTxLoop<block_flow::TransactionsRollingKeccakHasher, ()>;
    type PostTxLoopOp = block_flow::ZKHeaderStructurePostTxOpProvingSingleblockBatch<true>;
}

impl<O: IOOracle, L: Logger + Default> EthereumLikeBasicSTF for ProofRunningSystemTypes<O, L> {}

pub type ProvingBootloader<O, L> = BasicBootloader<
    ProofRunningSystemTypes<O, L>,
    ZkTransactionFlowOnlyEOA<ProofRunningSystemTypes<O, L>>,
>;
