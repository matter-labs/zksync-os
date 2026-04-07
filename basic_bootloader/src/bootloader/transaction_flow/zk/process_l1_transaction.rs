use crate::bootloader::config::BasicBootloaderExecutionConfig;
use crate::bootloader::constants::{
    ASSET_TRACKER_INTRINSIC_PUBDATA, FREE_L1_TX_NATIVE_PER_GAS, L1_TX_INTRINSIC_L2_GAS,
    L1_TX_INTRINSIC_NATIVE_COST, L1_TX_INTRINSIC_PUBDATA, L1_TX_NATIVE_PRICE,
};
use crate::bootloader::errors::BootloaderInterfaceError;
use crate::bootloader::errors::TxError;
use crate::bootloader::runner::RunnerMemoryBuffers;
use crate::bootloader::transaction::abi_encoded::AbiEncodedTransaction;
use crate::bootloader::transaction_flow::gas_helpers::{
    check_enough_resources_for_pubdata, create_resources_for_tx,
    get_resources_to_charge_for_pubdata, L1ResourcesPolicy, ResourcesForTx,
};
use crate::bootloader::transaction_flow::refund_calculation::{compute_gas_refund, RefundInfo};
use crate::bootloader::transaction_flow::{ExecutionOutput, ExecutionResult};
use crate::bootloader::{BasicBootloader, BootloaderSubsystemError};
use crate::require_internal;
use alloc::vec::Vec;
use arrayvec::ArrayVec;
use core::fmt::Write;
use ruint::aliases::{B160, U256};
use zk_ee::common_structs::system_hooks::HooksStorage;
use zk_ee::execution_environment_type::ExecutionEnvironmentType;
use zk_ee::system::errors::root_cause::GetRootCause;
use zk_ee::system::errors::root_cause::RootCause;
use zk_ee::system::errors::runtime::RuntimeError;
use zk_ee::system::errors::subsystem::SubsystemError;
use zk_ee::system::metadata::basic_metadata::{BasicMetadata, ZkSpecificPricingMetadata};
use zk_ee::system::metadata::zk_metadata::TxLevelMetadata;
use zk_ee::system::tracer::Tracer;
use zk_ee::system::validator::TxValidator;
use zk_ee::system::Resource;
use zk_ee::system::System;
use zk_ee::system::{CompletedExecution, Computational};
use zk_ee::system::{EthereumLikeTypes, Resources};
#[allow(unused_imports)]
use zk_ee::system::{IOSubsystem, IOSubsystemExt, MAX_NATIVE_COMPUTATIONAL};
use zk_ee::system_log;
use zk_ee::utils::{u256_to_b160_checked, u256_try_to_u64, Bytes32};
use zk_ee::{interface_error, internal_error, wrap_error};

use system_hooks::addresses_constants::{L2_ASSET_TRACKER_ADDRESS, L2_BASE_TOKEN_ADDRESS};

use super::validation_impl::compute_calldata_tokens;
use super::{ZkTransactionFlowOnlyEOA, ZkTxResult};

pub(crate) fn process_l1_transaction<
    'a,
    S: EthereumLikeTypes + 'a,
    Config: BasicBootloaderExecutionConfig,
