use super::{context::ErrorContext, location::ErrorLocation};

#[cfg_attr(target_arch = "riscv32", derive(Copy))]
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Metadata {
    pub location: ErrorLocation,
    pub context: ErrorContext,
}

impl Metadata {
    pub fn new(location: ErrorLocation) -> Self {
        Self {
            location,
            context: Default::default(),
        }
    }

    pub fn get_context(&self) -> &ErrorContext {
        &self.context
    }

    pub fn replace_context(self, context: ErrorContext) -> Metadata {
        let Self {
            location,
            context: _,
        } = self;
        Self { location, context }
    }
}

impl From<ErrorLocation> for Metadata {
    fn from(location: ErrorLocation) -> Self {
        Self::new(location)
    }
}

impl core::fmt::Display for Metadata {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let Self { location, context } = self;
        writeln!(f, "-- at {location}")?;
        writeln!(f, "{context}")?;
        Ok(())
    }
}
