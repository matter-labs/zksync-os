//! Rollback-aware state of a cached element, shared by the storage caches: its
//! EIP-2929 warmth and what is known about its block-start value. Both live in
//! history records, so they follow the frame rollback rules described in
//! [`super`] (charging invariant), and are meant to describe accounts as well as
//! storage slots.

/// Identifies a transaction for warmth purposes. It does not have to be the
/// position of the transaction in the block: it only has to differ between
/// transactions.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default)]
pub struct TransactionId(pub u32);

/// EIP-2929 warmth of a cache element.
///
/// Warmth is bound to the transaction that established it, so at the start of
/// the next transaction every element is cold again without visiting it. It
/// lives in rollback-aware metadata: an access that is reverted makes the
/// element cold again, whether or not the value was observed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Warmth {
    #[default]
    Cold,
    Warm {
        in_tx: TransactionId,
    },
}

impl Warmth {
    /// Whether the element is warm for `current_tx`. Warmth from another
    /// transaction counts as cold.
    #[inline(always)]
    pub fn is_warm(&self, current_tx: TransactionId) -> bool {
        match self {
            Self::Cold => false,
            Self::Warm { in_tx } => *in_tx == current_tx,
        }
    }
}

/// What is known about the value of a cache element.
///
/// An element gets into the cache either by a read, which observes its
/// block-start value, or by a touch (access list, precompile warm-up), which
/// only declares it. A touched element creates no proof obligation until it is
/// read, exactly like on Ethereum, where warming a slot does not load it.
///
/// Observation is monotonic: once a value is `Observed` it stays so through
/// rollbacks (see how the storage cache fills it in), so `Unobserved` never
/// follows `Observed` in an element's history. The "new element" fact travels
/// with the value: it is only meaningful once the value is known, and keeping
/// it here lets the `bool` niche hold the discriminant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObservedValue<V> {
    /// Declared but never read: nothing is known about the element.
    Unobserved,
    /// Read from the oracle. `is_new` is the block-start fact whether the
    /// element was absent from the tree (its initial value is then trivial).
    Observed { value: V, is_new: bool },
}

impl<V> ObservedValue<V> {
    #[inline(always)]
    pub fn is_unobserved(&self) -> bool {
        matches!(self, Self::Unobserved)
    }

    /// The value, if known
    #[inline(always)]
    pub fn observed(&self) -> Option<&V> {
        match self {
            Self::Unobserved => None,
            Self::Observed { value, .. } => Some(value),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn warmth_is_bound_to_transaction() {
        assert!(!Warmth::Cold.is_warm(TransactionId(1)));
        let warm = Warmth::Warm {
            in_tx: TransactionId(1),
        };
        assert!(warm.is_warm(TransactionId(1)));
        assert!(!warm.is_warm(TransactionId(2)));
    }

    #[test]
    fn observed_value_uses_the_bool_niche() {
        assert_eq!(
            core::mem::size_of::<ObservedValue<[u64; 4]>>(),
            core::mem::size_of::<([u64; 4], bool)>()
        );
        assert!(ObservedValue::<u8>::Unobserved.is_unobserved());
        assert_eq!(
            ObservedValue::Observed {
                value: 7u8,
                is_new: true
            }
            .observed(),
            Some(&7)
        );
    }
}
