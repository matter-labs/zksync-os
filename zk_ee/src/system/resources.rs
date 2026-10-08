//! Resources for system and EE work.
//! We track two resources:
//! - EE resource: measured in ergs. Includes legacy EVM gas, converted as
//!   1 gas = `GAS_TO_ERGS_FACTOR` ergs, where the factor is a parameter of the ergs type.
//! - Native resource: model for prover complexity.

use crate::out_of_ergs_error;

use super::errors::system::SystemError;

/// Legacy (EVM) gas to ergs conversion factor used by the default ergs type.
pub const DEFAULT_GAS_TO_ERGS_FACTOR: u64 = 256;

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
/// A resource that is not tracked at all: every charge succeeds and nothing is
/// ever consumed. Used as the native resource by systems that only meter ergs.
///
impl Resource for () {
    const FORMAL_INFINITE: Self = ();

    #[inline(always)]
    fn empty() -> Self {}

    #[inline(always)]
    fn is_empty(&self) -> bool {
        true
    }

    #[inline(always)]
    fn charge(&mut self, _to_charge: &Self) -> Result<(), SystemError> {
        Ok(())
    }

    #[inline(always)]
    fn charge_unchecked(&mut self, _to_charge: &Self) {}

    #[inline(always)]
    fn has_enough(&self, _to_spend: &Self) -> bool {
        true
    }

    #[inline(always)]
    fn reclaim(&mut self, _to_reclaim: Self) {}

    #[inline(always)]
    fn reclaim_withheld(&mut self, _to_reclaim: Self) {}

    #[inline(always)]
    fn diff(&self, _other: Self) -> Self {}

    #[inline(always)]
    fn remaining(&self) -> Self {}
}

impl Computational for () {
    #[inline(always)]
    fn from_computational(_value: u64) -> Self {}

    #[inline(always)]
    fn as_u64(&self) -> u64 {
        0
    }
}

/// `a - b`, or `None` on borrow.
///
/// On the 32-bit proving target this is written as an explicit borrow chain over the
/// two words: it is the hottest check of the EVM interpreter (once per instruction),
/// and the generic `u64` compare-then-subtract lowers to a 64-bit compare chain plus
/// the subtraction (18 instructions), while a signed-domain formulation lets LLVM fold
/// product costs into a `mulhsu`, which the proving machine does not have. The chain
/// keeps everything unsigned, so constant costs still fold into immediates.
#[inline(always)]
fn sub_with_borrow(a: u64, b: u64) -> Option<u64> {
    #[cfg(target_pointer_width = "32")]
    {
        let (lo, borrow_lo) = (a as u32).overflowing_sub(b as u32);
        let (hi, borrow_hi) = ((a >> 32) as u32).overflowing_sub((b >> 32) as u32);
        let (hi, borrow_carry) = hi.overflowing_sub(borrow_lo as u32);
        if borrow_hi | borrow_carry {
            None
        } else {
            Some(((hi as u64) << 32) | lo as u64)
        }
    }
    #[cfg(not(target_pointer_width = "32"))]
    {
        a.checked_sub(b)
    }
}

///
/// Ergs, the resource for EEs. `GAS_TO_ERGS_FACTOR` is the number of ergs in one
/// unit of legacy (EVM) gas. It must be in `1..=DEFAULT_GAS_TO_ERGS_FACTOR`, so
/// that gas limits derived from the default factor ([crate::system::MAX_BLOCK_GAS_LIMIT])
/// stay representable for every instantiation; the range is checked at compile time
/// when the type is used.
///
#[derive(Clone, Copy, core::fmt::Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Ergs<const GAS_TO_ERGS_FACTOR: u64 = DEFAULT_GAS_TO_ERGS_FACTOR>(pub u64);

