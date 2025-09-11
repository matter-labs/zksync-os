#![cfg_attr(target_arch = "riscv32", no_std)]
#![feature(allocator_api)]
#![feature(btreemap_alloc)]
#![allow(clippy::needless_borrow)]
#![allow(clippy::needless_borrows_for_generic_args)]
#![allow(clippy::result_unit_err)]
#![allow(clippy::type_complexity)]
// TODO: We use final type constants for many cost constants, but we could use smaller ones to reduce binary size

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
use crate::contract_deployer::contract_deployer_hook;
use crate::l1_messenger::l1_messenger_hook;
use crate::l2_base_token::l2_base_token_hook;
use crate::precompiles::precompile_hook_impl;
use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use core::marker::PhantomData;
use core::{alloc::Allocator, mem::MaybeUninit};
use precompiles::{pure_system_function_hook_impl, IdentityPrecompile, IdentityPrecompileErrors};
use zk_ee::system::errors::subsystem::SubsystemError;
use zk_ee::system::errors::system::SystemError;
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
pub mod mock_precompiles;

// Temporarily disabled, only used for AA.
// pub mod nonce_holder;
mod contract_deployer;
pub mod eip_152;
pub mod eip_2537;
mod l1_messenger;
mod l2_base_token;
mod precompiles;

pub use self::eip_2537::*;

pub use self::precompiles::PurePrecompileInvocation;

/// System hooks process the given call request.
///
/// The inputs are:
/// - call request
/// - caller ee(logic may depend on it some cases)
/// - system
/// - output buffer
pub enum SystemHook<S: SystemTypes> {
    Stateless(SystemStatelessHook<S>),
    StatefulImmutable(Box<dyn StatefulImmutableSystemHook<S>, S::Allocator>),
}

pub trait StatefulImmutableSystemHookImpl<'a, S: SystemTypes> {
    fn invoke(
        &'_ self,
        request: ExternalCallRequest<'_, S>,
        caller_ee: u8,
        system: &'_ mut System<S>,
        return_memory: &'a mut [MaybeUninit<u8>],
    ) -> Result<(CompletedExecution<'a, S>, &'a mut [MaybeUninit<u8>]), SystemError>;
}

pub trait StatefulImmutableSystemHook<S: SystemTypes>:
    for<'a> StatefulImmutableSystemHookImpl<'a, S>
{
    fn invoke<'a>(
        &'_ self,
        request: ExternalCallRequest<'_, S>,
        caller_ee: u8,
        system: &'_ mut System<S>,
        return_memory: &'a mut [MaybeUninit<u8>],
    ) -> Result<(CompletedExecution<'a, S>, &'a mut [MaybeUninit<u8>]), SystemError>
    where
        System<S>: 'a,
    {
        StatefulImmutableSystemHookImpl::invoke(self, request, caller_ee, system, return_memory)
    }
}

pub struct SystemStatelessHook<S: SystemTypes>(
    for<'a> fn(
        ExternalCallRequest<'_, S>,
        u8,
        &'_ mut System<S>,
        &'a mut [MaybeUninit<u8>],
    ) -> Result<(CompletedExecution<'a, S>, &'a mut [MaybeUninit<u8>]), SystemError>,
);

