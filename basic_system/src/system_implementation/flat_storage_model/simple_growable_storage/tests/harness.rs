use super::*;

thread_local! {
    /// Whether the hardening checks run, see `hardening_enabled`
    pub(in super::super) static HARDENING: Cell<bool> = const { Cell::new(true) };
    static COUNTERS: RefCell<[u64; NUM_COUNTERS]> = const { RefCell::new([0; NUM_COUNTERS]) };
}

/// Events of the implementation that the tests count.
#[derive(Clone, Copy, Debug)]
pub(in super::super) enum Counter {
    /// The relink found a leaf opened at the index of a new write
    NewWriteAliasResolved,
    /// The fold found a path opened at the index of a new write
    NewWriteAliasPath,
    /// Two keys of `KeyIndex::overflow` were compared
    OverflowKeyCmp,
}

pub(super) const NUM_COUNTERS: usize = 3;

pub(in super::super) fn count(counter: Counter) {
    COUNTERS.with_borrow_mut(|counters| counters[counter as usize] += 1);
}

/// The counts since the last call.
pub(super) fn take_counts() -> [u64; NUM_COUNTERS] {
    COUNTERS.replace([0; NUM_COUNTERS])
}

/// The input of a query, as the oracle read it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Input {
    Key(Bytes32),
    Index(u64),
}

/// A query as the oracle answered it: the query ID, the input and the response words.
pub(super) type LoggedQuery = (u32, Input, Vec<u32>);

/// Changes the response to a query, given its position in the log, the query ID and the input.
pub(super) type Tamper = Box<dyn FnMut(usize, u32, &Input, &mut Vec<u32>)>;

/// Supplies a response before the honest tree is queried, including for malformed sources.
pub(super) type Script = Box<dyn FnMut(usize, u32, &Input) -> Option<Vec<u32>>>;

/// Answers the tree queries from a snapshot of a tree, and logs them. A tamper may change the
/// responses, including their length.
pub(super) struct TreeOracle<const R: bool> {
    tree: TestingTree<R>,
    response: ResponseBuffer,
    pub(super) log: Vec<LoggedQuery>,
    tamper: Option<Tamper>,
    script: Option<Script>,
    fixture_failed: bool,
    changed_responses: usize,
    scripted_responses: usize,
}

impl<const R: bool> TreeOracle<R> {
    pub(super) fn new(tree: TestingTree<R>, tamper: Option<Tamper>) -> Self {
        Self {
            tree,
            response: ResponseBuffer::default(),
            log: Vec::new(),
            tamper,
            script: None,
            fixture_failed: false,
            changed_responses: 0,
            scripted_responses: 0,
        }
    }

    fn with_script(tree: TestingTree<R>, tamper: Option<Tamper>, script: Option<Script>) -> Self {
        let mut oracle = Self::new(tree, tamper);
        oracle.script = script;
        oracle
    }
}

impl<const R: bool> MemoryOracle for TreeOracle<R> {
    fn send_query(&mut self, query_id: u32, input_word: usize) -> Result<(), InternalError> {
        let result = catch_unwind(AssertUnwindSafe(|| {
            // SAFETY: the test sends the queries from this process
            let memory = unsafe { NativeQuerierMemory::new() };
            let input = match query_id {
                ExactIndexQuery::QUERY_ID | PreviousIndexQuery::QUERY_ID => {
                    Input::Key(Bytes32::read_input(&memory, input_word)?)
                }
                PROOF_FOR_INDEX_QUERY_ID => Input::Index(u64::read_input(&memory, input_word)?),
                _ => panic!("unsupported query type 0x{query_id:08x}"),
            };
            let scripted = self
                .script
                .as_mut()
                .and_then(|script| script(self.log.len(), query_id, &input));
            let mut response = if let Some(response) = scripted {
                self.scripted_responses += 1;
                response
            } else {
                let mut response = vec![];
                match (query_id, input) {
                    (ExactIndexQuery::QUERY_ID, Input::Key(key)) => self
                        .tree
                        .get_index_for_existing(&key)
                        .write_output(&mut response),
                    (PreviousIndexQuery::QUERY_ID, Input::Key(key)) => {
                        self.tree.get_prev_index(&key).write_output(&mut response)
                    }
                    (PROOF_FOR_INDEX_QUERY_ID, Input::Index(index)) => {
                        write_proof_for_index_response(
                            &self.tree.get_proof_for_position(index),
                            &mut response,
                        )
                    }
                    _ => unreachable!("query input decoded above"),
                }
                response
            };
            let original_response = response.clone();
            if let Some(tamper) = self.tamper.as_mut() {
                tamper(self.log.len(), query_id, &input, &mut response);
            }
            self.changed_responses += usize::from(response != original_response);
            self.log.push((query_id, input, response.clone()));
            self.response.set(response)
        }));
        match result {
            Ok(Ok(())) => Ok(()),
            Ok(Err(error)) => {
                self.fixture_failed = true;
                Err(error)
            }
            Err(error) => {
                self.fixture_failed = true;
                std::panic::resume_unwind(error)
            }
        }
    }

