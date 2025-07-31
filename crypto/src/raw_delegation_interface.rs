use super::BigIntOps;

/// # Safety
///
/// Requires pointers to be 32-byte alligned, and point to RAM based allocation of at least 32 byte size
#[inline(always)]
pub unsafe fn bigint_op_delegation_raw(a: *mut (), b: *const (), op: BigIntOps) -> u32 {
    crate::bigint_delegation::delegation::bigint_op_delegation_with_carry_bit_by_ptr(
        a.cast(),
        b.cast(),
        false,
        op,
    )
}

/// # Safety
///
/// Requires pointers to be 32-byte alligned, and point to RAM based allocation of at least 32 byte size
#[inline(always)]
pub unsafe fn bigint_op_delegation_with_carry_bit_raw(
    a: *mut (),
    b: *const (),
    carry: bool,
    op: BigIntOps,
) -> u32 {
    crate::bigint_delegation::delegation::bigint_op_delegation_with_carry_bit_by_ptr(
        a.cast(),
        b.cast(),
        carry,
        op,
    )
}
