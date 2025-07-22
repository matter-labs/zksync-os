//! Reference implementations.
//! For now, only for resources.
//! We track two resources:
//! - EE resource: measured in ergs. Includes EVM gas, converted as 1 gas = ERGS_PER_GAS ergs.
//! - Native resource: model for prover complexity.

use crate::{
    out_of_native_resources,
    system::{errors::system::SystemError, Computational, Ergs, Resource, Resources},
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

#[derive(Clone, core::fmt::Debug, PartialEq, Eq, Default)]
pub struct BaseResources<Native: Resource> {
    ergs: Ergs,
    native: Native,
}

impl<Native: Resource> Resource for BaseResources<Native> {
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

    fn charge(&mut self, to_charge: &Self) -> Result<(), SystemError> {
        if let Err(e) = self.native.charge(&to_charge.native) {
            // If both out of ergs and native, just keep the native
            // error.
            let _ = self.ergs.charge(&to_charge.ergs);
            return Err(e);
        } else {
            self.ergs.charge(&to_charge.ergs)?
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

impl<Native: Resource + Computational> Resources for BaseResources<Native> {
    type Native = Native;

    fn from_ergs(ergs: Ergs) -> Self {
        Self {
            ergs,
            native: Native::empty(),
        }
    }

    fn from_native(native: Native) -> Self {
        Self {
            ergs: Ergs(0),
            native,
        }
    }

    fn from_ergs_and_native(ergs: Ergs, native: Native) -> Self {
        Self { ergs, native }
    }

    fn add_ergs(&mut self, to_add: Ergs) {
        self.ergs.0 += to_add.0;
    }

    fn ergs(&self) -> Ergs {
        self.ergs
    }

    fn native(&self) -> Native {
        self.native.clone()
    }

    fn exhaust_ergs(&mut self) {
        self.ergs = Ergs(0)
    }

    fn give_native_to(&mut self, other: &mut Self) {
        let n = core::mem::replace(&mut self.native, Native::empty());
        other.native = n;
    }

    fn take(&mut self) -> Self {
        core::mem::replace(self, Self::empty())
    }

    fn with_infinite_ergs<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> R {
        let old_ergs = self.ergs;
        self.ergs = Ergs(u64::MAX);
        let o = f(self);
        self.ergs = old_ergs;
        o
    }
}
