pub mod memory;
pub mod runtime;
pub mod verification_time;

use alloc::string::String;

#[cfg(feature = "testing")]
use alloc::boxed::Box;

#[cfg(feature = "testing")]
#[allow(dead_code)]
#[derive(Debug)]
pub struct InterpreterError {
    msg: String,
    line: u32,
    file: &'static str,
    next: Option<Box<InterpreterError>>,
}

#[cfg(not(feature = "testing"))]
#[derive(Debug)]
pub struct InterpreterError {}

impl InterpreterError {
    #[track_caller]
    pub fn new(_msg: String) -> Self {
        #[cfg(feature = "testing")]
        {
            Self {
                msg: _msg,
                line: core::panic::Location::caller().line(),
                file: core::panic::Location::caller().file(),
                next: None,
            }
        }

        #[cfg(not(feature = "testing"))]
        InterpreterError {}
    }

    #[allow(unused_mut)]
    pub fn link<T: Into<InterpreterError>>(mut self, _other: T) -> Self {
        #[cfg(feature = "testing")]
        {
            self.next = Some(Box::new(_other.into()));
            self
        }

        #[cfg(not(feature = "testing"))]
        {
            self
        }
    }
}

impl From<&str> for InterpreterError {
    #[track_caller]
    fn from(_value: &str) -> Self {
        #[cfg(feature = "testing")]
        {
            Self {
                msg: String::from(_value),
                line: core::panic::Location::caller().line(),
                file: core::panic::Location::caller().file(),
                next: None,
            }
        }

        #[cfg(not(feature = "testing"))]
        ().into()
    }
}

impl From<String> for InterpreterError {
    #[track_caller]
    fn from(_value: String) -> Self {
        #[cfg(feature = "testing")]
        {
            Self {
                msg: _value,
                next: None,
                line: core::panic::Location::caller().line(),
                file: core::panic::Location::caller().file(),
            }
        }

        #[cfg(not(feature = "testing"))]
        ().into()
    }
}

/// Exists to aid with updating the error types in on demand manner.
impl From<()> for InterpreterError {
    #[track_caller]
    #[allow(clippy::unconditional_recursion)]
    fn from(_value: ()) -> Self {
        #[cfg(feature = "testing")]
        {
            let x = Self {
                msg: String::from("() to Error transition."),
                next: None,
                line: core::panic::Location::caller().line(),
                file: core::panic::Location::caller().file(),
            };
            panic!("{:#?}", x);
        }

        #[cfg(not(feature = "testing"))]
        ().into()
    }
}

impl From<InterpreterError> for () {
    fn from(_value: InterpreterError) -> Self {
        #[cfg(feature = "testing")]
        panic!("{:?}", _value);

        #[cfg(not(feature = "testing"))]
        ()
    }
}
