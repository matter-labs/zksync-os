use std::alloc::Global;

use basic_bootloader::bootloader::block_flow::ZKHeaderPostInitOp;
use basic_bootloader::bootloader::block_flow::ZKHeaderStructurePreTxOp;
use basic_bootloader::bootloader::block_flow::ZKHeaderStructureTxLoop;
use basic_bootloader::bootloader::block_flow::{
    NopTxHashesAccumulator, ZKBasicBlockDataKeeper, ZKHeaderStructurePostTxOpSequencing,
};
use basic_bootloader::bootloader::stf::BasicSTF;
use basic_bootloader::bootloader::stf::EthereumLikeBasicSTF;
use basic_bootloader::bootloader::transaction_flow::zk::ZkTransactionFlowOnlyEOA;
use basic_bootloader::bootloader::BasicBootloader;
use basic_system::system_functions::NoStdSystemFunctions;
use basic_system::system_implementation::flat_storage_model::FlatTreeWithAccountsUnderHashesStorageModel;
use basic_system::system_implementation::system::EthereumLikeStorageAccessCostModel;
use basic_system::system_implementation::system::FullIO;
use oracle_provider::DummyMemorySource;
use oracle_provider::ZkEENonDeterminismSource;
use zk_ee::memory::stack_implementations::vec_stack::VecStackFactory;
use zk_ee::oracle::IOOracle;
use zk_ee::reference_implementations::BaseResources;
use zk_ee::system::{EthereumLikeTypes, SystemTypes};
use zk_ee::types_config::EthereumIOTypesConfig;

pub mod ethereum;

/// Logger implementation selected based on compilation features
#[cfg(not(feature = "no_print"))]
type Logger = crate::system::logger::StdIOLogger;

#[cfg(feature = "no_print")]
type Logger = zk_ee::system::NullLogger;

/// Forward execution system type configuration.
///
/// This system is used for "forward" execution (sequencing mode) where
/// transactions are executed to produce state changes and results.
/// The oracle parameter `O` provides transaction data and block metadata.
pub struct ForwardSystemTypes<O>(O);

/// Native resource implementation that decreases during execution
type Native = zk_ee::reference_implementations::DecreasingNative;

impl<O: IOOracle> SystemTypes for ForwardSystemTypes<O> {
    /// Ethereum-compatible I/O configuration (addresses, hashes, etc.)
    type IOTypes = EthereumIOTypesConfig;
    /// Resource tracking with native decreasing implementation
    type Resources = BaseResources<Native>;
    /// Full I/O subsystem with Ethereum storage costs and Vec-based stacks
    type IO = FullIO<
        Self::Allocator,
        Self::Resources,
        EthereumLikeStorageAccessCostModel,
        VecStackFactory,
        0, // VecStackFactory ignores N (node size), so 0 is fine here
        O, // Oracle implementation
        FlatTreeWithAccountsUnderHashesStorageModel<
            Self::Allocator,
            Self::Resources,
            EthereumLikeStorageAccessCostModel,
            VecStackFactory,
            0,
            false,
        >,
        false,
    >;
    /// System functions implementation (contracts, precompiles)
    type SystemFunctions = NoStdSystemFunctions;
    /// Extended system functions (same as basic for forward execution)
    type SystemFunctionsExt = NoStdSystemFunctions;
    /// Standard library allocator for forward execution
    type Allocator = Global;
    /// Conditional logger based on compilation features
    type Logger = Logger;
    /// ZKsync-specific metadata structure
    type Metadata = zk_ee::system::metadata::zk_metadata::ZkMetadata;
}

/// Marker implementation indicating Ethereum-like execution semantics
impl<O: IOOracle> EthereumLikeTypes for ForwardSystemTypes<O> {}

/// STF implementation for forward execution systems
impl<O: IOOracle> BasicSTF for ForwardSystemTypes<O> {
    /// ZKsync transaction data tracker with hash accumulators and resource counts
    type BlockDataKeeper = ZKBasicBlockDataKeeper<NopTxHashesAccumulator>;
    /// ZKsync blocks data tracker
    type BatchDataKeeper = ();
    /// Standard ZKsync block header format
    type BlockHeader = basic_bootloader::bootloader::block_header::BlockHeader;
    /// Post-initialization setup: precompiles and system contracts
    type PostSystemInitOp = ZKHeaderPostInitOp;
    /// Metadata initialization using ZKsync metadata format
    type MetadataOp = zk_ee::system::metadata::zk_metadata::ZkMetadata;
    /// Pre-transaction setup: initialize data keeper
    type PreTxLoopOp = ZKHeaderStructurePreTxOp<NopTxHashesAccumulator>;
    /// Main transaction loop: ZK-specific processing with resource limits
    type TxLoopOp = ZKHeaderStructureTxLoop<NopTxHashesAccumulator, ()>;
    /// Post-transaction finalization: build header and commit (false = sequencing mode)
    type PostTxLoopOp = ZKHeaderStructurePostTxOpSequencing;
}

/// Marker implementation for Ethereum-compatible STF
impl<O: IOOracle> EthereumLikeBasicSTF for ForwardSystemTypes<O> {}

/// Forward execution system used in sequencing mode
/// Uses dummy memory source for oracle data storage
pub type ForwardRunningSystem = ForwardSystemTypes<ZkEENonDeterminismSource<DummyMemorySource>>;

/// Call simulation system with same configuration as forward execution
pub type CallSimulationSystem = ForwardSystemTypes<ZkEENonDeterminismSource<DummyMemorySource>>;

/// Bootloader for forward execution using ZK transaction flow (EOA only)
pub type ForwardBootloader =
    BasicBootloader<ForwardRunningSystem, ZkTransactionFlowOnlyEOA<ForwardRunningSystem>>;

/// Bootloader for call simulation using ZK transaction flow (EOA only)
pub type CallSimulationBootloader =
    BasicBootloader<CallSimulationSystem, ZkTransactionFlowOnlyEOA<ForwardRunningSystem>>;