impl<const GAS_TO_ERGS_FACTOR: u64> Ergs<GAS_TO_ERGS_FACTOR> {
    /// Largest ergs value: the sign bit is kept clear, so a charge is a wrapping
    /// subtraction whose sign bit is the borrow. Every constructor of an ergs
    /// amount that can be charged from keeps values within this bound
    /// ([Resource::FORMAL_INFINITE], [ErgsResource::MAX_LEGACY_GAS]).
    pub const MAX_ERGS: u64 = i64::MAX as u64;

    /// Evaluated at monomorphization (no runtime cost): rejects factors outside
    /// `1..=DEFAULT_GAS_TO_ERGS_FACTOR`.
    const FACTOR_IN_RANGE: () = assert!(
        GAS_TO_ERGS_FACTOR > 0 && GAS_TO_ERGS_FACTOR <= DEFAULT_GAS_TO_ERGS_FACTOR,
        "GAS_TO_ERGS_FACTOR must be in 1..=DEFAULT_GAS_TO_ERGS_FACTOR"
    );
}

impl<const GAS_TO_ERGS_FACTOR: u64> core::ops::Add for Ergs<GAS_TO_ERGS_FACTOR> {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

///
/// The EE resource of a system: ergs with a fixed legacy (EVM) gas conversion.
/// Callers express EVM costs in gas and convert through this trait, so the
/// conversion factor stays private to the resource type.
///
pub trait ErgsResource:
    Resource + Computational + Copy + Default + Ord + core::ops::Add<Output = Self>
{
    /// Number of ergs in one unit of legacy gas.
    const GAS_TO_ERGS_FACTOR: u64;

    /// Largest amount of legacy gas of one transaction: every `u32` amount is
    /// representable as ergs (the block gas limit is not bounded by this).
    const MAX_LEGACY_GAS: u32 = u32::MAX;

    /// Converts legacy gas to ergs, `None` if it doesn't fit.
    fn from_legacy_gas(gas: u32) -> Option<Self>;

    /// Converts legacy gas to ergs, saturating if it doesn't fit.
    fn from_legacy_gas_saturating(gas: u32) -> Self;

    /// Converts ergs to legacy gas, rounding down.
    fn as_legacy_gas(&self) -> u64;

    /// Converts ergs to legacy gas, rounding up.
    fn as_legacy_gas_ceil(&self) -> u64;

    /// Multiplies by a (non-negative) coefficient.
    fn times(self, coeff: u64) -> Self;
}

// The `GAS_TO_ERGS_FACTOR == 1` branches below are resolved at monomorphization:
// with a unit factor the conversions are plain moves, without a checked
// multiplication or a division on the path.
impl<const GAS_TO_ERGS_FACTOR: u64> ErgsResource for Ergs<GAS_TO_ERGS_FACTOR> {
    const GAS_TO_ERGS_FACTOR: u64 = {
        let () = Self::FACTOR_IN_RANGE;
        GAS_TO_ERGS_FACTOR
    };

    #[inline(always)]
    fn from_legacy_gas(gas: u32) -> Option<Self> {
        let () = Self::FACTOR_IN_RANGE;
        // a `u32` times a factor of at most `DEFAULT_GAS_TO_ERGS_FACTOR` fits
        Some(Self(gas as u64 * GAS_TO_ERGS_FACTOR))
    }

    #[inline(always)]
    fn from_legacy_gas_saturating(gas: u32) -> Self {
        let () = Self::FACTOR_IN_RANGE;
        Self(gas as u64 * GAS_TO_ERGS_FACTOR)
    }

    #[inline(always)]
    fn as_legacy_gas(&self) -> u64 {
        let () = Self::FACTOR_IN_RANGE;
        if GAS_TO_ERGS_FACTOR == 1 {
            self.0
        } else {
            self.0 / GAS_TO_ERGS_FACTOR
        }
    }

    #[inline(always)]
    fn as_legacy_gas_ceil(&self) -> u64 {
        let () = Self::FACTOR_IN_RANGE;
        if GAS_TO_ERGS_FACTOR == 1 {
            self.0
        } else {
            self.0.div_ceil(GAS_TO_ERGS_FACTOR)
        }
    }

