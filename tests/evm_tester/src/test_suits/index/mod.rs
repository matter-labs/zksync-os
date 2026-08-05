//!
//! The Solidity tests file system entity.
//!

mod changes;
mod directory;
pub mod enabled;
pub(crate) mod test_file;

use std::collections::BTreeMap;
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

use serde::Deserialize;
use serde::Serialize;

use self::changes::Changes;
use self::directory::Directory;
use self::enabled::EnabledTest;
use self::test_file::TestFile;
use alloy::primitives::*;

///
/// The Solidity tests file system entity.
///
#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FSEntity {
    /// The directory.
    Directory(Directory),
    /// The test file.
    File(TestFile),
}

impl FSEntity {
    ///
    /// Indexes the specified directory.
    ///
    pub fn index(path: &Path) -> anyhow::Result<FSEntity> {
        let mut entries = BTreeMap::new();

        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let path = entry.path();
            let entry_type = entry.file_type()?;

            if entry.file_name().to_string_lossy().starts_with('.') {
                continue;
            }

            if entry_type.is_dir() && entry.file_name() != "src" {
                entries.insert(
                    path.file_name()
                        .ok_or_else(|| anyhow::anyhow!("Failed to get filename"))?
                        .to_string_lossy()
                        .to_string(),
                    Self::index(&path)?,
                );
                continue;
            }

            if !entry_type.is_file() {
                anyhow::bail!("Invalid entry type");
            }

            entries.insert(
                path.file_name()
                    .ok_or_else(|| anyhow::anyhow!("Failed to get filename"))?
                    .to_string_lossy()
                    .to_string(),
                Self::File(TestFile::try_from(path.as_path())?),
            );
        }

