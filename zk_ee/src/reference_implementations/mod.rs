//! Reference implementations.
//! For now, only for resources.
//! We track two resources:
//! - EE resource: measured in ergs. Includes EVM gas, converted as 1 gas = GAS_TO_ERGS_FACTOR ergs.
//! - Native resource: model for prover complexity.

use crate::{
    out_of_native_resources,
    system::{
        errors::system::SystemError, Computational, Ergs, Resource, Resources, StateGasResources,
        DEFAULT_GAS_TO_ERGS_FACTOR,
    },
};

/// Native resource that counts down, as done for ergs.
#[derive(Clone, core::fmt::Debug, Default, PartialEq, Eq)]
pub struct DecreasingNative(u64);

impl Resource for DecreasingNative {
    const FORMAL_INFINITE: Self = DecreasingNative(u64::MAX);

    fn empty() -> Self {
        DecreasingNative(0)
    }

    fn is_empty(&self) -> bool {
        self.0 == 0
    }

    fn charge(&mut self, to_charge: &Self) -> Result<(), SystemError> {
        if self.0 < to_charge.0 {
            self.0 = 0;
            return Err(out_of_native_resources!().into());
        }
        self.0 -= to_charge.0;
        Ok(())
    }

    fn charge_unchecked(&mut self, to_charge: &Self) {
        self.0 -= to_charge.0
    }

    fn has_enough(&self, to_spend: &Self) -> bool {
        self.0 >= to_spend.0
    }

    fn reclaim(&mut self, to_reclaim: Self) {
        // This is only used to "give back" the native resource.
        assert!(self.0 == 0 || to_reclaim.0 == 0);
        self.0 += to_reclaim.0
    }

    fn reclaim_withheld(&mut self, to_reclaim: Self) {
        self.0 += to_reclaim.0
    }

    fn diff(&self, other: Self) -> Self {
        Self(self.0.abs_diff(other.0))
    }

    fn remaining(&self) -> Self {
        self.clone()
    }
}

impl Computational for DecreasingNative {
    fn from_computational(value: u64) -> Self {
        Self(value)
    }

    fn as_u64(&self) -> u64 {
        self.0
    }
}

/// Resources of a system that meters only legacy gas: ergs are gas 1:1 and
/// the native resource is not tracked.
pub type GasOnlyResources = BaseResources<(), 1>;

#[derive(Clone, core::fmt::Debug, PartialEq, Eq, Default)]
pub struct BaseResources<
    Native: Resource,
    const GAS_TO_ERGS_FACTOR: u64 = DEFAULT_GAS_TO_ERGS_FACTOR,
> {
    ergs: Ergs<GAS_TO_ERGS_FACTOR>,
    native: Native,
}

impl<Native: Resource, const GAS_TO_ERGS_FACTOR: u64> Resource
    for BaseResources<Native, GAS_TO_ERGS_FACTOR>
{
    const FORMAL_INFINITE: Self = Self {
        ergs: Ergs::FORMAL_INFINITE,
        native: Native::FORMAL_INFINITE,
    };

    fn empty() -> Self {
        Self {
            ergs: Ergs::empty(),
            native: Native::empty(),
        }
    }

    fn is_empty(&self) -> bool {
        self.ergs.is_empty() && self.native.is_empty()
    }

    fn has_enough(&self, to_spend: &Self) -> bool {
        self.ergs.has_enough(&to_spend.ergs) && self.native.has_enough(&to_spend.native)
    }

    #[inline(always)]
    fn charge(&mut self, to_charge: &Self) -> Result<(), SystemError> {
        if let Err(e) = self.ergs.charge(&to_charge.ergs) {
            // This method pre-charges for computation, both in ergs and native.
            // We first charge ergs, if they are insufficient, we do not charge
            // native, as the execution will halt with OOE and the computation
            // being charged for isn't performed.
            return Err(e);
        } else {
            self.native.charge(&to_charge.native)?
        };
        Ok(())
    }

    fn charge_unchecked(&mut self, to_charge: &Self) {
        self.ergs.charge_unchecked(&to_charge.ergs);
        self.native.charge_unchecked(&to_charge.native);
    }

    fn reclaim(&mut self, to_reclaim: Self) {
        self.ergs.reclaim(to_reclaim.ergs);
        self.native.reclaim(to_reclaim.native);
    }

    fn reclaim_withheld(&mut self, to_reclaim: Self) {
        self.ergs.reclaim(to_reclaim.ergs);
        self.native.reclaim_withheld(to_reclaim.native);
    }

    fn diff(&self, other: Self) -> Self {
        Self {
            ergs: self.ergs.diff(other.ergs),
            native: self.native.diff(other.native),
        }
    }

    fn remaining(&self) -> Self {
        Self {
            ergs: self.ergs.remaining(),
            native: self.native.remaining(),
        }
    }
}