    #[inline(always)]
    fn times(self, coeff: u64) -> Self {
        Self(self.0 * coeff)
    }
}

impl<const GAS_TO_ERGS_FACTOR: u64> Computational for Ergs<GAS_TO_ERGS_FACTOR> {
    #[inline(always)]
    fn from_computational(value: u64) -> Self {
        Self(value)
    }

    #[inline(always)]
    fn as_u64(&self) -> u64 {
        self.0
    }
}

impl<const GAS_TO_ERGS_FACTOR: u64> Resource for Ergs<GAS_TO_ERGS_FACTOR> {
    const FORMAL_INFINITE: Self = {
        let () = Self::FACTOR_IN_RANGE;
        Self(Self::MAX_ERGS)
    };

    fn empty() -> Self {
        let () = Self::FACTOR_IN_RANGE;
        Self(0)
    }

    fn is_empty(&self) -> bool {
        self.0 == 0
    }

    fn has_enough(&self, to_spend: &Self) -> bool {
        self >= to_spend
    }

    #[inline(always)]
    fn charge(&mut self, to_charge: &Self) -> Result<(), SystemError> {
        // The failure path is kept inline on purpose: a call, even a cold one, on the
        // failure path of every inlined handler of the interpreter changes the
        // register allocation of the handler's hot path.
        match sub_with_borrow(self.0, to_charge.0) {
            Some(remaining) => {
                self.0 = remaining;
                Ok(())
            }
            None => {
                self.0 = 0;
                Err(out_of_ergs_error!())
            }
        }
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
/// Legacy (EVM) gas is never converted by callers: the `*_legacy_gas*` helpers
/// convert it through [Resources::Ergs].
///
/// Ergs on a 32-bit counter, for a system whose execution gas is bounded per
/// transaction (EIP-7825 caps it at `2^24`, and a state gas spill repaid into
/// the execution gas is bounded by the same cap): one word on the RISC-V
/// target instead of two, over the full `u32` range. The gas to ergs factor
/// is one. A charge is a subtraction with its borrow (one branch), and a
/// legacy gas amount fits when its high word is zero (one test).
#[derive(Clone, Copy, core::fmt::Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Ergs32(pub u32);

impl Ergs32 {
    /// Largest ergs value: the whole range
    pub const MAX_ERGS: u32 = u32::MAX;

    #[inline(always)]
    fn saturated(value: u64) -> Self {
        Self(value.min(Self::MAX_ERGS as u64) as u32)
    }
}

impl core::ops::Add for Ergs32 {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0.saturating_add(rhs.0))
    }
}

impl ErgsResource for Ergs32 {
    const GAS_TO_ERGS_FACTOR: u64 = 1;

    #[inline(always)]
    fn from_legacy_gas(gas: u32) -> Option<Self> {
        Some(Self(gas))
    }

    #[inline(always)]
    fn from_legacy_gas_saturating(gas: u32) -> Self {
        Self(gas)
    }

    #[inline(always)]
    fn as_legacy_gas(&self) -> u64 {
        self.0 as u64
    }

    #[inline(always)]
    fn as_legacy_gas_ceil(&self) -> u64 {
        self.0 as u64
    }

    #[inline(always)]
    fn times(self, coeff: u64) -> Self {
        Self::saturated((self.0 as u64).saturating_mul(coeff))
    }
}

impl Computational for Ergs32 {
    #[inline(always)]
    fn from_computational(value: u64) -> Self {
        Self::saturated(value)
    }

    #[inline(always)]
    fn as_u64(&self) -> u64 {
        self.0 as u64
    }
}

impl Resource for Ergs32 {
    const FORMAL_INFINITE: Self = Self(Self::MAX_ERGS);

    fn empty() -> Self {
        Self(0)
    }

    fn is_empty(&self) -> bool {
        self.0 == 0
    }