pub trait SystemFunctionInvocation<S: SystemTypes, E: Subsystem>
where
    S::IO: IOSubsystemExt,
{
    fn invoke<D: Extend<u8> + ?Sized, A: core::alloc::Allocator + Clone>(
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
    fn invoke<D: Extend<u8> + ?Sized, A: core::alloc::Allocator + Clone>(
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
    fn invoke<D: Extend<u8> + ?Sized, A: core::alloc::Allocator + Clone>(
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
/// System hooks storage.
/// Stores hooks implementations and processes calls to system addresses.
///
pub struct HooksStorage<S: SystemTypes, A: Allocator + Clone> {
    inner: BTreeMap<u16, SystemHook<S>, A>,
}

impl<S: SystemTypes, A: Allocator + Clone> HooksStorage<S, A> {
    ///
    /// Creates empty hooks storage with a given allocator.
    ///
    pub fn new_in(allocator: A) -> Self {
        Self {
            inner: BTreeMap::new_in(allocator),
        }
    }

    ///
    /// Adds a new hook into a given address.
    /// Fails if there was another hook registered there before.
    ///
    #[track_caller]
    pub fn add_hook(&mut self, for_address_low: u16, hook: SystemHook<S>) {
        let existing = self.inner.insert(for_address_low, hook);
        // TODO: internal error?
        assert!(existing.is_none());
    }

    ///
    /// Intercepts calls to low addresses (< 2^16) and executes hooks
    /// stored under that address. If no hook is stored there, return `Ok(None)`.
    /// Always return unused return_memory.
    ///
    pub fn try_intercept<'a>(
        &'_ mut self,
        address_low: u16,
        request: ExternalCallRequest<'_, S>,
        caller_ee: u8,
        system: &'_ mut System<S>,
        return_memory: &'a mut [MaybeUninit<u8>],
    ) -> Result<(Option<CompletedExecution<'a, S>>, &'a mut [MaybeUninit<u8>]), SystemError>
    where
        S: 'a,
    {
        let Some(hook) = self.inner.get(&address_low) else {
            return Ok((None, return_memory));
        };
        let (res, remaining_memory) = match hook {
            SystemHook::Stateless(hook) => hook.0(request, caller_ee, system, return_memory)?,
            SystemHook::StatefulImmutable(hook) => StatefulImmutableSystemHook::invoke(
                hook.as_ref(),
                request,
                caller_ee,
                system,
                return_memory,
            )?,
        };

        Ok((Some(res), remaining_memory))
    }

    ///
    /// Checks if there is a hook stored for a given low address (<16 bits).
    ///
    pub fn has_hook_for(&mut self, address_low: u16) -> bool {
        self.inner.contains_key(&address_low)
    }

    pub fn all_hooked_addresses_iter(&'_ self) -> impl Iterator<Item = u16> + '_ {
        self.inner.keys().copied()
    }
}

impl<S: EthereumLikeTypes, A: Allocator + Clone> HooksStorage<S, A>
where
    S::IO: IOSubsystemExt,
{
    ///
    /// Adds EVM precompiles hooks.
    ///
    pub fn add_precompiles(&mut self) {
        self.add_precompile::<<S::SystemFunctions as SystemFunctions<_>>::Secp256k1ECRecover, Secp256k1ECRecoverErrors>(
            ECRECOVER_HOOK_ADDRESS_LOW,
        );
        self.add_precompile::<<S::SystemFunctions as SystemFunctions<_>>::Sha256, Sha256Errors>(
            SHA256_HOOK_ADDRESS_LOW,
        );
        self.add_precompile::<<S::SystemFunctions as SystemFunctions<_>>::RipeMd160, RipeMd160Errors>(
            RIPEMD160_HOOK_ADDRESS_LOW,
        );
        self.add_precompile::<IdentityPrecompile, IdentityPrecompileErrors>(ID_HOOK_ADDRESS_LOW);
        self.add_precompile_ext::<<S::SystemFunctionsExt as SystemFunctionsExt<_>>::ModExp, ModExpErrors>(
            MODEXP_HOOK_ADDRESS_LOW,
        );
        self.add_precompile::<<S::SystemFunctions as SystemFunctions<_>>::Bn254Add, Bn254AddErrors>(
            ECADD_HOOK_ADDRESS_LOW,
        );
        self.add_precompile::<<S::SystemFunctions as SystemFunctions<_>>::Bn254Mul, Bn254MulErrors>(
            ECMUL_HOOK_ADDRESS_LOW,
        );
        self.add_precompile::<<S::SystemFunctions as SystemFunctions<_>>::Bn254PairingCheck, Bn254PairingCheckErrors>(
            ECPAIRING_HOOK_ADDRESS_LOW,
        );
        self.add_precompile_from_pure_invocation::<crate::eip_152::Blake2FPrecompile>(
            BLAKE_HOOK_ADDRESS_LOW,
        );

        #[cfg(feature = "mock-unsupported-precompiles")]
        {
            use zk_ee::system::base_system_functions::MissingSystemFunctionErrors;

            self.add_precompile::<crate::mock_precompiles::mock_precompiles::PointEval, MissingSystemFunctionErrors>(
                POINT_EVAL_HOOK_ADDRESS_LOW,
            );
        }

        #[cfg(feature = "p256_precompile")]
        {
            self.add_precompile::<<S::SystemFunctions as SystemFunctions<_>>::P256Verify, P256VerifyErrors>(
                P256_VERIFY_PREHASH_HOOK_ADDRESS_LOW,
            );
        }
    }

    pub fn add_l1_messenger(&mut self) {
        self.add_hook(
            L1_MESSENGER_ADDRESS_LOW,
            SystemHook::Stateless(SystemStatelessHook(l1_messenger_hook)),
        )
    }

    pub fn add_l2_base_token(&mut self) {
        self.add_hook(
            L2_BASE_TOKEN_ADDRESS_LOW,
            SystemHook::Stateless(SystemStatelessHook(l2_base_token_hook)),
        )
    }

    pub fn add_contract_deployer(&mut self) {
        self.add_hook(
            CONTRACT_DEPLOYER_ADDRESS_LOW,
            SystemHook::Stateless(SystemStatelessHook(contract_deployer_hook)),
        )
    }

    pub fn add_precompile<P, E>(&mut self, address_low: u16)
    where
        P: SystemFunction<S::Resources, E>,
        E: Subsystem,
    {
        self.add_hook(
            address_low,
            SystemHook::Stateless(SystemStatelessHook(
                pure_system_function_hook_impl::<SystemFunctionInvocationUser<S, E, P>, E, S>,
            )),
        )
    }

    pub fn add_precompile_ext<P: SystemFunctionExt<S::Resources, E>, E: Subsystem>(
        &mut self,
        address_low: u16,
    ) {
        self.add_hook(
            address_low,
            SystemHook::Stateless(SystemStatelessHook(
                pure_system_function_hook_impl::<SystemFunctionInvocationExt<S, E, P>, E, S>,
            )),
        )
    }

    pub fn add_precompile_from_pure_invocation<P>(&mut self, address_low: u16)
    where
        P: PurePrecompileInvocation,
    {
        self.add_hook(
            address_low,
            SystemHook::Stateless(SystemStatelessHook(precompile_hook_impl::<S, P>)),
        )
    }

    // ///
    // /// Adds nonce holder system hook.
    // ///
    // pub fn add_nonce_holder(&mut self) {
    //     self.add_hook(NONCE_HOLDER_HOOK_ADDRESS_LOW, nonce_holder_hook)
    // }
}

///
/// Utility function to create empty revert state.
///
pub fn make_error_return_state<'a, S: SystemTypes>(
    remaining_resources: S::Resources,
) -> CompletedExecution<'a, S> {
    CompletedExecution {
        return_values: ReturnValues::empty(),
        resources_returned: remaining_resources,
        reverted: true,
    }
}

///
/// Utility function to create return state with returndata region reference.
///
pub fn make_return_state_from_returndata_region<S: SystemTypes>(
    remaining_resources: S::Resources,
    returndata: &'_ [u8],
) -> CompletedExecution<'_, S> {
    let return_values = ReturnValues {
        returndata,
        return_scratch_space: None,
    };
    CompletedExecution {
        return_values,
        resources_returned: remaining_resources,
        reverted: false,
    }
}
