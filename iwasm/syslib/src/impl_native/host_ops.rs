use iwasm_specification::host_ops::LongHostOp;

#[allow(dead_code)]
pub(crate) fn long_host_op(
    op: u32,
    _op_param: u64,
    _op1: *const u64,
    _op2: *const u64,
    _dst1: *mut u64,
    _dst2: *mut u64,
) -> (bool, u64) {
    let operation = unsafe { core::mem::transmute::<u32, LongHostOp>(op) };
    let (_operand_0_size, _operand_1_size, _dst_0_size, _dst_1_size) = match operation {
        LongHostOp::OverflowingAdd => (0, 0, 0, 0),
        _ => unimplemented!(),
    };

    (false, 0)
}
