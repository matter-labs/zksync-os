#![cfg_attr(target_arch = "riscv32", no_std)]
#![feature(allocator_api)]
#![feature(array_chunks)]
#![feature(get_mut_unchecked)]
#![feature(const_type_id)]
#![feature(vec_push_within_capacity)]
#![feature(ptr_alignment_type)]
#![feature(btreemap_alloc)]
#![feature(maybe_uninit_array_assume_init)]
#![feature(ptr_metadata)]
#![allow(incomplete_features)]
#![feature(generic_const_exprs)]
#![feature(alloc_layout_extra)]
#![feature(array_windows)]
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
use crate::call_hooks::contract_deployer::contract_deployer_hook;
use crate::call_hooks::l1_messenger::l1_messenger_hook;
use crate::call_hooks::l2_base_token::l2_base_token_hook;
use crate::event_hooks::interop_root_reporter::interop_root_reporter_event_hook;
use call_hooks::precompiles::{
    pure_system_function_hook_impl, IdentityPrecompile, IdentityPrecompileErrors,
};
use core::marker::PhantomData;
use core::{alloc::Allocator, mem::MaybeUninit};
use evm_interpreter::ERGS_PER_GAS;
use zk_ee::common_structs::system_hooks::{HooksStorage, SystemCallHook, SystemEventHook};
use zk_ee::common_traits::TryExtend;
use zk_ee::system::errors::internal::InternalError;
use zk_ee::system::errors::subsystem::SubsystemError;
#[cfg(feature = "mock-unsupported-precompiles")]
use zk_ee::system::MissingSystemFunctionErrors;
use zk_ee::{
    memory::slice_vec::SliceVec,
    system::{
        base_system_functions::{
            Bn254AddErrors, Bn254MulErrors, Bn254PairingCheckErrors, ModExpErrors,
            P256VerifyErrors, RipeMd160Errors, Secp256k1ECRecoverErrors, Sha256Errors,
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

///
/// Adds EVM precompiles hooks.
///
pub fn add_precompiles<S: EthereumLikeTypes, A: Allocator + Clone>(
    hooks: &mut HooksStorage<S, A>,
) -> Result<(), InternalError>
where
    S::IO: IOSubsystemExt,
{
    add_precompile::<
        _,
        _,
        <S::SystemFunctions as SystemFunctions<_>>::Secp256k1ECRecover,
        Secp256k1ECRecoverErrors,
    >(hooks, ECRECOVER_HOOK_ADDRESS_LOW)?;
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
    #[cfg(feature = "mock-unsupported-precompiles")]
    {
        add_precompile::<
            _,
            _,
            crate::call_hooks::mock_precompiles::mock_precompiles::Blake2f,
            MissingSystemFunctionErrors,
        >(hooks, BLAKE2F_HOOK_ADDRESS_LOW)?;

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
    Ok(())
}

pub fn add_l1_messenger<S: EthereumLikeTypes, A: Allocator + Clone>(
    hooks: &mut HooksStorage<S, A>,
) -> Result<(), InternalError> {
    hooks.add_call_hook(
        L1_MESSENGER_ADDRESS_LOW,
        SystemCallHook::new(l1_messenger_hook),
    )
}

pub fn add_l2_base_token<S: EthereumLikeTypes, A: Allocator + Clone>(
    hooks: &mut HooksStorage<S, A>,
) -> Result<(), InternalError>
where
    S::IO: IOSubsystemExt,
{
    hooks.add_call_hook(
        L2_BASE_TOKEN_ADDRESS_LOW,
        SystemCallHook::new(l2_base_token_hook),
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
        SystemCallHook::new(contract_deployer_hook),
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

fn add_precompile<S: EthereumLikeTypes, A: Allocator + Clone, P, E>(
    hooks: &mut HooksStorage<S, A>,
    address_low: u16,
) -> Result<(), InternalError>
where
    S::IO: IOSubsystemExt,
    P: SystemFunction<S::Resources, E>,
    E: Subsystem,
{
    hooks.add_call_hook(
        address_low,
        SystemCallHook::new(
            pure_system_function_hook_impl::<SystemFunctionInvocationUser<S, E, P>, E, S>,
        ),
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
    hooks.add_call_hook(
        address_low,
        SystemCallHook::new(
            pure_system_function_hook_impl::<SystemFunctionInvocationExt<S, E, P>, E, S>,
        ),
    )
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
) -> CompletedExecution<S> {
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

/// Base ergs cost for calling a system hook (100 gas)
const HOOK_BASE_ERGS_COST: Ergs = Ergs(100 * ERGS_PER_GAS);

/// Ergs cost per byte of bytecode for force deployments.
const SET_BYTECODE_DETAILS_EXTRA_ERGS_PER_BYTE: Ergs = Ergs(50 * ERGS_PER_GAS);
