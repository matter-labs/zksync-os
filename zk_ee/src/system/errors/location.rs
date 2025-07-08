pub trait IError: Clone + core::fmt::Debug + Eq + Sized {
    fn get_location() -> ErrorLocation;
    fn get_message() -> &'static str;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ErrorLocation {
    pub line: u32,
    pub file: &'static str,
}
