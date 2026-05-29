#![cfg_attr(target_arch = "riscv32", no_std)]
#![feature(allocator_api)]
#![feature(get_mut_unchecked)]
#![feature(vec_push_within_capacity)]
#![feature(ptr_alignment_type)]
#![feature(btreemap_alloc)]
#![feature(maybe_uninit_array_assume_init)]
#![feature(ptr_metadata)]
#![allow(incomplete_features)]
#![allow(clippy::needless_borrow)]
#![allow(clippy::needless_borrows_for_generic_args)]
#![allow(clippy::result_unit_err)]
#![allow(clippy::type_complexity)]

//!
//! This crate contains system hooks implementation.
//!
//! System hook - special system logic, that can be triggered by call on a specific system address(less than 2^16).
//! It's implemented as function that receives system object, call request and returns execution result.
//!
//! They used to process EVM precompiles, EraVM system contracts/precompiles calls.
//! They are implemented on a system level(as rust code).
//!
extern crate alloc;

use crate::addresses_constants::*;
use crate::call_hooks::contract_deployer_temp::contract_deployer_temp_hook;
use crate::call_hooks::fri_precompile::fri_precompile_hook;
use crate::call_hooks::l1_messenger::l1_messenger_hook;
use crate::call_hooks::mint_base_token::mint_base_token_hook;
use crate::call_hooks::set_bytecode_on_address::set_bytecode_on_address_hook;
use crate::event_hooks::interop_root_reporter::interop_root_reporter_event_hook;
use crate::event_hooks::system_context::system_context_event_hook;
use call_hooks::precompiles::{
    pure_system_function_hook_impl, IdentityPrecompile, IdentityPrecompileErrors,
};
use core::marker::PhantomData;
use core::{alloc::Allocator, mem::MaybeUninit};
use evm_interpreter::precompile_addresses::*;
use evm_interpreter::ERGS_PER_GAS;
use zk_ee::common_structs::system_hooks::{HooksStorage, SystemCallHook, SystemEventHook};
use zk_ee::common_traits::TryExtend;
use zk_ee::internal_error;
#[cfg(feature = "blake2f")]
use zk_ee::system::base_system_functions::Blake2FPrecompileErrors;
#[cfg(feature = "bls12_381")]
use zk_ee::system::base_system_functions::Bls12PrecompileErrors;
#[cfg(feature = "p256_precompile")]
use zk_ee::system::base_system_functions::P256VerifyErrors;
use zk_ee::system::errors::internal::InternalError;
use zk_ee::system::errors::subsystem::SubsystemError;
#[cfg(all(
    feature = "mock-unsupported-precompiles",
    any(not(feature = "blake2f"), not(feature = "point_eval_precompile"))
))]
use zk_ee::system::MissingSystemFunctionErrors;
use zk_ee::{
    memory::slice_vec::SliceVec,
    system::{
        base_system_functions::{
            Bn254AddErrors, Bn254MulErrors, Bn254PairingCheckErrors, ModExpErrors, RipeMd160Errors,
            Secp256k1ECRecoverErrors, Sha256Errors,
        },
        errors::subsystem::Subsystem,
        EthereumLikeTypes, System, SystemTypes, *,
    },
};

pub mod addresses_constants;
pub mod call_hooks;
pub mod event_hooks;

pub trait SystemFunctionInvocation<S: SystemTypes, E: Subsystem>
where
    S::IO: IOSubsystemExt,
{
    fn invoke<D: TryExtend<u8> + ?Sized, A: core::alloc::Allocator + Clone>(
        oracle: &mut <S::IO as IOSubsystemExt>::IOOracle,
        logger: &mut S::Logger,
        input: &[u8],
        output: &mut D,
        resources: &mut S::Resources,
        allocator: A,
    ) -> Result<(), SubsystemError<E>>;
}

struct SystemFunctionInvocationUser<
    S: SystemTypes,
    E: Subsystem,
    F: SystemFunction<S::Resources, E>,