>(
    system: &mut System<S>,
    system_functions: &mut HooksStorage<S, S::Allocator>,
    mut memories: RunnerMemoryBuffers<'a>,
    transaction: &AbiEncodedTransaction<S::Allocator>,
    is_priority_op: bool,
    tracer: &mut impl Tracer<S>,
    validator: &mut impl TxValidator<S>,
) -> Result<ZkTxResult<'a>, BootloaderSubsystemError>
where
    S::IO: IOSubsystemExt,
    S::Metadata: ZkSpecificPricingMetadata
        + BasicMetadata<S::IOTypes, TransactionMetadata = TxLevelMetadata<S::IOTypes>>,
{
    // The work done by the bootloader (outside of EE or EOA specific
    // computation) is charged as part of the intrinsic gas cost.
    let gas_limit = transaction.gas_limit.read();

    // The invariant that the user deposited more than the value needed
    // for the transaction must be enforced on L1, but we double-check it here
    // Note, that for now the property of block.base <= tx.maxFeePerGas does not work
    // for L1->L2 transactions. For now, these transactions are processed with the same gasPrice
    // they were provided on L1. In the future, we may apply a new logic for it.
    let gas_price = transaction.max_fee_per_gas.read();

    // For L1->L2 transactions we always use the pubdata price provided by the transaction.
    // This is needed to ensure DDoS protection. All the excess expenditure
    // will be refunded to the user.
    let gas_per_pubdata = transaction.gas_per_pubdata_limit.read();

    // It's important to ensure that the amount of pubdata estimated during
    // transaction simulation is never less than the amount estimated
    // during execution of the same transaction.
    // Since the introduction of the asset tracker calls during the base token
    // minting, the pubdata for the storage diff of the first of such calls
    // depends indirectly on the gas price, which can fluctuate from simulation
    // to execution.
    // Even though the pubdata for this storage change is already considered in
    // L1_TX_INTRINSIC_PUBDATA (for the calls related to refunds), we need to
    // add an extra worst case when in simulation to ensure that simulation
    // never underestimates pubdata used.
    let extra_pubdata_for_simulation = if Config::SIMULATION {
        ASSET_TRACKER_INTRINSIC_PUBDATA
    } else {
        0
    };
    let intrinsic_pubdata = L1_TX_INTRINSIC_PUBDATA + extra_pubdata_for_simulation;

    // Compute resource and fee information, making sure we handle
    // all possible validation errors carefully.
    // L1 transactions cannot be invalidated. Therefore, the following
    // function makes sure L1 transactions are processable even when
    // some checks that should be performed by the L1 don't hold.
    let ResourceAndFeeInfo {
        resources:
            ResourcesForTx {
                main_resources: mut resources,
                withheld: withheld_resources,
                intrinsic_computational_native_charged,
            },
        native_per_gas,
        native_per_pubdata,
        minimal_gas_used,
    } = prepare_and_check_resources::<S, Config>(
        system,
        transaction,
        is_priority_op,
        gas_limit,
        gas_price,
        gas_per_pubdata,
        intrinsic_pubdata,
    )?;

    // Just used for computing native used
    let initial_resources = resources.clone();

    let tx_internal_cost = gas_price
        .checked_mul(U256::from(gas_limit))
        .ok_or(internal_error!("gp*gl"))?;
    let value = transaction.value.read();
    let total_deposited = transaction.reserved[0].read();
    require_internal!(
        total_deposited >= tx_internal_cost,
        "Deposited amount too low",
        system
    )?;

    // TODO: l1 transaction preparation (marking factory deps)

    let (tx_hash, preparation_out_of_resources): (Bytes32, bool) =
        match transaction.calculate_hash(&mut resources) {
            Ok(h) => (h.into(), false),
            Err(e) => {
                match e {
                    TxError::Internal(e) if !matches!(e.root_cause(), RootCause::Runtime(_)) => {
                        return Err(e);
                    }
                    // Only way hashing of L1 tx can fail due to Validation or Runtime is
                    // due to running out of native.
                    _ => {
                        system_log!(
                            system,
                            "Transaction preparation exhausted native resources: {e:?}\n"
                        );

                        resources.exhaust_ergs();
                        // We need to compute the hash anyways, we do with inf resources
                        let mut inf_resources = S::Resources::FORMAL_INFINITE;
                        (
                            transaction
                                .calculate_hash(&mut inf_resources)
                                .expect("must succeed")
                                .into(),
                            true,
                        )
                    }
                }
            }
        };

    let l1_chain_id = read_l1_chain_id(system);

    // pubdata_info = (pubdata_used, to_charge_for_pubdata) can be cached
    // to used in the refund step only if the execution succeeded.
    // Otherwise, this value needs to be recomputed after reverting
    // state changes.
    let (is_success, saved_returndata, pubdata_info, resources_before_refund, mut memories) =
        if !preparation_out_of_resources {
            // Take a snapshot in case we need to revert due to out of native.
            let rollback_handle = system.start_global_frame()?;

            // Tx execution
            let from = transaction.from.read();
            let to = transaction.to.read();
            match execute_l1_transaction_and_notify_result::<S, Config>(
                system,
                system_functions,
                &mut memories,
                &transaction,
                from,
                to,
                value,
                l1_chain_id,
                native_per_pubdata,
                &mut resources,
                withheld_resources,
                tracer,
                validator,
            ) {
                Ok(outcome) => {
                    let pubdata_info = if outcome.is_success {
                        system.finish_global_frame(None)?;
                        Some((outcome.pubdata_used, outcome.to_charge_for_pubdata))
                    } else {
                        system.finish_global_frame(Some(&rollback_handle))?;
                        None
                    };
                    (
                        outcome.is_success,
                        outcome.returndata,
                        pubdata_info,
                        outcome.resources_before_refund,
                        memories,
                    )
                }
                Err(e) => {
                    match e.root_cause() {
                        // Out of native / memory is converted to a top-level
                        // revert so post-execution L1 accounting can still run.
                        RootCause::Runtime(runtime @ RuntimeError::FatalRuntimeError(_)) => {
                            system_log!(
                                system,
                                "L1 transaction ran out of native resources or memory {runtime:?}\n"
                            );
                            resources.exhaust_ergs();
                            system.finish_global_frame(Some(&rollback_handle))?;
                            (
                                false,
                                Vec::new_in(system.get_allocator()),
                                None,
                                S::Resources::empty(),
                                memories,
                            )
                        }
                        _ => {
                            system.finish_global_frame(Some(&rollback_handle))?;
                            return Err(e);
                        }
                    }
                }
            }
        } else {
            (
                false,
                Vec::new_in(system.get_allocator()),
                None,
                S::Resources::empty(),
                memories,
            )
        };

    // Compute gas to refund
    // TODO: consider operator refund
    #[allow(unused_variables)]
    let (pubdata_used, to_charge_for_pubdata) = match pubdata_info {
        Some(r) => r,
        None => get_resources_to_charge_for_pubdata(system, native_per_pubdata, None)?,
    };

    #[allow(unused_variables)]
    let RefundInfo {
        gas_used,
        evm_refund,
        native_used,
    } = compute_gas_refund(
        system,
        to_charge_for_pubdata,
        gas_limit,
        minimal_gas_used,
        native_per_gas,
        &mut resources,
    )?;

    // Transfer fee from treasury to operator
    // We already checked that total_gas_refund <= gas_limit
    let pay_to_operator = U256::from(gas_used)
        .checked_mul(U256::from(gas_price))
        .ok_or(internal_error!("gu*gp"))?;
    // Use FORMAL_INFINITE for post-execution operations (coinbase transfer,
    // asset tracker notifications, refund transfer, log emission).
    // These cannot fail due to resource exhaustion. Their native cost is
    // accounted for as intrinsic and is not included in
    // computational_native_used (native_used only reflects native for
    // pubdata + native used for charged computation).
    let mut inf_resources = S::Resources::FORMAL_INFINITE;

    let coinbase = system.get_coinbase();
    // Mint operator fee portion of the deposit to coinbase.
    mint_base_token::<S, Config>(
        system,
        system_functions,
        memories.reborrow(),
        &pay_to_operator,
        &coinbase,
        l1_chain_id,
        &mut inf_resources,
        tracer,
        validator,
    )
    .map_err(|e| match e.root_cause() {
        RootCause::Runtime(RuntimeError::OutOfErgs(_)) => {
            internal_error!("Out of ergs on infinite ergs").into()
        }
        RootCause::Runtime(RuntimeError::FatalRuntimeError(_)) => {
            internal_error!("Out of native on infinite").into()
        }
        _ => e,
    })?;

    // Refund
    let to_refund_recipient = if !is_success {
        // Upgrade transactions must always succeed
        if !is_priority_op {
            return Err(internal_error!("Upgrade transaction must succeed").into());
        }
        // If the transaction reverts, then the minting of the deposit
        // reverted too. Thus, we need to refund the entire deposit minus
        // the fee (`pay_to_operator`).
        total_deposited
            .checked_sub(pay_to_operator)
            .ok_or(internal_error!("td-pto"))
    } else {
        // If the transaction succeeds, then it is assumed that the
        // mint to `from` address was transferred correctly too.
        // In this case, we just refund the unused gas that the
        // transaction paid for initially.
        let prepaid_fee = gas_price
            .checked_mul(U256::from(transaction.gas_limit.read()))
            .ok_or(internal_error!("gp*gl"))?;
        prepaid_fee
            .checked_sub(pay_to_operator)
            .ok_or(internal_error!("pf-pto"))
    }?;
    // Mint refund portion of the deposit to the refund recipient.
    if to_refund_recipient > U256::ZERO {
        let refund_recipient = u256_to_b160_checked(transaction.reserved[1].read());
        mint_base_token::<S, Config>(
            system,
            system_functions,
            memories.reborrow(),
            &to_refund_recipient,
            &refund_recipient,
            l1_chain_id,
            &mut inf_resources,
            tracer,
            validator,
        )
        .map_err(|e| -> BootloaderSubsystemError {
            match e.root_cause() {
                RootCause::Runtime(RuntimeError::OutOfErgs(_)) => {
                    internal_error!("Out of ergs on infinite ergs").into()
                }
                RootCause::Runtime(RuntimeError::FatalRuntimeError(_)) => {
                    internal_error!("Out of native on infinite").into()
                }
                _ => e,
            }
        })?;
    }

    // Emit log
    // We don't send logs for upgrade txs by protocol convention
    if is_priority_op {
        system.io.emit_l1_l2_tx_log(
            ExecutionEnvironmentType::NoEE,
            &mut inf_resources,
            tx_hash,
            is_success,
        )?;
    }

    // Add back the intrinsic native charged in get_resources_for_tx,
    // as initial_resources doesn't include them.
    let computational_native_used = resources_before_refund
        .diff(initial_resources)
        .native()
        .as_u64()
        + intrinsic_computational_native_charged;

    // Restore the saved returndata into the return buffer so that the
    // ExecutionResult can borrow it with the correct lifetime.
    let returndata_slice = if saved_returndata.is_empty() {
        &[][..]
    } else {
        let buf = &mut memories.return_data[..saved_returndata.len()];
        buf.write_copy_of_slice(&saved_returndata)
    };

    let result = if is_success {
        ExecutionResult::Success {
            output: ExecutionOutput::Call(returndata_slice),
        }
    } else {
        ExecutionResult::Revert {
            output: returndata_slice,
        }
    };

    Ok(ZkTxResult {
        result,
        tx_hash,
        is_priority_tx: is_priority_op,
        is_upgrade_tx: !is_priority_op,
        is_service_tx: false,
        gas_used,
        gas_refunded: evm_refund,
        computational_native_used,
        native_used,
        pubdata_used: pubdata_used + intrinsic_pubdata,
        blob_gas_used: 0,
    })
}

