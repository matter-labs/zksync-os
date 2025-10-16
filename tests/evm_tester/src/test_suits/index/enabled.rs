//!
//! The enabled test entity description.
//!

use alloy::primitives::*;
use std::path::PathBuf;

///
/// The enabled test entity description.
///
#[derive(Debug, Clone)]
pub struct EnabledTest {
    /// The test path.
    pub path: PathBuf,
    pub skip_calldatas: Option<Vec<Bytes>>,
    pub skip_cases: Option<Vec<String>>,
    pub skip_names: Option<Vec<String>>,
    pub hardfork_override: Option<String>,
}

impl EnabledTest {
    ///
    /// A shortcut constructor.
    ///
    pub fn new(
        path: PathBuf,
        skip_calldatas: Option<Vec<Bytes>>,
        skip_cases: Option<Vec<String>>,
        skip_names: Option<Vec<String>>,
        hardfork_override: Option<String>,
    ) -> Self {
        Self {
            path,
            skip_calldatas,
            skip_cases,
            skip_names,
            hardfork_override,
        }
    }
}
