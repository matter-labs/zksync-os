//!
//! The evm tester summary element outcome.
//!

pub mod passed_variant;

use self::passed_variant::PassedVariant;

///
/// The evm tester summary element outcome.
///
#[derive(Debug)]
pub enum Outcome {
    /// The `passed` outcome.
    Passed {
        /// The outcome variant.
        variant: PassedVariant,
    },
    /// The `failed` outcome. The output result is incorrect.
    Failed {
        // exception: bool,
        expected: Option<String>,
        actual: Option<String>,
    },
    /// The `invalid` outcome. The test is incorrect.
    Invalid {
        /// The building error description.
        error: String,
    },
    /// The `panicked` outcome. The test execution raised a panic.
    Panicked {
        /// The building error description.
        error: String,
    },
    /// The `ignored` outcome. The test is ignored.
    Ignored,
}

impl Outcome {
    ///
    /// A shortcut constructor.
    ///
    pub fn passed(variant: PassedVariant) -> Self {
        Self::Passed { variant }
    }

    ///
    /// A shortcut constructor.
    ///
    pub fn failed(expected: Option<String>, actual: Option<String>) -> Self {
        Self::Failed {
            // exception,
            expected,
            actual,
        }
    }

    ///
    /// A shortcut constructor.
    ///
    pub fn invalid<S>(error: S) -> Self
    where
        S: ToString,
    {
        Self::Invalid {
            error: error.to_string(),
        }
    }

    ///
    /// A shortcut constructor.
    ///
    pub fn panicked<S>(error: S) -> Self
    where
        S: ToString,
    {
        Self::Panicked {
            error: error.to_string(),
        }
    }

    ///
    /// A shortcut constructor.
    ///
    pub fn ignored() -> Self {
        Self::Ignored
    }
}
