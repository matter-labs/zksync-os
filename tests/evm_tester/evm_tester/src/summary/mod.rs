//!
//! The evm tester summary.
//!

pub mod element;

use std::sync::Arc;
use std::sync::Mutex;

use colored::Colorize;

use self::element::outcome::passed_variant::PassedVariant;
use self::element::outcome::Outcome;
use self::element::Element;
use alloy::primitives::*;

///
/// The evm tester summary.
///
#[derive(Debug)]
pub struct Summary {
    /// The summary elements.
    elements: Vec<Element>,
    /// The output verbosity.
    verbosity: bool,
    /// Whether the output is suppressed.
    quiet: bool,
    /// The passed tests counter.
    passed: usize,
    /// The failed tests counter.
    failed: usize,
    /// The invalid tests counter.
    invalid: usize,
    /// The ignored tests counter.
    ignored: usize,
    /// The panicked tests counter.
    panicked: usize,
}

impl Summary {
    /// The elements vector default capacity.
    pub const ELEMENTS_INITIAL_CAPACITY: usize = 1024 * 4096;

    ///
    /// A shortcut constructor.
    ///
    pub fn new(verbosity: bool, quiet: bool) -> Self {
        Self {
            elements: Vec::with_capacity(Self::ELEMENTS_INITIAL_CAPACITY),
            verbosity,
            quiet,
            passed: 0,
            failed: 0,
            invalid: 0,
            ignored: 0,
            panicked: 0,
        }
    }

    ///
    /// Whether the test run has been successful.
    ///
    pub fn is_successful(&self) -> bool {
        for element in self.elements.iter() {
            match element.outcome {
                Outcome::Passed { .. } => continue,
                Outcome::Failed { .. } => return false,
                Outcome::Invalid { .. } => return false,
                Outcome::Panicked { .. } => return false,
                Outcome::Ignored => continue,
            }
        }

        true
    }

    ///
    /// Wraps data into a thread-safe shared reference.
    ///
    pub fn wrap(self) -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(self))
    }

    ///
    /// Extracts the data from the thread-safe shared reference.
    ///
    pub fn unwrap_arc(summary: Arc<Mutex<Self>>) -> Self {
        Arc::try_unwrap(summary)
            .expect("Last shared reference")
            .into_inner()
            .expect("Last shared reference")
    }

    ///
    /// Adds a passed outcome of a deploy call.
    ///
    pub fn passed_deploy(
        summary: Arc<Mutex<Self>>,
        name: String,
        size: usize,
        cycles: usize,
        ergs: u64,
        gas: u64,
    ) {
        let passed_variant = PassedVariant::Deploy {
            size,
            cycles,
            ergs,
            _gas: gas,
        };
        Self::passed(summary, name, passed_variant);
    }

    ///
    /// Adds a passed outcome of an ordinary call.
    ///
    pub fn passed_runtime(summary: Arc<Mutex<Self>>, name: String) {
        let passed_variant = PassedVariant::Runtime;
        Self::passed(summary, name, passed_variant);
    }

    ///
    /// Adds a passed outcome of a special call, like `storageEmpty` or `balance`.
    ///
    pub fn passed_special(summary: Arc<Mutex<Self>>, name: String) {
        let passed_variant = PassedVariant::Special;
        Self::passed(summary, name, passed_variant);
    }

    ///
    /// Adds a failed outcome.
    ///
    pub fn failed(
        summary: Arc<Mutex<Self>>,
        name: String,
        // exception: bool,
        expected: Option<String>,
        actual: Option<String>,
    ) {
        let element = Element::new(name, Outcome::failed(expected, actual));
        summary.lock().expect("Sync").push_element(element);
    }

    ///
    /// Adds an invalid outcome.
    ///
    pub fn invalid<S>(summary: Arc<Mutex<Self>>, name: String, error: S)
    where
        S: ToString,
    {
        let element = Element::new(name, Outcome::invalid(error));
        summary.lock().expect("Sync").push_element(element);
    }

    ///
    /// Adds a panicked outcome.
    ///
    pub fn panicked<S>(summary: Arc<Mutex<Self>>, name: String, error: S)
    where
        S: ToString,
    {
        let element = Element::new(name, Outcome::panicked(error));
        summary.lock().expect("Sync").push_element(element);
    }

    ///
    /// Adds an ignored outcome.
    ///
    pub fn ignored(summary: Arc<Mutex<Self>>, name: String) {
        let element = Element::new(name, Outcome::ignored());
        summary.lock().expect("Sync").push_element(element);
    }

    ///
    /// The unified function for passed outcomes.
    ///
    fn passed(summary: Arc<Mutex<Self>>, name: String, passed_variant: PassedVariant) {
        let element = Element::new(name, Outcome::passed(passed_variant));
        summary.lock().expect("Sync").push_element(element);
    }

    ///
    /// Pushes an element to the summary, printing it.
    ///
    fn push_element(&mut self, element: Element) {
        if let Some(string) = element.print(self.verbosity) {
            println!("{string}");
        }

        let is_executed = match element.outcome {
            Outcome::Passed { .. } => {
                self.passed += 1;
                true
            }
            Outcome::Failed { .. } => {
                self.failed += 1;
                true
            }
            Outcome::Invalid { .. } => {
                self.invalid += 1;
                true
            }
            Outcome::Panicked { .. } => {
                self.panicked += 1;
                true
            }
            Outcome::Ignored => {
                self.ignored += 1;
                false
            }
        };

        if is_executed {
            let milestone = if self.verbosity {
                usize::pow(10, 3)
            } else {
                usize::pow(10, 5)
            };

            if (self.passed + self.failed + self.invalid) % milestone == 0 {
                println!("{self}");
            }
        }

        self.elements.push(element);
    }
}

impl std::fmt::Display for Summary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.quiet {
            return Ok(());
        }

        writeln!(
            f,
            "╔═══════════════════╡ INTEGRATION TESTING ╞════════════════════╗"
        )?;
        writeln!(
            f,
            "║                                                              ║"
        )?;
        writeln!(
            f,
            "║     {:8}                                  {:10}     ║",
            "PASSED".green(),
            self.passed.to_string().green(),
        )?;
        writeln!(
            f,
            "║     {:8}                                  {:10}     ║",
            "FAILED".bright_red(),
            self.failed.to_string().bright_red(),
        )?;
        writeln!(
            f,
            "║     {:8}                                  {:10}     ║",
            "PANICKED".bright_magenta(),
            self.panicked.to_string().bright_magenta(),
        )?;
        writeln!(
            f,
            "║     {:8}                                  {:10}     ║",
            "INVALID".red(),
            self.invalid.to_string().red(),
        )?;
        writeln!(
            f,
            "║     {:8}                                  {:10}     ║",
            "IGNORED".bright_black(),
            self.ignored.to_string().bright_black(),
        )?;
        writeln!(
            f,
            "║               {:10} TESTS MILESTONE                     ║",
            self.passed + self.failed + self.invalid,
        )?;
        writeln!(
            f,
            "╚══════════════════════════════════════════════════════════════╝"
        )?;

        Ok(())
    }
}
