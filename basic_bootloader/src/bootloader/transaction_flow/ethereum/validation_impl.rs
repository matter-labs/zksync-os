use super::*;
use crate::bootloader::constants::*;
use crate::bootloader::errors::InvalidTransaction::CreateInitCodeSizeLimit;
use crate::bootloader::errors::{InvalidTransaction, TxError};
use crate::bootloader::ethereum::EthereumTxContext;
use crate::bootloader::transaction::ethereum_tx_format::BlobHashesList;
use crate::bootloader::transaction::ethereum_tx_format::{
    AccessList, AccessListForAddress, AuthorizationList,
};
use crate::bootloader::BasicBootloaderExecutionConfig;
use crate::bootloader::Bytes32;
use crate::require;
use core::alloc::Allocator;
use core::fmt::Write;
use core::u64;
use evm_interpreter::{ERGS_PER_GAS, MAX_INITCODE_SIZE};
use ruint::aliases::{B160, U256};
use zk_ee::execution_environment_type::ExecutionEnvironmentType;
use zk_ee::internal_error;
use zk_ee::memory::ArrayBuilder;
use zk_ee::metadata_markers::basic_metadata::BasicBlockMetadata;
use zk_ee::system::errors::interface::InterfaceError;
use zk_ee::system::errors::runtime::RuntimeError;
use zk_ee::system::errors::subsystem::SubsystemError;
use zk_ee::system::tracer::Tracer;
use zk_ee::system::{errors::system::SystemError, EthereumLikeTypes, System};
use zk_ee::utils::u256_mul_by_word;
use zk_ee::wrap_error;

