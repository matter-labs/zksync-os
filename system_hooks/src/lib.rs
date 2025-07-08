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
use crate::contract_deployer::contract_deployer_hook;
use crate::l1_messenger::l1_messenger_hook;
use crate::l2_base_token::l2_base_token_hook;
use alloc::collections::BTreeMap;
use core::{alloc::Allocator, mem::MaybeUninit};
use errors::FatalError;
use precompiles::{pure_system_function_hook_impl, IdentityPrecompile};
use zk_ee::{
    memory::slice_vec::SliceVec,
    system::{errors::InternalError, EthereumLikeTypes, System, SystemTypes, *},
};

pub mod addresses_constants;
#[cfg(feature = "mock-unsupported-precompiles")]
mod mock_precompiles;

// Temporarily disabled, only used for AA.
// pub mod nonce_holder;
mod contract_deployer;
mod l1_messenger;
mod l2_base_token;
mod precompiles;

/// System hooks process the given call request.
///
/// The inputs are:
/// - call request
/// - caller ee(logic may depend on it some cases)
/// - system
/// - output buffer
pub struct SystemHook<S: SystemTypes>(
    for<'a> fn(
        ExternalCallRequest<S>,
        u8,
        &mut System<S>,
        &'a mut [MaybeUninit<u8>],
    ) -> Result<(CompletedExecution<'a, S>, &'a mut [MaybeUninit<u8>]), FatalError>,
);

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
        &mut self,
        address_low: u16,
        request: ExternalCallRequest<S>,
        caller_ee: u8,
        system: &mut System<S>,
        return_memory: &'a mut [MaybeUninit<u8>],
    ) -> Result<(Option<CompletedExecution<'a, S>>, &'a mut [MaybeUninit<u8>]), FatalError> {
        let Some(hook) = self.inner.get(&address_low) else {
            return Ok((None, return_memory));
        };
        let (res, remaining_memory) = hook.0(request, caller_ee, system, return_memory)?;

        Ok((Some(res), remaining_memory))
    }

    ///
    /// Checks if there is a hook stored for a given low address (<16 bits).
    ///
    pub fn has_hook_for(&mut self, address_low: u16) -> bool {
        self.inner.contains_key(&address_low)
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
        self.add_precompile::<<S::SystemFunctions as SystemFunctions<_>>::Secp256k1ECRecover>(
            ECRECOVER_HOOK_ADDRESS_LOW,
        );
        self.add_precompile::<<S::SystemFunctions as SystemFunctions<_>>::Sha256>(
            SHA256_HOOK_ADDRESS_LOW,
        );
        self.add_precompile::<<S::SystemFunctions as SystemFunctions<_>>::RipeMd160>(
            RIPEMD160_HOOK_ADDRESS_LOW,
        );
        self.add_precompile::<IdentityPrecompile>(ID_HOOK_ADDRESS_LOW);
        self.add_precompile::<<S::SystemFunctions as SystemFunctions<_>>::ModExp>(
            MODEXP_HOOK_ADDRESS_LOW,
        );
        self.add_precompile::<<S::SystemFunctions as SystemFunctions<_>>::Bn254Add>(
            ECADD_HOOK_ADDRESS_LOW,
        );
        self.add_precompile::<<S::SystemFunctions as SystemFunctions<_>>::Bn254Mul>(
            ECMUL_HOOK_ADDRESS_LOW,
        );
        self.add_precompile::<<S::SystemFunctions as SystemFunctions<_>>::Bn254PairingCheck>(
            ECPAIRING_HOOK_ADDRESS_LOW,
        );
        #[cfg(feature = "mock-unsupported-precompiles")]
        {
            self.add_precompile::<crate::mock_precompiles::mock_precompiles::Blake>(
                BLAKE_HOOK_ADDRESS_LOW,
            );
            self.add_precompile::<crate::mock_precompiles::mock_precompiles::PointEval>(
                POINT_EVAL_HOOK_ADDRESS_LOW,
            );
        }

        #[cfg(feature = "p256_precompile")]
        {
            self.add_precompile::<<S::SystemFunctions as SystemFunctions<_>>::P256Verify>(
                P256_VERIFY_PREHASH_HOOK_ADDRESS_LOW,
            );
        }
    }

    pub fn add_l1_messenger(&mut self) {
        self.add_hook(L1_MESSENGER_ADDRESS_LOW, SystemHook(l1_messenger_hook))
    }

    pub fn add_l2_base_token(&mut self) {
        self.add_hook(L2_BASE_TOKEN_ADDRESS_LOW, SystemHook(l2_base_token_hook))
    }

    pub fn add_contract_deployer(&mut self) {
        self.add_hook(
            CONTRACT_DEPLOYER_ADDRESS_LOW,
            SystemHook(contract_deployer_hook),
        )
    }

    fn add_precompile<P: SystemFunction<S::Resources>>(&mut self, address_low: u16) {
        self.add_hook(
            address_low,
            SystemHook(pure_system_function_hook_impl::<P, S>),
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
fn make_error_return_state<'a, S: SystemTypes>(
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
fn make_return_state_from_returndata_region<S: SystemTypes>(
    remaining_resources: S::Resources,
    returndata: &[u8],
) -> CompletedExecution<S> {
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