    fn has_enough(&self, to_spend: &Self) -> bool {
        self >= to_spend
    }

    #[inline(always)]
    fn charge(&mut self, to_charge: &Self) -> Result<(), SystemError> {
        // the subtraction and its borrow: one branch (see `Ergs::charge` for why
        // the failure path stays inline)
        let (remaining, borrow) = self.0.overflowing_sub(to_charge.0);
        // the hint keeps the success path as the fall-through of the branch: with
        // the failure path laid out first, every charge of the interpreter's
        // handlers took an extra jump (measured as 5% of the cycles)
        if core::hint::unlikely(borrow) {
            self.0 = 0;
            Err(out_of_ergs_error!())
        } else {
            self.0 = remaining;
            Ok(())
        }
    }

    fn charge_unchecked(&mut self, to_charge: &Self) {
        self.0 -= to_charge.0
    }

    fn reclaim(&mut self, to_reclaim: Self) {
        self.0 = self.0.saturating_add(to_reclaim.0)
    }

    fn reclaim_withheld(&mut self, to_reclaim: Self) {
        self.0 = self.0.saturating_add(to_reclaim.0)
    }

    fn diff(&self, other: Self) -> Self {
        Self(self.0.abs_diff(other.0))
    }

    fn remaining(&self) -> Self {
        *self
    }
}

/// A legacy gas amount computed in `u64` (lengths times rates) as the `u32`
/// amount a charge takes: an amount beyond the range is out of ergs, as no
/// transaction can pay it
#[inline(always)]
pub fn legacy_gas_u32(gas: u64) -> Result<u32, SystemError> {
    u32::try_from(gas).map_err(|_| out_of_ergs_error!())
}

pub trait Resources:
    'static + Sized + Clone + core::fmt::Debug + PartialEq + Eq + Resource
{
    /// Type of native computational resource.
    type Native: Resource + Computational;

    /// Type of the EE resource.
    type Ergs: ErgsResource;

    /// Largest amount of legacy gas of one transaction representable by
    /// [Resources::Ergs]; transaction gas limits are bounded by it, block gas
    /// limits are not.
    const MAX_LEGACY_GAS: u32 = <Self::Ergs as ErgsResource>::MAX_LEGACY_GAS;

    /// Constructor from EE resource, all other resources are set to empty.
    fn from_ergs(ergs: Self::Ergs) -> Self;

    /// Constructor from native resource, all other resources are set to empty.
    fn from_native(native: Self::Native) -> Self;

    /// Constructor from all sub-resources.
    fn from_ergs_and_native(ergs: Self::Ergs, native: Self::Native) -> Self;

    /// Increments the EE resource.
    fn add_ergs(&mut self, to_add: Self::Ergs);

    /// Gets the available ergs (EE resource).
    fn ergs(&self) -> Self::Ergs;

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

    /// Constructor from legacy gas, `None` if it doesn't fit into ergs.
    #[inline(always)]
    fn from_legacy_gas(gas: u32) -> Option<Self> {
        Self::Ergs::from_legacy_gas(gas).map(Self::from_ergs)
    }

    /// Constructor from legacy gas, saturating if it doesn't fit into ergs.
    #[inline(always)]
    fn from_legacy_gas_saturating(gas: u32) -> Self {
        Self::from_ergs(Self::Ergs::from_legacy_gas_saturating(gas))
    }

    /// Available ergs expressed as legacy gas, rounded down.
    #[inline(always)]
    fn legacy_gas(&self) -> u64 {
        self.ergs().as_legacy_gas()
    }

    /// Increments the EE resource by an amount of legacy gas (saturating).
    #[inline(always)]
    fn add_legacy_gas(&mut self, gas: u32) {
        self.add_ergs(Self::Ergs::from_legacy_gas_saturating(gas))
    }

    /// Charges an amount of legacy gas. Fails with out of ergs if the amount
    /// doesn't fit into ergs.
    #[inline(always)]
    fn charge_legacy_gas(&mut self, gas: u32) -> Result<(), SystemError> {
        let ergs = Self::Ergs::from_legacy_gas(gas).ok_or(out_of_ergs_error!())?;
        self.charge(&Self::from_ergs(ergs))
    }

    /// Charges an amount of legacy gas and native resources. Fails with out of
    /// ergs if the gas amount doesn't fit into ergs.
    #[inline(always)]
    fn charge_legacy_gas_and_native(&mut self, gas: u32, native: u64) -> Result<(), SystemError> {
        let ergs = Self::Ergs::from_legacy_gas(gas).ok_or(out_of_ergs_error!())?;
        self.charge(&Self::from_ergs_and_native(
            ergs,
            Self::Native::from_computational(native),
        ))
    }

    /// Charges only native resources.
    #[inline(always)]
    fn charge_native(&mut self, native: u64) -> Result<(), SystemError> {
        self.charge(&Self::from_native(Self::Native::from_computational(native)))
    }
}

///
/// Resources with the state gas dimension of EIP-8037: state creation (new
/// accounts, new storage slots, deployed code, delegation indicators) is paid in
/// state gas, which is drawn from a reservoir first and, once the reservoir is
/// empty, from the ergs (the "spill"). The reservoir is bounded by the
/// transaction gas limit, which fits in a `u32`.
///
/// A frame's reservoir rides into a call in full (no 63/64 rule) and comes back
/// with it; a frame that fails refills its state charges to its entry level (the
/// baseline, kept by the frame) and hands any spill back to its ergs. Refunds
/// of state gas credit the pools in the reverse order of charging: the ergs up
/// to the spill, then the reservoir.
///
/// Invariant kept by every operation: the reservoir and the spill are never
/// both non-zero (a charge spills only once the reservoir is empty, a credit
/// repays the spill before it fills the reservoir, and a reclaim repays the
/// spill from the reclaimed reservoir).
///
pub trait StateGasResources: Resources {
    /// The state gas reservoir
    fn state_gas(&self) -> u32;

