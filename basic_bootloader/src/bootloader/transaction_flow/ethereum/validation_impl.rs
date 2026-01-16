use super::*;
use crate::bootloader::constants::*;
use crate::bootloader::errors::InvalidTransaction::CreateInitCodeSizeLimit;
use crate::bootloader::errors::{InvalidTransaction, TxError};
use crate::bootloader::transaction::access_list::parse_and_warm_up_access_list;
use crate::bootloader::transaction::authorization_list::parse_authorization_list_and_apply_delegations;
use crate::bootloader::transaction::blobs::parse_blobs_list;
use crate::bootloader::BasicBootloaderExecutionConfig;
use crate::require;
use core::fmt::Write;
use crypto::secp256k1::SECP256K1N_HALF;
use evm_interpreter::{ERGS_PER_GAS, MAX_INITCODE_SIZE};
use ruint::aliases::{B160, U256};
use tx_level_metadata::EthereumTransactionMetadata;
use zk_ee::execution_environment_type::ExecutionEnvironmentType;
use zk_ee::internal_error;
use zk_ee::memory::ArrayBuilder;
use zk_ee::system::errors::interface::InterfaceError;
use zk_ee::system::errors::runtime::RuntimeError;
use zk_ee::system::errors::subsystem::SubsystemError;
use zk_ee::system::metadata::basic_metadata::BasicBlockMetadata;
use zk_ee::system::tracer::Tracer;
use zk_ee::system::{errors::system::SystemError, EthereumLikeTypes, System};
use zk_ee::system_log;
use zk_ee::utils::{u256_mul_by_word, u256_to_u64_saturated};

fn create_resources_for_tx<S: EthereumLikeTypes>(
    gas_limit: u64,
    is_deployment: bool,
    calldata_len: u64,
    calldata_tokens: u64,
) -> Result<ResourcesForEthereumTx<S>, TxError> {
    let mut intrinsic_overhead = L2_TX_INTRINSIC_GAS;
    if is_deployment {
        if calldata_len > MAX_INITCODE_SIZE as u64 {
            return Err(TxError::Validation(CreateInitCodeSizeLimit));
        }
        intrinsic_overhead = intrinsic_overhead.saturating_add(DEPLOYMENT_TX_EXTRA_INTRINSIC_GAS);
        let initcode_gas_cost = evm_interpreter::gas_constants::INITCODE_WORD_COST
            * (calldata_len.next_multiple_of(32) / 32);
        intrinsic_overhead = intrinsic_overhead.saturating_add(initcode_gas_cost);
    }
    intrinsic_overhead =
        intrinsic_overhead.saturating_add(calldata_tokens.saturating_mul(CALLDATA_TOKEN_GAS_COST));

    if intrinsic_overhead > gas_limit {
        Err(TxError::Validation(
            InvalidTransaction::OutOfGasDuringValidation,
        ))
    } else {
        let gas_limit_for_tx = gas_limit - intrinsic_overhead;
        let ergs = gas_limit_for_tx.saturating_mul(ERGS_PER_GAS); // we checked at the very start that gas_limit * ERGS_PER_GAS doesn't overflow
        let native_limit =
            <<S as zk_ee::system::SystemTypes>::Resources as Resources>::Native::from_computational(
                u64::MAX,
            );
        let main_resources = S::Resources::from_ergs_and_native(Ergs(ergs), native_limit);

        Ok(ResourcesForEthereumTx { main_resources })
    }
}

