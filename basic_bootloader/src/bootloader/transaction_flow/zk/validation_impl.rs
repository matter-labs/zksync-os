use super::TxContextForPreAndPostProcessing;
use crate::bootloader::constants::*;
use crate::bootloader::errors::{InvalidTransaction, TxError};
use crate::bootloader::transaction::access_list::parse_and_warm_up_access_list;
use crate::bootloader::transaction::blobs::parse_blobs_list;
use crate::bootloader::transaction::rlp_encoded::AccessListForAddress;
use crate::bootloader::transaction::{charge_keccak, Transaction};
use crate::bootloader::transaction_flow::gas_helpers::{
    calculate_l2_tx_intrinsic_computational_native_resources, calculate_l2_tx_intrinsic_pubdata,
    calculate_tx_intrinsic_gas, create_resources_for_tx, get_gas_price, L2TxIntrinsicNativeInput,
};
use crate::bootloader::BasicBootloaderExecutionConfig;
use crate::require;
use basic_system::cost_constants::ECRECOVER_NATIVE_COST;
use basic_system::system_functions::keccak256::keccak256_native_cost_for_rounds_u64;
use core::fmt::Write;
use crypto::secp256k1::SECP256K1N_HALF;
use evm_interpreter::native_resource_constants::COPY_BYTE_NATIVE_COST;
use evm_interpreter::{ERGS_PER_GAS, MAX_INITCODE_SIZE};
use ruint::aliases::{B160, U256};
use zk_ee::execution_environment_type::ExecutionEnvironmentType;
use zk_ee::memory::ArrayBuilder;
use zk_ee::system::errors::interface::InterfaceError;
use zk_ee::system::errors::runtime::RuntimeError;
use zk_ee::system::errors::subsystem::SubsystemError;
use zk_ee::system::metadata::basic_metadata::BasicTransactionMetadata;
use zk_ee::system::metadata::basic_metadata::{BasicMetadata, ZkSpecificMetadata};
use zk_ee::system::metadata::zk_metadata::TxLevelMetadata;
use zk_ee::system::tracer::Tracer;
use zk_ee::system::{errors::system::SystemError, Computational, EthereumLikeTypes, System};
use zk_ee::system::{AccountDataRequest, SystemFunctionsExt};
use zk_ee::system::{Ergs, IOSubsystemExt, Resources};
use zk_ee::system::{IOSubsystem, NonceError};
use zk_ee::system::{Resource, SystemTypes};
use zk_ee::system::{GAS_PER_BLOB, MAX_BLOBS_PER_TX};
use zk_ee::system_log;
use zk_ee::{internal_error, out_of_native_resources};
use zk_ee::{utils::*, wrap_error};