>(PhantomData<(S, E, F)>);
struct SystemFunctionInvocationExt<
    S: SystemTypes,
    E: Subsystem,
    F: SystemFunctionExt<S::Resources, E>,
>(PhantomData<(S, E, F)>);

impl<S: SystemTypes, E: Subsystem, F: SystemFunction<S::Resources, E>>
    SystemFunctionInvocation<S, E> for SystemFunctionInvocationUser<S, E, F>
where
    S::IO: IOSubsystemExt,
{
    fn invoke<D: TryExtend<u8> + ?Sized, A: core::alloc::Allocator + Clone>(
        _oracle: &mut <S::IO as IOSubsystemExt>::IOOracle,
        _logger: &mut S::Logger,
        input: &[u8],
        output: &mut D,
        resources: &mut S::Resources,
        allocator: A,
    ) -> Result<(), SubsystemError<E>> {
        F::execute(input, output, resources, allocator)
    }
}

impl<S: SystemTypes, E: Subsystem, F: SystemFunctionExt<S::Resources, E>>
    SystemFunctionInvocation<S, E> for SystemFunctionInvocationExt<S, E, F>
where
    S::IO: IOSubsystemExt,
{
    fn invoke<D: TryExtend<u8> + ?Sized, A: core::alloc::Allocator + Clone>(
        oracle: &mut <S::IO as IOSubsystemExt>::IOOracle,
        logger: &mut S::Logger,
        input: &[u8],
        output: &mut D,
        resources: &mut S::Resources,
        allocator: A,
    ) -> Result<(), SubsystemError<E>> {
        F::execute(input, output, resources, oracle, logger, allocator)
    }
}

/// EE-triggered ecrecover invocation. Wraps the existing ecrecover dispatch
/// in an outer `"ecrecover_execution_environment"` cycle marker so
/// per-execution cycles can be joined cleanly with the
/// `PrecompileStatsTracer` `ecrecover.samples` (which only sees EE precompile
/// dispatch frames). The bootloader's intrinsic sig-recovery calls do not go
/// through this path, so the new marker fires only for EE-triggered calls.
/// The inner `"ecrecover"` marker (from `EcRecoverImpl::execute`) still
/// fires, preserving backward compatibility for older consumers.
struct EcRecoverEEInvocation<S: SystemTypes>(PhantomData<S>);

impl<S: SystemTypes>
    SystemFunctionInvocation<S, zk_ee::system::base_system_functions::Secp256k1ECRecoverErrors>
    for EcRecoverEEInvocation<S>
where
    S::IO: IOSubsystemExt,
    S: EthereumLikeTypes,
{
    fn invoke<D: TryExtend<u8> + ?Sized, A: core::alloc::Allocator + Clone>(
        oracle: &mut <S::IO as IOSubsystemExt>::IOOracle,
        logger: &mut S::Logger,
        input: &[u8],
        output: &mut D,
        resources: &mut S::Resources,
        allocator: A,
    ) -> Result<(), SubsystemError<zk_ee::system::base_system_functions::Secp256k1ECRecoverErrors>>
    {
        cycle_marker::wrap_with_resources!("ecrecover_execution_environment", resources, {
            <S::SystemFunctionsExt as SystemFunctionsExt<_>>::Secp256k1ECRecover::execute(
                input, output, resources, oracle, logger, allocator,
            )
        })
    }
}

