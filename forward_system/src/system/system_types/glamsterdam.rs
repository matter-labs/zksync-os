//! The system types of a Glamsterdam transaction range (EIP-7773), run on the host:
//! the access-list storage model and IO, the two-dimensional resources of EIP-8037 and
//! the third-generation EVM interpreter.

use super::*;
use basic_bootloader::bootloader::block_flow::ethereum::EthereumPostInitOp;
use basic_bootloader::bootloader::block_flow::ethereum_glamsterdam::block_header::GlamsterdamHeader;
use basic_bootloader::bootloader::block_flow::ethereum_glamsterdam::tx_range::block_flow::{
    GlamsterdamLoopOp, GlamsterdamPostOp, GlamsterdamPreOp, GlamsterdamRangeData,
};
use basic_bootloader::bootloader::block_flow::ethereum_glamsterdam::tx_range::io::GlamsterdamIO;
use basic_bootloader::bootloader::block_flow::ethereum_glamsterdam::tx_range::metadata_op::{
    GlamsterdamBlockMetadata, GlamsterdamMetadataOp,
};
use basic_bootloader::bootloader::block_flow::ethereum_glamsterdam::tx_range::storage_policy::GlamsterdamStorageAccessCostModel;
use basic_system::system_implementation::ethereum_storage_model::vec_trait::VecCtor;
use evm_interpreter::v3::GlamsterdamTypes;
use zk_ee::reference_implementations::GlamsterdamResources;

/// `PROOF_ENV` selects the proving-style IO (oracle advice for the system functions and
/// the preimage queries of the proving binary), so that the host run records the
/// non-determinism of the guest
pub struct GlamsterdamSystemTypes<O, const PROOF_ENV: bool>(O);

impl<O: IOOracle, const PROOF_ENV: bool> SystemTypes for GlamsterdamSystemTypes<O, PROOF_ENV> {
    type IOTypes = EthereumIOTypesConfig;
    type Resources = GlamsterdamResources<()>;
    type IO = GlamsterdamIO<
        Self::Allocator,
        Self::Resources,
        GlamsterdamStorageAccessCostModel,
        VecStackFactory,
        0,
        O,
        PROOF_ENV,
    >;
    type SystemFunctions = NoStdSystemFunctions<PROOF_ENV>;
    type SystemFunctionsExt = NoStdSystemFunctions<PROOF_ENV>;
    type Allocator = Global;
    type Logger = Logger;
    type Metadata = GlamsterdamBlockMetadata;
}

impl<O: IOOracle, const PROOF_ENV: bool> EthereumLikeTypes
    for GlamsterdamSystemTypes<O, PROOF_ENV>
{
    // the Ethereum preimage cache pads every code buffer
    const CODE_IS_PADDED: bool = true;
}

impl<O: IOOracle, const PROOF_ENV: bool> EthereumLikeSystem
    for GlamsterdamSystemTypes<O, PROOF_ENV>
{
    type EVM<'a>
        = evm_interpreter::v3::Interpreter<'a, Self>
    where
        Self: 'a;
}

impl<O: IOOracle, const PROOF_ENV: bool> GlamsterdamTypes for GlamsterdamSystemTypes<O, PROOF_ENV> {
    fn slot_number(metadata: &Self::Metadata) -> u64 {
        metadata.block_level.slot_number()
    }
}

impl<O: IOOracle, const PROOF_ENV: bool> BasicSTF for GlamsterdamSystemTypes<O, PROOF_ENV> {
    type BlockDataKeeper = GlamsterdamRangeData<Global>;
    type BatchDataKeeper = ();
    type BlockHeader = GlamsterdamHeader;
    type MetadataOp = GlamsterdamMetadataOp;
    type PostSystemInitOp = EthereumPostInitOp;
    type PreTxLoopOp = GlamsterdamPreOp;
    type TxLoopOp = GlamsterdamLoopOp;
    type PostTxLoopOp = GlamsterdamPostOp<VecCtor>;
}

impl<O: IOOracle, const PROOF_ENV: bool> EthereumLikeBasicSTF
    for GlamsterdamSystemTypes<O, PROOF_ENV>
{
}
