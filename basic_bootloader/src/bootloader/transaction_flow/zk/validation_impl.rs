use super::TxContextForPreAndPostProcessing;
use crate::bootloader::constants::*;
use crate::bootloader::errors::{InvalidTransaction, TxError};
use crate::bootloader::transaction::access_list::parse_and_warm_up_access_list;
use crate::bootloader::transaction::blobs::parse_blobs_list;
use crate::bootloader::transaction::{charge_keccak, Transaction};
use crate::bootloader::transaction_flow::gas_helpers::{
    create_resources_for_tx, get_gas_price, L2ResourcesPolicy,
};
use crate::bootloader::BasicBootloaderExecutionConfig;
use crate::require;
use basic_system::cost_constants::ECRECOVER_NATIVE_COST;
use core::fmt::Write;
use crypto::secp256k1::SECP256K1N_HALF;
use evm_interpreter::ERGS_PER_GAS;
use ruint::aliases::{B160, U256};
use zk_ee::execution_environment_type::ExecutionEnvironmentType;
use zk_ee::memory::ArrayBuilder;
use zk_ee::system::errors::interface::InterfaceError;
use zk_ee::system::errors::runtime::RuntimeError;
use zk_ee::system::errors::subsystem::SubsystemError;
use zk_ee::system::metadata::basic_metadata::BasicTransactionMetadata;
use zk_ee::system::metadata::basic_metadata::{BasicMetadata, ZkSpecificPricingMetadata};
use zk_ee::system::metadata::zk_metadata::TxLevelMetadata;
use zk_ee::system::resources::Computational;
use zk_ee::system::tracer::Tracer;
use zk_ee::system::{errors::system::SystemError, EthereumLikeTypes, System};
use zk_ee::system::{AccountDataRequest, SystemFunctionsExt};
use zk_ee::system::{Ergs, IOSubsystemExt, Resources};
use zk_ee::system::{IOSubsystem, NonceError};
use zk_ee::system::{Resource, SystemTypes};
use zk_ee::system::{GAS_PER_BLOB, MAX_BLOBS_PER_BLOCK};
use zk_ee::system_log;
use zk_ee::{internal_error, out_of_native_resources};
use zk_ee::{utils::*, wrap_error};

///
/// Will perform basic validation, namely - checking signature, minimal resource requirements for transaction validity,
/// and will pre-charge sender to cover worst case cost. It may perform IO if needed to e.g. warm up some storage slots,
/// or mark delegation
///
/// NOTE: This function will open and close IO frame
pub(crate) fn validate_and_compute_fee_for_transaction<
    S: EthereumLikeTypes,
    Config: BasicBootloaderExecutionConfig,
