pub trait KeyLikeWithBounds:
    'static + Clone + Copy + core::cmp::Ord + core::cmp::Eq + core::fmt::Debug
{
    type Subspace: 'static + Clone + core::fmt::Debug;
    fn lower_bound(subspace: Self::Subspace) -> Self;
    fn upper_bound(subspace: Self::Subspace) -> Self;
}
