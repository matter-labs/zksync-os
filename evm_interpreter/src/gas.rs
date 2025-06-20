use zk_ee::system::{Computational, EthereumLikeTypes, Resource, Resources};

use crate::{
    utils::{spend_gas_and_native_from_resources, spend_gas_from_resources},
    ExitCode, ERGS_PER_GAS,
};

pub struct Gas<S: EthereumLikeTypes> {
    /// Generic resources
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

    pub(crate) fn native(&mut self) -> u64 {
        self.resources.native().as_u64()
    }

    pub(crate) fn resources_mut(&mut self) -> &mut S::Resources {
        &mut self.resources
    }

    pub(crate) fn take_resources(&mut self) -> S::Resources {
        self.resources.take()
    }

    pub fn reclaim_resources(&mut self, resources: S::Resources) {
        self.resources.reclaim(resources);
    }

    pub(crate) fn consume_all_gas(&mut self) {
        self.resources.exhaust_ergs();
    }

    #[inline(always)]
    pub(crate) fn spend_gas(&mut self, to_spend: u64) -> Result<(), ExitCode> {
        spend_gas_from_resources(&mut self.resources, to_spend)
    }

    #[inline(always)]
    pub(crate) fn spend_gas_and_native(&mut self, gas: u64, native: u64) -> Result<(), ExitCode> {
        spend_gas_and_native_from_resources(&mut self.resources, gas, native)
    }

    #[inline(always)]
    pub(crate) fn gas_left(&self) -> u64 {
        self.resources.ergs().0 / ERGS_PER_GAS
    }
}