struct ResourceAndFeeInfo<S: EthereumLikeTypes> {
    resources: ResourcesForTx<S>,
    native_per_pubdata: u64,
    native_per_gas: u64,
    minimal_gas_used: u64,
}

///
/// Compute and perform some checks on fee/resource parameters.
/// This function handles cases that for L2 transactions would be
/// validation errors, as "invalidating" an L1 transaction can halt
/// the chain (due to the priority queue).
/// Note that the "validation errors" are practically unreachable, as
/// gas_limit, gas_price and gas_per_pubdata are either checked or set
/// by the L1 contracts. We decide to handle these cases as a fallback in
/// case the L1 contracts aren't properly updated to reflect a change in
/// ZKsync OS.
/// The approach is to use saturating arithmetic and emit a system
/// log if this situation ever happens.
///
fn prepare_and_check_resources<
    'a,
    S: EthereumLikeTypes + 'a,
    Config: BasicBootloaderExecutionConfig,
>(
    system: &mut System<S>,
    transaction: &AbiEncodedTransaction<S::Allocator>,
    is_priority_op: bool,
    gas_limit: u64,
    gas_price: U256,
    gas_per_pubdata: u32,
    intrinsic_pubdata: u64,
) -> Result<ResourceAndFeeInfo<S>, BootloaderSubsystemError>
where
    S::IO: IOSubsystemExt,
    S::Metadata: ZkSpecificPricingMetadata
        + BasicMetadata<S::IOTypes, TransactionMetadata = TxLevelMetadata<S::IOTypes>>,
{
    // For L1->L2 txs, we use a constant native price to avoid censorship.
    let native_price = L1_TX_NATIVE_PRICE;
    let native_per_gas = if is_priority_op {
        if gas_price.is_zero() {
            if Config::SIMULATION {
                u256_try_to_u64(&system.get_eip1559_basefee().div_ceil(native_price))
                    .unwrap_or_else(|| {
                        system_log!(
                            system,
                            "Native per gas calculation for L1 tx overflows, using saturated arithmetic instead");
                        u64::MAX
                    })
            } else {
                FREE_L1_TX_NATIVE_PER_GAS
            }
        } else {
            u256_try_to_u64(&gas_price.div_ceil(native_price)).unwrap_or_else(|| {
                system_log!(
                    system,
                    "Native per gas calculation for L1 tx overflows, using saturated arithmetic instead");
                u64::MAX
            })
        }
    } else {
        // Upgrade txs are paid by the protocol, so we use a fixed native per gas
        FREE_L1_TX_NATIVE_PER_GAS
    };

    let native_per_pubdata = (gas_per_pubdata as u64)
        .checked_mul(native_per_gas)
        .unwrap_or_else(|| {
            system_log!(
                system,
                "Native per pubdata calculation for L1 tx overflows, using saturated arithmetic instead");
                u64::MAX
        });

    let native_prepaid_from_gas = native_per_gas.checked_mul(gas_limit)
        .unwrap_or_else(|| {
            system_log!(
                system,
                "Native prepaid from gas calculation for L1 tx overflows, using saturated arithmetic instead");
                u64::MAX
        });

    let (calldata_tokens, minimal_gas_used) =
        compute_calldata_tokens(system, transaction.calldata(), true);

    // With L1ResourcesPolicy, this returns Result<ResourcesForTx<S>, BootloaderSubsystemError>
    // Validation errors are type-safe impossible - they're logged and saturated instead
    let resources = create_resources_for_tx::<S, L1ResourcesPolicy>(
        system,
        gas_limit,
        native_per_gas == 0,
        native_prepaid_from_gas,
        native_per_pubdata,
        false, // is_deployment
        transaction.calldata().len() as u64,
        calldata_tokens,
        L1_TX_INTRINSIC_L2_GAS,
        intrinsic_pubdata,
        L1_TX_INTRINSIC_NATIVE_COST,
    )?;

    // L1 transactions might have a gas limit < minimal_gas_used. This should be
    // prevented by L1 validation, but we log and saturate if it happens.
    if gas_limit < minimal_gas_used {
        system_log!(
            system,
            "L1 tx gas limit below intrinsic cost, using saturated arithmetic instead"
        );
    }
    // Pick the min to keep processing L1 txs even if the L1 validation is wrong.
    let minimal_gas_used = minimal_gas_used.min(gas_limit);

    Ok(ResourceAndFeeInfo {
        resources,
        native_per_pubdata,
        native_per_gas,
        minimal_gas_used,
    })
}

