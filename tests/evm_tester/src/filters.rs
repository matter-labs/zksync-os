//!
//! The evm tester filters.
//!

use std::collections::HashSet;

///
/// The evm tester filters.
///
#[derive(Debug)]
pub struct Filters {
    /// The path filters.
    path_filters: HashSet<String>,
    /// The label filters.
    label_filters: HashSet<String>,
    /// The name filters.
    name_filters: HashSet<String>,
    /// Hash filters.
    hash_filters: HashSet<String>,
}

impl Filters {
    ///
    /// A shortcut constructor.
    ///
    pub fn new(
        path_filters: Vec<String>,
        label_filters: Vec<String>,
        name_filters: Vec<String>,
        hash_filters: Vec<String>,
    ) -> Self {
        Self {
            path_filters: path_filters.into_iter().collect(),
            label_filters: label_filters.into_iter().collect(),
            name_filters: name_filters.into_iter().collect(),
            hash_filters: hash_filters.into_iter().collect(),
        }
    }

    ///
    /// Check if the test path is compatible with the filters.
    ///
    pub fn check_test_path(&self, path: &str) -> bool {
        if self.path_filters.is_empty() {
            return true;
        }

        self.path_filters
            .iter()
            .any(|filter| path.contains(&filter[..filter.find("::").unwrap_or(filter.len())]))
    }

    ///
    /// Check if the test case path is compatible with the filters.
    ///
    pub fn check_case_path(&self, path: &str) -> bool {
        self.path_filters.is_empty() || self.path_filters.iter().any(|filter| path.contains(filter))
    }

    ///
    /// Check if the test case label is compatible with the filters.
    ///
    pub fn check_case_label(&self, label: &str) -> bool {
        self.label_filters.is_empty() || self.label_filters.contains(label)
    }

    ///
    /// Check if the test case label is compatible with the filters.
    ///
    pub fn check_test_name(&self, name: &str) -> bool {
        self.name_filters.is_empty() || self.name_filters.contains(name)
    }

    ///
    /// Check if the test case hash is compatible with the filters.
    ///
    pub fn check_case_hash(&self, hash: &str) -> bool {
        self.hash_filters.is_empty() || self.hash_filters.contains(hash)
    }
}
