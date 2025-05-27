use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConstantExpression {
    I64(i64),
    I32(i32),
    RefNull,
    FuncRef(u16),
    Global(u32),
}

impl ConstantExpression {
    #[allow(clippy::result_unit_err)]
    pub fn simple_value_type_match(&self, value_type: ValueType) -> Result<bool, ()> {
        match self {
            Self::I64(..) => Ok(value_type == ValueType::I64),
            Self::I32(..) => Ok(value_type == ValueType::I32),
            Self::RefNull | Self::FuncRef(..) => Ok(value_type == ValueType::FuncRef),
            _ => Err(()),
        }
    }

    pub const fn can_be_u32_expr(&self) -> bool {
        matches!(self, Self::I32(..) | Self::Global(..))
    }
}
