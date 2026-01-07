use super::*;
use zk_ee::system::metadata::basic_metadata::ZkSpecificPricingMetadata;
use zk_ee::system::MAX_NATIVE_COMPUTATIONAL;
use zk_ee::{internal_error, system_log, types_config::*};

mod block_data;
mod metadata_op;
mod post_init_op;
mod post_tx_op;
mod pre_tx_loop;
mod tx_loop;

pub use self::block_data::*;

pub struct ZKHeaderPostInitOp;

pub struct ZKHeaderStructurePreTxOp;

pub struct ZKHeaderStructureTxLoop;

pub struct ZKHeaderStructurePostTxOp<const PROOF_ENV: bool>;

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
) -> Result<(), InvalidTransaction>
where
    S::IO: IOSubsystemExt,
    <S as SystemTypes>::Metadata: ZkSpecificPricingMetadata,
{
    if cfg!(feature = "resources_for_tester") {
        // EVM tester uses some really high gas limits,
        // so we don't limit the block's native resource.
        Ok(())
    } else if gas_used > system.get_gas_limit() {
        system_log!(
            system,
            "Block gas limit reached, invalidating transaction\n"
        );
        Err(InvalidTransaction::BlockGasLimitReached)
    } else if computational_native_used > MAX_NATIVE_COMPUTATIONAL {
        system_log!(
            system,
            "Block native limit reached, invalidating transaction\n"
        );
        Err(InvalidTransaction::BlockNativeLimitReached)
    } else if pubdata_used > system.get_pubdata_limit() {
        system_log!(
            system,
            "Block pubdata limit reached, invalidating transaction\n"
        );
        Err(InvalidTransaction::BlockPubdataLimitReached)
    } else if logs_used > MAX_NUMBER_OF_LOGS {
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
/// 2. Service transactions can only be processed in service blocks
/// 3. Non-service transactions cannot be processed in service blocks
fn check_for_service_block_invariants(
    is_service_block: &mut bool,
    is_first_tx: bool,
    is_service_tx: bool,
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
        // 2. Service transactions can only be processed in service blocks
        if is_service_tx {
            return Err(internal_error!("Service tx in non-service block"));
        }
    }
    Ok(())
}
