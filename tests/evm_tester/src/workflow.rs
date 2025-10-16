//!
//! The evm tester workflows.
//!

use std::str::FromStr;

///
/// Describes sets of actions that evm tester is able to perform.
///
#[derive(Debug)]
pub enum Workflow {
    /// Only build tests but not execute them.
    BuildOnly,
    /// Build and execute tests.
    BuildAndRun,
    /// Build, execute and benchmark tests.
    Bench,
}

impl FromStr for Workflow {
    type Err = anyhow::Error;

    fn from_str(day: &str) -> Result<Self, Self::Err> {
        match day {
            "build" => Ok(Workflow::BuildOnly),
            "run" => Ok(Workflow::BuildAndRun),
            "bench" => Ok(Workflow::Bench),
            string => anyhow::bail!(
                "Unknown workflow `{}`. Supported workflows: {}",
                string,
                vec![Self::BuildOnly, Self::BuildAndRun]
                    .into_iter()
                    .map(|element| element.to_string())
                    .collect::<Vec<String>>()
                    .join(", ")
            ),
        }
    }
}

impl std::fmt::Display for Workflow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Workflow::BuildOnly => write!(f, "build"),
            Workflow::BuildAndRun => write!(f, "run"),
            Workflow::Bench => write!(f, "bench"),
        }
    }
}
