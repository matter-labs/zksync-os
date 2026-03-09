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
    cli_hardfork: Option<String>,
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

            // CLI hardfork takes precedence over per-test/per-directory overrides.
            // The hardfork is considered "overridden" when the final hardfork
            // differs from what the STF natively supports — i.e., either the CLI
            // or the index forces a hardfork the compiled STF doesn't match.
            // The only exception is when the CLI explicitly sets the same hardfork
            // that the test already targets — that's not an override.
            let hardfork_was_overridden = match (&cli_hardfork, &test.hardfork_override) {
                (Some(cli), Some(test_hf)) => cli != test_hf,
                (Some(_), None) | (None, Some(_)) => true,
                (None, None) => false,
            };
            let hardfork_override = cli_hardfork.clone().or(test.hardfork_override);

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
                hardfork_override,
                hardfork_was_overridden,
            ))
        })
        .flatten()
        .collect())
}
