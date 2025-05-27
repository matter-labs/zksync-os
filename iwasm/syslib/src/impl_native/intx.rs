use iwasm_specification::intx::U256Repr;

use super::{size_bound, Assert, Endianness, IntX, IsTrue, BE, LE};

pub(crate) fn fill_bytes<const N: usize, E: Endianness>(
    value: &mut IntX<N, E>,
    bytes: [u8; core::mem::size_of::<U256Repr>()],
) {
    let src = bytes.as_slice() as *const _ as *const u8;
    let tgt = value.repr.as_raw_bytes_mut() as *mut _ as *mut u8;

    unsafe {
        core::ptr::copy_nonoverlapping(src, tgt, value.repr.as_raw_bytes().len());
    }
}

pub(crate) fn partial_eq<const N: usize, E: Endianness>(
    left: &IntX<N, E>,
    right: &IntX<N, E>,
) -> bool {
    left.repr.bytes_eq::<N>(&right.repr)
}

pub(crate) fn into_be<const N: usize>(mut value: IntX<N, LE>) -> IntX<N, BE> {
    value.repr.swap_endianness_inplace();

    // Safety: Only switching the marker generic param.
    unsafe { core::mem::transmute(value) }
}

/***** Endianness *****/

pub(crate) fn to_be<const N: usize>(value: &IntX<N, LE>) -> IntX<N, BE>
where
    Assert<{ size_bound(N) }>: IsTrue,
{
    let mut r = IntX::new();

    let src = value.repr.as_u64_le_lsb_limbs();
    let tgt = r.repr.as_u64_be_lsb_limbs_mut();

    src.into_iter().zip(tgt).for_each(|(s, t)| *t = s.to_be());

    r
}

pub(crate) fn into_le<const N: usize>(mut value: IntX<N, BE>) -> IntX<N, LE> {
    value.repr.swap_endianness_inplace();

    // Safety: Only switching the marker generic param.
    unsafe { core::mem::transmute(value) }
}

pub(crate) fn to_le<const N: usize>(value: &IntX<N, BE>) -> IntX<N, LE>
where
    Assert<{ size_bound(N) }>: IsTrue,
{
    let mut r = IntX::new();

    let src = value.repr.as_u64_be_lsb_limbs();
    let tgt = r.repr.as_u64_le_lsb_limbs_mut();

    src.into_iter()
        .zip(tgt)
        // We're on little endian and are only interested in unconditionally swapping bytes.
        .for_each(|(s, t)| *t = s.to_be());

    r
}

/***** LE - Arithmetics *****/

pub(crate) fn le_add<const N: usize>(left: &IntX<N, LE>, right: &IntX<N, LE>) -> IntX<N, LE>
where
    Assert<{ size_bound(N) }>: IsTrue,
{
    let mut r = IntX::new();
    U256Repr::le_add_into(N, &left.repr, &right.repr, &mut r.repr);

    r
}

pub(crate) fn le_sub<const N: usize>(left: &IntX<N, LE>, right: &IntX<N, LE>) -> IntX<N, LE>
where
    Assert<{ size_bound(N) }>: IsTrue,
{
    let mut r = IntX::new();
    U256Repr::le_sub_into(N, &left.repr, &right.repr, &mut r.repr);

    r
}