/// Outcome of executing the L1 transaction body.
///
/// This deliberately does NOT carry `ExecutionResult<'a>` (which borrows
/// returndata from the runner memory buffers). Keeping the buffers
/// un-borrowed lets `process_l1_transaction` reborrow them for the
/// post-execution asset-tracker notification calls. The returndata from
/// the main tx call is saved in `returndata` so it can be restored
/// into the return buffer after the asset-tracker calls complete.
struct L1ExecutionOutcome<S: EthereumLikeTypes> {
    is_success: bool,
    returndata: Vec<u8, S::Allocator>,
    pubdata_used: u64,
    to_charge_for_pubdata: S::Resources,
    resources_before_refund: S::Resources,
}

fn execute_l1_transaction_and_notify_result<
    'a,
    S: EthereumLikeTypes + 'a,
    Config: BasicBootloaderExecutionConfig,
>(
    system: &mut System<S>,
    system_functions: &mut HooksStorage<S, S::Allocator>,
    memories: &mut RunnerMemoryBuffers<'a>,
    transaction: &AbiEncodedTransaction<S::Allocator>,
    from: B160,
    to: B160,
    value: U256,
    l1_chain_id: U256,
    native_per_pubdata: u64,
    resources: &mut S::Resources,
    withheld_resources: S::Resources,
    tracer: &mut impl Tracer<S>,
    validator: &mut impl TxValidator<S>,
) -> Result<L1ExecutionOutcome<S>, BootloaderSubsystemError>
where
    S::IO: IOSubsystemExt,
    S::Metadata: ZkSpecificPricingMetadata
        + BasicMetadata<S::IOTypes, TransactionMetadata = TxLevelMetadata<S::IOTypes>>,
{
    system_log!(system, "Executing L1 transaction\n");

    let gas_price = U256::from(transaction.max_fee_per_gas.read());
    system.set_tx_context(TxLevelMetadata {
        tx_gas_price: gas_price,
        tx_origin: from,
        blobs: ArrayVec::new(),
    });

    // Start a frame, to revert minting of value if execution fails
    let rollback_handle = system.start_global_frame()?;

    // Fee payment is done in two steps.
    // The first step is here, where the max fee (gas limit * gas price)
    // is committed to.
    // This fee is deducted from the deposit to be minted (transferred from
    // the treasury).
    // After the execution of the transaction, the actual fee
    // (gas used * gas price) is paid to the operator, while the
    // rest of the max fee is refunded.
    let max_fee_commitment = gas_price
        .checked_mul(U256::from(transaction.gas_limit.read()))
        .ok_or(internal_error!("gp*gl"))?;
    let total_deposited = transaction.reserved[0].read();
    let to_transfer = total_deposited
        .checked_sub(max_fee_commitment)
        .ok_or(internal_error!("mfc+tic"))?;

    // Transfer value from treasury to sender (the deposit minus max fee).
    // We want to ensure that the simulation of a transaction
    // never underestimates gas/pubdata compared to the actual execution
    // of said transaction.
    // During simulation the gas price is typically set to 0. So we need
    // to be conservative about operations that incur in gas/pubdata depending
    // on the value of the fee. For that reason, we always perform the
    // following transfer on simulation, and avoid compressing the pubdata
    // for the balance changes resulting from it.
    //
    // Mint the value portion of the deposit (total deposited minus max fee)
    // to the sender inside the execution frame, it does not
    // persist if the main tx body reverts.
    //
    // Use with_infinite_ergs so the call cannot fail due to out-of-gas,
    // but native consumption is still tracked against the user's resources.
    if to_transfer > U256::ZERO || Config::SIMULATION {
        resources
            .with_infinite_ergs(|inf_resources| {
                mint_base_token::<S, Config>(
                    system,
                    system_functions,
                    memories.reborrow(),
                    &to_transfer,
                    &from,
                    l1_chain_id,
                    inf_resources,
                    tracer,
                    validator,
                )
            })
            .map_err(|e| match e.root_cause() {
                RootCause::Runtime(RuntimeError::OutOfErgs(_)) => {
                    system_log!(
                        system,
                        "Out of ergs on infinite ergs: inner error was {e:?}"
                    );
                    BootloaderSubsystemError::LeafDefect(internal_error!(
                        "Out of ergs on infinite ergs"
                    ))
                }
                _ => e,
            })?;
    }

    let resources_for_tx = resources.clone();

    // transaction is in managed region, so we can recast it back
    let calldata = transaction.calldata();

    // TODO: add support for deployment transactions,
    // probably unify with execution logic for EOA
    let (reverted, returndata) =
        match BasicBootloader::<S, ZkTransactionFlowOnlyEOA<S>>::run_single_interaction(
            system,
            system_functions,
            memories.reborrow(),
            calldata,
            &from,
            &to,
            resources_for_tx,
            &value,
            false,
            tracer,
            validator,
        ) {
            Ok(CompletedExecution {
                resources_returned,
                result,
            }) => {
                let reverted = result.failed();
                // Save the returndata before asset-tracker calls overwrite
                // the runner memory buffer. Use the system allocator (not
                // global) to avoid panics in proving mode.
                let rd = result.return_values().returndata;
                let mut returndata = Vec::with_capacity_in(rd.len(), system.get_allocator());
                returndata.extend_from_slice(rd);
                *resources = resources_returned;
                system.finish_global_frame(reverted.then_some(&rollback_handle))?;
                (reverted, returndata)
            }
            Err(e) => {
                system.finish_global_frame(Some(&rollback_handle))?;
                return Err(e);
            }
        };

    system_log!(system, "Main TX body successful = {}\n", !reverted);

    // Just used for computing native used
    // Needs to use the resources before we reclaim withheld
    let resources_before_refund = resources.clone();

    // After the transaction is executed, we reclaim the withheld resources.
    // This is needed to ensure correct "gas_used" calculation, also these
    // resources could be spent for pubdata.
    resources.reclaim_withheld(withheld_resources);

    let (enough, to_charge_for_pubdata, pubdata_used) =
        check_enough_resources_for_pubdata(system, native_per_pubdata, resources, None)?;
    let is_success = !reverted && enough;
    if !enough {
        system_log!(system, "Not enough gas for pubdata after execution\n");
        // Burn all remaining ergs.
        resources.exhaust_ergs();
    }

    Ok(L1ExecutionOutcome {
        is_success,
        returndata,
        pubdata_used,
        to_charge_for_pubdata,
        resources_before_refund,
    })
}