    zk_ee::memory_oracle_response_methods!(response);
}

impl<const R: bool> IOOracle for TreeOracle<R> {
    type RawIterator<'a> = core::iter::Empty<usize>;

    fn raw_query<'a, I: UsizeSerializable + UsizeDeserializable>(
        &'a mut self,
        query_type: u32,
        _input: &I,
    ) -> Result<Self::RawIterator<'a>, InternalError> {
        self.fixture_failed = true;
        panic!("unsupported query type 0x{query_type:08x}")
    }
}

// Response layout of `ProofForIndexQuery`, in words: the index, the leaf (key, value, next) and
// the path
pub(super) const PROOF_KEY: usize = 2;

pub(super) const PROOF_VALUE: usize = 10;

pub(super) const PROOF_NEXT: usize = 18;

pub(super) const PROOF_PATH: usize = 20;

pub(super) fn u64_words(value: u64) -> Vec<u32> {
    let mut words = vec![];
    value.write_output(&mut words);
    words
}

pub(super) fn leaf_words(leaf: &FlatStorageLeaf<TESTING_TREE_HEIGHT>) -> Vec<u32> {
    let mut words = vec![];
    leaf.write_output(&mut words);
    words
}

pub(super) fn bytes32_of_words(words: &[u32]) -> Bytes32 {
    let mut bytes = [0u8; 32];
    for (dst, word) in bytes.as_chunks_mut::<4>().0.iter_mut().zip(words) {
        dst.copy_from_slice(&word.to_le_bytes());
    }
    Bytes32::from_array(bytes)
}

/// The leaf of a proof response.
pub(super) fn leaf_of_response(response: &[u32]) -> FlatStorageLeaf<TESTING_TREE_HEIGHT> {
    FlatStorageLeaf {
        key: bytes32_of_words(&response[PROOF_KEY..PROOF_VALUE]),
        value: bytes32_of_words(&response[PROOF_VALUE..PROOF_NEXT]),
        next: u64::from(response[PROOF_NEXT]) | (u64::from(response[PROOF_NEXT + 1]) << 32),
    }
}

/// Replaces the leaf of a proof response.
pub(super) fn forge_leaf(response: &mut [u32], leaf: &FlatStorageLeaf<TESTING_TREE_HEIGHT>) {
    response[PROOF_KEY..PROOF_PATH].copy_from_slice(&leaf_words(leaf));
}

/// A tamper factory from a function of the query's position, ID and input.
pub(super) fn tamper(
    f: impl Fn(usize, u32, &Input, &mut Vec<u32>) + Clone + 'static,
) -> impl Fn() -> Option<Tamper> {
    move || Some(Box::new(f.clone()) as Tamper)
}

pub(super) fn script(
    f: impl Fn(usize, u32, &Input) -> Option<Vec<u32>> + Clone + 'static,
) -> impl Fn() -> Option<Script> {
    move || Some(Box::new(f.clone()) as Script)
}

