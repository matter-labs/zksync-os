use core::ptr;

use iwasm_specification::{host_ops::LongHostOp, intx::U256Repr};

use super::{size_bound, Assert, Endianness, IntX, IsTrue, BE, LE};

pub(crate) fn fill_bytes<const N: usize, E: Endianness>(
    value: &mut IntX<N, E>,
    bytes: [u8; core::mem::size_of::<U256Repr>()],
) {
    let src = bytes.as_slice() as *const _ as *const _;
    let dst = value.repr.as_raw_bytes_mut() as *mut _ as *mut _;

    unsafe {
        super::long_host_op(
            LongHostOp::IntxFillBytes,
            0,
            src,
            ptr::dangling(),
            dst,
            ptr::dangling_mut(),
        )
    };
}

/***** Endianness *****/

fn swap_endianness_inplace(value: &mut U256Repr) {
    let ptr = value as *mut _ as *mut _;

    unsafe {
        super::long_host_op(
            LongHostOp::IntxSwapEndianness,
            0,
            ptr::dangling(),
            ptr::dangling(),
            ptr,
            ptr::dangling_mut(),
        )
    };
}

fn swap_endianness_into(src: &U256Repr, dst: &mut U256Repr) {
    let src_ptr = src as *const _ as *const _;
    let dst_ptr = dst as *mut _ as *mut _;

    unsafe {
        super::long_host_op(
            LongHostOp::IntxSwapEndianness,
            1,
            src_ptr,
            ptr::dangling(),
            dst_ptr,
            ptr::dangling_mut(),
        )
    };
}

pub(crate) fn into_be<const N: usize>(mut value: IntX<N, LE>) -> IntX<N, BE> {
    swap_endianness_inplace(&mut value.repr);

    // Safety: Only switching the marker generic param.
    unsafe { core::mem::transmute(value) }
}

pub(crate) fn into_le<const N: usize>(mut value: IntX<N, BE>) -> IntX<N, LE> {
    swap_endianness_inplace(&mut value.repr);

    // Safety: Only switching the marker generic param.
    unsafe { core::mem::transmute(value) }
}

pub(crate) fn to_be<const N: usize>(value: &IntX<N, LE>) -> IntX<N, BE>
where
    Assert<{ size_bound(N) }>: IsTrue,
{
    let mut r = IntX::new();

    swap_endianness_into(&value.repr, &mut r.repr);

    r
}

pub(crate) fn to_le<const N: usize>(value: &IntX<N, BE>) -> IntX<N, LE>
where
    Assert<{ size_bound(N) }>: IsTrue,
{
    let mut r = IntX::new();

    swap_endianness_into(&value.repr, &mut r.repr);

    r
}

/***** Compares *****/

pub(crate) fn partial_eq<const N: usize, E: Endianness + 'static>(
    left: &IntX<N, E>,
    right: &IntX<N, E>,
) -> bool {
    let param = match core::any::TypeId::of::<E>() {
        x if x == core::any::TypeId::of::<BE>() => 0 << 32,
        x if x == core::any::TypeId::of::<LE>() => 1 << 32,
        _ => unreachable!(),
    };

    let left_ptr = &left.repr as *const _ as *const _;
    let right_ptr = &right.repr as *const _ as *const _;

    let (success, result) = unsafe {
        super::long_host_op(
            LongHostOp::IntxCompare,
            param,
            left_ptr,
            right_ptr,
            ptr::dangling_mut(),
            ptr::dangling_mut(),
        )
        .into()
    };

    // Safety: the host swears it returns 0 or 1.
    unsafe { core::mem::transmute(result as u8) }
}


/***** Arith *****/

pub(crate) fn le_add<const N: usize>(left: &IntX<N, LE>, right: &IntX<N, LE>) -> IntX<N, LE>
where
    Assert<{ size_bound(N) }>: IsTrue,
{
    let param = N as u64;

    let mut r = IntX::new();

    let (success, result) = unsafe {
        super::long_host_op(
            LongHostOp::IntxOverflowingAdd,
            param,
            left.repr.as_unit_ptr(),
            right.repr.as_unit_ptr(),
            r.repr.as_unit_mut_ptr(),
            ptr::dangling_mut(),
        )
        .into()
    };

    r
}

pub(crate) fn le_sub<const N: usize>(left: &IntX<N, LE>, right: &IntX<N, LE>) -> IntX<N, LE>
where
    Assert<{ size_bound(N) }>: IsTrue,
{
    let param = N as u64;

    let mut r = IntX::new();

    let (success, result) = unsafe {
        super::long_host_op(
            LongHostOp::IntxOverflowingSub,
            param,
            left.repr.as_unit_ptr(),
            right.repr.as_unit_ptr(),
            r.repr.as_unit_mut_ptr(),
            ptr::dangling_mut(),
        )
        .into()
    };

    r
}
