use super::*;
use core::marker::PhantomData;
use zk_ee::system::metadata::basic_metadata::ZkSpecificPricingMetadata;
use zk_ee::system::MAX_NATIVE_COMPUTATIONAL;
use zk_ee::{internal_error, system_log, types_config::*};

mod batch_data;
mod block_data;
mod metadata_op;
mod post_init_op;
mod post_tx_op;
mod pre_tx_loop;
mod tx_loop;

pub use self::batch_data::*;
pub use self::block_data::*;
pub use self::post_tx_op::*;

pub struct ZKHeaderPostInitOp;

pub struct ZKHeaderStructurePreTxOp<EA: TxHashesAccumulator> {
    _marker: PhantomData<EA>,
}

pub struct ZKHeaderStructureTxLoop<BlockEA: TxHashesAccumulator, BatchEA: TxHashesAccumulator> {
    _marker: PhantomData<BlockEA>,
    _marker2: PhantomData<BatchEA>,
}

/// ZK header sequencing post tx op (generates block header, returns outputs)
pub struct ZKHeaderStructurePostTxOpSequencing;

/// ZK header proving post tx op for aggregation (generates single block batch, return public input hash)
/// If `STATE_DIFFS_HASH` is true - returns state diffs hash instead of PI hash, used only for testing to compare state diffs with forward run.
pub struct ZKHeaderStructurePostTxOpProvingSingleblockBatch<const STATE_DIFFS_HASH: bool>;

/// ZK header proving post tx op for aggregation (applies block data into accumulator passed from outside, to later form multiblock batch)
pub struct ZKHeaderStructurePostTxOpProvingMultiblockBatch;

/// Check if the transaction made the block reach any of the limits
/// for gas, native, pubdata or logs.
/// If one such limit is reached, return the corresponding validation
/// error.
fn check_for_block_limits<S: EthereumLikeTypes>(
    system: &mut System<S>,
    gas_used: u64,
    computational_native_used: u64,
    pubdata_used: u64,
    logs_used: u64,
    blob_gas_used: u64,
) -> Result<(), InvalidTransaction>
where
    S::IO: IOSubsystemExt,
    <S as SystemTypes>::Metadata: ZkSpecificPricingMetadata,
{
    if gas_used > system.get_gas_limit() {
        system_log!(
            system,
            "Block gas limit reached, invalidating transaction\n"
        );
        Err(InvalidTransaction::BlockGasLimitReached)
    } else if blob_gas_used > system.get_blob_gas_limit() {
        system_log!(
            system,
            "Block blob gas limit reached, invalidating transaction\n"
        );
        Err(InvalidTransaction::BlockBlobGasLimitReached)
    } else if !cfg!(feature = "resources_for_tester")
        && computational_native_used > MAX_NATIVE_COMPUTATIONAL
    {
        // ZKsync OS-specific resources are not checked for evm tester
        system_log!(
            system,
            "Block native limit reached, invalidating transaction\n"
        );
        Err(InvalidTransaction::BlockNativeLimitReached)
    } else if !cfg!(feature = "resources_for_tester") && pubdata_used > system.get_pubdata_limit() {
        // ZKsync OS-specific resources are not checked for evm tester
        system_log!(
            system,
            "Block pubdata limit reached, invalidating transaction\n"
        );
        Err(InvalidTransaction::BlockPubdataLimitReached)
    } else if !cfg!(feature = "resources_for_tester") && logs_used > MAX_NUMBER_OF_LOGS {
        // ZKsync OS-specific resources are not checked for evm tester
        system_log!(
            system,
            "Block logs limit reached, invalidating transaction\n"
        );
        Err(InvalidTransaction::BlockL2ToL1LogsLimitReached)
    } else {
        Ok(())
    }
}

/// Check the service block invariants:
/// 1. If the first tx is a service tx, then the block is a service block
/// 2. Service transactions can only be processed in service blocks,
///    unless the first tx in the block was an upgrade tx
/// 3. Non-service transactions cannot be processed in service blocks
fn check_for_service_block_invariants(
    is_service_block: &mut bool,
    is_first_tx: bool,
    is_service_tx: bool,
    first_tx_was_upgrade: bool,
) -> Result<(), InternalError> {
    //  1. If the first tx is a service tx, then the block is a service block
    if is_first_tx && is_service_tx {
        *is_service_block = true;
    }
    if *is_service_block {
        if !is_service_tx {
            // 3. Non-service transactions cannot be processed in service blocks
            return Err(internal_error!("Non-service tx in service block"));
        }
    } else {
        // 2. Service transactions can only be processed in service blocks,
        //    unless the first tx in the block was an upgrade tx
        if is_service_tx && !first_tx_was_upgrade {
            return Err(internal_error!("Service tx in non-service block"));
        }
    }
    Ok(())
}
