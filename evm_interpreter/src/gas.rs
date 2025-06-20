use zk_ee::system::{EthereumLikeTypes, Resource, Resources};

use crate::{
    utils::{spend_gas_and_native_from_resources, spend_gas_from_resources},
    ExitCode, ERGS_PER_GAS,
};

pub struct Gas<S: EthereumLikeTypes> {
    /// Generic resources
    pub resources: S::Resources,
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