    /// Ergs spent on state charges since the last refill, not yet credited back
    fn state_gas_spilled(&self) -> u32;

    /// Adds to the reservoir (a transaction's grant, or a child's)
    fn add_state_gas(&mut self, amount: u32);

    /// Empties the reservoir, for a child frame
    fn drain_state_gas(&mut self) -> u32;

    /// Charges state gas: from the reservoir, then from the ergs as spill. Out
    /// of ergs when both together do not cover `amount`; the ergs are exhausted
    /// then, as for any failed charge.
    fn charge_state_gas(&mut self, amount: u32) -> Result<(), SystemError>;

    /// Credits state gas back: the ergs up to the spill first, the reservoir
    /// for the rest
    fn credit_state_gas(&mut self, amount: u32);

    /// Rolls the state gas back to `baseline` (the reservoir at the frame's
    /// entry): the spill returns to the ergs, the reservoir is refilled
    fn restore_state_gas(&mut self, baseline: u32);

    /// Marks the state gas spent so far as not refillable by a later
    /// [`Self::restore_state_gas`] (the delegations a transaction applied
    /// survive a failure of its code): the spill is folded out and returned,
    /// the reservoir stays as it is and becomes the new baseline
    fn commit_state_gas(&mut self) -> u32;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ergs32_charges_and_bounds() {
        let mut e = Ergs32(100);
        e.charge(&Ergs32(60)).unwrap();
        assert_eq!(e, Ergs32(40));
        assert!(e.charge(&Ergs32(41)).is_err());
        assert_eq!(e, Ergs32(0));
        let mut infinite = Ergs32::FORMAL_INFINITE;
        infinite.charge(&Ergs32(Ergs32::MAX_ERGS)).unwrap();
        assert_eq!(infinite, Ergs32(0));
        assert_eq!(Ergs32::from_legacy_gas(1 << 24), Some(Ergs32(1 << 24)));
        assert_eq!(Ergs32::from_legacy_gas(u32::MAX), Some(Ergs32(u32::MAX)));
        assert_eq!(
            Ergs32::from_legacy_gas_saturating(u32::MAX),
            Ergs32(u32::MAX)
        );
        assert_eq!(Ergs32(7).times(u64::MAX), Ergs32(Ergs32::MAX_ERGS));
        assert_eq!(Ergs32::MAX_LEGACY_GAS, u32::MAX);
        assert_eq!(Ergs32(u32::MAX) + Ergs32(1), Ergs32(u32::MAX));
    }