///
/// Adds EVM precompiles hooks.
///
pub fn add_precompiles<S: EthereumLikeTypes, A: Allocator + Clone>(
    hooks: &mut HooksStorage<S, A>,
) -> Result<(), InternalError>
where
    S::IO: IOSubsystemExt,
{
    // EE-frame ecrecover dispatch uses a dedicated invocation wrapper that
    // emits the `"ecrecover_execution_environment"` cycle marker around the
    // underlying system function call, so per-execution stats can be joined
    // without the positional bootloader/intrinsic filter heuristic. Routed
    // through `install_precompile_hook` so the address sanity check stays
    // in lockstep with `add_precompile` / `add_precompile_ext`.
    install_precompile_hook::<S, A, EcRecoverEEInvocation<S>, Secp256k1ECRecoverErrors>(
        hooks,
        ECRECOVER_HOOK_ADDRESS_LOW,
    )?;
    add_precompile::<_, _, <S::SystemFunctions as SystemFunctions<_>>::Sha256, Sha256Errors>(
        hooks,
        SHA256_HOOK_ADDRESS_LOW,
    )?;
    add_precompile::<_, _, <S::SystemFunctions as SystemFunctions<_>>::RipeMd160, RipeMd160Errors>(
        hooks,
        RIPEMD160_HOOK_ADDRESS_LOW,
    )?;
    add_precompile::<_, _, IdentityPrecompile, IdentityPrecompileErrors>(
        hooks,
        ID_HOOK_ADDRESS_LOW,
    )?;
    add_precompile_ext::<
        _,
        _,
        <S::SystemFunctionsExt as SystemFunctionsExt<_>>::ModExp,
        ModExpErrors,
    >(hooks, MODEXP_HOOK_ADDRESS_LOW)?;
    add_precompile::<_, _, <S::SystemFunctions as SystemFunctions<_>>::Bn254Add, Bn254AddErrors>(
        hooks,
        ECADD_HOOK_ADDRESS_LOW,
    )?;
    add_precompile::<_, _, <S::SystemFunctions as SystemFunctions<_>>::Bn254Mul, Bn254MulErrors>(
        hooks,
        ECMUL_HOOK_ADDRESS_LOW,
    )?;
    add_precompile::<
        _,
        _,
        <S::SystemFunctions as SystemFunctions<_>>::Bn254PairingCheck,
        Bn254PairingCheckErrors,
    >(hooks, ECPAIRING_HOOK_ADDRESS_LOW)?;
    #[cfg(feature = "blake2f")]
    add_precompile::<
        _,
        _,
        <S::SystemFunctions as SystemFunctions<_>>::Blake2F,
        Blake2FPrecompileErrors,
    >(hooks, BLAKE2F_HOOK_ADDRESS_LOW)?;

    #[cfg(all(feature = "mock-unsupported-precompiles", not(feature = "blake2f")))]
    add_precompile::<
        _,
        _,
        crate::call_hooks::mock_precompiles::mock_precompiles::Blake2f,
        MissingSystemFunctionErrors,
    >(hooks, BLAKE2F_HOOK_ADDRESS_LOW)?;

    #[cfg(feature = "mock-unsupported-precompiles")]
    {
        #[cfg(not(feature = "point_eval_precompile"))]
        add_precompile::<
            _,
            _,
            crate::call_hooks::mock_precompiles::mock_precompiles::PointEvaluation,
            MissingSystemFunctionErrors,
        >(hooks, POINT_EVAL_HOOK_ADDRESS_LOW)?;
    }
    #[cfg(feature = "point_eval_precompile")]
    add_precompile::<
        _,
        _,
        <S::SystemFunctions as SystemFunctions<_>>::PointEvaluation,
        PointEvaluationErrors,
    >(hooks, POINT_EVAL_HOOK_ADDRESS_LOW)?;

    #[cfg(feature = "p256_precompile")]
    {
        add_precompile::<
            _,
            _,
            <S::SystemFunctions as SystemFunctions<_>>::P256Verify,
            P256VerifyErrors,
        >(hooks, P256_VERIFY_PREHASH_HOOK_ADDRESS_LOW)?;
    }

    #[cfg(feature = "bls12_381")]
    {
        add_precompile::<
            _,
            _,
            <S::SystemFunctions as SystemFunctions<_>>::Bls12G1Add,
            Bls12PrecompileErrors,
        >(hooks, BLS12_G1ADD_ADDRESS_LOW)?;
        add_precompile::<
            _,
            _,
            <S::SystemFunctions as SystemFunctions<_>>::Bls12G2Add,
            Bls12PrecompileErrors,
        >(hooks, BLS12_G2ADD_ADDRESS_LOW)?;
        add_precompile::<
            _,
            _,
            <S::SystemFunctions as SystemFunctions<_>>::Bls12G1Msm,
            Bls12PrecompileErrors,
        >(hooks, BLS12_G1MSM_ADDRESS_LOW)?;
        add_precompile::<
            _,
            _,
            <S::SystemFunctions as SystemFunctions<_>>::Bls12G2Msm,
            Bls12PrecompileErrors,
        >(hooks, BLS12_G2MSM_ADDRESS_LOW)?;
        add_precompile::<
            _,
            _,
            <S::SystemFunctions as SystemFunctions<_>>::Bls12PairingCheck,
            Bls12PrecompileErrors,
        >(hooks, BLS12_PAIRING_CHECK_ADDRESS_LOW)?;
        add_precompile::<
            _,
            _,
            <S::SystemFunctions as SystemFunctions<_>>::Bls12MapFpToG1,
            Bls12PrecompileErrors,
        >(hooks, BLS12_MAP_FP_TO_G1_ADDRESS_LOW)?;
        add_precompile::<
            _,
            _,
            <S::SystemFunctions as SystemFunctions<_>>::Bls12MapFp2ToG2,
            Bls12PrecompileErrors,
        >(hooks, BLS12_MAP_FP2_TO_G2_ADDRESS_LOW)?;
    }

    Ok(())
}

