//!
//! The tester environment to run tests on.
//!

///
/// The tester environment to run tests on.
///
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Deserialize)]
pub enum Environment {
    ZKsyncOS,
}

impl std::str::FromStr for Environment {
    type Err = anyhow::Error;

    fn from_str(string: &str) -> Result<Self, Self::Err> {
        match string {
            "ZKsyncOS" => Ok(Self::ZKsyncOS),
            string => anyhow::bail!(
                "Unknown environment `{}`. Supported environments: {:?}",
                string,
                vec![Self::ZKsyncOS]
                    .into_iter()
                    .map(|element| element.to_string())
                    .collect::<Vec<String>>()
                    .join(", ")
            ),
        }
    }
}

impl std::fmt::Display for Environment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ZKsyncOS => write!(f, "ZKsync OS"),
        }
    }
}
