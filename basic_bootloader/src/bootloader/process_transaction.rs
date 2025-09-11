use super::transaction::ZkSyncTransaction;
use super::*;
use crate::bootloader::config::BasicBootloaderExecutionConfig;
use crate::bootloader::errors::TxError::Validation;
use crate::bootloader::errors::{InvalidTransaction, TxError};
use crate::bootloader::runner::RunnerMemoryBuffers;
use crate::bootloader::zk::ZkTransactionFlowOnlyEOA;
pub use crate::bootloader::zk::ZkTxResult as TxProcessingResult;
use crate::{require, require_internal};
use evm_interpreter::ERGS_PER_GAS;
use system_hooks::HooksStorage;
use zk_ee::metadata_markers::basic_metadata::BasicMetadata;
use zk_ee::metadata_markers::basic_metadata::ZkSpecificPricingMetadata;
use zk_ee::system::errors::internal::InternalError;
use zk_ee::system::{EthereumLikeTypes, Resources};
use zk_ee::wrap_error;

// NOTE: less bounds here
impl<S: EthereumLikeTypes> BasicBootloader<S>
where
    S::IO: IOSubsystemExt,
{
    pub(crate) fn get_gas_price(
        system: &mut System<S>,
        max_fee_per_gas: u128,
        max_priority_fee_per_gas: u128,
    ) -> Result<U256, TxError> {
        let max_fee_per_gas = U256::from(max_fee_per_gas);
        let max_priority_fee_per_gas = U256::from(max_priority_fee_per_gas);
        let base_fee = system.get_eip1559_basefee();
        require!(
            max_priority_fee_per_gas <= max_fee_per_gas,
            TxError::Validation(InvalidTransaction::PriorityFeeGreaterThanMaxFee,),
            system
        )?;
        require!(
            base_fee <= max_fee_per_gas,
            TxError::Validation(InvalidTransaction::BaseFeeGreaterThanMaxFee,),
            system
        )?;
        let priority_fee_per_gas = if cfg!(feature = "charge_priority_fee") {
            core::cmp::min(max_priority_fee_per_gas, max_fee_per_gas - base_fee)
        } else {
            U256::ZERO
        };
        Ok(base_fee + priority_fee_per_gas)
    }

    /// Returns (gas_refund, gas_used, evm_refund)
    pub(crate) fn compute_gas_refund(
        system: &mut System<S>,
        to_charge_for_pubdata: S::Resources,
        gas_limit: u64,
        minimal_gas_used: u64,
        native_per_gas: U256,
        resources: &mut S::Resources,
    ) -> Result<(u64, u64, u64), InternalError> {
        // Already checked
        resources.charge_unchecked(&to_charge_for_pubdata);

        let mut gas_used = gas_limit - resources.ergs().0.div_floor(ERGS_PER_GAS);
        resources.exhaust_ergs();

        let _ = system.get_logger().write_fmt(format_args!(
            "Gas used before refund calculations: {gas_used}\n"
        ));

        // Following EIP-3529, refunds are capped to 1/5 of the gas used
        #[cfg(feature = "evm_refunds")]
        let refund_before_native = {
            let possible_refund = if let Some(refund) = system.io.get_refund_counter() {
                let ergs = refund.ergs();

                ergs.0.div_floor(ERGS_PER_GAS)
            } else {
                0
            };
            let refund_cap = gas_used / 5;
            let _ = system.get_logger().write_fmt(format_args!(
                "Refund counter has {possible_refund} gas, with cap of {refund_cap}\n"
            ));

            core::cmp::min(possible_refund, refund_cap)
        };

        #[cfg(not(feature = "evm_refunds"))]
        let refund_before_native = 0;

        let _ = system.get_logger().write_fmt(format_args!(
            "Gas refund from refund counters = {refund_before_native}\n"
        ));

        gas_used -= refund_before_native;

        let _ = system.get_logger().write_fmt(format_args!(
            "Minimal gas used from validation = {minimal_gas_used}\n"
        ));

        #[allow(unused_mut)]
        let mut gas_used = core::cmp::max(gas_used, minimal_gas_used);

        #[cfg(not(feature = "unlimited_native"))]
        {
            // Adjust gas_used with difference with used native
            let native_per_gas = u256_to_u64_saturated(&native_per_gas);
            let full_native_limit = gas_limit.saturating_mul(native_per_gas);
            let computational_native_used =
                full_native_limit - resources.native().remaining().as_u64();

            let delta_gas = if native_per_gas == 0 {
                0
            } else {
                (computational_native_used / native_per_gas) as i64 - (gas_used as i64)
            };

            if delta_gas > 0 {
                // In this case, the native resource consumption is more than the
                // gas consumption accounted for. Consume extra gas.
                gas_used += delta_gas as u64;
            }
            // TODO: return delta_gas to gas_used?
        }

        #[cfg(feature = "unlimited_native")]
        let _ = native_per_gas;

        let total_gas_refund = gas_limit - gas_used;
        let _ = system
            .get_logger()
            .write_fmt(format_args!("Refund after accounting for unused gas, refund counters and native cost: {total_gas_refund}\n"));
        require_internal!(
            total_gas_refund <= gas_limit,
            "Gas refund greater than gas limit",
            system
        )?;

        let _ = system
            .get_logger()
            .write_fmt(format_args!("Final gas used: {gas_used}\n"));

        Ok((total_gas_refund, gas_used, refund_before_native))
    }
}