/// Register the FRI proof verification hook at
/// `FRI_PRECOMPILE_ADDRESS`.
pub fn add_fri_proof_verification_hook<S: EthereumLikeTypes, A: Allocator + Clone>(
    hooks: &mut HooksStorage<S, A>,
) -> Result<(), InternalError>
where
    S::IO: IOSubsystemExt,
{
    hooks.add_call_hook(
        FRI_PRECOMPILE_ADDRESS_LOW,
        SystemCallHook::new(fri_precompile_hook),
    )
}

pub fn add_l1_messenger<S: EthereumLikeTypes, A: Allocator + Clone>(
    hooks: &mut HooksStorage<S, A>,
) -> Result<(), InternalError> {
    hooks.add_call_hook(
        L1_MESSENGER_ADDRESS_HOOK_LOW,
        SystemCallHook::new(l1_messenger_hook),
    )
}

pub fn add_set_bytecode_on_address_hook<S: EthereumLikeTypes, A: Allocator + Clone>(
    hooks: &mut HooksStorage<S, A>,
) -> Result<(), InternalError>
where
    S::IO: IOSubsystemExt,
{
    hooks.add_call_hook(
        SET_BYTECODE_ON_ADDRESS_HOOK_LOW,
        SystemCallHook::new(set_bytecode_on_address_hook),
    )
}

pub fn add_contract_deployer<S: EthereumLikeTypes, A: Allocator + Clone>(
    hooks: &mut HooksStorage<S, A>,
) -> Result<(), InternalError>
where
    S::IO: IOSubsystemExt,
{
    hooks.add_call_hook(
        CONTRACT_DEPLOYER_ADDRESS_LOW,
        SystemCallHook::new(contract_deployer_temp_hook),
    )
}

pub fn add_base_token_mint<S: EthereumLikeTypes, A: Allocator + Clone>(
    hooks: &mut HooksStorage<S, A>,
) -> Result<(), InternalError>
where
    S::IO: IOSubsystemExt,
{
    hooks.add_call_hook(
        MINT_HOOK_ADDRESS_LOW,
        SystemCallHook::new(mint_base_token_hook),
    )
}

pub fn add_interop_root_reporter<S: EthereumLikeTypes, A: Allocator + Clone>(
    hooks: &mut HooksStorage<S, A>,
) -> Result<(), InternalError> {
    hooks.add_event_hook(
        L2_INTEROP_ROOT_STORAGE_ADDRESS_LOW,
        SystemEventHook::new(interop_root_reporter_event_hook),
    )
}

