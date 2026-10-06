//! Reference implementations.
//! For now, only for resources.
//! We track two resources:
//! - EE resource: measured in ergs. Includes EVM gas, converted as 1 gas = GAS_TO_ERGS_FACTOR ergs.
//! - Native resource: model for prover complexity.

use crate::{
    out_of_native_resources,
    system::{
        errors::system::SystemError, Computational, Ergs, Resource, Resources,
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

impl<
        Native: Resource + Computational + crate::system::NativeTracking,
        const GAS_TO_ERGS_FACTOR: u64,
    > Resources for BaseResources<Native, GAS_TO_ERGS_FACTOR>
{
    type Native = Native;
    type Ergs = Ergs<GAS_TO_ERGS_FACTOR>;
    // With an untracked native resource: (C1) the native field has a single value, and the
    // fields have plain accessors and constructors; (C2) `charge` is `Ergs::charge`, which
    // succeeds and subtracts when the ergs can pay, then the native charge, which succeeds.
    const ERGS_ONLY: crate::system::ErgsOnly<Self> =
        crate::system::ErgsOnly::if_native_untracked(Native::UNTRACKED);

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

impl crate::system::NativeTracking for DecreasingNative {}

#[cfg(test)]
mod tests {
    use crate::system::{
        errors::system::SystemError, Computational, Ergs, NativeTracking, Resource, Resources,
    };

    use super::{BaseResources, DecreasingNative, GasOnlyResources};

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

    /// A native resource that says nothing about its tracking
    #[derive(Clone, Debug, Default, PartialEq, Eq)]
    struct OpaqueNative;

    impl Resource for OpaqueNative {
        const FORMAL_INFINITE: Self = Self;
        fn empty() -> Self {
            Self
        }
        fn is_empty(&self) -> bool {
            true
        }
        fn charge(&mut self, _to_charge: &Self) -> Result<(), SystemError> {
            Ok(())
        }
        fn charge_unchecked(&mut self, _to_charge: &Self) {}
        fn has_enough(&self, _to_spend: &Self) -> bool {
            true
        }
        fn reclaim(&mut self, _to_reclaim: Self) {}
        fn reclaim_withheld(&mut self, _to_reclaim: Self) {}
        fn diff(&self, _other: Self) -> Self {
            Self
        }
        fn remaining(&self) -> Self {
            Self
        }
    }

    impl Computational for OpaqueNative {
        fn from_computational(_value: u64) -> Self {
            Self
        }
        fn as_u64(&self) -> u64 {
            0
        }
    }

    impl NativeTracking for OpaqueNative {}

    #[test]
    fn ergs_only_forms() {
        assert!(GasOnlyResources::ERGS_ONLY.holds());
        assert!(BaseResources::<(), 256>::ERGS_ONLY.holds());
        assert!(!BaseResources::<DecreasingNative>::ERGS_ONLY.holds());
        assert!(!BaseResources::<DecreasingNative, 1>::ERGS_ONLY.holds());
        // even a native resource that behaves as `()` must be stated untracked by this crate
        assert!(!BaseResources::<OpaqueNative, 1>::ERGS_ONLY.holds());
    }

    fn grid() -> Vec<u64> {
        let mut values = vec![
            0,
            1,
            19,
            20,
            (1 << 31) - 1,
            1 << 31,
            (1 << 31) + 1,
            (1 << 32) - 1,
            1 << 32,
            (1 << 32) + 1,
            3 << 31,
            i64::MAX as u64,
            1 << 63,
            u64::MAX - 1,
            u64::MAX,
        ];
        let mut x = 0x2545_f491_4f6c_dd1du64;
        for _ in 0..8 {
            x = x
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            values.extend([x, x >> 33]);
        }
        values
    }

    /// (C1) and (C2) of `Resources::ERGS_ONLY` for the resources that say it holds
    fn check_ergs_only_contract<R: Resources>() {
        assert!(R::ERGS_ONLY.holds());
        for ergs in grid() {
            let r = R::from_ergs_and_native(
                R::Ergs::from_computational(ergs),
                R::Native::from_computational(ergs),
            );
            assert_eq!(R::from_ergs(r.ergs()), r);
            assert_eq!(r.ergs().as_u64(), ergs);
            for cost in grid() {
                if ergs < cost {
                    continue;
                }
                for native in [0, 20, 1 << 31, u64::MAX] {
                    let mut charged = r.clone();
                    let cost = R::from_ergs_and_native(
                        R::Ergs::from_computational(cost),
                        R::Native::from_computational(native),
                    );
                    assert!(charged.charge(&cost).is_ok());
                    assert_eq!(
                        charged,
                        R::from_ergs(R::Ergs::from_computational(ergs - cost.ergs().as_u64()))
                    );
                }
            }
        }
    }

    #[test]
    fn ergs_only_contract_holds() {
        check_ergs_only_contract::<GasOnlyResources>();
        check_ergs_only_contract::<BaseResources<(), 256>>();
    }
}