// effective_gas_price, priority_fee_per_gas
fn get_gas_prices<S: EthereumLikeTypes>(
    system: &mut System<S>,
    max_fee_per_gas: &U256,
    max_priority_fee_per_gas: Option<&U256>,
) -> Result<(U256, U256), TxError> {
    let max_priority_fee_per_gas = if let Some(max_priority_fee_per_gas) = max_priority_fee_per_gas
    {
        max_priority_fee_per_gas
    } else {
        max_fee_per_gas
    };
    require!(
        max_priority_fee_per_gas <= max_fee_per_gas,
        TxError::Validation(InvalidTransaction::PriorityFeeGreaterThanMaxFee,),
        system
    )?;

    let base_fee = system.get_eip1559_basefee();
    let (max_fee_minus_base_fee, uf) = max_fee_per_gas.overflowing_sub(base_fee);
    require!(
        uf == false,
        TxError::Validation(InvalidTransaction::BaseFeeGreaterThanMaxFee,),
        system
    )?;

    let priority_fee_per_gas = core::cmp::min(*max_priority_fee_per_gas, max_fee_minus_base_fee);

    let effective_gas_price = base_fee + priority_fee_per_gas;

    Ok((effective_gas_price, priority_fee_per_gas))
}

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
) -> Result<EthereumTxContext<S>, TxError>
where
    S::IO: IOSubsystemExt,
{
    let tx_gas_limit = transaction.gas_limit();

    // we perform single check to make sure that we can use saturating operations to accumulate some costs,
    // and even if those would saturate, we can still catch this case
    require!(
        tx_gas_limit.saturating_mul(ERGS_PER_GAS) < u64::MAX,
        internal_error!("TX gas limit overflows ergs counter"),
        system
    )?;

    let calldata = transaction.calldata();

    // Validate block-level invariants
    {
        // Validate that the transaction's gas limit is not larger than
        // the block's gas limit.
        let tx_limit = system.metadata.individual_tx_gas_limit();
        require!(
            tx_gas_limit <= tx_limit,
            InvalidTransaction::CallerGasLimitMoreThanTxLimit,
            system
        )?;
    }

    // EIP-7623
    let (calldata_tokens, minimal_gas_used) = {
        let zero_bytes = calldata.iter().filter(|byte| **byte == 0).count() as u64;
        let non_zero_bytes = (calldata.len() as u64) - zero_bytes;
        let zero_bytes_factor = zero_bytes.saturating_mul(CALLDATA_ZERO_BYTE_TOKEN_FACTOR);
        let non_zero_bytes_factor =
            non_zero_bytes.saturating_mul(CALLDATA_NON_ZERO_BYTE_TOKEN_FACTOR);
        let num_tokens = zero_bytes_factor.saturating_add(non_zero_bytes_factor);

        #[cfg(feature = "eip_7623")]
        {
            let floor_tokens_gas_cost = num_tokens.saturating_mul(TOTAL_COST_FLOOR_PER_TOKEN);
            let intrinsic_gas = L2_TX_INTRINSIC_GAS.saturating_add(floor_tokens_gas_cost);

            require!(
                intrinsic_gas <= tx_gas_limit,
                InvalidTransaction::EIP7623IntrinsicGasIsTooLow,
                system
            )?;

            (num_tokens, intrinsic_gas)
        }

        #[cfg(not(feature = "eip_7623"))]
        {
            (num_tokens, L2_TX_INTRINSIC_GAS)
        }
    };

    let (effective_gas_price, priority_fee_per_gas) = get_gas_prices(
        system,
        transaction.max_fee_per_gas(),
        transaction.max_priority_fee_per_gas(),
    )?;

    let _ = system.get_logger().write_fmt(format_args!(
        "Effective gas price for transaction is {}, priority fee = {}\n",
        &effective_gas_price, &priority_fee_per_gas,
    ));

    let is_deployment = transaction.is_deployment().is_some();

    // Now we will materialize resources, from which we will try to charge intrinsic cost on top
    let mut tx_resources = create_resources_for_tx::<S>(
        tx_gas_limit,
        is_deployment,
        calldata.len() as u64,
        calldata_tokens,
    )?;

    let _ = system.get_logger().write_fmt(format_args!(
        "Prepared resources for transaction: {:?}\n",
        &tx_resources
    ));

    let suggested_signed_hash: Bytes32 = transaction.signed_hash()?;
    let from = *transaction.from();
    let Some((parity, r, s)) = transaction.sig_parity_r_s() else {
        // Ethereum txs should have signature
        return Err(InvalidTransaction::InvalidStructure.into());
    };

    if !Config::VALIDATE_EOA_SIGNATURE | Config::SIMULATION {
        // No native for Eth STF
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
        tx_resources
            .main_resources
            .with_infinite_ergs(|resources| {
                S::SystemFunctions::secp256k1_ec_recover(
                    ecrecover_input.as_slice(),
                    &mut ecrecover_output,
                    resources,
                    system.get_allocator(),
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
    if originator_account_data.is_contract() {
        return Err(InvalidTransaction::RejectCallerWithCode.into());
    }

    // Now we can apply access list and authorization list, while simultaneously charging for them

    // Originator's nonce is incremented before authorization list
    let old_nonce = match tx_resources.main_resources.with_infinite_ergs(|resources| {
        system
            .io
            .increment_nonce(ExecutionEnvironmentType::NoEE, resources, &from, 1u64)
    }) {
        Ok(x) => x,
        Err(SubsystemError::LeafUsage(InterfaceError(NonceError::NonceOverflow, _))) => {
            return Err(TxError::Validation(
                InvalidTransaction::NonceOverflowInTransaction,
            ));
        }
        Err(SubsystemError::LeafDefect(e)) => {
            return Err(TxError::Internal(e.into()));
        }
        Err(SubsystemError::LeafRuntime(RuntimeError::OutOfErgs(_))) => {
            unreachable!();
        }
        Err(SubsystemError::LeafRuntime(RuntimeError::FatalRuntimeError(_))) => {
            // TODO: decide if we wan to allow such cases at all
            return Err(TxError::Validation(
                InvalidTransaction::OutOfNativeResourcesDuringValidation,
            ));
        }
        Err(SubsystemError::Cascaded(cascaded)) => match cascaded {},
    };
    let Some(originator_expected_nonce) = transaction.nonce().as_ref().map(u256_to_u64_saturated)
    else {
        // Ethereum txs should have nonce
        return Err(InvalidTransaction::InvalidStructure.into());
    };
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

    // Access list
    parse_and_warm_up_access_list(system, &mut tx_resources.main_resources, &transaction)?;

    let blobs = if let Some(blobs_list) = transaction.blobs() {
        let tx_max_fee_per_blob_gas = transaction
            .max_fee_per_blob_gas()
            .expect("must be present in such TXes");
        let block_base_fee_per_blob_gas = system.metadata.blob_base_fee_per_gas();
        if &block_base_fee_per_blob_gas > tx_max_fee_per_blob_gas {
            return Err(TxError::Validation(
                InvalidTransaction::BlobElementIsNotSupported,
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

    // NOTE: it's a special resource - not transaction gas. Will be used to charge fee only
    let blob_gas_used = (blobs.len() as u64) * GAS_PER_BLOB;

    if let Some(auth_list) = transaction.authorization_list() {
        parse_authorization_list_and_apply_delegations(
            system,
            &mut tx_resources.main_resources,
            auth_list,
        )?
    }

    let worst_case_fee_amount = {
        let (value, of) = u256_mul_by_word(transaction.max_fee_per_gas(), tx_gas_limit);
        if of > 0 {
            return Err(internal_error!("max gas price by tx gas limit").into());
        }

        value
    };

    let fee_for_blob_gas = if blob_gas_used > 0 {
        let _ = system.get_logger().write_fmt(format_args!(
            "Blob gas price = {}\n",
            &system.metadata.blob_base_fee_per_gas()
        ));

        let (value, of) = u256_mul_by_word(&system.metadata.blob_base_fee_per_gas(), blob_gas_used);
        if of > 0 {
            return Err(internal_error!("blob gas price by blob gas used").into());
        }

        value
    } else {
        U256::ZERO
    };

    debug_assert!(transaction.max_fee_per_gas() >= &effective_gas_price);

    // Balance check - originator must cover fee prepayment plus whatever "value" it would like to send along
    let tx_value = transaction.value();

    let mut total_required_balance = tx_value
        .checked_add(worst_case_fee_amount)
        .ok_or(internal_error!("transaction amount + fee"))?;
    total_required_balance = total_required_balance
        .checked_add(fee_for_blob_gas)
        .ok_or(internal_error!("transaction amount + fee + blob gas"))?;
    if total_required_balance > originator_account_data.nominal_token_balance.0 {
        return Err(TxError::Validation(
            InvalidTransaction::LackOfFundForMaxFee {
                fee: total_required_balance,
                balance: originator_account_data.nominal_token_balance.0,
            },
        ));
    }

    // But the fee to charge is based on current block context, and not worst case of max fee (backward-compatible manner)
    let fee_amount_execution_gas = {
        let (value, of) = u256_mul_by_word(&effective_gas_price, tx_gas_limit);
        if of > 0 {
            return Err(internal_error!("effective gas price by tx gas limit").into());
        }

        value
    };

    let total_fee = fee_amount_execution_gas
        .checked_add(fee_for_blob_gas)
        .ok_or(internal_error!("transaction fee + blob gas"))?;

    // let tx_hash = *transaction.transaction_hash();

    let tx_level_metadata = EthereumTransactionMetadata {
        tx_gas_price: effective_gas_price,
        tx_origin: from,
        blobs,
    };

    let context = EthereumTxContext::<S> {
        resources: tx_resources,
        fee_to_prepay: total_fee,
        priority_fee_per_gas,
        minimal_gas_to_charge: minimal_gas_used,
        originator_nonce_to_use: old_nonce,
        // tx_hash,
        tx_gas_limit,
        gas_used: 0,
        blob_gas_used,
        tx_level_metadata,
    };

    Ok(context)
}