fn create_resources_for_tx<S: EthereumLikeTypes>(
    gas_limit: u64,
    is_deployment: bool,
    calldata_len: u64,
    calldata_tokens: u64,
) -> Result<ResourcesForEthereumTx<S>, TxError> {
    let mut intrinsic_overhead = L2_TX_INTRINSIC_GAS as u64;
    if is_deployment {
        if calldata_len > MAX_INITCODE_SIZE as u64 {
            return Err(TxError::Validation(CreateInitCodeSizeLimit));
        }
        intrinsic_overhead =
            intrinsic_overhead.saturating_add(DEPLOYMENT_TX_EXTRA_INTRINSIC_GAS as u64);
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

pub fn parse_and_warm_up_access_list<S: EthereumLikeTypes>(
    system: &mut System<S>,
    resources: &mut S::Resources,
    access_list: AccessList<'_>,
) -> Result<(), TxError>
where
    S::IO: IOSubsystemExt,
{
    use evm_interpreter::ERGS_PER_GAS;
    use zk_ee::system::Ergs;

    for per_address_list in access_list.iter() {
        // per-address charge
        resources.charge(&S::Resources::from_ergs(Ergs(
            evm_interpreter::gas_constants::ACCESS_LIST_ADDRESS * ERGS_PER_GAS,
        )))?;
        let AccessListForAddress {
            address,
            slots_list,
        } = per_address_list.into_inner();

        let _ = system.get_logger().write_fmt(format_args!(
            "Will touch address 0x{:040x} as warm\n",
            address.as_uint()
        ));

        resources.with_infinite_ergs(|resources| {
            system.io.touch_account(
                ExecutionEnvironmentType::NoEE,
                resources,
                &address,
                true,
                false,
            )
        })?;
        for slot in slots_list.iter() {
            // per-slot charge
            resources.charge(&S::Resources::from_ergs(Ergs(
                evm_interpreter::gas_constants::ACCESS_LIST_STORAGE_KEY * ERGS_PER_GAS,
            )))?;
            // From type definition it's NOP, but compiler can not deduce it
            let key =
                Bytes32::from_array(*slot.map_err(|()| InvalidTransaction::InvalidStructure)?);

            let _ = system.get_logger().write_fmt(format_args!(
                "Will touch address 0x{:040x}, slot {:?} as warm\n",
                address.as_uint(),
                &key,
            ));

            resources.with_infinite_ergs(|resources| {
                system.io.storage_touch(
                    ExecutionEnvironmentType::NoEE,
                    resources,
                    &address,
                    &key,
                    true,
                )
            })?;
        }
    }
    Ok(())
}

pub fn parse_blobs_list<const MAX_BLOBS_IN_TX: usize>(
    blobs_list: BlobHashesList<'_>,
) -> Result<arrayvec::ArrayVec<Bytes32, MAX_BLOBS_IN_TX>, TxError> {
    let mut result = arrayvec::ArrayVec::<_, MAX_BLOBS_IN_TX>::new();
    if blobs_list.count > MAX_BLOBS_IN_TX {
        // transactions that allow blobs should have at least one
        return Err(TxError::Validation(
            InvalidTransaction::BlobElementIsNotSupported,
        ));
    }

    for blob_hash in blobs_list.iter() {
        let Ok(blob_hash) = blob_hash else {
            return Err(TxError::Validation(
                InvalidTransaction::BlobElementIsNotSupported,
            ));
        };

        if blob_hash[0] != VERSIONED_HASH_VERSION_KZG {
            return Err(TxError::Validation(
                InvalidTransaction::BlobElementIsNotSupported,
            ));
        }

        // NOTE: we do NOT check that this blob hash is meaningful - we are not worried about block validity
        // from consensus perspective. And KZG blob precompile requires explicit preimage anyway

        let blob_hash = Bytes32::from_array(*blob_hash);

        result.push(blob_hash);
    }

    if result.is_empty() {
        // transactions that allow blobs should have at least one
        return Err(TxError::Validation(
            InvalidTransaction::BlobElementIsNotSupported,
        ));
    }

    Ok(result)
}

pub fn parse_authorization_list_and_apply_delegations<S: EthereumLikeTypes>(
    system: &mut System<S>,
    resources: &mut S::Resources,
    auth_list: AuthorizationList<'_>,
) -> Result<(), TxError>
where
    S::IO: IOSubsystemExt,
{
    use crate::bootloader::transaction::ethereum_tx_format::AuthorizationEntry;
    let mut hasher = crypto::sha3::Keccak256::new();

    let count = auth_list.count.expect("prevalidated list contains count");
    if count == 0 {
        return Err(TxError::Validation(InvalidTransaction::AuthListIsEmpty));
    }

    for entry in auth_list.iter() {
        let AuthorizationEntry {
            chain_id,
            address,
            nonce,
            y_parity,
            r,
            s,
        } = entry.into_inner();
        let success = validate_and_apply_delegation(
            system,
            resources,
            &chain_id,
            nonce,
            address,
            (y_parity, r, s),
            &mut hasher,
        )?;
        let _ = system
            .get_logger()
            .write_fmt(format_args!("Delegation success: {success}\n"));

        if !success {}
    }
    Ok(())
}

fn compute_auth_message_signed_hash<S: EthereumLikeTypes>(
    resources: &mut S::Resources,
    auth_chain_id: &U256,
    auth_nonce: u64,
    delegation_address: &[u8; 20],
    hasher: &mut crypto::sha3::Keccak256,
) -> Result<[u8; 32], TxError> {
    use crate::bootloader::rlp;
    use crate::bootloader::transaction::EIP7702_MAGIC;
    use crypto::MiniDigest;

    let list_payload_len = rlp::estimate_number_encoding_len(&auth_chain_id.to_be_bytes::<32>())
        + rlp::ADDRESS_ENCODING_LEN
        + rlp::estimate_number_encoding_len(&auth_nonce.to_be_bytes());
    let total_list_len = rlp::estimate_length_encoding_len(list_payload_len) + list_payload_len;
    let encoding_len = 1 + total_list_len;
    crate::bootloader::transaction::charge_keccak(encoding_len, resources)?;
    hasher.update([EIP7702_MAGIC]);
    rlp::apply_list_length_encoding_to_hash(list_payload_len, hasher);
    rlp::apply_number_encoding_to_hash(&auth_chain_id.to_be_bytes::<32>(), hasher);
    rlp::apply_bytes_encoding_to_hash(delegation_address, hasher);
    rlp::apply_number_encoding_to_hash(&auth_nonce.to_be_bytes(), hasher);

    Ok(hasher.finalize_reset())
}

fn recover_authority<S: EthereumLikeTypes>(
    system: &mut System<S>,
    resources: &mut S::Resources,
    auth_sig_data: (u8, &[u8], &[u8]),
    msg: &[u8; 32],
) -> Result<Option<B160>, TxError> {
    use zk_ee::system::SystemFunctions;
    let mut ecrecover_input = [0u8; 128];
    let (parity, r, s) = auth_sig_data;
    ecrecover_input[0..32].copy_from_slice(msg);
    ecrecover_input[63] = if parity <= 1 { parity + 27 } else { parity };
    ecrecover_input[64..96][(32 - r.len())..].copy_from_slice(r);
    ecrecover_input[96..128][(32 - s.len())..].copy_from_slice(s);
    let mut ecrecover_output = ArrayBuilder::default();
    // Recover is counted in intrinsic gas
    resources
        .with_infinite_ergs(|inf_ergs| {
            S::SystemFunctions::secp256k1_ec_recover(
                ecrecover_input.as_slice(),
                &mut ecrecover_output,
                inf_ergs,
                system.get_allocator(),
            )
        })
        .map_err(SystemError::from)?;
    if ecrecover_output.is_empty() {
        Ok(None)
    } else {
        Ok(Some(
            B160::try_from_be_slice(&ecrecover_output.build()[12..])
                .ok_or(internal_error!("Invalid ecrecover return value"))?,
        ))
    }
}

#[inline]
fn validate_and_apply_delegation<S: EthereumLikeTypes>(
    system: &mut System<S>,
    resources: &mut S::Resources,
    auth_chain_id: &U256,
    auth_nonce: u64,
    delegation_address: &[u8; 20],
    auth_sig_data: (u8, &[u8], &[u8]),
    hasher: &mut crypto::sha3::Keccak256,
) -> Result<bool, TxError>
where
    S::IO: IOSubsystemExt,
{
    use zk_ee::system::Ergs;

    // pre-charge
    resources.charge(&S::Resources::from_ergs_and_native(
        Ergs(evm_interpreter::gas_constants::NEWACCOUNT * ERGS_PER_GAS),
        <<S::Resources as Resources>::Native as zk_ee::system::Computational>::from_computational(
            crate::bootloader::constants::PER_AUTH_INTRINSIC_COST,
        ),
    ))?;

    let chain_id = system.get_chain_id();
    // 1. Check chain id
    if !auth_chain_id.is_zero() && auth_chain_id != &U256::from(chain_id) {
        return Ok(false);
    }
    // 2. Check for nonce overflow
    if auth_nonce == u64::MAX {
        return Ok(false);
    }
    // 3. Signature
    // EIP-2 check
    let (_, _, auth_s) = auth_sig_data;
    let s = U256::try_from_be_slice(auth_s)
        .ok_or::<TxError>(InvalidTransaction::InvalidStructure.into())?;
    if s > crypto::secp256k1::SECP256K1N_HALF_U256 {
        return Ok(false);
    }
    let msg = resources.with_infinite_ergs(|inf_ergs| {
        compute_auth_message_signed_hash::<S>(
            inf_ergs,
            auth_chain_id,
            auth_nonce,
            delegation_address,
            hasher,
        )
    })?;
    let Some(authority) = resources
        .with_infinite_ergs(|inf_ergs| recover_authority(system, inf_ergs, auth_sig_data, &msg))?
    else {
        return Ok(false);
    };

    // 4. Read authority account
    let account_properties = resources.with_infinite_ergs(|inf_ergs| {
        system.io.read_account_properties(
            ExecutionEnvironmentType::NoEE,
            inf_ergs,
            &authority,
            AccountDataRequest::empty()
                .with_nonce()
                .with_has_bytecode()
                .with_is_delegated()
                .with_nominal_token_balance(),
        )
    })?;
    // 5. Check authority is not a contract
    if account_properties.is_contract() {
        return Ok(false);
    }
    // 6. Check nonce
    if account_properties.nonce.0 != auth_nonce {
        return Ok(false);
    }
    // 7. Add refund if authority is not empty.
    {
        let is_empty = account_properties.nonce.0 == 0
            && account_properties.has_bytecode.0 == false
            && account_properties.nominal_token_balance.0.is_zero();
        if !is_empty {
            system
                .io
                .add_to_refund_counter(S::Resources::from_ergs(Ergs(
                    (evm_interpreter::gas_constants::NEWACCOUNT
                        - evm_interpreter::gas_constants::PER_AUTH_BASE_COST)
                        * ERGS_PER_GAS,
                )))?;
        }
    }

    let delegation_address = B160::from_be_bytes(*delegation_address);
    let _ = system.get_logger().write_fmt(format_args!(
        "Will delegate address 0x{:040x} -> 0x{:040x}\n",
        authority.as_uint(),
        delegation_address.as_uint()
    ));

    // 8. Set code for authority, system function
    //    will handle the two cases (unsetting).
    resources.with_infinite_ergs(|inf_ergs| {
        system
            .io
            .set_delegation(inf_ergs, &authority, &delegation_address)
    })?;
    // 9.Bump nonce
    resources
        .with_infinite_ergs(|inf_ergs| {
            system
                .io
                .increment_nonce(ExecutionEnvironmentType::NoEE, inf_ergs, &authority, 1)
        })
        .map_err(|e| -> BootloaderSubsystemError {
            match e {
                SubsystemError::LeafUsage(InterfaceError(NonceError::NonceOverflow, _)) => {
                    internal_error!("Cannot overflow, already checked").into()
                }
                _ => wrap_error!(e),
            }
        })?;
    Ok(true)
}

///
/// Will perform basic validation, namely - checking signature, minimal resource requirements for transaction validity,
/// and will pre-charge sender to cover worst case cost. It may perform IO if needed to e.g. warm up some storage slots,
/// or mark delegation
///
/// NOTE: This function will open and close IO frame
pub(crate) fn validate_and_compute_fee_for_transaction<
    'a,
    S: EthereumLikeTypes,
    Config: BasicBootloaderExecutionConfig,
    A: Allocator,
>(
    system: &mut System<S>,
    mut transaction: EthereumTransactionWithBuffer<A>,
    _tracer: &mut impl Tracer<S>,
) -> Result<(EthereumTxContext<S>, EthereumTransactionWithBuffer<A>), TxError>
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
    let originator_expected_nonce = transaction.nonce();

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
            let intrinsic_gas = (L2_TX_INTRINSIC_GAS as u64).saturating_add(floor_tokens_gas_cost);

            require!(
                intrinsic_gas <= tx_gas_limit,
                InvalidTransaction::EIP7623IntrinsicGasIsTooLow,
                system
            )?;

            (num_tokens, intrinsic_gas)
        }

        #[cfg(not(feature = "eip_7623"))]
        {
            (num_tokens, L2_TX_INTRINSIC_GAS as u64)
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

    let is_deployment = transaction.destination().is_none();

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

    // now we should recover signature. Transaction was already pre-processed under assumption of particular chain ID,
    // and we will double-check
    if let Some(chain_id) = transaction.chain_id() {
        assert_eq!(system.get_chain_id(), chain_id);
    }

    // We need sender before any IO
    if Config::ONLY_SIMULATE == false {
        transaction.recover_signer(|tx| {
            let signed_hash = tx.hash_for_signature_verification();
            let (parity, r, s) = tx.sig_parity_r_s();

            if U256::from_be_slice(s) > crypto::secp256k1::SECP256K1N_HALF_U256 {
                return Err(TxError::Validation(InvalidTransaction::MalleableSignature));
            }

            let mut ecrecover_input = [0u8; 128];
            ecrecover_input[0..32].copy_from_slice(signed_hash.as_u8_array_ref());
            ecrecover_input[63] = (parity as u8) + 27;
            ecrecover_input[64..96][(32 - r.len())..].copy_from_slice(r);
            ecrecover_input[96..128][(32 - s.len())..].copy_from_slice(s);

            let mut ecrecover_output = ArrayBuilder::default();
            tx_resources
                .main_resources
                .with_infinite_ergs(|resources| {
                    S::SystemFunctions::secp256k1_ec_recover(
                        &ecrecover_input[..],
                        &mut ecrecover_output,
                        resources,
                        system.get_allocator(),
                    )
                    .map_err(SystemError::from)
                })?;

            if ecrecover_output.is_empty() {
                return Err(InvalidTransaction::InvalidStructure.into());
            }

            let recovered_from = B160::try_from_be_slice(&ecrecover_output.build()[12..])
                .ok_or(internal_error!("Invalid ecrecover return value"))?;

            Ok(recovered_from)
        })?;
    } else {
        // Ask oracle
        todo!();
    }

    // any IO starts here
    let from = transaction.signer();

    // now we can perform IO related parts. Getting originator's properties is included into the
    // intrinsic cost charnged above
    let originator_account_data =
        tx_resources
            .main_resources
            .with_infinite_ergs(|inf_resources| {
                system.io.read_account_properties(
                    ExecutionEnvironmentType::NoEE,
                    inf_resources,
                    from,
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
        Err(SubsystemError::LeafRuntime(RuntimeError::OutOfNativeResources(_))) => {
            // TODO: decide if we wan to allow such cases at all
            return Err(TxError::Validation(
                InvalidTransaction::OutOfNativeResourcesDuringValidation,
            ));
        }
        Err(SubsystemError::Cascaded(cascaded)) => match cascaded {},
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
    if let Some(access_list) = transaction.access_list() {
        parse_and_warm_up_access_list(system, &mut tx_resources.main_resources, access_list)?
    }

    let blobs = if let Some(blobs_list) = transaction.blobs_list() {
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
        tx_origin: *transaction.signer(),
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

    Ok((context, transaction))
}
