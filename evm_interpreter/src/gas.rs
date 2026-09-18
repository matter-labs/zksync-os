//! Contains internal implementation of gas accounting
//!
//! Note: EVM gas accounting is implemented on top of the underlying ZKsync OS system resources,
//! including the "native" (proving) resource, which reflects the actual cost of proving.
//! As a result, there is an element of double accounting.

use zk_ee::system::evm::EvmError;
use zk_ee::system::{
    Computational, ErgsResource, EthereumLikeTypes, Resource, Resources, SystemTypes,
};

use crate::{
    native_resource_constants::{
        HEAP_EXPANSION_BASE_NATIVE_COST, HEAP_EXPANSION_PER_BYTE_NATIVE_COST, STEP_NATIVE_COST,
    },
    ExitCode,
};
use zk_ee::system::errors::{internal::InternalError, runtime::RuntimeError, system::SystemError};

/// Wraps underlying system resources and implements gas accounting on top of it
pub struct Gas<S: SystemTypes> {
    /// Underlying system resources
    pub resources: S::Resources,
    /// Keep track of gas spent on heap resizes
    pub gas_paid_for_heap_growth: u64,
    /// Internal error of a charge (not expected from any resource implementation), kept
    /// here as the charge has no access to the interpreter
    pub defect: Option<InternalError>,
}

impl<S: EthereumLikeTypes> Gas<S> {
    pub fn new() -> Self {
        Self {
            resources: S::Resources::empty(),
            gas_paid_for_heap_growth: 0,
            defect: None,
        }
    }

    #[inline(always)]
    /// Returns remaining "native" (proving) resource
    pub(crate) fn native(&self) -> u64 {
        self.resources.native().as_u64()
    }

    #[inline(always)]
    /// Returns remaining EVM gas
    pub(crate) fn gas_left(&self) -> u64 {
        self.resources.legacy_gas()
    }

    #[inline(always)]
    pub(crate) fn resources_mut(&mut self) -> &mut S::Resources {
        &mut self.resources
    }

    #[inline(always)]
    /// Moves underlying resources out of this struct. Leads to 0 gas (empty system resources).
    pub(crate) fn take_resources(&mut self) -> S::Resources {
        self.resources.take()
    }

    #[inline(always)]
    pub fn reclaim_resources(&mut self, resources: S::Resources) {
        self.resources.reclaim(resources);
    }

    #[inline(always)]
    pub(crate) fn consume_all_gas(&mut self) {
        self.resources.exhaust_ergs();
    }

    #[inline(always)]
    pub(crate) fn spend_gas(&mut self, to_spend: u64) -> Result<(), ExitCode> {
        let Some(ergs_cost) = <S::Resources as Resources>::Ergs::from_legacy_gas(to_spend) else {
            return Err(EvmError::OutOfGas.into());
        };
        let resource_cost = S::Resources::from_ergs(ergs_cost);
        self.charge(&resource_cost)
    }

    #[inline(always)]
    /// Spend gas and "native" (proving) resource. This double accounting approach is used to keep track of actual proving cost
    pub(crate) fn spend_gas_and_native(&mut self, gas: u64, native: u64) -> Result<(), ExitCode> {
        let Some(ergs_cost) = <S::Resources as Resources>::Ergs::from_legacy_gas(gas) else {
            return Err(EvmError::OutOfGas.into());
        };
        let resource_cost = S::Resources::from_ergs_and_native(
            ergs_cost,
            Computational::from_computational(native),
        );
        self.charge(&resource_cost)
    }

    /// Charge, mapping the error to the (small) exit code
    #[inline(always)]
    fn charge(&mut self, to_charge: &S::Resources) -> Result<(), ExitCode> {
        match self.resources.charge(to_charge) {
            Ok(()) => Ok(()),
            Err(e) => Err(self.charge_error(e)),
        }
    }

    #[cold]
    #[inline(never)]
    fn charge_error(&mut self, e: SystemError) -> ExitCode {
        match e {
            SystemError::LeafRuntime(RuntimeError::OutOfErgs(_)) => {
                ExitCode::EvmError(EvmError::OutOfGas)
            }
            SystemError::LeafRuntime(RuntimeError::FatalRuntimeError(f)) => {
                ExitCode::FatalRuntime(f)
            }
            SystemError::LeafDefect(e) => {
                self.defect = Some(e);
                ExitCode::FatalError
            }
        }
    }