pub fn add_system_context_reporter<S: EthereumLikeTypes, A: Allocator + Clone>(
    hooks: &mut HooksStorage<S, A>,
) -> Result<(), InternalError> {
    hooks.add_event_hook(
        SYSTEM_CONTEXT_ADDRESS_LOW,
        SystemEventHook::new(system_context_event_hook),
    )
}

pub fn add_precompile<S: EthereumLikeTypes, A: Allocator + Clone, P, E>(
    hooks: &mut HooksStorage<S, A>,
    address_low: u16,
) -> Result<(), InternalError>
where
    S::IO: IOSubsystemExt,
    P: SystemFunction<S::Resources, E>,
    E: Subsystem,
{
    // Sanity check to ensure that the address being added is indeed in the precompile addresses list
    if !PRECOMPILE_ADDRESSES_LOWS.contains(&address_low) {
        return Err(internal_error!(
            "Attempted to add a precompile that is not in the precompile addresses list"
        ));
    }
    hooks.add_call_hook(
        address_low,
        SystemCallHook::new(
            pure_system_function_hook_impl::<SystemFunctionInvocationUser<S, E, P>, E, S>,
        ),
    )
}

/// Install a precompile hook with a hand-picked invocation type.
///
/// Centralizes the `PRECOMPILE_ADDRESSES_LOWS` sanity check so any future
/// hook wired through this helper inherits the same defensive guard as
/// `add_precompile`. Used both by `add_precompile_ext` (generic SystemFunctionExt
/// dispatch) and the ecrecover EE dispatch below (which uses a custom
/// invocation type to inject the `ecrecover_execution_environment` cycle
/// marker).
fn install_precompile_hook<S, A, I, E>(
    hooks: &mut HooksStorage<S, A>,
    address_low: u16,
) -> Result<(), InternalError>
where
    S: EthereumLikeTypes,
    S::IO: IOSubsystemExt,
    A: Allocator + Clone,
    I: SystemFunctionInvocation<S, E>,
    E: Subsystem,
{
    if !PRECOMPILE_ADDRESSES_LOWS.contains(&address_low) {
        return Err(internal_error!(
            "Attempted to add a precompile that is not in the precompile addresses list"
        ));
    }
    hooks.add_call_hook(
        address_low,
        SystemCallHook::new(pure_system_function_hook_impl::<I, E, S>),
    )
}

fn add_precompile_ext<
    S: EthereumLikeTypes,
    A: Allocator + Clone,
    P: SystemFunctionExt<S::Resources, E>,
    E: Subsystem,
>(
    hooks: &mut HooksStorage<S, A>,
    address_low: u16,
) -> Result<(), InternalError>
where
    S::IO: IOSubsystemExt,
{
    install_precompile_hook::<S, A, SystemFunctionInvocationExt<S, E, P>, E>(hooks, address_low)
}

///
/// Utility function to create empty revert state.
///
fn make_error_return_state<'a, S: SystemTypes>(
    remaining_resources: S::Resources,
) -> CompletedExecution<'a, S> {
    CompletedExecution {
        resources_returned: remaining_resources,
        result: CallResult::Failed {
            return_values: ReturnValues::empty(),
        },
    }
}

///
/// Utility function to create return state with returndata region reference.
///
fn make_return_state_from_returndata_region<S: SystemTypes>(
    remaining_resources: S::Resources,
    returndata: &[u8],
) -> CompletedExecution<'_, S> {
    let return_values = ReturnValues {
        returndata,
        return_scratch_space: None,
    };
    CompletedExecution {
        resources_returned: remaining_resources,
        result: CallResult::Successful { return_values },
    }
}

/// Base cost for calling into a system hook
const HOOK_BASE_NATIVE_COST: u64 = 1000;

/// Ergs cost per byte of bytecode for force deployments.
const SET_BYTECODE_DETAILS_EXTRA_ERGS_PER_BYTE: Ergs = Ergs(50 * ERGS_PER_GAS);
