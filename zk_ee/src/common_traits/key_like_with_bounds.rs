pub trait KeyLikeWithBounds:
    'static + Clone + Copy + core::cmp::Ord + core::cmp::Eq + core::fmt::Debug
{
    type Subspace: 'static + Clone + core::fmt::Debug;
    fn lower_bound(subspace: Self::Subspace) -> Self;
    fn upper_bound(subspace: Self::Subspace) -> Self;
}

/// Helper trait until type equalities on methods are available
pub trait TyEq<T> {
    fn rw(self) -> T;
    fn rwi(x: T) -> Self;
}

impl<T> TyEq<T> for T {
    fn rw(self) -> T {
        self
    }
    fn rwi(x: T) -> Self {
        x
    }
}
