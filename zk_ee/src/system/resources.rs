//! Resources for system and EE work.
//! We track two resources:
//! - EE resource: measured in ergs. Includes EVM gas, converted as 1 gas = ERGS_PER_GAS ergs.
//! - Native resource: model for prover complexity.

use crate::out_of_ergs_error;

use super::errors::system::SystemError;

///
/// Single resource, both resources will implement this, as well as
/// the combined Resources trait.
///
pub trait Resource: 'static + Sized + Clone + core::fmt::Debug + PartialEq + Eq {
    /// Max value, to be used carefully. See [Resources::with_infinite_ergs].
    const FORMAL_INFINITE: Self;

    /// Empty resource.
    fn empty() -> Self;

    /// Determines if the resource is empty.
    fn is_empty(&self) -> bool;

    /// Try to charge an amount from a given resource.
    /// If it fails, it will leave the resource as empty.
    fn charge(&mut self, to_charge: &Self) -> Result<(), SystemError>;

    // Charges the amount without verifying.
    /// WARNING: this might underflow the underlying resources.
    fn charge_unchecked(&mut self, to_charge: &Self);

    /// Checks if the resource can spend a given amount.
    fn has_enough(&self, to_spend: &Self) -> bool;

    /// Adds [to_reclaim] to a given resource.
    fn reclaim(&mut self, to_reclaim: Self);

    /// Reclaims a withheld resource. Should be only used by the bootloader at the end
    /// of a transaction.
    fn reclaim_withheld(&mut self, to_reclaim: Self);

    /// Computes the absolute difference between [self] and [other].
    fn diff(&self, other: Self) -> Self;

    // Returns the remaining part of the resource.
    fn remaining(&self) -> Self;
}

///
/// Computational resources can be represented as a single u64.
///
pub trait Computational: 'static + Sized + Clone + core::fmt::Debug + PartialEq + Eq {
    fn from_computational(value: u64) -> Self;
    fn as_u64(&self) -> u64;
}

///
/// Ergs, the resource for EEs.
///
#[derive(Clone, Copy, core::fmt::Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Ergs(pub u64);

impl core::ops::Add for Ergs {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl Ergs {
    pub fn times(self, coeff: u64) -> Self {
        Self(self.0 * coeff)
    }
}

impl Resource for Ergs {
    const FORMAL_INFINITE: Self = Ergs(u64::MAX);

    fn empty() -> Self {
        Self(0)
    }

    fn is_empty(&self) -> bool {
        self.0 == 0
    }

    fn has_enough(&self, to_spend: &Self) -> bool {
        self >= to_spend
    }

    fn charge(&mut self, to_charge: &Self) -> Result<(), SystemError> {
        if self.0 < to_charge.0 {
            self.0 = 0;
            return Err(out_of_ergs_error!());
        }
        self.0 -= to_charge.0;
        Ok(())
    }

    fn charge_unchecked(&mut self, to_charge: &Self) {
        self.0 -= to_charge.0
    }

    fn reclaim(&mut self, to_reclaim: Self) {
        self.0 += to_reclaim.0
    }

    fn reclaim_withheld(&mut self, to_reclaim: Self) {
        self.0 += to_reclaim.0
    }

    fn diff(&self, other: Self) -> Self {
        Self(self.0.abs_diff(other.0))
    }

    fn remaining(&self) -> Self {
        *self
    }
}

///
/// Trait to represent all resources together
/// (for now EE and native computational resources).
/// It can be used as a single resource, but it provides constructors
/// from each kind of resource. It also provides some special operations that
/// should only be applied to the EE resource.
///
pub trait Resources:
    'static + Sized + Clone + core::fmt::Debug + PartialEq + Eq + Resource
{
    /// Type of native computational resource.
    type Native: Resource + Computational;

    /// Constructor from EE resource, all other resources are set to empty.
    fn from_ergs(ergs: Ergs) -> Self;

    /// Constructor from native resource, all other resources are set to empty.
    fn from_native(native: Self::Native) -> Self;

    /// Constructor from all sub-resources.
    fn from_ergs_and_native(ergs: Ergs, native: Self::Native) -> Self;

    /// Increments the EE resource.
    fn add_ergs(&mut self, to_add: Ergs);

    /// Gets the available ergs (EE resource).
    fn ergs(&self) -> Ergs;

    /// Gets the available native.
    fn native(&self) -> Self::Native;

    /// Consumes all remaining EE resource.
    fn exhaust_ergs(&mut self);

    /// Move all the native resources from [self] to [other].
    fn give_native_to(&mut self, other: &mut Self);

    /// Make a copy of [self], replacing it with the empty resources.
    fn take(&mut self) -> Self;

    /// Run a computation [f] using the native resources from [self]
    /// but with "infinite" ergs.
    /// Used whenever the system has to do some work the EE already paid for
    /// in terms of EE resources, but the system should track native resource
    /// consumption.
    ///
    /// Example:
    /// resources.with_infinite_ergs(|inf_resources|
    ///   system.do_something(inf_resources,...)
    /// )
    fn with_infinite_ergs<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> R;
}
