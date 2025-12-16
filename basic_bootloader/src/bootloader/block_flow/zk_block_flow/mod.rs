use super::*;
use zk_ee::system::metadata::basic_metadata::ZkSpecificPricingMetadata;
use zk_ee::types_config::*;

mod block_data;
mod metadata_op;
mod post_init_op;
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
    S::IO: IOSubsystemExt + IOTeardown<S::IOTypes>,
    <S as SystemTypes>::Metadata: ZkSpecificPricingMetadata,
{
    if cfg!(feature = "resources_for_tester") {
        // EVM tester uses some really high gas limits,
        // so we don't limit the block's native resource.
        Ok(())
    } else {
        let mut logger = system.get_logger();

        if gas_used > system.get_gas_limit() {
            let _ = logger.write_fmt(format_args!(
                "Block gas limit reached, invalidating transaction\n"
            ));
            Err(InvalidTransaction::BlockGasLimitReached)
        } else if computational_native_used > MAX_NATIVE_COMPUTATIONAL {
            let _ = logger.write_fmt(format_args!(
                "Block native limit reached, invalidating transaction\n"
            ));
            Err(InvalidTransaction::BlockNativeLimitReached)
        } else if pubdata_used > system.get_pubdata_limit() {
            let _ = logger.write_fmt(format_args!(
                "Block pubdata limit reached, invalidating transaction\n"
            ));
            Err(InvalidTransaction::BlockPubdataLimitReached)
        } else if logs_used > MAX_NUMBER_OF_LOGS {
            let _ = logger.write_fmt(format_args!(
                "Block logs limit reached, invalidating transaction\n"
            ));
            Err(InvalidTransaction::BlockL2ToL1LogsLimitReached)
        } else {
            Ok(())
        }
    }
}