impl<Native: Resource + Computational, const GAS_TO_ERGS_FACTOR: u64> Resources
    for BaseResources<Native, GAS_TO_ERGS_FACTOR>
{
    type Native = Native;
    type Ergs = Ergs<GAS_TO_ERGS_FACTOR>;

    #[inline(always)]
    fn from_ergs(ergs: Self::Ergs) -> Self {
        Self {
            ergs,
            native: Native::empty(),
        }
    }

    #[inline(always)]
    fn from_native(native: Native) -> Self {
        Self {
            ergs: Ergs::empty(),
            native,
        }
    }

    #[inline(always)]
    fn from_ergs_and_native(ergs: Self::Ergs, native: Native) -> Self {
        Self { ergs, native }
    }

    #[inline(always)]
    fn add_ergs(&mut self, to_add: Self::Ergs) {
        self.ergs.0 += to_add.0;
    }

    #[inline(always)]
    fn ergs(&self) -> Self::Ergs {
        self.ergs
    }

    #[inline(always)]
    fn native(&self) -> Native {
        self.native.clone()
    }

    #[inline(always)]
    fn exhaust_ergs(&mut self) {
        self.ergs = Ergs::empty()
    }

    fn give_native_to(&mut self, other: &mut Self) {
        let n = core::mem::replace(&mut self.native, Native::empty());
        other.native = n;
    }

    #[inline(always)]
    fn take(&mut self) -> Self {
        core::mem::replace(self, Self::empty())
    }

    fn with_infinite_ergs<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> R {
        let old_ergs = self.ergs;
        self.ergs = Ergs::FORMAL_INFINITE;
        let o = f(self);
        self.ergs = old_ergs;
        o
    }
}

/// Resources of a Glamsterdam (EIP-8037) system: execution gas as ergs 1:1, a
/// state gas reservoir and its spill (see [`StateGasResources`]), and a native
/// resource.
#[derive(Clone, core::fmt::Debug, PartialEq, Eq, Default)]
pub struct GlamsterdamResources<Native: Resource> {
    ergs: Ergs<1>,
    native: Native,
    /// The state gas reservoir
    state_gas: u32,
    /// Ergs spent on state charges once the reservoir was empty
    state_gas_spilled: u32,
}

impl<Native: Resource> GlamsterdamResources<Native> {
    /// Repays the spill from the reservoir, so that at most one of them is
    /// non-zero: after a reclaim the two pools of a frame and of its returned
    /// child are merged, and a credit the child received for a charge of the
    /// parent belongs to the parent's ergs
    #[inline(always)]
    fn repay_spill(&mut self) {
        let repayment = self.state_gas.min(self.state_gas_spilled);
        self.ergs.0 += repayment as u64;
        self.state_gas -= repayment;
        self.state_gas_spilled -= repayment;
    }
}

impl<Native: Resource> Resource for GlamsterdamResources<Native> {
    const FORMAL_INFINITE: Self = Self {
        ergs: Ergs::FORMAL_INFINITE,
        native: Native::FORMAL_INFINITE,
        state_gas: 0,
        state_gas_spilled: 0,
    };

    fn empty() -> Self {
        Self {
            ergs: Ergs::empty(),
            native: Native::empty(),
            state_gas: 0,
            state_gas_spilled: 0,
        }
    }

    fn is_empty(&self) -> bool {
        self.ergs.is_empty()
            && self.native.is_empty()
            && self.state_gas == 0
            && self.state_gas_spilled == 0
    }

    fn has_enough(&self, to_spend: &Self) -> bool {
        self.ergs.has_enough(&to_spend.ergs)
            && self.native.has_enough(&to_spend.native)
            && self.state_gas >= to_spend.state_gas
    }

    /// Charges the ergs and the native; a charge never carries state gas (that
    /// goes through [`StateGasResources::charge_state_gas`])
    #[inline(always)]
    fn charge(&mut self, to_charge: &Self) -> Result<(), SystemError> {
        debug_assert_eq!(to_charge.state_gas, 0);
        debug_assert_eq!(to_charge.state_gas_spilled, 0);
        // ergs first: out of ergs halts the computation, so its native is not charged
        self.ergs.charge(&to_charge.ergs)?;
        self.native.charge(&to_charge.native)
    }

    fn charge_unchecked(&mut self, to_charge: &Self) {
        self.ergs.charge_unchecked(&to_charge.ergs);
        self.native.charge_unchecked(&to_charge.native);
        self.state_gas -= to_charge.state_gas;
    }

