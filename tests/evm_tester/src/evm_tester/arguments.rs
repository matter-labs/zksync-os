//!
//! The evm tester arguments.
//!

use structopt::StructOpt;

///
/// The evm tester arguments.
///
#[derive(Debug, StructOpt)]
#[structopt(name = "evm-tester", about = "EVM Implementations Testing Framework")]
pub struct Arguments {
    /// The logging level.
    #[structopt(short = "v", long = "verbose")]
    pub verbosity: bool,

    /// Suppresses the output completely.
    #[structopt(short = "q", long = "quiet")]
    pub quiet: bool,

    /// Runs only tests whose name contains any string from the specified ones.
    #[structopt(short = "p", long = "path")]
    pub paths: Vec<String>,

    /// Runs only tests with specified names.
    #[structopt(short = "n", long = "name")]
    pub names: Vec<String>,

    /// Runs only tests with specified hashes.
    #[structopt(short = "h", long = "hash")]
    pub hashes: Vec<String>,

    /// Runs only tests with the specified labels.
    #[structopt(short = "l", long = "label")]
    pub labels: Vec<String>,

    /// Sets the number of threads, which execute the tests concurrently.
    #[structopt(short = "t", long = "threads")]
    pub threads: Option<usize>,

    /// Choose between `build` to compile tests only without running, `run` to compile and run
    /// or `bench` to also produce flamegraphs.
    /// Note that you might want to set the ZKSYNC_OS_DIR env var to point to the directory
    /// containing the app.elf and app.bin from ZKsync OS to run benchmarks.
    #[structopt(long = "workflow", default_value = "run")]
    pub workflow: evm_tester::Workflow,

    /// Will run generated mutation tests for test cases.
    #[structopt(short = "m", long = "mutation")]
    pub mutation: bool,

    /// The path to the mutated tests directory
    #[structopt(long = "mutation_path")]
    pub mutation_path: Option<String>,
}

impl Arguments {
    ///
    /// A shortcut constructor.
    ///
    pub fn new() -> Self {
        Self::from_args()
    }
}