/// Notifies L2AssetTracker and transfers base tokens from the treasury
/// to [to] in a single operation.
///
/// This function replicates the behaviour of the corresponding call from bootloader to era contracts:
/// https://github.com/matter-labs/era-contracts/blob/2f024c5764e7a873ce1dda5fb990331559996441/l1-contracts/contracts/l2-system/era/L2BaseTokenEra.sol#L86
///
/// Notify the asset tracker BEFORE changing balances/totalSupply, so that
/// _needToForceSetAssetMigrationOnL2 can use totalSupply() == 0 consistently.
fn mint_base_token<'a, S: EthereumLikeTypes + 'a, Config: BasicBootloaderExecutionConfig>(
    system: &mut System<S>,
    system_functions: &mut HooksStorage<S, S::Allocator>,
    memories: RunnerMemoryBuffers<'a>,
    amount: &U256,
    to: &B160,
    l1_chain_id: U256,
    resources: &mut S::Resources,
    tracer: &mut impl Tracer<S>,
    validator: &mut impl TxValidator<S>,
) -> Result<(), BootloaderSubsystemError>
where
    S::IO: IOSubsystemExt,
    S::Metadata: ZkSpecificPricingMetadata
        + BasicMetadata<S::IOTypes, TransactionMetadata = TxLevelMetadata<S::IOTypes>>,
{
    notify_l2_asset_tracker::<S, Config>(
        system,
        system_functions,
        memories,
        *amount,
        l1_chain_id,
        resources,
        tracer,
        validator,
    )?;

    transfer_from_treasury::<S>(system, amount, to, resources, Config::SIMULATION)
}