    /// Merges returned resources back: the sums of every pool, then the spill
    /// repaid from the reservoir (see [`StateGasResources`])
    fn reclaim(&mut self, to_reclaim: Self) {
        self.ergs.reclaim(to_reclaim.ergs);
        self.native.reclaim(to_reclaim.native);
        self.state_gas += to_reclaim.state_gas;
        self.state_gas_spilled += to_reclaim.state_gas_spilled;
        self.repay_spill();
    }

    fn reclaim_withheld(&mut self, to_reclaim: Self) {
        self.ergs.reclaim(to_reclaim.ergs);
        self.native.reclaim_withheld(to_reclaim.native);
        self.state_gas += to_reclaim.state_gas;
        self.state_gas_spilled += to_reclaim.state_gas_spilled;
        self.repay_spill();
    }

    fn diff(&self, other: Self) -> Self {
        Self {
            ergs: self.ergs.diff(other.ergs),
            native: self.native.diff(other.native),
            state_gas: self.state_gas.abs_diff(other.state_gas),
            state_gas_spilled: self.state_gas_spilled.abs_diff(other.state_gas_spilled),
        }
    }

    fn remaining(&self) -> Self {
        self.clone()
    }
}

impl<Native: Resource + Computational> Resources for GlamsterdamResources<Native> {
    type Native = Native;
    type Ergs = Ergs<1>;

    #[inline(always)]
    fn from_ergs(ergs: Self::Ergs) -> Self {
        Self {
            ergs,
            ..Self::empty()
        }
    }

    #[inline(always)]
    fn from_native(native: Native) -> Self {
        Self {
            native,
            ..Self::empty()
        }
    }

    #[inline(always)]
    fn from_ergs_and_native(ergs: Self::Ergs, native: Native) -> Self {
        Self {
            ergs,
            native,
            ..Self::empty()
        }
    }

    #[inline(always)]
    fn add_ergs(&mut self, to_add: Self::Ergs) {
        self.ergs.0 += to_add.0;
    }

    #[inline(always)]
    fn ergs(&self) -> Self::Ergs {
        self.ergs
    }

    #[inline(always)]
    fn native(&self) -> Native {
        self.native.clone()
    }

    #[inline(always)]
    fn exhaust_ergs(&mut self) {
        self.ergs = Ergs::empty()
    }

    fn give_native_to(&mut self, other: &mut Self) {
        let n = core::mem::replace(&mut self.native, Native::empty());
        other.native = n;
    }

    #[inline(always)]
    fn take(&mut self) -> Self {
        core::mem::replace(self, Self::empty())
    }

    fn with_infinite_ergs<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> R {
        let old_ergs = self.ergs;
        self.ergs = Ergs::FORMAL_INFINITE;
        let o = f(self);
        self.ergs = old_ergs;
        o
    }
}

impl<Native: Resource + Computational> StateGasResources for GlamsterdamResources<Native> {
    #[inline(always)]
    fn state_gas(&self) -> u32 {
        self.state_gas
    }

    #[inline(always)]
    fn state_gas_spilled(&self) -> u32 {
        self.state_gas_spilled
    }

    fn add_state_gas(&mut self, amount: u32) {
        self.state_gas += amount;
        self.repay_spill();
    }

    fn drain_state_gas(&mut self) -> u32 {
        core::mem::take(&mut self.state_gas)
    }

    fn charge_state_gas(&mut self, amount: u32) -> Result<(), SystemError> {
        if self.state_gas >= amount {
            self.state_gas -= amount;
            return Ok(());
        }
        let remainder = amount - self.state_gas;
        match self.ergs.0.checked_sub(remainder as u64) {
            Some(ergs) => {
                self.ergs.0 = ergs;
                self.state_gas = 0;
                self.state_gas_spilled += remainder;
                Ok(())
            }
            None => {
                self.ergs = Ergs::empty();
                Err(crate::out_of_ergs_error!())
            }
        }
    }

    fn credit_state_gas(&mut self, amount: u32) {
        let to_ergs = amount.min(self.state_gas_spilled);
        self.ergs.0 += to_ergs as u64;
        self.state_gas_spilled -= to_ergs;
        self.state_gas += amount - to_ergs;
    }

    fn restore_state_gas(&mut self, baseline: u32) {
        self.ergs.0 += self.state_gas_spilled as u64;
        self.state_gas_spilled = 0;
        self.state_gas = baseline;
    }

    fn commit_state_gas(&mut self) -> u32 {
        core::mem::take(&mut self.state_gas_spilled)
    }
}

