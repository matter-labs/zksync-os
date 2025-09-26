//!
//! Helpers to read test suite.
//!

pub mod index;

use crate::filters::Filters;
use crate::test::Test;
use crate::utils::create_index;
use crate::utils::read_index;
use crate::Environment;
use std::path::Path;
use std::path::PathBuf;

pub fn read_all(
    directory_path: &Path,
    filters: &Filters,
    environment: Environment,
    mutation_path: Option<String>,
    index_path: &Path,
) -> anyhow::Result<Vec<Test>> {
    let mut index_maybe = read_index(index_path);

    if index_maybe.is_err() {
        create_index(&index_path, directory_path)?;
        index_maybe = read_index(index_path);
        assert!(index_maybe.is_ok());
    }

    //update_index(index_path, directory_path)?;

    Ok(index_maybe?
        .into_enabled_list(directory_path)
        .into_iter()
        .filter_map(|test| {
            let identifier = test.path.to_string_lossy().to_string();

            if !filters.check_case_path(&identifier) {
                return None;
            }

            let file = std::fs::read_to_string(test.path.clone())
                .unwrap_or_else(|_| panic!("Test not found: {:?}", test.path));

            let dir_name = directory_path.file_name().unwrap();
            let relative_path: PathBuf = test
                .path
                .iter() // iterate over path components
                .skip_while(|s| *s != dir_name)
                .skip(1)
                .collect();

            Some(Test::from_ethereum_spec_test(
                &file,
                test.skip_calldatas,
                test.skip_cases,
                test.skip_names,
                filters,
                test.path,
                relative_path,
                mutation_path.clone(),
                None,
                test.hardfork_override,
            ))
        })
        .flatten()
        .collect())
}
