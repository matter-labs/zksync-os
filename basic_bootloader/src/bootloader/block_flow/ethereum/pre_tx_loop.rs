use zk_ee::{system::IOResultKeeper, types_config::EthereumIOTypesConfig};

use super::*;
use crate::bootloader::block_flow::ethereum::{
    eip_2935_historical_block_hash::eip2935_system_part,
    eip_4788_historical_beacon_root::eip4788_system_part,
};

impl<S: EthereumLikeTypes<Metadata = EthereumBlockMetadata>> PreTxLoopOp<S> for EthereumPreOp
where
    S::IO: IOSubsystemExt,
{
    type PreTxLoopResult = EthereumBasicTransactionDataKeeper<S::Allocator, S::Allocator>;

    fn pre_op(
        system: &mut System<S>,
        _result_keeper: &mut impl IOResultKeeper<EthereumIOTypesConfig>,
    ) -> Self::PreTxLoopResult {
        // EIP-4788
        let beacon_root_hash = system.metadata.block_level.header.parent_beacon_block_root;
        eip4788_system_part(system, &beacon_root_hash).expect("must perform EIP-4788");

        // EIP-2935
        eip2935_system_part(system).expect("must perform EIP-2935");

        // Create data keeper
        EthereumBasicTransactionDataKeeper::new_in(system.get_allocator())
    }
}
