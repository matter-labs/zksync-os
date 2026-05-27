use super::super::*;
use ruint::aliases::U256;
use zk_ee::internal_error;
use zk_ee::memory::slice_vec::SliceVec;
use zk_ee::system::errors::system::SystemError;
use zk_ee::system::metadata::basic_metadata::BasicTransactionMetadata;
use zk_ee::utils::Bytes32;

/// Gateway-only precompile that reports whether a statement hash was verified
/// earlier in the current transaction.
///
/// Input:  32 bytes — `statement_versioned_hash`
/// Output: 32 bytes — ABI-encoded `bool`: `0x00..01` if verified, `0x00..00` otherwise
pub fn fri_precompile_hook<'a, S: EthereumLikeTypes>(
    request: ExternalCallRequest<S>,
    _caller_ee: u8,
    system: &mut System<S>,
    return_memory: &'a mut [MaybeUninit<u8>],
) -> Result<(CompletedExecution<'a, S>, &'a mut [MaybeUninit<u8>]), SystemError>
where
    S::IO: IOSubsystemExt,
    S::Metadata: BasicTransactionMetadata<S::IOTypes>,
{
    let ExternalCallRequest {
        available_resources,
        ergs_to_pass: _,
        input: calldata,
        call_scratch_space: _,
        nominal_token_value,
        caller: _,
        callee: _,
        callers_caller: _,
        modifier,
    } = request;

    if modifier == CallModifier::Constructor {
        return Err(internal_error!("FRI precompile called with constructor modifier").into());
    }

    if nominal_token_value != U256::ZERO {
        return Ok((make_error_return_state(available_resources), return_memory));
    }

    if calldata.len() != 32 {
        return Ok((make_error_return_state(available_resources), return_memory));
    }

    let mut resources = available_resources;
    evm_interpreter::charge_native_and_ergs::<S::Resources>(
        &mut resources,
        HOOK_BASE_NATIVE_COST,
        Ergs(0),
    )?;

    let mut statement_versioned_hash_bytes = [0u8; 32];
    statement_versioned_hash_bytes.copy_from_slice(calldata);
    let statement_versioned_hash = Bytes32::from_array(statement_versioned_hash_bytes);

    let verified = system
        .metadata
        .is_fri_statement_verified(&statement_versioned_hash);

    // ABI-encode bool: 31 zero bytes followed by 0x01 (true) or 0x00 (false).
    let mut returndata_bytes = [0u8; 32];
    if verified {
        returndata_bytes[31] = 1;
    }

    let mut return_vec = SliceVec::new(return_memory);
    return_vec
        .try_extend(returndata_bytes)
        .map_err(|_| internal_error!("FRI precompile returndata does not fit"))?;
    let (returndata, rest) = return_vec.destruct();

    Ok((
        make_return_state_from_returndata_region(resources, returndata),
        rest,
    ))
}
