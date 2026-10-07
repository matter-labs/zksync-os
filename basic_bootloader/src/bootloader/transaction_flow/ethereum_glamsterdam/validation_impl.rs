//! Validation of a Glamsterdam transaction and its resources: the intrinsic gas of
//! EIP-2780 with the calldata floor of EIP-7976 and the access list surcharge of EIP-7981,
//! the gas limits of EIP-7825 and EIP-8037, and the split of the gas into the execution
//! grant and the state gas reservoir. The delegations are applied later, with the fee
//! charged (`prepare_dispatch`).

use super::*;
use crate::bootloader::errors::InvalidTransaction::CreateInitCodeSizeLimit;
use crate::bootloader::transaction::blobs::parse_blobs_list;
use crate::require;
use crypto::secp256k1::SECP256K1N_HALF;
use evm_interpreter::v3::gas::{
    ACCESS_LIST_ADDRESS_FLOOR_TOKENS, ACCESS_LIST_STORAGE_KEY_FLOOR_TOKENS, CODE_INIT_PER_WORD,
    COLD_ACCOUNT_ACCESS, CREATE_ACCESS, EXECUTION_PER_AUTH_BASE_COST, MAX_INITCODE_SIZE,
    TX_ACCESS_LIST_ADDRESS, TX_ACCESS_LIST_STORAGE_KEY, TX_BASE, TX_DATA_TOKEN_FLOOR,
    TX_DATA_TOKEN_STANDARD, TX_MAX_TOTAL_GAS_LIMIT, TX_VALUE_COST,
};
use zk_ee::memory::ArrayBuilder;
use zk_ee::system::errors::system::SystemError;
use zk_ee::system::metadata::basic_metadata::BasicBlockMetadata;
use zk_ee::utils::{u256_mul_by_word, u256_to_u64_saturated};

/// The gas charged before a transaction runs (EIP-2780), and its calldata floor
#[derive(Clone, Copy, Debug, Default)]
pub struct IntrinsicGas {
    /// Execution gas: the sender's base, the recipient's access, the value transfer, the
    /// init code, the calldata, the access list with its data surcharge, the
    /// authorizations' state-independent part
    pub execution: u64,
    /// The least the transaction pays: the base and the recipient with every calldata byte
    /// and access list byte at the floor rate
    pub calldata_floor: u64,
}

/// `calculate_intrinsic_cost` of the specification
pub fn calculate_intrinsic_cost<A: core::alloc::Allocator>(
    transaction: &Transaction<A>,
    sender: &B160,
) -> Result<IntrinsicGas, TxError> {
    let calldata = transaction.calldata();
    let zero_bytes = calldata.iter().filter(|byte| **byte == 0).count() as u64;
    let non_zero_bytes = calldata.len() as u64 - zero_bytes;
    let tokens_in_calldata = zero_bytes.saturating_add(non_zero_bytes.saturating_mul(4));
    let data_cost = tokens_in_calldata.saturating_mul(TX_DATA_TOKEN_STANDARD);

    let to = transaction.to();
    let is_create = to.is_none();
    let is_self_transfer = to.as_ref() == Some(sender);
    let mut recipient_execution_gas = 0u64;
    let mut init_code_gas = 0u64;
    if is_create {
        recipient_execution_gas = CREATE_ACCESS;
        init_code_gas = CODE_INIT_PER_WORD * (calldata.len() as u64).div_ceil(32);
    } else if !is_self_transfer {
        recipient_execution_gas = COLD_ACCOUNT_ACCESS;
        if !transaction.value().is_zero() {
            recipient_execution_gas += TX_VALUE_COST;
        }
    }

    let mut access_list_cost = 0u64;
    let mut tokens_in_access_list = 0u64;
    if let Some(iter) = transaction.access_list_iter() {
        for entry in iter {
            access_list_cost = access_list_cost.saturating_add(TX_ACCESS_LIST_ADDRESS);
            tokens_in_access_list =
                tokens_in_access_list.saturating_add(ACCESS_LIST_ADDRESS_FLOOR_TOKENS);
            for key in entry.slots_list.iter() {
                key?;
                access_list_cost = access_list_cost.saturating_add(TX_ACCESS_LIST_STORAGE_KEY);
                tokens_in_access_list =
                    tokens_in_access_list.saturating_add(ACCESS_LIST_STORAGE_KEY_FLOOR_TOKENS);
            }
        }
    }
    // the surcharge is on both sides of the maximum
    let access_list_data_cost = tokens_in_access_list.saturating_mul(TX_DATA_TOKEN_FLOOR);

    let auth_cost = transaction.authorization_list().map_or(0, |list| {
        (list.iter().count() as u64).saturating_mul(EXECUTION_PER_AUTH_BASE_COST)
    });

    // EIP-7976: every calldata byte is four floor tokens
    let floor_tokens_in_calldata = (calldata.len() as u64).saturating_mul(TX_DATA_TOKEN_STANDARD);
    let base_execution_gas = TX_BASE + recipient_execution_gas;
    let calldata_floor = base_execution_gas
        .saturating_add(floor_tokens_in_calldata.saturating_mul(TX_DATA_TOKEN_FLOOR))
        .saturating_add(access_list_data_cost);
    let execution = base_execution_gas
        .saturating_add(init_code_gas)
        .saturating_add(data_cost)
        .saturating_add(access_list_cost)
        .saturating_add(access_list_data_cost)
        .saturating_add(auth_cost);
    Ok(IntrinsicGas {
        execution,
        calldata_floor,
    })
}

