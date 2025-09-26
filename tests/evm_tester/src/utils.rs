//!
//! The evm tester utils.
//!

use std::path::Path;

use crate::test_suits::index;

pub(crate) fn address_to_b160(value: alloy::primitives::Address) -> ruint::aliases::B160 {
    ruint::aliases::B160::from_be_bytes(value.0 .0)
}

///
/// Reads the Ethereum test index.
///
pub fn read_index(index_path: &Path) -> anyhow::Result<index::FSEntity> {
    let index_data = std::fs::read_to_string(index_path)?;
    let index: index::FSEntity = serde_yaml::from_str(index_data.as_str())?;
    Ok(index)
}

pub fn create_index(index_path: &Path, directory_path: &Path) -> anyhow::Result<()> {
    let index = index::FSEntity::index(directory_path)?;
    let _ = std::fs::write(index_path, serde_yaml::to_string(&index)?.as_bytes());

    Ok(())
}

pub fn update_index(index_path: &str, directory_path: &str) -> anyhow::Result<()> {
    let index_path = Path::new(index_path);
    let directory_path = Path::new(directory_path);

    let old_index = read_index(index_path)?;

    let mut new_index = index::FSEntity::index(directory_path)?;

    let changes = old_index.update(&mut new_index, directory_path, true)?;

    println!("Index updated\n {}", changes);

    let _ = std::fs::write(index_path, serde_yaml::to_string(&new_index)?.as_bytes());

    Ok(())
}