/// Transfers [value] from the treasury account to address [to].
///
/// Returns `TreasuryTransferFailed` if:
/// - Treasury has insufficient balance
/// - Balance overflow occurs
pub fn transfer_from_treasury<'a, S: EthereumLikeTypes + 'a>(
    system: &mut System<S>,
    nominal_token_value: &U256,
    to: &B160,
    resources: &mut S::Resources,
    fee_payment_in_simulation: bool,
) -> Result<(), BootloaderSubsystemError>
where
    S::IO: IOSubsystemExt,
{
    system_log!(
        system,
        "Transferring {nominal_token_value:?} tokens from treasury to {to:?}\n"
    );

    let treasury_address = &system_hooks::addresses_constants::BASE_TOKEN_HOLDER_ADDRESS;

    let _ = system
        .io
        .update_account_nominal_token_balance(
            zk_ee::execution_environment_type::ExecutionEnvironmentType::EVM,
            resources,
            treasury_address,
            nominal_token_value,
            true, // true = subtract from balance
            fee_payment_in_simulation,
        )
        .map_err(|e| -> BootloaderSubsystemError {
            match e {
                SubsystemError::LeafUsage(balance_error) => {
                    system_log!(system, "Treasury transfer failed: {balance_error:?}");
                    interface_error!(BootloaderInterfaceError::TreasuryTransferFailed)
                }
                _ => wrap_error!(e),
            }
        })?;

    let _ = system
        .io
        .update_account_nominal_token_balance(
            zk_ee::execution_environment_type::ExecutionEnvironmentType::EVM,
            resources,
            to,
            nominal_token_value,
            false, // false = add to balance
            fee_payment_in_simulation,
        )
        .map_err(|e| -> BootloaderSubsystemError {
            match e {
                SubsystemError::LeafUsage(balance_error) => {
                    system_log!(system, "Error while minting: {balance_error:?}");
                    interface_error!(BootloaderInterfaceError::MintingBalanceOverflow)
                }
                _ => wrap_error!(e),
            }
        })?;

    Ok(())
}