// effective_gas_price, priority_fee_per_gas
fn get_gas_prices<S: EthereumLikeSystem>(
    system: &mut System<S>,
    max_fee_per_gas: &U256,
    max_priority_fee_per_gas: Option<&U256>,
) -> Result<(U256, U256), TxError> {
    let max_priority_fee_per_gas = max_priority_fee_per_gas.unwrap_or(max_fee_per_gas);
    require!(
        max_priority_fee_per_gas <= max_fee_per_gas,
        TxError::Validation(InvalidTransaction::PriorityFeeGreaterThanMaxFee),
        system
    )?;
    let base_fee = system.get_eip1559_basefee();
    let (max_fee_minus_base_fee, uf) = max_fee_per_gas.overflowing_sub(base_fee);
    require!(
        !uf,
        TxError::Validation(InvalidTransaction::BaseFeeGreaterThanMaxFee),
        system
    )?;
    let priority_fee_per_gas = core::cmp::min(*max_priority_fee_per_gas, max_fee_minus_base_fee);
    Ok((base_fee + priority_fee_per_gas, priority_fee_per_gas))
}

/// Validates the transaction (signature, limits, intrinsic gas, nonce, balance), warms
/// the access list and pre-computes the fee. Opens no frame of its own; the caller does.
pub(crate) fn validate_and_compute_fee_for_transaction<
    S: EthereumLikeSystem<Metadata = GlamsterdamBlockMetadata>,
    Config: BasicBootloaderExecutionConfig,