impl<S: EthereumLikeTypes> BasicBootloader<S>
where
    S::IO: IOSubsystemExt,
    S::Metadata: ZkSpecificPricingMetadata,
    <S::Metadata as BasicMetadata<S::IOTypes>>::TransactionMetadata: From<(B160, U256)>,
{
    ///
    /// Process transaction.
    ///
    /// We are passing callstack from outside to reuse its memory space between different transactions.
    /// It's expected to be empty.
    ///
    pub fn process_transaction<'a, Config: BasicBootloaderExecutionConfig>(
        initial_calldata_buffer: &mut [u8],
        system: &mut System<S>,
        system_functions: &mut HooksStorage<S, S::Allocator>,
        memories: RunnerMemoryBuffers<'a>,
        is_first_tx: bool,
        tracer: &mut impl Tracer<S>,
    ) -> Result<TxProcessingResult<'a>, TxError>
    where
        S: 'a,
    {
        let transaction = ZkSyncTransaction::try_from_slice(initial_calldata_buffer)
            .map_err(|_| TxError::Validation(InvalidTransaction::InvalidEncoding))?;

        // Safe to unwrap here, as this should have been validated in the
        // previous call.
        let tx_type = transaction.tx_type.read();

        match tx_type {
            ZkSyncTransaction::UPGRADE_TX_TYPE => {
                if !is_first_tx {
                    Err(Validation(InvalidTransaction::UpgradeTxNotFirst))
                } else {
                    Self::process_l1_transaction::<Config>(
                        system,
                        system_functions,
                        memories,
                        transaction,
                        false,
                        tracer,
                    )
                }
            }
            ZkSyncTransaction::L1_L2_TX_TYPE => Self::process_l1_transaction::<Config>(
                system,
                system_functions,
                memories,
                transaction,
                true,
                tracer,
            ),
            _ => Self::process_l2_transaction::<Config>(
                system,
                system_functions,
                memories,
                transaction,
                tracer,
            ),
        }
    }

    fn process_l2_transaction<'a, Config: BasicBootloaderExecutionConfig>(
        system: &mut System<S>,
        system_functions: &mut HooksStorage<S, S::Allocator>,
        memories: RunnerMemoryBuffers<'a>,
        transaction: ZkSyncTransaction<'_>,
        tracer: &mut impl Tracer<S>,
    ) -> Result<TxProcessingResult<'a>, TxError>
    where
        S: 'a,
    {
        ZkTransactionFlowOnlyEOA::<S>::before_validation(&*system, &transaction, tracer)?;

        // Here we will follow basic Ethereum EOA flow, but caller is responsible to manage frames

        let validation_rollback_handle = system.start_global_frame()?;

        let (mut tx_context, transaction) =
            match ZkTransactionFlowOnlyEOA::<S>::validate_and_prepare_context::<Config>(
                system,
                transaction,
                tracer,
            ) {
                Ok(v) => v,
                Err(e) => {
                    system.finish_global_frame(Some(&validation_rollback_handle))?;
                    return Err(e);
                }
            };

        let _ = system.get_logger().write_fmt(format_args!(
            "Transaction was validated and can be processed to collect fees\n"
        ));

        match ZkTransactionFlowOnlyEOA::<S>::precharge_fee::<Config>(
            system,
            &transaction,
            &mut tx_context,
            tracer,
        ) {
            Ok(_) => {
                system.finish_global_frame(None)?;
            }
            Err(e) => {
                system.finish_global_frame(Some(&validation_rollback_handle))?;
                return Err(e);
            }
        };
        drop(validation_rollback_handle);

        let _ = system
            .get_logger()
            .write_fmt(format_args!("Fees were collected\n"));

        ZkTransactionFlowOnlyEOA::<S>::before_execute_transaction_payload(
            system,
            &transaction,
            &mut tx_context,
            tracer,
        )?;

        let (execution_result, pubdata_info) =
            ZkTransactionFlowOnlyEOA::<S>::create_frame_and_execute_transaction_payload::<Config>(
                system,
                system_functions,
                memories,
                &transaction,
                &mut tx_context,
                tracer,
            )?;

        ZkTransactionFlowOnlyEOA::<S>::before_refund::<Config>(
            system,
            &transaction,
            &mut tx_context,
            &execution_result,
            pubdata_info,
            tracer,
        )?;

        let _ = system
            .get_logger()
            .write_fmt(format_args!("Start of refund\n"));

        let refund_rollback_handle = system.start_global_frame()?;

        match ZkTransactionFlowOnlyEOA::<S>::refund_and_commit_fee::<Config>(
            system,
            &transaction,
            &mut tx_context,
            tracer,
        ) {
            Ok(_) => {
                system.finish_global_frame(None)?;
            }
            Err(e) => {
                let _ = system
                    .get_logger()
                    .write_fmt(format_args!("Error on refund {:?}\n", &e));
                system.finish_global_frame(Some(&refund_rollback_handle))?;
                return Err(wrap_error!(e).into());
            }
        }
        drop(refund_rollback_handle);

        use crate::bootloader::block_flow::NopTransactionDataKeeper;
        Ok(ZkTransactionFlowOnlyEOA::<S>::after_execution::<Config>(
            system,
            transaction,
            tx_context,
            execution_result,
            &mut NopTransactionDataKeeper,
            tracer,
        ))
    }
}
