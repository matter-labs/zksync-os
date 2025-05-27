pub use iwasm_specification::sys::SliceRef;

pub mod intx {
    use iwasm_specification::intx::U256Repr;

    use crate::types::uintx::{size_bound, Assert, Endianness, IntX, IsTrue, BE, LE};

    pub(crate) fn fill_bytes<const N: usize, E: Endianness>(
        value: &mut IntX<N, E>,
        bytes: [u8; core::mem::size_of::<U256Repr>()],
    ) {
        crate::impl_arch::intx::fill_bytes(value, bytes)
    }

    pub(crate) fn partial_eq<const N: usize, E: Endianness + 'static>(
        left: &IntX<N, E>,
        right: &IntX<N, E>,
    ) -> bool {
        crate::impl_arch::intx::partial_eq(left, right)
    }

    pub(crate) fn is_zero<const N: usize, E: Endianness>(value: &IntX<N, E>) -> bool
    where 
        Assert<{ size_bound(N) }>: IsTrue,
    {
        E::limbs_usize_lsb(&value.repr).into_iter().all(|x| *x == 0)
    }

    pub(crate) fn cmp<const N: usize, E: Endianness>(left: &IntX<N, E>, right: &IntX<N, E>)
    -> core::cmp::Ordering
    where 
        Assert<{ size_bound(N) }>: IsTrue,
    {
        // TODO: check asm
        match 
            E::limbs_usize_msb(&left.repr).into_iter().zip(
            E::limbs_usize_msb(&right.repr).into_iter()).find_map(|(l, r)| {
                match l.cmp(r) {
                    core::cmp::Ordering::Equal => None,
                    x => Some(x)
                }
            }) {
                Some(x) => x,
                None => core::cmp::Ordering::Equal,
            }
    }

    /***** Endianness *****/

    pub(crate) fn into_be<const N: usize>(value: IntX<N, LE>) -> IntX<N, BE> {
        crate::impl_arch::intx::into_be(value)
    }

    pub(crate) fn to_be<const N: usize>(value: &IntX<N, LE>) -> IntX<N, BE>
    where
        Assert<{ size_bound(N) }>: IsTrue,
    {
        crate::impl_arch::intx::to_be(value)
    }

    pub(crate) fn into_le<const N: usize>(value: IntX<N, BE>) -> IntX<N, LE> {
        crate::impl_arch::intx::into_le(value)
    }

    pub(crate) fn to_le<const N: usize>(value: &IntX<N, BE>) -> IntX<N, LE>
    where
        Assert<{ size_bound(N) }>: IsTrue,
    {
        crate::impl_arch::intx::to_le(value)
    }

    
    /***** LE - Arithmetics *****/

    pub(crate) fn le_add<const N: usize>(left: &IntX<N, LE>, right: &IntX<N, LE>) -> IntX<N, LE>
    where
        Assert<{ size_bound(N) }>: IsTrue,
    {
        crate::impl_arch::intx::le_add(left, right)
    }

    pub(crate) fn le_sub<const N: usize>(left: &IntX<N, LE>, right: &IntX<N, LE>) -> IntX<N, LE>
    where
        Assert<{ size_bound(N) }>: IsTrue,
    {
        crate::impl_arch::intx::le_sub(left, right)
    }
}
