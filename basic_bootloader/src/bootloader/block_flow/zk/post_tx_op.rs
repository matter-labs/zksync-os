use super::*;

use zk_ee::{internal_error, utils::Bytes32};

impl<
        S: EthereumLikeTypes<Metadata = zk_ee::system::metadata::zk_metadata::ZkMetadata>,
        const PROOF_ENV: bool, // TODO we should refactor it and potentially split into several versions (proving/sequencing)
    > PostTxLoopOp<S> for ZKHeaderStructurePostTxOp<PROOF_ENV>
where
    S::IO: IOSubsystemExt,
{
    type BlockDataKeeper = ZKBasicBlockDataKeeper;
    type BlockHeader = crate::bootloader::block_header::BlockHeader;

    fn post_op(
        system: System<S>,
        block_data: Self::BlockDataKeeper,
        result_keeper: &mut impl ResultKeeperExt<EthereumIOTypesConfig, BlockHeader = Self::BlockHeader>,
    ) -> Result<<S::IO as IOSubsystemExt>::FinalData, BootloaderSubsystemError> {
        let tx_rolling_hash = block_data.transaction_hashes_accumulator.finish();
        let upgrade_tx_hash = block_data.upgrade_tx_recorder.finish();
        let l1_to_l2_tx_hash = block_data.enforced_transaction_hashes_accumulator.finish();

        let block_gas_used = block_data.block_gas_used;

        let block_number = system.get_block_number();

        let previous_block_hash = if block_number == 0 {
            Bytes32::ZERO
        } else {
            system.get_blockhash(block_number - 1)?
        };
        let beneficiary = system.get_coinbase();
        let gas_limit = system.get_gas_limit();
        let timestamp = system.get_timestamp();
        let consensus_random = system.get_mix_hash()?;
        let base_fee_per_gas = system.get_eip1559_basefee();
        // TODO: add pubdata price and native price
        let base_fee_per_gas = base_fee_per_gas
            .try_into()
            .map_err(|_| internal_error!("base_fee_per_gas exceeds max u64"))?;

        let block_header = BlockHeader::new(
            previous_block_hash,
            beneficiary,
            tx_rolling_hash,
            block_number,
            gas_limit,
            block_gas_used,
            timestamp,
            consensus_random,
            base_fee_per_gas,
        );
        let block_hash = Bytes32::from(block_header.hash());
        result_keeper.block_sealed(block_header);

        system_log!(system, "Bootloader completed\n");
        system_log!(
            system,
            "Bootloader execution is complete, will proceed with applying changes\n"
        );

        let r = system.finish(block_hash, l1_to_l2_tx_hash, upgrade_tx_hash, result_keeper);
        #[allow(clippy::let_and_return)]
        Ok(r)
    }
}