>(
    system: &mut System<S>,
    transaction: &mut Transaction<S::Allocator>,
    _tracer: &mut impl Tracer<S>,
) -> Result<TxContextForPreAndPostProcessing<S>, TxError>
where
    S::IO: IOSubsystemExt,
    S::Metadata: ZkSpecificPricingMetadata
        + BasicMetadata<S::IOTypes, TransactionMetadata = TxLevelMetadata<S::IOTypes>>,
{
    // NOTE: this function checks the transaction validity a-la Ethereum one,
    // but also takes into account ZK/L2 specific pieces, such as pubdata in state-diffs model,
    // or heavy mismatch between Ethereum/EVM cost model and proving complexity

    // safe to panic, validated by the structure
    let from = *transaction.from();
    let tx_gas_limit = transaction.gas_limit();

    // we perform single check to make sure that we can use saturating operations to accumulate some costs,
    // and even if those would saturate, we can still catch this case
    require!(
        tx_gas_limit.saturating_mul(ERGS_PER_GAS) < u64::MAX,
        InvalidTransaction::CallerGasLimitTooHigh,
        system
    )?;

    let calldata = transaction.calldata();

    // Validate block-level invariants (for non-service transactions)
    if !transaction.is_service() {
        {
            // Validate that the transaction's gas limit is not larger than
            // the block's gas limit.
            let block_gas_limit = system.get_gas_limit();
            // First, check block gas limit can be represented as ergs.
            require!(
                block_gas_limit <= MAX_BLOCK_GAS_LIMIT,
                InvalidTransaction::BlockGasLimitTooHigh,
                system
            )?;
            require!(
                tx_gas_limit <= block_gas_limit,
                InvalidTransaction::CallerGasLimitMoreThanBlock,
                system
            )?;
        }
    }

    // EIP-7623
    let (calldata_tokens, minimal_gas_used) = compute_calldata_tokens(system, calldata, false);
    #[cfg(feature = "eip_7623")]
    require!(
        minimal_gas_used <= tx_gas_limit,
        InvalidTransaction::EIP7623IntrinsicGasIsTooLow,
        system
    )?;

    let pubdata_price = system.get_pubdata_price();
    let native_price = system.get_native_price();

    let gas_price = if transaction.is_service() {
        // Service transactions do not pay gas fees,
        // their gas price is allowed to be < block base fee.
        U256::ZERO
    } else {
        get_gas_price::<S, Config>(
            system,
            transaction.max_fee_per_gas(),
            transaction.max_priority_fee_per_gas(),
        )?
    };

    let native_per_gas = {
        if native_price.is_zero() {
            return Err(internal_error!("Native price cannot be 0").into());
        }

        if cfg!(feature = "resources_for_tester") {
            crate::bootloader::constants::TESTER_NATIVE_PER_GAS
        } else if Config::SIMULATION && gas_price.is_zero() {
            // For simulation, if gas price isn't set, we use base fee
            // for native calculation
            u256_try_to_u64(&system.get_eip1559_basefee().div_ceil(native_price)).ok_or(
                TxError::Validation(InvalidTransaction::NativeResourcesAreTooExpensive),
            )?
        } else {
            u256_try_to_u64(&gas_price.div_ceil(native_price)).ok_or(TxError::Validation(
                InvalidTransaction::NativeResourcesAreTooExpensive,
            ))?
        }
    };

    // We checked native_price != 0 above
    let native_per_pubdata = u256_try_to_u64(&pubdata_price.wrapping_div(native_price))
        .ok_or(TxError::Validation(InvalidTransaction::PubdataPriceTooHigh))?;
    let native_prepaid_from_gas = native_per_gas.saturating_mul(tx_gas_limit);

    // Now we will materialize resources, from which we will try to charge intrinsic cost on top
    let mut tx_resources = create_resources_for_tx::<S, L2ResourcesPolicy>(
        system,
        tx_gas_limit,
        native_per_gas == 0,
        native_prepaid_from_gas,
        native_per_pubdata,
        transaction.is_deployment().is_some(),
        calldata.len() as u64,
        calldata_tokens,
        L2_TX_INTRINSIC_GAS,
        L2_TX_INTRINSIC_PUBDATA,
        L2_TX_INTRINSIC_NATIVE_COST,
    )?;

    system_log!(
        system,
        "Prepared resources for transaction: {:?}\n",
        &tx_resources
    );

    // NOTE: we provided a "hint" for "from", so it's sequencer's risks here:
    // - either "from" is valid at it has at least enough balance, valid signature, etc to eventually pay for all validation
    // - or we will perform non-mutating operations without any payment

    // steps below are all not free, so the choice there is rather arbitrary. Let's first check the signature, as it's compute-only

    // We have to charge native for this hash, as it's computed during parsing
    // for RLP-encoded transactions.
    // We over-estimate using the total tx length
    charge_keccak(transaction.len(), &mut tx_resources.main_resources)?;
    let suggested_signed_hash: Bytes32 = transaction.signed_hash()?;

    // Only service transactions have no signature,
    // we don't even charge gas/native related to ecrecover for them.
    if let Some((parity, r, s)) = transaction.sig_parity_r_s() {
        // Even if we don't validate a signature, we still need to charge for ecrecover for equivalent behavior
        // Note that gas is charged already in intrinsic cost, so now
        // we only need to charge native resources.
        if !Config::VALIDATE_EOA_SIGNATURE | Config::SIMULATION {
            tx_resources
                .main_resources
                .charge(&Resources::from_ergs_and_native(
                    Ergs::empty(),
                    <<S as SystemTypes>::Resources as Resources>::Native::from_computational(
                        ECRECOVER_NATIVE_COST,
                    ),
                ))?;
        } else {
            if U256::from_be_slice(s) > U256::from_be_bytes(SECP256K1N_HALF) {
                return Err(InvalidTransaction::MalleableSignature.into());
            }

            let mut ecrecover_input = [0u8; 128];
            ecrecover_input[0..32].copy_from_slice(suggested_signed_hash.as_u8_array_ref());
            ecrecover_input[63] = (parity as u8) + 27;
            ecrecover_input[64..96][(32 - r.len())..].copy_from_slice(r);
            ecrecover_input[96..128][(32 - s.len())..].copy_from_slice(s);

            let mut ecrecover_output = ArrayBuilder::default();
            // We already charged gas for ecrecover in intrinsic cost, so we only need to charge native resources here.
            let mut logger = system.get_logger();
            let allocator = system.get_allocator();
            tx_resources
                .main_resources
                .with_infinite_ergs(|resources| {
                    S::SystemFunctionsExt::secp256k1_ec_recover(
                        ecrecover_input.as_slice(),
                        &mut ecrecover_output,
                        resources,
                        system.io.oracle(),
                        &mut logger,
                        allocator,
                    )
                    .map_err(SystemError::from)
                })?;

            if ecrecover_output.is_empty() {
                return Err(InvalidTransaction::IncorrectFrom {
                    recovered: B160::ZERO,
                    tx: from,
                }
                .into());
            }

            let recovered_from = B160::try_from_be_slice(&ecrecover_output.build()[12..])
                .ok_or(internal_error!("Invalid ecrecover return value"))?;

            if recovered_from != from {
                return Err(InvalidTransaction::IncorrectFrom {
                    recovered: recovered_from,
                    tx: from,
                }
                .into());
            }
        }
    };
    let tx_hash: Bytes32 = transaction.transaction_hash(&mut tx_resources.main_resources)?;

    // any IO starts here

    // now we can perform IO related parts. Getting originator's properties is included into the
    // intrinsic cost charnged above
    let originator_account_data =
        tx_resources
            .main_resources
            .with_infinite_ergs(|inf_resources| {
                system.io.read_account_properties(
                    ExecutionEnvironmentType::NoEE,
                    inf_resources,
                    &from,
                    AccountDataRequest::empty()
                        .with_ee_version()
                        .with_nonce()
                        .with_has_bytecode()
                        .with_is_delegated()
                        .with_nominal_token_balance(),
                )
            })?;

    // EIP-3607: Reject transactions from senders with deployed code modulo delegations
    // We skip it for simulation to allow simulate calls between contracts
    if Config::SIMULATION == false && originator_account_data.is_contract() {
        return Err(InvalidTransaction::RejectCallerWithCode.into());
    }

    // Originator's nonce is incremented before authorization list
    // skipped for service transactions, for which we do not track nonce
    let old_nonce = if transaction.nonce().is_some() {
        match tx_resources.main_resources.with_infinite_ergs(|resources| {
            system
                .io
                .increment_nonce(ExecutionEnvironmentType::NoEE, resources, &from, 1u64)
        }) {
            Ok(x) => Ok(x),
            Err(SubsystemError::LeafUsage(InterfaceError(NonceError::NonceOverflow, _))) => {
                return Err(TxError::Validation(
                    InvalidTransaction::NonceOverflowInTransaction,
                ))
            }
            Err(SubsystemError::LeafRuntime(runtime_error)) => match runtime_error {
                RuntimeError::FatalRuntimeError(_) => {
                    return Err(TxError::oon_as_validation(
                        out_of_native_resources!().into(),
                    ))
                }
                RuntimeError::OutOfErgs(_) => {
                    return Err(TxError::Validation(
                        InvalidTransaction::OutOfGasDuringValidation,
                    ))
                }
            },
            Err(e) => Err(wrap_error!(e)),
        }?
    } else {
        // For service transactions, nonce is not used
        0
    };

    if !Config::SIMULATION {
        // Nonce validation - skipped for service transactions
        if let Some(originator_expected_nonce) =
            transaction.nonce().as_ref().map(u256_to_u64_saturated)
        {
            let err = if old_nonce > originator_expected_nonce {
                TxError::Validation(InvalidTransaction::NonceTooLow {
                    tx: originator_expected_nonce,
                    state: old_nonce,
                })
            } else {
                TxError::Validation(InvalidTransaction::NonceTooHigh {
                    tx: originator_expected_nonce,
                    state: old_nonce,
                })
            };

            require!(old_nonce == originator_expected_nonce, err, system)?;
        }
    }

    // Access list
    parse_and_warm_up_access_list(system, &mut tx_resources.main_resources, &transaction)?;

    // Parse blobs, if any
    // No need to feature gate this part, as blobs() should return an empty list
    // for non-EIP4844 transactions.
    let block_base_fee_per_blob_gas = system.get_blob_base_fee_per_gas();

    #[cfg(not(feature = "eip-4844"))]
    crate::require_internal!(
        block_base_fee_per_blob_gas == U256::ONE,
        "Blob base fee should be set to 1 if EIP 4844 is disabled",
        system
    )?;

    let blobs = if let Some(blobs_list) = transaction.blobs() {
        let tx_max_fee_per_blob_gas = transaction.max_fee_per_blob_gas().ok_or(internal_error!(
            "Tx with blobs must define max_fee_per_blob_gas"
        ))?;

        if &block_base_fee_per_blob_gas > tx_max_fee_per_blob_gas && !Config::SIMULATION {
            return Err(TxError::Validation(
                InvalidTransaction::BlobBaseFeeGreaterThanMaxFeePerBlobGas,
            ));
        }

        match parse_blobs_list::<MAX_BLOBS_PER_BLOCK>(blobs_list) {
            Ok(blobs) => blobs,
            Err(e) => {
                return Err(e);
            }
        }
    } else {
        arrayvec::ArrayVec::new()
    };

    // Now we can apply access list and authorization list, while simultaneously charging for them
    // Parse, validate and apply authorization list, following EIP-7702
    #[cfg(feature = "eip-7702")]
    {
        if let Some(authorization_list) = transaction.authorization_list() {
            crate::bootloader::transaction::authorization_list:: parse_authorization_list_and_apply_delegations(
                    system,
                    &mut tx_resources.main_resources,
                    authorization_list,
                )?;
        }
    }

    // Balance check - originator must cover fee prepayment plus whatever "value" it would like to send along
    let Some(total_required_balance) = transaction.required_balance() else {
        return Err(TxError::Validation(
            InvalidTransaction::OverflowPaymentInTransaction,
        ));
    };
    if total_required_balance > originator_account_data.nominal_token_balance.0 {
        return Err(TxError::Validation(
            InvalidTransaction::LackOfFundForMaxFee {
                fee: total_required_balance,
                balance: originator_account_data.nominal_token_balance.0,
            },
        ));
    }

    system.set_tx_context(TxLevelMetadata {
        tx_origin: *transaction.from(),
        tx_gas_price: gas_price,
        blobs,
    });

    // But the fee to charge is based on current block context, and not worst case of max fee (backward-compatible manner)
    let gas_fee_amount = gas_price
        .checked_mul(U256::from(tx_gas_limit))
        .ok_or(internal_error!("gas price by tx gas limit"))?;

    // Note: no need to feature gate this part, as for non-EIP4844 transactions
    // num_blobs will be 0.
    let num_blobs = system.metadata.num_blobs();
    // NOTE: it's a special resource - not transaction gas. Will be used to charge fee only
    let blob_gas_used = num_blobs as u64 * GAS_PER_BLOB;
    let fee_for_blob_gas = if blob_gas_used > 0 {
        system_log!(
            system,
            "Blob gas price = {}\n",
            &system.get_blob_base_fee_per_gas()
        );

        let Some(value) = system
            .get_blob_base_fee_per_gas()
            .checked_mul(U256::from(blob_gas_used))
        else {
            return Err(TxError::Validation(
                InvalidTransaction::OverflowPaymentInTransaction,
            ));
        };

        value
    } else {
        U256::ZERO
    };
    let fee_to_prepay = gas_fee_amount
        .checked_add(fee_for_blob_gas)
        .ok_or(internal_error!("gfa+ffbg"))?;

    Ok(TxContextForPreAndPostProcessing {
        resources: tx_resources,
        fee_to_prepay,
        gas_price,
        minimal_ergs_to_charge: Ergs(minimal_gas_used.saturating_mul(ERGS_PER_GAS)),
        originator_nonce_to_use: old_nonce,
        tx_hash,
        native_per_pubdata,
        native_per_gas,
        tx_gas_limit,
        gas_used: 0,
        gas_refunded: 0,
        native_used: 0,
        validation_pubdata: 0,
        total_pubdata: 0,
        initial_resources: S::Resources::empty(),
        resources_before_refund: S::Resources::empty(),
    })
}