/// Notify L2AssetTracker about base token bridging from L1.
///
/// Calls handleFinalizeBaseTokenBridgingOnL2(uint256 _fromChainId, uint256 _amount)
/// as L2_BASE_TOKEN_ADDRESS (0x800a) to pass the onlyBaseTokenHolderOrL2BaseToken modifier.
///
/// This is called separately for each token movement (value mint, operator
/// payment, refund) so that the asset tracker's accounting stays correct even
/// if the main transaction body reverts.
///
/// Resource usage depends on the caller — value-mint tracks native against user resources;
/// operator-fee and refund use FORMAL_INFINITE.
///
/// Failure halts block processing — if the asset tracker reverts, the
/// chain's token accounting would be inconsistent, so we treat it as
/// fatal rather than silently continuing with incorrect bookkeeping.
///
/// If no contract is deployed at L2AssetTracker, the call succeeds silently
/// (a call to an empty address returns success with no returndata in EVM).
/// However, we are certain that L2AssetTracker is available after the upgrade.
fn notify_l2_asset_tracker<'a, S: EthereumLikeTypes + 'a, Config: BasicBootloaderExecutionConfig>(
    system: &mut System<S>,
    system_functions: &mut HooksStorage<S, S::Allocator>,
    memories: RunnerMemoryBuffers<'a>,
    amount: U256,
    l1_chain_id: U256,
    resources: &mut S::Resources,
    tracer: &mut impl Tracer<S>,
    validator: &mut impl TxValidator<S>,
) -> Result<(), BootloaderSubsystemError>
where
    S::IO: IOSubsystemExt,
    S::Metadata: ZkSpecificPricingMetadata
        + BasicMetadata<S::IOTypes, TransactionMetadata = TxLevelMetadata<S::IOTypes>>,
{
    if amount > U256::ZERO || Config::SIMULATION {
        // Encode calldata for handleFinalizeBaseTokenBridgingOnL2(uint256,uint256):
        // selector 0x03117c8c + abi-encoded (fromChainId, amount)
        let mut calldata = [0u8; 68];
        calldata[0..4].copy_from_slice(&[0x03, 0x11, 0x7c, 0x8c]);
        calldata[4..36].copy_from_slice(&l1_chain_id.to_be_bytes::<32>());
        calldata[36..68].copy_from_slice(&amount.to_be_bytes::<32>());

        let failed = resources.with_infinite_ergs(|inf_ergs| {
            let CompletedExecution {
                resources_returned,
                result: asset_tracker_result,
            } = BasicBootloader::<S, ZkTransactionFlowOnlyEOA<S>>::run_single_interaction(
                system,
                system_functions,
                memories,
                &calldata,
                &L2_BASE_TOKEN_ADDRESS,
                &L2_ASSET_TRACKER_ADDRESS,
                inf_ergs.clone(),
                &U256::ZERO,
                true, // should_make_frame - isolate state changes
                tracer,
                validator,
            )?;
            // Overwrite resources inside the closure so that
            // with_infinite_ergs correctly restores ergs afterwards.
            *inf_ergs = resources_returned;
            Ok::<bool, BootloaderSubsystemError>(asset_tracker_result.failed())
        })?;

        if failed {
            system_log!(
                system,
                "L2AssetTracker.handleFinalizeBaseTokenBridgingOnL2 failed for amount {amount:?}\n"
            );
            // A revert here means the chain's token accounting would be inconsistent.
            // Treated as a fatal system error — block processing cannot continue.
            return Err(internal_error!(
                "L2AssetTracker.handleFinalizeBaseTokenBridgingOnL2 reverted"
            )
            .into());
        }
    }
    Ok(())
}

