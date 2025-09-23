//!
//! The test.
//!

pub mod case;
pub mod filler_structure;
pub mod test_structure;

use std::collections::HashMap;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::Mutex;

use filler_structure::FillerStructure;
use lazy_static::lazy_static;
use regex::Regex;
use test_structure::TestStructure;

use crate::summary::Summary;
use crate::test::case::Case;
use crate::Filters;
use crate::ZKsyncOS;
use alloy::primitives::*;

lazy_static! {
    static ref MUTATION_TESTS_RE: Regex = Regex::new(r"^(.+)_m_[0-9a-fA-F]+\.json").unwrap();
}

fn wrap_numbers_in_quotes(input: &str) -> String {
    // Match numbers not already inside quotes
    //let re = Regex::new(r#": "?\b(\d+)\b"?"#).unwrap();
    //let res1 = re.replace_all(input, ": \"$1\"").to_string();

    //let re2 = Regex::new(r#""?\b(\d+)\b"?:"#).unwrap();
    //let res2 = re2.replace_all(&res1, "\"$1\":").to_string();

    let re3 = Regex::new(r#"\s((0x)?[0-9a-fA-F]{2,}):"#).unwrap();
    let res3 = re3.replace_all(input, " \"$1\":").to_string();

    let re4 = Regex::new(r#": ((0x)?[0-9a-fA-F]{2,})\b"#).unwrap();
    re4.replace_all(&res3, ": \"$1\"").to_string()
}

///
/// The test.
///
#[derive(Debug)]
pub struct Test {
    /// The test name.
    pub name: String,
    /// The test cases.
    pub cases: Vec<Case>,
    /// The EVM version.
    // evm_version: Option<EVMVersion>,
    skipped_calldatas: Option<Vec<Bytes>>,
    skipped_cases: Option<Vec<String>>,
    pub path: PathBuf,
    pub mutants: Vec<Test>,
}

impl Test {
    ///
    /// A shortcut constructor.
    ///
    pub fn new(
        name: String,
        cases: Vec<Case>,
        skipped_calldatas: Option<Vec<Bytes>>,
        skipped_cases: Option<Vec<String>>,
        path: PathBuf,
        mutants: Vec<Test>,
    ) -> Self {
        Self {
            name,
            cases,
            skipped_calldatas,
            skipped_cases,
            path,
            mutants,
        }
    }

    // TODO: reimplement using ethereum spec tests (prefill expected state)
    pub fn from_ethereum_test(
        str: &str,
        filler_str: &str,
        is_json: bool,
        skipped_calldatas: Option<Vec<Bytes>>,
        skipped_cases: Option<Vec<String>>,
        filters: &Filters,
        path: PathBuf,
        relative_path: PathBuf,
        mutation_path: Option<String>,
        name_override: Option<String>,
    ) -> Self {
        let cleaned_str = str.replace("0x:bigint ", "");
        let test_structure: HashMap<String, TestStructure> =
            serde_json::from_str(&cleaned_str).unwrap();

        let keys: Vec<_> = test_structure.keys().collect();
        let test_name = keys[0];

        let test_filler_structure: HashMap<String, FillerStructure> = if is_json {
            serde_json::from_str(filler_str).unwrap()
        } else {
            let wrapped_numbers = wrap_numbers_in_quotes(filler_str);
            //fs::write("out.yaml", wrapped_numbers.clone());
            serde_yaml::from_str(&wrapped_numbers).unwrap()
        };

        let test_definition = test_structure.get(keys[0]).expect("Always exists");
        let test_filler = test_filler_structure.get(keys[0]).expect("Always exists");

        let cases = if filters.check_test_name(&test_name) {
            Case::from_ethereum_test(test_definition, test_filler, filters)
        } else {
            vec![]
        };

        // read mutants
        // filter all files in directory by regexp and run
        let test_path = path.clone();
        let mut directory = test_path.clone();
        directory.pop();

        let base_test_name = test_path.file_stem().unwrap().to_str().unwrap();

        let mut mutation_tests_directory = directory;

        if let Some(mutation_path) = mutation_path.as_ref() {
            let base_directory_path = PathBuf::from_str(&mutation_path).unwrap();

            mutation_tests_directory = base_directory_path.join(relative_path.clone());
            mutation_tests_directory.pop();
        }

        // read all mutation tests
        let files: Vec<_> = std::fs::read_dir(mutation_tests_directory)
            .unwrap()
            .map(|x| x.unwrap())
            .filter(|x| {
                let filename = x.file_name();
                let filename = filename.to_str().unwrap();
                if MUTATION_TESTS_RE.is_match(&filename) {
                    let base_name = MUTATION_TESTS_RE
                        .captures(&filename)
                        .unwrap()
                        .get(1)
                        .unwrap()
                        .as_str();
                    if base_name == base_test_name {
                        return true;
                    }
                }
                false
            })
            .collect();

        let mutants: Vec<_> = files
            .into_iter()
            .map(|file| {
                let test_str = std::fs::read_to_string(file.path()).unwrap();
                Test::from_ethereum_test(
                    &test_str,
                    filler_str,
                    is_json,
                    skipped_calldatas.clone(),
                    skipped_cases.clone(),
                    filters,
                    file.path(),
                    relative_path.clone(),
                    mutation_path.clone(),
                    Some(
                        file.path()
                            .file_stem()
                            .unwrap()
                            .to_str()
                            .unwrap()
                            .to_string(),
                    ),
                )
            })
            .collect();

        let name = if let Some(name) = name_override {
            name
        } else {
            test_name.clone()
        };

        Self {
            name,
            cases,
            skipped_calldatas,
            skipped_cases,
            path,
            mutants,
        }
    }

    pub fn from_ethereum_spec_test(
        str: &str,
        skipped_calldatas: Option<Vec<Bytes>>,
        skipped_cases: Option<Vec<String>>,
        skipped_names: Option<Vec<String>>,
        filters: &Filters,
        path: PathBuf,
        _relative_path: PathBuf,
        _mutation_path: Option<String>,
        name_override: Option<String>,
        hardfork_override: Option<String>,
    ) -> Vec<Self> {
        let cleaned_str = str.replace("0x:bigint ", "");
        let test_structure: HashMap<String, TestStructure> =
            serde_json::from_str(&cleaned_str).unwrap();

        let mut tests = vec![];

        let hardfork = hardfork_override.unwrap_or("Cancun".to_string());

        for (test_name, test_definition) in test_structure {
            if !filters.check_test_name(&test_name) {
                continue;
            }

            if let Some(skipped_names_refs) = skipped_names.as_ref() {
                if skipped_names_refs.contains(&test_name) {
                    continue;
                }
            }

            let cases = Case::from_ethereum_spec_test(&test_definition, filters, &hardfork);

            // read mutants
            // filter all files in directory by regexp and run
            let test_path = path.clone();
            let mut directory = test_path.clone();
            directory.pop();

            let name = if let Some(name) = name_override.as_ref() {
                name.clone()
            } else {
                test_name.clone()
            };

            // TODO mutantion not supported here

            tests.push(Self {
                name,
                cases,
                skipped_calldatas: skipped_calldatas.clone(), // TODO not convenient
                skipped_cases: skipped_cases.clone(),         // TODO not convenient
                path: path.clone(),
                mutants: vec![],
            });
        }

        tests
    }

    ///
    /// Runs the test on ZKsync OS.
    ///
    pub fn run_zksync_os(self, summary: Arc<Mutex<Summary>>, bench: bool) {
        for case in self.cases {
            if let Some(filter_calldata) = self.skipped_calldatas.as_ref() {
                if filter_calldata.contains(
                    &case
                        .pre_blocks
                        .get(0)
                        .unwrap()
                        .transactions
                        .get(0)
                        .unwrap()
                        .common()
                        .data,
                ) {
                    Summary::ignored(summary.clone(), case.label);
                    continue;
                }
            }

            if let Some(filter_cases) = self.skipped_cases.as_ref() {
                if filter_cases.contains(&case.label) {
                    Summary::ignored(summary.clone(), case.label);
                    continue;
                }
            }

            let vm = ZKsyncOS::new();
            case.run_zksync_os(summary.clone(), vm, self.name.clone(), bench);
        }
    }
}
