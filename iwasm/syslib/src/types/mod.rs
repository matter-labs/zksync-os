pub mod ints;
pub mod uintx;

pub type Address = uintx::IntX<20, uintx::LE>;
pub type ExecutionResult<'a, T> = Result<T, &'a str>;