    #[test]
    fn legacy_gas_conversions() {
        let ergs = Ergs::<256>::from_legacy_gas(10).unwrap();
        assert_eq!(ergs.as_u64(), 2560);
        assert_eq!(ergs.as_legacy_gas(), 10);
        assert_eq!(Ergs::<256>(2561).as_legacy_gas(), 10);
        assert_eq!(Ergs::<256>(2561).as_legacy_gas_ceil(), 11);
        // a `u32` amount always fits, with any factor
        assert_eq!(
            Ergs::<256>::from_legacy_gas(u32::MAX).unwrap().as_u64(),
            u32::MAX as u64 * 256
        );
        assert_eq!(
            Ergs::<256>::from_legacy_gas_saturating(u32::MAX).as_u64(),
            u32::MAX as u64 * 256
        );
        // the transaction gas bound is the `u32` range for every factor
        assert_eq!(<Ergs<256> as ErgsResource>::MAX_LEGACY_GAS, u32::MAX);

        let unit = Ergs::<1>::from_legacy_gas(u32::MAX).unwrap();
        assert_eq!(unit.as_u64(), u32::MAX as u64);
        assert_eq!(unit.as_legacy_gas(), u32::MAX as u64);
        assert_eq!(unit.as_legacy_gas_ceil(), u32::MAX as u64);
        assert_eq!(<Ergs<1> as ErgsResource>::MAX_LEGACY_GAS, u32::MAX);
        assert_eq!(
            <crate::reference_implementations::GasOnlyResources as Resources>::MAX_LEGACY_GAS,
            u32::MAX
        );
        assert_eq!(
            <crate::reference_implementations::BaseResources<()> as Resources>::MAX_LEGACY_GAS,
            u32::MAX
        );
    }

    #[test]
    fn charge_borrow_semantics() {
        let mut e = Ergs::<1>(10);
        assert!(e.charge(&Ergs(3)).is_ok());
        assert_eq!(e.0, 7);
        assert!(e.charge(&Ergs(7)).is_ok());
        assert_eq!(e.0, 0);
        let mut e = Ergs::<1>(10);
        assert!(e.charge(&Ergs(11)).is_err());
        assert_eq!(e.0, 0);
        // a saturated cost never succeeds, even against infinite ergs
        let mut e = Ergs::<1>::FORMAL_INFINITE;
        assert!(e.charge(&Ergs(u64::MAX)).is_err());
        assert_eq!(e.0, 0);
        let mut e = Ergs::<1>(1 << 63);
        assert!(e.charge(&Ergs((1 << 63) - 1)).is_ok());
        assert_eq!(e.0, 1);
        let mut e = Ergs::<1>(u64::MAX);
        assert!(e.charge(&Ergs(u64::MAX - 5)).is_ok());
        assert_eq!(e.0, 5);
        let mut e = Ergs::<1>::FORMAL_INFINITE;
        assert!(e.charge(&Ergs(Ergs::<1>::MAX_ERGS)).is_ok());
        assert_eq!(e.0, 0);
    }

    #[test]
    fn unit_native_is_free() {
        let mut native = ();
        assert!(native.charge(&()).is_ok());
        assert!(native.has_enough(&()));
        assert!(native.is_empty());
        assert_eq!(<() as Computational>::as_u64(&native), 0);
    }
}