    #[inline(always)]
    /// Charge only the "native" (proving) resource
    pub(crate) fn spend_native(&mut self, native: u64) -> Result<(), ExitCode> {
        let resource_cost = S::Resources::from_native(Computational::from_computational(native));
        self.charge(&resource_cost)
    }

    #[inline(always)]
    /// The first charge of an instruction: same as `spend_gas_and_native`, plus the per-step
    /// native cost of the dispatch. Charging both at once instead of a separate step charge
    /// before the instruction saves a resource check per instruction; the accounting is the
    /// same, including out of gas, where the step is still paid as it used to be charged first.
    pub(crate) fn spend_step_gas_and_native(
        &mut self,
        gas: u64,
        native: u64,
    ) -> Result<(), ExitCode> {
        let Some(ergs_cost) = <S::Resources as Resources>::Ergs::from_legacy_gas(gas) else {
            self.spend_native(STEP_NATIVE_COST)?;
            return Err(EvmError::OutOfGas.into());
        };
        let resource_cost = S::Resources::from_ergs_and_native(
            ergs_cost,
            Computational::from_computational(native + STEP_NATIVE_COST),
        );
        match self.resources.charge(&resource_cost) {
            Ok(()) => Ok(()),
            Err(e) => {
                if let SystemError::LeafRuntime(RuntimeError::OutOfErgs(_)) = &e {
                    // ergs are checked first and nothing is charged then
                    self.spend_native(STEP_NATIVE_COST)?;
                }
                Err(self.charge_error(e))
            }
        }
    }

    #[inline(always)]
    /// current_msize is expected to be divisible by 32
    pub(crate) fn pay_for_memory_growth(
        &mut self,
        current_msize: usize,
        new_msize: usize,
    ) -> Result<(), ExitCode> {
        let net_byte_increase = new_msize - current_msize;
        let new_heap_size_words = new_msize as u64 / 32;

        debug_assert_eq!(new_heap_size_words * 32, new_msize as u64);

        let end_cost = crate::gas_constants::MEMORY
            .saturating_mul(new_heap_size_words)
            .saturating_add(new_heap_size_words.saturating_mul(new_heap_size_words) / 512);
        let net_cost_gas = end_cost - self.gas_paid_for_heap_growth;
        let net_cost_native = HEAP_EXPANSION_BASE_NATIVE_COST.saturating_add(
            HEAP_EXPANSION_PER_BYTE_NATIVE_COST.saturating_mul(net_byte_increase as u64),
        );
        self.spend_gas_and_native(net_cost_gas, net_cost_native)?;

        self.gas_paid_for_heap_growth = end_cost;

        Ok(())
    }
}

pub mod gas_utils {
    use zk_ee::system::evm::EvmError;
    use zk_ee::system::ErgsResource;

    use crate::ExitCode;

    #[inline]
    /// Returns gas and natve cost of copying 'len' bytes
    pub(crate) fn copy_cost(len: u64) -> Result<(u64, u64), ExitCode> {
        let get_cost = |len: u64| -> Option<(u64, u64)> {
            let num_words = len.checked_next_multiple_of(32)? / 32;
            let gas = crate::gas_constants::COPY.checked_mul(num_words)?;
            let native = crate::native_resource_constants::COPY_BYTE_NATIVE_COST
                .checked_mul(len)?
                .checked_add(crate::native_resource_constants::COPY_BASE_NATIVE_COST)?;
            Some((gas, native))
        };
        get_cost(len).ok_or(EvmError::OutOfGas.into())
    }

    #[inline]
    /// Returns gas and natve cost of copying 'len' bytes. Gas is additionally increased by VERYLOW - often used by EVM opcodes
    pub(crate) fn copy_cost_plus_very_low_gas(len: u64) -> Result<(u64, u64), ExitCode> {
        let (gas_cost, native_cost) = copy_cost(len)?;
        if let Some(gas_cost) = gas_cost.checked_add(crate::gas_constants::VERYLOW) {
            Ok((gas_cost, native_cost))
        } else {
            Err(EvmError::OutOfGas.into())
        }
    }

    /// Returns the result of subtracting 1/64th of EVM gas.
    /// Note: it works with ergs, making conversions inside.
    #[inline(always)]
    pub(crate) fn apply_63_64_rule<E: ErgsResource>(ergs: E) -> E {
        // We need to apply the rule over gas, not ergs
        let gas = ergs.as_legacy_gas();
        // `(gas / 64) * factor <= ergs`, so the subtraction can't underflow
        E::from_computational(ergs.as_u64() - E::from_legacy_gas_saturating(gas / 64).as_u64())
    }
}