///
/// Will perform basic validation, namely - checking signature, minimal resource requirements for transaction validity.
/// It may perform IO if needed to e.g. warm up some storage slots,
/// or mark delegation
///
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
    S::Metadata: ZkSpecificMetadata
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

    // Validate that the tx gas limit doesn't exceed the effective per-tx
    // limit, for non-service transactions. Call simulation intentionally skips
    // normal tx-admission checks so RPC callers can estimate with a high gas
    // ceiling. The `block_gas_limit <= MAX_BLOCK_GAS_LIMIT` invariant is
    // enforced once per block in `MetadataOp::metadata_op`, so it is not
    // re-checked here per transaction.
    if !Config::SIMULATION && !transaction.is_service() {
        let individual_limit = system.get_individual_tx_gas_limit();
        require!(
            tx_gas_limit <= individual_limit,
            InvalidTransaction::CallerGasLimitMoreThanTxLimit,
            system
        )?;
    }

    // EIP-7623
    let (calldata_tokens, minimal_gas_used) = compute_calldata_tokens(system, calldata);
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

    // `native_price == 0` means the chain doesn't price native. Downstream
    // treats `native_per_gas == 0` as "unlimited native budget"
    let native_per_gas = if native_price.is_zero() {
        0u64
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
    };
    // If native resources are free (native_price == 0), pubdata is free too:
    // `checked_div` returns `None` and we fall back to 0.
    let native_per_pubdata =
        u256_try_to_u64(&pubdata_price.checked_div(native_price).unwrap_or_default())
            .ok_or(TxError::Validation(InvalidTransaction::PubdataPriceTooHigh))?;
    let native_prepaid_from_gas = native_per_gas.saturating_mul(tx_gas_limit);
    let statement_versioned_hashes_num = transaction.statement_versioned_hashes_len();
    let blob_versioned_hashes_num = transaction.blobs().map_or(0, |blobs| blobs.count as u64);

    let mut access_list_accounts = 0;
    let mut access_list_storage_keys = 0;
    if let Some(iter) = transaction.access_list_iter() {
        for AccessListForAddress {
            address: _,
            slots_list,
        } in iter
        {
            access_list_accounts += 1;
            access_list_storage_keys += slots_list.count as u64;
        }
    }
    let authorization_list_num = if let Some(authorization_list) = transaction.authorization_list()
    {
        authorization_list.len() as u64
    } else {
        0u64
    };

    let is_deployment = transaction.is_deployment().is_some();
    if is_deployment && calldata.len() as u64 > MAX_INITCODE_SIZE as u64 {
        return Err(TxError::Validation(
            InvalidTransaction::CreateInitCodeSizeLimit,
        ));
    }
    let intrinsic_gas = calculate_tx_intrinsic_gas(
        calldata.len() as u64,
        calldata_tokens,
        is_deployment,
        access_list_accounts,
        access_list_storage_keys,
        authorization_list_num,
        statement_versioned_hashes_num,
    );
    let intrinsic_computational_native =
        calculate_l2_tx_intrinsic_computational_native_resources(&L2TxIntrinsicNativeInput {
            calldata_byte_length: calldata.len() as u64,
            access_list_accounts,
            access_list_storages: access_list_storage_keys,
            authorization_list_num,
            blob_versioned_hashes_num,
            statement_versioned_hashes_num,
            is_service: transaction.is_service(),
            free_native: native_per_gas == 0,
        });
    let intrinsic_pubdata = calculate_l2_tx_intrinsic_pubdata(
        authorization_list_num,
        transaction.is_service(),
        system.get_chain_config().pubdata_content(),
    );

    // Materialize the tx's resource budget and charge the intrinsic overheads.
    // Underflow on any of the charges surfaces as a validation error
    let (tx_resources, charge_err) = create_resources_for_tx::<S>(
        tx_gas_limit,
        native_per_gas == 0,
        native_prepaid_from_gas,
        native_per_pubdata,
        intrinsic_gas,
        intrinsic_computational_native,
        intrinsic_pubdata,
    );
    if let Some(e) = charge_err {
        return Err(TxError::Validation(e));
    }

    system_log!(
        system,
        "Prepared resources for transaction: {:?}\n",
        &tx_resources
    );

    // We use `intrinsic_resources` for operations whose native cost is already
    // included in the intrinsic computational native formula (ecrecover, tx
    // hash keccak, originator account read, nonce increment, and later the
    // fee prepayment / refund / operator payment). The tracker is always
    // `FORMAL_INFINITE`; under `verify_intrinsic_native` the actual native
    // consumption is recovered by subtracting the residual from the initial
    // FORMAL_INFINITE value.
    let mut intrinsic_resources = S::Resources::FORMAL_INFINITE;

    // There are 2 things that are done outside the tx flow, but we still need to charge the user for them
    // 1. Calldata copying
    intrinsic_resources.charge(&Resources::from_native(
        <<S as SystemTypes>::Resources as Resources>::Native::from_computational(
            calldata.len() as u64 * COPY_BYTE_NATIVE_COST,
        ),
    ))?;
    // 2. Hashing tx hash into the rolling hash after the execution
    intrinsic_resources.charge(&Resources::from_native(
        <<S as SystemTypes>::Resources as Resources>::Native::from_computational(
            keccak256_native_cost_for_rounds_u64(1),
        ),
    ))?;

    // NOTE: we provided a "hint" for "from", so it's sequencer's risks here:
    // - either "from" is valid at it has at least enough balance, valid signature, etc to eventually pay for all validation
    // - or we will perform non-mutating operations without any payment

    // steps below are all not free, so the choice there is rather arbitrary. Let's first check the signature, as it's compute-only

    // We have to charge native for this hash, as it's computed during parsing
    // for RLP-encoded transactions.
    // We over-estimate using the total tx length
    if !transaction.is_service() {
        charge_keccak(transaction.len(), &mut intrinsic_resources)?;
    }
    let suggested_signed_hash: Bytes32 = transaction.signed_hash()?;

    // Only service transactions have no signature,
    // we don't even charge gas/native related to ecrecover for them.
    if let Some((parity, r, s)) = transaction.sig_parity_r_s() {
        // Even if we don't validate a signature, we still need to charge for ecrecover for equivalent behavior
        // Note that gas is charged already in intrinsic cost, so now
        // we only need to charge native resources.
        if !Config::VALIDATE_EOA_SIGNATURE | Config::SIMULATION {
            intrinsic_resources.charge(&Resources::from_native(
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
            let mut logger = system.get_logger();
            let allocator = system.get_allocator();
            // We already charged gas for ecrecover in intrinsic cost, so we only need to charge native resources here.
            intrinsic_resources.with_infinite_ergs(|resources| {
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
    let tx_hash: Bytes32 = transaction.transaction_hash(&mut intrinsic_resources)?;

    // Charge the per-statement FRI verifier native budget against
    // `intrinsic_resources` so `verify_intrinsic_native` exercises this
    // portion of the intrinsic formula. The verifier itself runs later
    // and is gated by this prepayment.
    if statement_versioned_hashes_num > 0 {
        intrinsic_resources.charge(&Resources::from_native(
            <<S as SystemTypes>::Resources as Resources>::Native::from_computational(
                FRI_PROOF_INTRINSIC_NATIVE_COST_PER_PROOF
                    .saturating_mul(statement_versioned_hashes_num),
            ),
        ))?;
    }

    // any IO starts here

    // now we can perform IO related parts. Getting originator's properties is included into the
    // intrinsic cost charged above
    let originator_account_data = intrinsic_resources.with_infinite_ergs(|inf_resources| {
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
        match intrinsic_resources.with_infinite_ergs(|resources| {
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

    // Access list.
    // Gas is already included in the intrinsic gas charged above, so we are only charging native.
    intrinsic_resources.with_infinite_ergs(|inf_resources| {
        parse_and_warm_up_access_list(system, inf_resources, &transaction)
    })?;

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

        match parse_blobs_list::<MAX_BLOBS_PER_TX>(blobs_list) {
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
    {
        if let Some(authorization_list) = transaction.authorization_list() {
            // Same as for the access list: gas is included in the intrinsic
            // gas above, so we are only charging native
            intrinsic_resources.with_infinite_ergs(|inf_resources| {
                crate::bootloader::transaction::authorization_list::parse_authorization_list_and_apply_delegations(
                    system,
                    inf_resources,
                    authorization_list,
                )
            })?;
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

    // FRI proof handling is centralized so the feature-off build can
    // encapsulate the entire path in one helper.
    let verified_fri_statements = maybe_drive_fri_verification::<S, Config>(system, transaction)?;

    system.set_tx_context(TxLevelMetadata {
        tx_origin: *transaction.from(),
        tx_gas_price: gas_price,
        blobs,
        verified_fri_statements,
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
        intrinsic_resources,
        intrinsic_computational_native,
        authorization_list_num,
        statement_versioned_hashes_num,
    })
}

///
/// Compute the number of calldata tokens and the EIP-7623 floor gas.
///
#[allow(unused_variables)]
pub(crate) fn compute_calldata_tokens<S: SystemTypes>(
    system: &mut System<S>,
    calldata: &[u8],
) -> (u64, u64) {
    let zero_bytes = calldata.iter().filter(|byte| **byte == 0).count() as u64;
    let non_zero_bytes = (calldata.len() as u64) - zero_bytes;
    let zero_bytes_factor = zero_bytes.saturating_mul(CALLDATA_ZERO_BYTE_TOKEN_FACTOR);
    let non_zero_bytes_factor = non_zero_bytes.saturating_mul(CALLDATA_NON_ZERO_BYTE_TOKEN_FACTOR);
    let num_tokens = zero_bytes_factor.saturating_add(non_zero_bytes_factor);

    let floor_tokens_gas_cost = num_tokens.saturating_mul(TOTAL_COST_FLOOR_PER_TOKEN);
    let floor_gas = TX_INTRINSIC_GAS.saturating_add(floor_tokens_gas_cost);

    (num_tokens, floor_gas)
}

#[cfg(feature = "fri_precompile")]
fn maybe_drive_fri_verification<S: EthereumLikeTypes, Config: BasicBootloaderExecutionConfig>(
    system: &mut System<S>,
    transaction: &Transaction<S::Allocator>,
) -> Result<
    arrayvec::ArrayVec<Bytes32, { zk_ee::system::constants::MAX_FRI_STATEMENTS_PER_TX }>,
    TxError,
>
where
    S::IO: IOSubsystemExt,
{
    if !transaction.is_fri_proof() {
        return Ok(arrayvec::ArrayVec::new());
    }

    let verified_fri_statements =
        super::fri::build_verified_fri_statements_list(system, transaction)?;
    if Config::VERIFY_FRI_PROOFS {
        super::fri::drive_fri_verification(system, &verified_fri_statements)?;
    }
    Ok(verified_fri_statements)
}

#[cfg(not(feature = "fri_precompile"))]
fn maybe_drive_fri_verification<S: EthereumLikeTypes, Config: BasicBootloaderExecutionConfig>(
    system: &mut System<S>,
    transaction: &Transaction<S::Allocator>,
) -> Result<
    arrayvec::ArrayVec<Bytes32, { zk_ee::system::constants::MAX_FRI_STATEMENTS_PER_TX }>,
    TxError,
>
where
    S::IO: IOSubsystemExt,
{
    let _ = (system, transaction, core::marker::PhantomData::<Config>);
    Ok(arrayvec::ArrayVec::new())
}