        Ok(Self::Directory(Directory::new(entries)))
    }

    ///
    /// Updates the new index, tests and returns changes.
    ///
    pub fn update(
        &self,
        new: &mut FSEntity,
        initial: &Path,
        index_only: bool,
    ) -> anyhow::Result<Changes> {
        let mut changes = Changes::default();
        self.update_recursive(new, initial, &mut changes, index_only)?;
        Ok(changes)
    }

    ///
    /// Returns the enabled tests list with the `initial` directory prefix.
    ///
    pub fn into_enabled_list(self, initial: &Path) -> Vec<EnabledTest> {
        let mut accumulator = Vec::with_capacity(16384);
        self.into_enabled_list_recursive(
            initial,
            &mut accumulator,
            &vec![],
            &vec![],
            &vec![],
            &None,
        );
        accumulator.sort_by_key(|test| test.path.to_owned());
        accumulator
    }

    ///
    /// Returns the enabled test by the path with the `initial` directory prefix (None if not found or test disabled).
    ///
    pub fn into_enabled_test(self, initial: &Path, path: &Path) -> Option<EnabledTest> {
        let mut skipped_calldatas: Vec<Bytes> = vec![];
        let mut skipped_cases: Vec<String> = vec![];
        let mut skipped_names: Vec<String> = vec![];
        let mut hardfork_override: Option<String> = None;

        let mut current_entity = self;
        for path_part in path.iter() {
            match current_entity {
                FSEntity::Directory(mut directory) => {
                    if let Some(additional_skipped_calldatas) = directory.skip_calldatas {
                        skipped_calldatas.extend(additional_skipped_calldatas);
                    }

                    if let Some(additional_skipped_cases) = directory.skip_cases {
                        skipped_cases.extend(additional_skipped_cases);
                    }

                    if let Some(additional_skipped_names) = directory.skip_names {
                        skipped_names.extend(additional_skipped_names);
                    }

                    if let Some(hardfork) = directory.hardfork_override {
                        hardfork_override = Some(hardfork);
                    }

                    current_entity = match directory
                        .entries
                        .remove(path_part.to_string_lossy().as_ref())
                    {
                        Some(entity) => entity,
                        None => return None,
                    }
                }
                FSEntity::File(_) => return None,
            }
        }
        match current_entity {
            FSEntity::Directory(_) => None,
            FSEntity::File(file) => {
                if !file.enabled {
                    return None;
                }
                let mut file_path = initial.to_path_buf();
                file_path.push(path);
                Some(EnabledTest::new(
                    file_path,
                    Some(skipped_calldatas),
                    Some(skipped_cases),
                    Some(skipped_names),
                    hardfork_override,
                ))
            }
        }
    }

    ///
    /// Updates new index, tests and lists changes.
    ///
    fn update_recursive(
        &self,
        new: &mut FSEntity,
        current: &Path,
        changes: &mut Changes,
        index_only: bool,
    ) -> anyhow::Result<()> {
        let (old_entities, new_entities) = match (self, new) {
            (Self::File(old_file), Self::File(new_file)) => {
                new_file.enabled = old_file.enabled;
                new_file.group = old_file.group.clone();
                new_file.comment = old_file.comment.clone();
                new_file.skip_cases = old_file.skip_cases.clone();
                new_file.skip_names = old_file.skip_names.clone();
                new_file.skip_calldatas = old_file.skip_calldatas.clone();
                new_file.hardfork_override = old_file.hardfork_override.clone();

                let new_hash = new_file
                    .hash
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("Test file hash is None: {:?}", current))?;

                if !old_file
                    .hash
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("Test file hash is None: {:?}", current))?
                    .eq(new_hash)
                {
                    if old_file.was_changed(current)? {
                        changes.conflicts.push(current.to_owned());
                    } else {
                        changes.updated.push(current.to_owned());
                    }
                    if !index_only {
                        TestFile::write_to_file(
                            current,
                            new_file
                                .data
                                .as_ref()
                                .ok_or_else(|| anyhow::anyhow!("Test data is None: {:?}", current))?
                                .as_bytes(),
                        )?;
                    }
                }
                return Ok(());
            }
            (
                Self::Directory(Directory {
                    enabled: old_enabled,
                    entries: old_entities,
                    comment: old_comment,
                    skip_calldatas: old_skip_calldatas,
                    skip_cases: old_skip_cases,
                    skip_names: old_skip_names,
                    hardfork_override: old_hardfork_override,
                }),
                Self::Directory(Directory {
                    enabled: new_enabled,
                    entries: new_entities,
                    comment: new_comment,
                    skip_calldatas: new_skip_calldatas,
                    skip_cases: new_skip_cases,
                    skip_names: new_skip_names,
                    hardfork_override: new_hardfork_override,
                }),
            ) => {
                *new_enabled = *old_enabled;
                *new_comment = old_comment.clone();
                *new_skip_calldatas = old_skip_calldatas.clone();
                *new_skip_cases = old_skip_cases.clone();
                *new_skip_names = old_skip_names.clone();
                *new_hardfork_override = old_hardfork_override.clone();

                (old_entities, new_entities)
            }
            (_, new) => {
                self.list_recursive(current, &mut changes.deleted);
                new.list_recursive(current, &mut changes.created);
                if !index_only {
                    self.delete(current)?;
                    new.create_recursive(current)?;
                }
                return Ok(());
            }
        };

        for (name, entity) in old_entities.iter() {
            let mut current = current.to_owned();
            current.push(name);
            if let Some(new_entity) = new_entities.get_mut(name) {
                entity.update_recursive(new_entity, &current, changes, index_only)?;
            } else {
                entity.list_recursive(&current, &mut changes.deleted);
                if !index_only {
                    entity.delete(&current)?;
                }
            }
        }
        for (name, entity) in new_entities.iter() {
            if !old_entities.contains_key(name) {
                let mut current = current.to_owned();
                current.push(name);
                entity.list_recursive(&current, &mut changes.created);
                if !index_only {
                    entity.create_recursive(&current)?;
                }
            }
        }

        Ok(())
    }

    ///
    /// Inner enabled accumulator function.
    ///
    fn into_enabled_list_recursive(
        self,
        current: &Path,
        accumulator: &mut Vec<EnabledTest>,
        skipped_calldatas: &Vec<Bytes>,
        skipped_cases: &Vec<String>,
        skipped_names: &Vec<String>,
        hardfork_override: &Option<String>,
    ) {
        let mut skipped_calldatas_new = skipped_calldatas.clone();
        let mut skipped_cases_new = skipped_cases.clone();
        let mut skipped_names_new = skipped_names.clone();
        let mut hardfork_override = hardfork_override.clone();

        let entries = match self {
            Self::File(file) => {
                if !file.enabled {
                    return;
                }

                if let Some(additional_skipped_calldatas) = file.skip_calldatas {
                    skipped_calldatas_new.extend(additional_skipped_calldatas);
                }

                if let Some(additional_skipped_cases) = file.skip_cases {
                    skipped_cases_new.extend(additional_skipped_cases);
                }

                if let Some(additional_skipped_names) = file.skip_names {
                    skipped_names_new.extend(additional_skipped_names);
                }
                if let Some(hardfork) = file.hardfork_override {
                    hardfork_override = Some(hardfork);
                }

                accumulator.push(EnabledTest::new(
                    current.to_owned(),
                    Some(skipped_calldatas_new),
                    Some(skipped_cases_new),
                    Some(skipped_names_new),
                    hardfork_override,
                ));
                return;
            }
            Self::Directory(directory) => {
                if !directory.enabled {
                    return;
                }

                if let Some(additional_skipped_calldatas) = directory.skip_calldatas {
                    skipped_calldatas_new.extend(additional_skipped_calldatas);
                }

                if let Some(additional_skipped_cases) = directory.skip_cases {
                    skipped_cases_new.extend(additional_skipped_cases);
                }

                if let Some(hardfork) = directory.hardfork_override {
                    hardfork_override = Some(hardfork);
                }

                directory.entries
            }
        };

        for (name, entity) in entries.into_iter() {
            let mut current = current.to_owned();
            current.push(name);
            entity.into_enabled_list_recursive(
                &current,
                accumulator,
                &skipped_calldatas_new,
                &skipped_cases_new,
                &skipped_names_new,
                &hardfork_override,
            );
        }
    }

    ///
    /// Creates files and folders from self.
    ///
    fn create_recursive(&self, current: &Path) -> anyhow::Result<()> {
        let entries = match self {
            Self::Directory(directory) => &directory.entries,
            Self::File(test) => {
                let mut file = File::create(current)
                    .map_err(|err| anyhow::anyhow!("Failed to create file: {}", err))?;
                file.write_all(
                    test.data
                        .as_ref()
                        .ok_or_else(|| anyhow::anyhow!("Test data is None: {:?}", current))?
                        .as_bytes(),
                )?;
                return Ok(());
            }
        };

        fs::create_dir(current)
            .map_err(|err| anyhow::anyhow!("Failed to create directory: {}", err))?;

        for (name, entity) in entries.iter() {
            let mut current = current.to_owned();
            current.push(name);
            entity.create_recursive(&current)?;
        }

        Ok(())
    }

    ///
    /// Deletes files and folders from self.
    ///
    fn delete(&self, current: &Path) -> anyhow::Result<()> {
        if let Self::Directory(_) = self {
            fs::remove_dir_all(current).map_err(|err| {
                anyhow::anyhow!("Failed to delete directory {:?}: {}", current, err)
            })?;
        } else {
            fs::remove_file(current)
                .map_err(|err| anyhow::anyhow!("Failed to delete file {:?}: {}", current, err))?;
        }

        Ok(())
    }

    ///
    /// Inner accumulator function.
    ///
    fn list_recursive(&self, current: &Path, accumulator: &mut Vec<PathBuf>) {
        let entries = match self {
            Self::Directory(directory) => &directory.entries,
            Self::File(_) => {
                accumulator.push(current.to_owned());
                return;
            }
        };

        for (name, entity) in entries.iter() {
            let mut current = current.to_owned();
            current.push(name);
            entity.list_recursive(&current, accumulator);
        }
    }
}