/// Reads L1 chain id from L2AssetTracker storage.
///
/// This is the chain tokens are bridged *from* during L1→L2 deposits,
/// passed as `_fromChainId` to `handleFinalizeBaseTokenBridgingOnL2`.
fn read_l1_chain_id<S: EthereumLikeTypes>(system: &mut System<S>) -> U256
where
    S::IO: IOSubsystemExt,
{
    // L2AssetTracker storage layout (verified via `forge inspect`):
    //   slots 0-100:   Initializable + OwnableUpgradeable + Ownable2StepUpgradeable
    //   slots 101-150: Ownable2Step __gap
    //   slot 151:      mapping chainBalance
    //   slot 152:      mapping assetMigrationNumber
    //   slot 153:      mapping isAssetRegistered
    //   slot 154:      uint256 L1_CHAIN_ID
    let l1_chain_id_slot = Bytes32::from_u256_be(&U256::from(154));
    let mut inf_resources = S::Resources::FORMAL_INFINITE;
    let chain_id = system
        .io
        .storage_read::<false>(
            ExecutionEnvironmentType::NoEE,
            &mut inf_resources,
            &L2_ASSET_TRACKER_ADDRESS,
            &l1_chain_id_slot,
        )
        .expect("must read L2AssetTracker L1_CHAIN_ID");
    U256::from_be_bytes(chain_id.as_u8_array())
}
