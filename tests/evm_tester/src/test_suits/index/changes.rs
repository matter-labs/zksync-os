//!
//! The tests changes.
//!

use std::fmt;
use std::path::PathBuf;

///
/// The tests changes.
///
#[derive(Debug, Default)]
pub struct Changes {
    /// Created tests.
    pub created: Vec<PathBuf>,
    /// Deleted tests.
    pub deleted: Vec<PathBuf>,
    /// Updated tests.
    pub updated: Vec<PathBuf>,
    /// Tests updated with conflicts.
    pub conflicts: Vec<PathBuf>,
}

impl fmt::Display for Changes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Created:")?;
        for x in self.created.iter() {
            writeln!(f, " - {:?}", x)?;
        }
        writeln!(f)?;

        writeln!(f, "Deleted:")?;
        for x in self.deleted.iter() {
            writeln!(f, " - {:?}", x)?;
        }
        writeln!(f)?;

        writeln!(f, "Updated:")?;
        for x in self.updated.iter() {
            writeln!(f, " - {:?}", x)?;
        }
        writeln!(f)?;

        writeln!(f, "Conflicts:")?;
        for x in self.conflicts.iter() {
            writeln!(f, " - {:?}", x)?;
        }
        writeln!(f)?;

        Ok(())
    }
}