///
/// Compute number of calldata tokens and intrinsic gas,
/// following EIP-7623 if enabled.
///
#[allow(unused_variables)]
pub(crate) fn compute_calldata_tokens<S: SystemTypes>(
    system: &mut System<S>,
    calldata: &[u8],
    is_l1_tx: bool,
) -> (u64, u64) {
    let zero_bytes = calldata.iter().filter(|byte| **byte == 0).count() as u64;
    let non_zero_bytes = (calldata.len() as u64) - zero_bytes;
    let zero_bytes_factor = zero_bytes.saturating_mul(CALLDATA_ZERO_BYTE_TOKEN_FACTOR);
    let non_zero_bytes_factor = non_zero_bytes.saturating_mul(CALLDATA_NON_ZERO_BYTE_TOKEN_FACTOR);
    let num_tokens = zero_bytes_factor.saturating_add(non_zero_bytes_factor);
    let intrinsic_gas = if is_l1_tx {
        L1_TX_INTRINSIC_L2_GAS
    } else {
        L2_TX_INTRINSIC_GAS
    };

    #[cfg(feature = "eip_7623")]
    {
        let floor_tokens_gas_cost = num_tokens.saturating_mul(TOTAL_COST_FLOOR_PER_TOKEN);
        let intrinsic_gas = intrinsic_gas.saturating_add(floor_tokens_gas_cost);

        (num_tokens, intrinsic_gas)
    }

    #[cfg(not(feature = "eip_7623"))]
    {
        (num_tokens, intrinsic_gas)
    }
}
