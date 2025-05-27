use super::host::Host;
use super::stack_value::*;
use crate::types::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FunctionCallFrame {
    pub return_pc: u32,
    pub function_idx: u16,
    pub frame_start: u32,
    pub inputs_locals_start: u32,
    pub num_locals: u32,
    pub sidetable_idx: i32,
}

pub(crate) fn evaluate_const_expr_as_u32<H: Host>(
    host: &'_ mut H,
    const_expr: ConstantExpression,
    num_imported_globals: u16,
    get_global_fn: impl Fn(&'_ mut H, usize) -> StackValue,
) -> Result<u32, ()> {
    match const_expr {
        ConstantExpression::I32(value) => Ok(value as u32),
        ConstantExpression::Global(global_idx) => {
            if global_idx >= num_imported_globals as u32 {
                return Err(());
            }
            let global_value = (get_global_fn)(host, global_idx as usize);
            Ok(global_value.as_i32() as u32)
        }
        _ => Err(()),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExecutionResult {
    Continue = 1,
    Return,
    Reverted,
    DidNotComplete,
    Exception,
    Preemption,
}