pub(super) fn no_tamper() -> Option<Tamper> {
    None
}

/// How a run ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Outcome {
    Accepted { root: Bytes32, next_free_slot: u64 },
    Failed(String),
    Panicked,
}

pub(super) struct Run {
    pub(super) outcome: Outcome,
    pub(super) log: Vec<LoggedQuery>,
    pub(super) counts: [u64; NUM_COUNTERS],
    has_tamper: bool,
    has_script: bool,
    changed_responses: usize,
    scripted_responses: usize,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum Implementation {
    Reference,
    New { hardening: bool },
}

pub(super) fn run<const R: bool>(
    implementation: Implementation,
    tree: &TestingTree<R>,
    entries: &[(WarmStorageKey, WarmStorageValue)],
    tamper: Option<Tamper>,
) -> Run {
    run_scripted(implementation, tree, entries, tamper, None)
}

pub(super) fn run_scripted<const R: bool>(
    implementation: Implementation,
    tree: &TestingTree<R>,
    entries: &[(WarmStorageKey, WarmStorageValue)],
    tamper: Option<Tamper>,
    script: Option<Script>,
) -> Run {
    let has_tamper = tamper.is_some();
    let has_script = script.is_some();
    let mut oracle = TreeOracle::with_script(tree.clone(), tamper, script);
    let mut commitment = FlatStorageCommitment::<TESTING_TREE_HEIGHT> {
        root: *tree.root(),
        next_free_slot: tree.next_free_slot,
    };
    take_counts();
    HARDENING.set(!matches!(
        implementation,
        Implementation::New { hardening: false }
    ));
    let result = catch_unwind(AssertUnwindSafe(|| match implementation {
        Implementation::Reference => commitment.verify_and_apply_batch_reference(
            &mut oracle,
            entries.iter().copied(),
            Global,
            &mut NullLogger,
        ),
        Implementation::New { .. } => commitment.verify_and_apply_batch(
            &mut oracle,
            entries.iter().copied(),
            Global,
            &mut NullLogger,
        ),
    }));
    HARDENING.set(true);
    assert!(
        !oracle.fixture_failed,
        "test oracle failed outside verifier validation"
    );
    let outcome = match result {
        Ok(Ok(())) => Outcome::Accepted {
            root: commitment.root,
            next_free_slot: commitment.next_free_slot,
        },
        Ok(Err(error)) => Outcome::Failed(format!("{error:?}")),
        Err(_) => Outcome::Panicked,
    };
    Run {
        outcome,
        log: oracle.log,
        counts: take_counts(),
        has_tamper,
        has_script,
        changed_responses: oracle.changed_responses,
        scripted_responses: oracle.scripted_responses,
    }
}

/// Asserts that two logs are equal, showing the first difference.
pub(super) fn assert_logs_eq(new: &[LoggedQuery], reference: &[LoggedQuery], what: &str) {
    for (i, (a, b)) in new.iter().zip(reference).enumerate() {
        assert!(
            a == b,
            "{what}: query {i} differs: {:?} vs {:?}",
            (a.0, a.1, a.2.len()),
            (b.0, b.1, b.2.len())
        );
    }
    assert_eq!(new.len(), reference.len(), "{what}: log lengths");
}

/// The runs of the three implementations on the same inputs and answers.
pub(super) struct DiffRuns {
    pub(super) reference: Run,
    pub(super) unhardened: Run,
    pub(super) hardened: Run,
}

impl DiffRuns {
    /// All three verifiers reject, and any configured response mutation was exercised.
    pub(super) fn assert_rejected(&self) {
        assert_eq!(self.reference.outcome, Outcome::Panicked);
        assert_eq!(self.unhardened.outcome, Outcome::Panicked);
        assert_eq!(self.hardened.outcome, Outcome::Panicked);
        for run in [&self.reference, &self.unhardened, &self.hardened] {
            assert!(
                !run.has_tamper || run.changed_responses > 0,
                "tamper never changed a response"
            );
            assert!(
                !run.has_script || run.scripted_responses > 0,
                "script never answered a query"
            );
        }
    }

