//! Contains internal implementation of gas accounting
//!
//! Note: EVM gas accounting is implemented on top of the underlying ZKsync OS system resources,
//! including the "native" (proving) resource, which reflects the actual cost of proving.
//! As a result, there is an element of double accounting.

use zk_ee::system::{Computational, Ergs, EthereumLikeTypes, Resource, Resources};

use crate::{
    native_resource_constants::{
        HEAP_EXPANSION_BASE_NATIVE_COST, HEAP_EXPANSION_PER_BYTE_NATIVE_COST,
    },
    ExitCode, ERGS_PER_GAS,
};

/// Wraps underlying system resources and implements gas accounting on top of it
pub struct Gas<S: EthereumLikeTypes> {
    /// Underlying system resources
    resources: S::Resources,
    /// Keep track of gas spent on heap resizes
    pub gas_paid_for_heap_growth: u64,
}

impl<S: EthereumLikeTypes> Gas<S> {
    pub fn new() -> Self {
        Self {
            resources: S::Resources::empty(),
            gas_paid_for_heap_growth: 0,
        }
    }

    #[inline(always)]
    /// Returns remaining "native" (proving) resource
    pub(crate) fn native(&mut self) -> u64 {
        self.resources.native().as_u64()
    }

    #[inline(always)]
    /// Returns remaining EVM gas
    pub(crate) fn gas_left(&self) -> u64 {
        self.resources.ergs().0 / ERGS_PER_GAS
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
        let Some(ergs_cost) = to_spend.checked_mul(ERGS_PER_GAS) else {
            return Err(ExitCode::OutOfGas);
        };
        let resource_cost = S::Resources::from_ergs(Ergs(ergs_cost));
        self.resources.charge(&resource_cost)?;
        Ok(())
    }

    #[inline(always)]
    /// Spend gas and "native" (proving) resource. This double accounting approach is used to keep track of actual proving cost
    pub(crate) fn spend_gas_and_native(&mut self, gas: u64, native: u64) -> Result<(), ExitCode> {
        use zk_ee::system::Computational;
        let Some(ergs_cost) = gas.checked_mul(ERGS_PER_GAS) else {
            return Err(ExitCode::OutOfGas);
        };
        let resource_cost = S::Resources::from_ergs_and_native(
            Ergs(ergs_cost),
            Computational::from_computational(native),
        );
        self.resources.charge(&resource_cost)?;
        Ok(())
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
    use zk_ee::system::Ergs;

    use crate::{ExitCode, ERGS_PER_GAS};

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
        get_cost(len).ok_or(ExitCode::OutOfGas)
    }

    #[inline]
    /// Returns gas and natve cost of copying 'len' bytes. Gas is additionally increased by VERYLOW - often used by EVM opcodes
    pub(crate) fn copy_cost_plus_very_low_gas(len: u64) -> Result<(u64, u64), ExitCode> {
        let (gas_cost, native_cost) = copy_cost(len)?;
        if let Some(gas_cost) = gas_cost.checked_add(crate::gas_constants::VERYLOW) {
            Ok((gas_cost, native_cost))
        } else {
            Err(ExitCode::OutOfGas)
        }
    }

    /// Returns the result of subtracting 1/64th of EVM gas.
    /// Note: it works with ergs, making conversions inside.
    #[inline(always)]
    pub(crate) fn apply_63_64_rule(ergs: Ergs) -> Ergs {
        // We need to apply the rule over gas, not ergs
        let gas = ergs.0 / ERGS_PER_GAS;
        Ergs(ergs.0 - (gas / 64) * ERGS_PER_GAS)
    }
}
