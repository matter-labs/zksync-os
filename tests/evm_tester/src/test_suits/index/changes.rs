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
        write!(f, "Created:\n")?;
        for x in self.created.iter() {
            write!(f, " - {:?}\n", x)?;
        }
        write!(f, "\n")?;

        write!(f, "Deleted:\n")?;
        for x in self.deleted.iter() {
            write!(f, " - {:?}\n", x)?;
        }
        write!(f, "\n")?;

        write!(f, "Updated:\n")?;
        for x in self.updated.iter() {
            write!(f, " - {:?}\n", x)?;
        }
        write!(f, "\n")?;

        write!(f, "Conflicts:\n")?;
        for x in self.conflicts.iter() {
            write!(f, " - {:?}\n", x)?;
        }
        write!(f, "\n")?;

        Ok(())
    }
}