    pub(super) fn assert_rejected_before_proof(&self, index: u64) {
        assert_logs_eq(
            &self.hardened.log,
            &self.reference.log[..self.hardened.log.len()],
            "bound rejection prefix",
        );
        let (id, input, _) = &self.reference.log[self.hardened.log.len()];
        assert_eq!((*id, *input), (PROOF, Input::Index(index)));
        assert!(!self
            .hardened
            .log
            .iter()
            .any(|(id, input, _)| (*id, *input) == (PROOF, Input::Index(index))));
    }

    /// The accepted commitment.
    pub(super) fn accepted(&self) -> (Bytes32, u64) {
        match self.reference.outcome {
            Outcome::Accepted {
                root,
                next_free_slot,
            } => (root, next_free_slot),
            ref outcome => panic!("the batch is rejected: {outcome:?}"),
        }
    }

    /// The number of queries of a type in the reference log.
    pub(super) fn num_queries(&self, query_id: u32) -> usize {
        self.reference
            .log
            .iter()
            .filter(|(id, ..)| *id == query_id)
            .count()
    }
}

/// Compares outcomes and complete oracle logs. Hardened rejection may stop earlier only
/// immediately before a proof outside the committed leaves, with an identical query prefix.
pub(super) fn diff_run<const R: bool>(
    tree: &TestingTree<R>,
    entries: &[(WarmStorageKey, WarmStorageValue)],
    tamper: impl Fn() -> Option<Tamper>,
) -> DiffRuns {
    diff_run_scripted(tree, entries, tamper, || None)
}

pub(super) fn diff_run_scripted<const R: bool>(
    tree: &TestingTree<R>,
    entries: &[(WarmStorageKey, WarmStorageValue)],
    tamper: impl Fn() -> Option<Tamper>,
    script: impl Fn() -> Option<Script>,
) -> DiffRuns {
    let reference = run_scripted(Implementation::Reference, tree, entries, tamper(), script());
    let unhardened = run_scripted(
        Implementation::New { hardening: false },
        tree,
        entries,
        tamper(),
        script(),
    );
    assert_eq!(unhardened.outcome, reference.outcome, "unhardened outcome");
    assert_logs_eq(&unhardened.log, &reference.log, "unhardened");
    let hardened = run_scripted(
        Implementation::New { hardening: true },
        tree,
        entries,
        tamper(),
        script(),
    );
    assert_eq!(hardened.outcome, reference.outcome, "hardened outcome");
    if hardened.log.len() == reference.log.len() {
        assert_logs_eq(&hardened.log, &reference.log, "hardened");
    } else {
        assert_eq!(hardened.outcome, Outcome::Panicked);
        assert!(hardened.log.len() < reference.log.len());
        assert_logs_eq(
            &hardened.log,
            &reference.log[..hardened.log.len()],
            "hardened prefix",
        );
        let (id, input, _) = &reference.log[hardened.log.len()];
        assert_eq!(*id, PROOF);
        assert!(
            matches!(input, Input::Index(index) if *index >= tree.next_free_slot),
            "early rejection must precede a proof outside the committed leaves"
        );
    }
    DiffRuns {
        reference,
        unhardened,
        hardened,
    }
}

#[test]
fn oracle_fixture_failure_is_not_verifier_rejection() {
    let tree = tree_of(&[]);
    let entries = [entry(&tree, storage_key(1), Access::Read)];
    let invoked = std::rc::Rc::new(Cell::new(false));
    let script_invoked = invoked.clone();
    let result = catch_unwind(AssertUnwindSafe(|| {
        run_scripted(
            Implementation::Reference,
            &tree,
            &entries,
            None,
            Some(Box::new(move |_, _, _| {
                script_invoked.set(true);
                panic!("broken fixture")
            })),
        )
    }));
    assert!(invoked.get());
    assert!(
        result.is_err(),
        "fixture failure must escape the verifier outcome"
    );
}