>(
    system: &mut System<S>,
    transaction: &mut Transaction<S::Allocator>,
    _tracer: &mut impl Tracer<S>,
) -> Result<GlamsterdamTxContext<S>, TxError>
where
    S::IO: IOSubsystemExt,
    S::Resources: StateGasResources,
{
    let tx_gas_limit = transaction.gas_limit();
    let from = *transaction.from();

    // EIP-8037: the gas limit, execution and state gas together
    require!(
        tx_gas_limit <= TX_MAX_TOTAL_GAS_LIMIT,
        InvalidTransaction::CallerGasLimitMoreThanTxLimit,
        system
    )?;
    if transaction.is_deployment().is_some() && transaction.calldata().len() > MAX_INITCODE_SIZE {
        return Err(TxError::Validation(CreateInitCodeSizeLimit));
    }

    let intrinsic = calculate_intrinsic_cost(transaction, &from)?;
    require!(
        intrinsic.execution <= tx_gas_limit,
        InvalidTransaction::OutOfGasDuringValidation,
        system
    )?;
    require!(
        intrinsic.calldata_floor <= tx_gas_limit,
        InvalidTransaction::EIP7623IntrinsicGasIsTooLow,
        system
    )?;
    // EIP-7825: the execution gas is capped, the intrinsic part included
    require!(
        intrinsic.execution <= TX_MAX_GAS_LIMIT && intrinsic.calldata_floor <= TX_MAX_GAS_LIMIT,
        InvalidTransaction::CallerGasLimitTooHigh,
        system
    )?;

    let (effective_gas_price, priority_fee_per_gas) = get_gas_prices(
        system,
        transaction.max_fee_per_gas(),
        transaction.max_priority_fee_per_gas(),
    )?;
    system_log!(
        system,
        "Effective gas price for transaction is {}, priority fee = {}, intrinsic gas = {:?}\n",
        &effective_gas_price,
        &priority_fee_per_gas,
        &intrinsic,
    );

    // `allocate_evm_gas`: the execution grant up to the cap, the rest is the reservoir
    let evm_gas = tx_gas_limit - intrinsic.execution;
    let execution_gas = core::cmp::min(TX_MAX_GAS_LIMIT - intrinsic.execution, evm_gas);
    let state_gas_reservoir = (evm_gas - execution_gas) as u32;
    let mut resources = S::Resources::from_ergs_and_native(
        <S::Resources as Resources>::Ergs::from_legacy_gas_saturating(execution_gas),
        <S::Resources as Resources>::Native::from_computational(u64::MAX),
    );
    resources.add_state_gas(state_gas_reservoir);

    let suggested_signed_hash: Bytes32 = transaction.signed_hash()?;
    let Some((parity, r, s)) = transaction.sig_parity_r_s() else {
        return Err(InvalidTransaction::InvalidStructure.into());
    };
    if Config::VALIDATE_EOA_SIGNATURE && !Config::SIMULATION {
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
        // the recovery is in the intrinsic gas: only the native is charged
        resources.with_infinite_ergs(|resources| {
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

    // the sender's access and write are in the base cost
    let originator_account_data = resources.with_infinite_ergs(|inf_resources| {
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
    // EIP-3607: no transactions from accounts with code, delegations excepted
    if originator_account_data.is_contract() {
        return Err(InvalidTransaction::RejectCallerWithCode.into());
    }

    let old_nonce = match resources.with_infinite_ergs(|resources| {
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
        Err(SubsystemError::LeafDefect(e)) => return Err(TxError::Internal(e.into())),
        Err(SubsystemError::LeafRuntime(RuntimeError::OutOfErgs(_))) => unreachable!(),
        Err(SubsystemError::LeafRuntime(RuntimeError::FatalRuntimeError(_))) => {
            return Err(TxError::Validation(
                InvalidTransaction::OutOfNativeResourcesDuringValidation,
            ));
        }
        Err(SubsystemError::Cascaded(cascaded)) => match cascaded {},
    };
    let Some(originator_expected_nonce) = transaction.nonce().as_ref().map(u256_to_u64_saturated)
    else {
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

    // the access list is paid in the intrinsic gas: only warmed here
    if let Some(iter) = transaction.access_list_iter() {
        for entry in iter {
            resources.with_infinite_ergs(|resources| {
                system
                    .io
                    .touch_account(ExecutionEnvironmentType::NoEE, resources, &entry.address)
            })?;
            for key in entry.slots_list.iter() {
                let key = key?;
                resources.with_infinite_ergs(|resources| {
                    system.io.storage_touch(
                        ExecutionEnvironmentType::NoEE,
                        resources,
                        &entry.address,
                        &Bytes32::from_array(*key),
                    )
                })?;
            }
        }
    }

    let blobs = if let Some(blobs_list) = transaction.blobs() {
        let tx_max_fee_per_blob_gas = transaction
            .max_fee_per_blob_gas()
            .expect("must be present in such TXes");
        let block_base_fee_per_blob_gas = system.metadata.blob_base_fee_per_gas();
        if &block_base_fee_per_blob_gas > tx_max_fee_per_blob_gas {
            return Err(TxError::Validation(
                InvalidTransaction::BlobBaseFeeGreaterThanMaxFeePerBlobGas,
            ));
        }
        parse_blobs_list::<MAX_BLOBS_PER_TX>(blobs_list)?
    } else {
        arrayvec::ArrayVec::new()
    };
    let blob_gas_used = (blobs.len() as u64) * GAS_PER_BLOB;

    let worst_case_fee_amount = {
        let (value, of) = u256_mul_by_word(transaction.max_fee_per_gas(), tx_gas_limit);
        if of > 0 {
            return Err(internal_error!("max gas price by tx gas limit").into());
        }
        value
    };
    let fee_for_blob_gas = if blob_gas_used > 0 {
        let (value, of) = u256_mul_by_word(&system.metadata.blob_base_fee_per_gas(), blob_gas_used);
        if of > 0 {
            return Err(internal_error!("blob gas price by blob gas used").into());
        }
        value
    } else {
        U256::ZERO
    };

    let tx_value = transaction.value();
    let total_required_balance = tx_value
        .checked_add(worst_case_fee_amount)
        .and_then(|v| v.checked_add(fee_for_blob_gas))
        .ok_or(internal_error!("transaction amount + fee + blob gas"))?;
    if total_required_balance > originator_account_data.nominal_token_balance.0 {
        return Err(TxError::Validation(
            InvalidTransaction::LackOfFundForMaxFee {
                fee: total_required_balance,
                balance: originator_account_data.nominal_token_balance.0,
            },
        ));
    }

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

    let tx_level_metadata = EthereumTransactionMetadata {
        tx_gas_price: effective_gas_price,
        tx_origin: from,
        blobs,
    };

    Ok(GlamsterdamTxContext::<S> {
        resources,
        fee_to_prepay: total_fee,
        priority_fee_per_gas,
        originator_nonce_to_use: old_nonce,
        tx_gas_limit,
        intrinsic,
        state_gas_reservoir,
        committed_spill: 0,
        blob_gas_used,
        tx_level_metadata: tx_level_metadata.clone(),
        settlement: GasSettlement::default(),
    })
}