#[cfg(test)]
mod tests {
    use crate::system::{
        errors::system::SystemError, Ergs, Resource, Resources, StateGasResources,
    };

    use super::{BaseResources, DecreasingNative, GlamsterdamResources};

    type G = GlamsterdamResources<()>;

    fn glamsterdam(ergs: u64, state_gas: u32) -> G {
        let mut r = G::from_ergs(Ergs(ergs));
        r.add_state_gas(state_gas);
        r
    }

    #[test]
    fn state_gas_draws_from_the_reservoir_then_spills() {
        let mut r = glamsterdam(1000, 300);
        r.charge_state_gas(200).unwrap();
        assert_eq!(
            (r.ergs().0, r.state_gas(), r.state_gas_spilled()),
            (1000, 100, 0)
        );
        r.charge_state_gas(250).unwrap();
        assert_eq!(
            (r.ergs().0, r.state_gas(), r.state_gas_spilled()),
            (850, 0, 150)
        );
        // out of ergs when the two pools do not cover the charge: ergs exhausted
        assert!(r.charge_state_gas(851).is_err());
        assert_eq!(
            (r.ergs().0, r.state_gas(), r.state_gas_spilled()),
            (0, 0, 150)
        );
    }

    #[test]
    fn credits_repay_the_spill_before_the_reservoir() {
        let mut r = glamsterdam(1000, 100);
        r.charge_state_gas(400).unwrap();
        assert_eq!(
            (r.ergs().0, r.state_gas(), r.state_gas_spilled()),
            (700, 0, 300)
        );
        r.credit_state_gas(100);
        assert_eq!(
            (r.ergs().0, r.state_gas(), r.state_gas_spilled()),
            (800, 0, 200)
        );
        r.credit_state_gas(500);
        assert_eq!(
            (r.ergs().0, r.state_gas(), r.state_gas_spilled()),
            (1000, 300, 0)
        );
    }

    #[test]
    fn restore_refills_to_the_baseline() {
        let mut r = glamsterdam(1000, 100);
        r.charge_state_gas(400).unwrap();
        r.charge(&G::from_ergs(Ergs(50))).unwrap();
        r.restore_state_gas(100);
        assert_eq!(
            (r.ergs().0, r.state_gas(), r.state_gas_spilled()),
            (950, 100, 0)
        );
    }

    #[test]
    fn reclaim_merges_the_pools_and_repays_the_spill() {
        // the parent spilled, the child returns a reservoir it was credited
        let mut parent = glamsterdam(1000, 0);
        parent.charge_state_gas(300).unwrap();
        let child_reservoir = parent.drain_state_gas();
        assert_eq!(child_reservoir, 0);
        let mut child = glamsterdam(500, child_reservoir);
        child.credit_state_gas(120);
        parent.reclaim(child);
        assert_eq!(
            (
                parent.ergs().0,
                parent.state_gas(),
                parent.state_gas_spilled()
            ),
            (700 + 500 + 120, 0, 180)
        );
        // the credit exceeding the spill fills the reservoir
        let mut child = glamsterdam(0, 0);
        child.credit_state_gas(250);
        parent.reclaim(child);
        assert_eq!(
            (
                parent.ergs().0,
                parent.state_gas(),
                parent.state_gas_spilled()
            ),
            (1500, 70, 0)
        );
    }

    #[test]
    fn plain_charges_leave_the_state_gas_alone() {
        let mut r = glamsterdam(100, 50);
        assert!(r.charge(&G::from_ergs(Ergs(101))).is_err());
        assert_eq!(
            (r.ergs().0, r.state_gas(), r.state_gas_spilled()),
            (0, 50, 0)
        );
        let mut r = glamsterdam(100, 50);
        let taken = r.take();
        assert!(r.is_empty());
        assert_eq!(taken.state_gas(), 50);
        r.with_infinite_ergs(|r| r.charge_state_gas(70).unwrap());
        assert_eq!(
            (r.ergs().0, r.state_gas(), r.state_gas_spilled()),
            (0, 0, 70)
        );
    }

    #[test]
    fn test_oog_does_not_charge_native() {
        let mut resources = BaseResources::<DecreasingNative>::from_ergs_and_native(
            Ergs(10),
            DecreasingNative(100),
        );
        let r = resources.charge(&BaseResources {
            ergs: Ergs(11),
            native: DecreasingNative(101),
        });
        assert!(r.is_err_and(|e| matches!(
            e,
            SystemError::LeafRuntime(crate::system::errors::runtime::RuntimeError::OutOfErgs(_))
        )));
        assert_eq!(resources.native().0, 100);
    }
}
