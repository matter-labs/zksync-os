//!
//! The evm tester library.
//!

#![feature(allocator_api)]
#![allow(non_camel_case_types)]
#![allow(clippy::upper_case_acronyms)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::type_complexity)]
#![allow(incomplete_features)]
#![feature(generic_const_exprs)]

pub mod constants;
pub(crate) mod environment;
pub(crate) mod filters;
pub(crate) mod summary;
pub(crate) mod test;
pub(crate) mod test_suits;
pub mod utils;
pub(crate) mod vm;
pub(crate) mod workflow;

use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;

use rayon::iter::IntoParallelIterator;
use rayon::iter::ParallelIterator;
use test::Test;

use crate::constants::*;
pub use crate::environment::Environment;
pub use crate::filters::Filters;
pub use crate::summary::Summary;
pub use crate::vm::zk_ee::ZKsyncOS;
pub use crate::workflow::Workflow;

///
/// The evm tester.
///
pub struct EvmTester {
    /// The summary.
    pub summary: Arc<Mutex<Summary>>,
    /// The filters.
    pub filters: Filters,
    /// Actions to perform.
    pub workflow: Workflow,
    /// Optional path to the mutated tests directory
    pub mutation_path: Option<String>,
    pub run_spec_tests: bool,
}

impl EvmTester {
    ///
    /// A shortcut constructor.
    ///
    pub fn new(
        summary: Arc<Mutex<Summary>>,
        filters: Filters,
        workflow: Workflow,
        mutation_path: Option<String>,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            summary,
            filters,
            workflow,
            mutation_path,
            run_spec_tests: true,
        })
    }

    ///
    /// Runs all tests on ZKsync OS.
    ///
    pub fn run_zksync_os(self, run_mutation_tests: bool) -> anyhow::Result<()> {
        let tests = self.all_tests(Environment::ZKsyncOS)?;
        let _: Vec<()> = tests
            .into_par_iter()
            .map(|mut test| {
                let mutants = test.mutants;
                test.mutants = vec![];

                test.run_zksync_os(
                    self.summary.clone(),
                    matches!(self.workflow, Workflow::Bench),
                );

                if run_mutation_tests {
                    for mutant in mutants {
                        mutant.run_zksync_os(
                            self.summary.clone(),
                            matches!(self.workflow, Workflow::Bench),
                        );
                    }
                }
            })
            .collect();

        Ok(())
    }

    ///
    /// Returns all tests from all directories.
    ///
    fn all_tests(&self, environment: Environment) -> anyhow::Result<Vec<Test>> {
        let mut tests = Vec::with_capacity(16384);

        tests.extend(self.directory(
            DEVELOP_STATE_TESTS,
            environment,
            DEVELOP_STATE_TESTS_INDEX_PATH,
        )?);

        tests.extend(self.directory(
            STABLE_STATE_TESTS,
            environment,
            STABLE_STATE_TESTS_INDEX_PATH,
        )?);

        tests.extend(self.directory(
            DEVELOP_BLOCKCHAIN_TESTS,
            environment,
            DEVELOP_BLOCKCHAIN_TESTS_INDEX_PATH,
        )?);

        tests.extend(self.directory(
            STABLE_BLOCKCHAIN_TESTS,
            environment,
            STABLE_BLOCKCHAIN_TESTS_INDEX_PATH,
        )?);

        Ok(tests)
    }

    ///
    /// Returns all tests from the specified directory.
    ///
    fn directory(
        &self,
        path: &str,
        environment: Environment,
        index_path: &str,
    ) -> anyhow::Result<Vec<Test>>
where {
        crate::test_suits::read_all(
            Path::new(path),
            &self.filters,
            environment,
            self.mutation_path.clone(),
            Path::new(index_path),
        )
        .map_err(|error| anyhow::anyhow!("Failed to read the tests directory `{path}`: {error}"))
    }
}
