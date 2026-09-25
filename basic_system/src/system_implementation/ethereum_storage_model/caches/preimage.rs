use alloc::{alloc::Global, collections::BTreeMap};
use core::{alloc::Allocator, marker::PhantomData, mem::MaybeUninit};
use storage_models::common_structs::{snapshottable_io::SnapshottableIo, PreimageCacheModel};
use zk_ee::common_structs::history_map::NopSnapshotId;
use zk_ee::oracle::memory_io::{DynamicOracleQuery, OracleQuery};
use zk_ee::oracle::query_ids::PREIMAGE_SUBSPACE_MASK;
use zk_ee::oracle::IOOracle;
use zk_ee::{
    common_structs::PreimageType,
    execution_environment_type::ExecutionEnvironmentType,
    internal_error,
    system::{
        errors::{internal::InternalError, system::SystemError},
        Resources,
    },
    utils::{Bytes32, UsizeAlignedByteBox, USIZE_SIZE},
};

use super::super::cost_constants::PREIMAGE_CACHE_GET_NATIVE_COST;
use crate::system_functions::keccak256::keccak256_native_cost;
use crate::system_implementation::ethereum_storage_model::ETHEREUM_STORAGE_SUBSPACE_MASK;

pub struct PreimageLengthQuery;

#[allow(clippy::identity_op)]
pub const ETHEREUM_BYTECODE_LENGTH_FROM_PREIMAGE_QUERY_ID: u32 =
    PREIMAGE_SUBSPACE_MASK | ETHEREUM_STORAGE_SUBSPACE_MASK | 0x00;
pub const ETHEREUM_BYTECODE_PREIMAGE_QUERY_ID: u32 =
    PREIMAGE_SUBSPACE_MASK | ETHEREUM_STORAGE_SUBSPACE_MASK | 0x01;

impl OracleQuery for PreimageLengthQuery {
    const QUERY_ID: u32 = ETHEREUM_BYTECODE_LENGTH_FROM_PREIMAGE_QUERY_ID;
    type Input = Bytes32;
    type Output = u32;
}

/// The bytecode with the given hash.
pub struct BytecodePreimageQuery;

impl DynamicOracleQuery for BytecodePreimageQuery {
    const QUERY_ID: u32 = ETHEREUM_BYTECODE_PREIMAGE_QUERY_ID;
    type Input = Bytes32;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PreimageRequestForUnknownLength {
    pub hash: Bytes32,
    pub preimage_type: PreimageType,
}

/// Bytecode with the place for its jumpdest artifacts, in the layout that EVM interpreter takes
/// for the code with cached artifacts: `code || padding to the alignment || artifacts`.
///
/// The artifacts only depend on the code, and the same code is usually executed many times over
/// a block (every call frame needs the artifacts), so they are computed at most once, on the
/// first request that needs them, and live as long as the code does. A request that does not
/// execute the code (`EXTCODESIZE`, `EXTCODECOPY`, delegation checks) does not compute them:
/// `artifacts_ready` marks whether the artifacts part holds them or is still zeroed.
pub struct BytecodeWithArtifacts<A: Allocator> {
    buffer: UsizeAlignedByteBox<A>,
    code_len: usize,
    artifacts_len: usize,
    /// Whether the artifacts were computed into the buffer (the preprocessing marker)
    artifacts_ready: bool,
}

impl<A: Allocator + Clone> BytecodeWithArtifacts<A> {
    /// Allocates `code || padding || zeroed artifacts space` for the code of `code_len` bytes.
    /// `write_code` fills the code words (the first `code_words` of the buffer) and returns how
    /// many of them it wrote; the rest of the buffer is zeroed here.
    fn allocate_in(
        code_len: usize,
        write_code: impl FnOnce(&mut [MaybeUninit<usize>]) -> usize,
        allocator: A,
    ) -> Self {
        // The code is followed by at least `CODE_PADDING_BYTES` zero bytes, which lets the
        // interpreter fetch past its end without a bounds check (`CODE_IS_PADDED`).
        // NOTE: we leave some slack for 64/32 bit arch mismatches
        let code_words = (code_len + evm_interpreter::CODE_PADDING_BYTES)
            .next_multiple_of(USIZE_SIZE)
            / USIZE_SIZE;
        let code_words = code_words.next_multiple_of(2);
        let artifacts_len = evm_interpreter::artifacts_byte_len(code_len);
        debug_assert_eq!(artifacts_len % USIZE_SIZE, 0);
        let artifacts_words = artifacts_len / USIZE_SIZE;
        let buffer = UsizeAlignedByteBox::from_init_fn_in(
            code_words + artifacts_words,
            |dst| {
                let (code_dst, artifacts_dst) = dst.split_at_mut(code_words);
                let written = write_code(code_dst);
                // whatever comes after the code in its last words is the padding
                for word in code_dst.iter_mut().skip(written) {
                    word.write(0);
                }
                for word in artifacts_dst.iter_mut() {
                    word.write(0);
                }

                code_words + artifacts_words
            },
            allocator,
        );

        Self {
            buffer,
            code_len,
            artifacts_len,
            artifacts_ready: false,
        }
    }

    /// Code known in pieces (a deployment), without the artifacts
    fn new_in(code: &[&[u8]], allocator: A) -> Self {
        let code_len: usize = code.iter().map(|el| el.len()).sum();
        Self::allocate_in(
            code_len,
            |code_dst| {
                // the pieces may end inside a word: zero the words first, so the padding
                // after the code is in place
                for word in code_dst.iter_mut() {
                    word.write(0);
                }
                let mut dst = code_dst.as_mut_ptr().cast::<u8>();
                for piece in code {
                    // SAFETY: the pieces sum up to `code_len` bytes, and the buffer has at
                    // least that many
                    unsafe {
                        core::ptr::copy_nonoverlapping(piece.as_ptr(), dst, piece.len());
                        dst = dst.add(piece.len());
                    }
                }
                code_dst.len()
            },
            allocator,
        )
    }

    pub fn code(&self) -> &[u8] {
        &self.buffer.as_slice()[..self.code_len]
    }

    /// Whether the artifacts were computed (the preprocessing marker)
    pub fn artifacts_ready(&self) -> bool {
        self.artifacts_ready
    }

    /// Computes the artifacts into their (zeroed) part of the buffer, once
    fn ensure_artifacts(&mut self) {
        if self.artifacts_ready {
            return;
        }
        // The buffer is the code with its padding followed by `artifacts_len` bytes of zeroed
        // artifacts space. The artifacts part is written only here, before any reference to it
        // is handed out (`as_executable` exposes the code only until the artifacts are ready).
        let code_words = (self.buffer.len() - self.artifacts_len) / USIZE_SIZE;
        let (code, artifacts) = self.buffer.as_mut_words().split_at_mut(code_words);
        // SAFETY: the code is the first `code_len` bytes of its words
        let code =
            unsafe { core::slice::from_raw_parts(code.as_ptr().cast::<u8>(), self.code_len) };
        evm_interpreter::analyze_into(code, artifacts);
        self.artifacts_ready = true;
    }

    /// NOTE: it is the buffer on the heap that lives as long as the cache does, not `self`:
    /// the map is free to move its values around when other ones are inserted.
    ///
    /// Safety: IO implementer that will use it is expected to live beoynd any frame (as it's part
    /// of the OS), the entries are never removed from the cache, and their buffers are never
    /// reallocated, so we can extend the lifetime. The artifacts part of the buffer is exposed
    /// only once it is written (`ensure_artifacts`), so it is never written under a reference.
    fn as_executable(&self) -> ExecutableBytecode {
        let (bytecode, artifacts_len) = if self.artifacts_ready {
            (self.buffer.as_slice(), self.artifacts_len)
        } else {
            (self.code(), 0)
        };
        let bytecode: &'static [u8] = unsafe { core::mem::transmute(bytecode) };

        ExecutableBytecode {
            bytecode,
            code_len: self.code_len as u32,
            artifacts_len: artifacts_len as u32,
        }
    }
}

/// Bytecode in the form the execution environment takes it
#[derive(Clone, Copy)]
pub struct ExecutableBytecode {
    /// `code || padding || artifacts` when the artifacts were requested, just the code otherwise
    pub bytecode: &'static [u8],
    pub code_len: u32,
    /// 0 when the artifacts were not requested
    pub artifacts_len: u32,
}

impl ExecutableBytecode {
    pub fn code(&self) -> &'static [u8] {
        &self.bytecode[..self.code_len as usize]
    }
}

pub struct BytecodeKeccakPreimagesStorage<R: Resources, A: Allocator + Clone = Global> {
    pub storage: BTreeMap<Bytes32, BytecodeWithArtifacts<A>, A>,
    pub(crate) allocator: A,
    /// Every preimage is hashed to be verified. The hasher (its state is big and aligned for
    /// the delegation) is kept here and reset after every use instead of being created, and then
    /// moved into `finalize`, every time.
    hasher: crypto::sha3::Keccak256,
    _marker: PhantomData<R>,
}

impl<R: Resources, A: Allocator + Clone> BytecodeKeccakPreimagesStorage<R, A> {
    pub fn new_from_parts(allocator: A) -> Self {
        use crypto::MiniDigest;

        Self {
            storage: BTreeMap::new_in(allocator.clone()),
            allocator,
            hasher: crypto::sha3::Keccak256::new(),
            _marker: PhantomData,
        }
    }

    fn keccak256(&mut self, input: &[u8]) -> Bytes32 {
        use crypto::MiniDigest;

        self.hasher.update(input);
        Bytes32::from_array(*self.hasher.finalize_reset())
    }

    /// Same as `get_preimage`, but gives the code in the form for execution. With
    /// `needs_artifacts` the jumpdest artifacts come with it (computed on the first such
    /// request for the code); a request that only observes the code (its length, its bytes)
    /// passes `false` and skips the preprocessing.
    pub fn get_executable_bytecode<const PROOF_ENV: bool>(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        hash: &Bytes32,
        resources: &mut R,
        oracle: &mut impl IOOracle,
        needs_artifacts: bool,
    ) -> Result<ExecutableBytecode, SystemError> {
        self.expose_preimage::<PROOF_ENV>(ee_type, hash, resources, oracle, needs_artifacts)
    }

    #[must_use]
    fn expose_preimage<const PROOF_ENV: bool>(
        &mut self,
        _ee_type: ExecutionEnvironmentType,
        hash: &Bytes32,
        resources: &mut R,
        oracle: &mut impl IOOracle,
        needs_artifacts: bool,
    ) -> Result<ExecutableBytecode, SystemError> {
        use zk_ee::system::Computational;
        resources.charge(&R::from_native(R::Native::from_computational(
            PREIMAGE_CACHE_GET_NATIVE_COST,
        )))?;
        if let Some(cached) = self.storage.get_mut(hash) {
            if needs_artifacts {
                cached.ensure_artifacts();
            }
            Ok(cached.as_executable())
        } else {
            // We do not charge for gas in this concrete implementation and
            // expect higher-level model todo so.
            // We charge for native.
            let expected_length_in_bytes =
                PreimageLengthQuery::get(oracle, hash).expect("must get preimage length") as usize;
            // The code is read right into the buffer that has the place for its artifacts
            // after it
            let mut buffered = BytecodeWithArtifacts::allocate_in(
                expected_length_in_bytes,
                |code_dst| {
                    BytecodePreimageQuery::get_into(oracle, hash, code_dst)
                        .expect("must get preimage")
                },
                self.allocator.clone(),
            );

            let native_cost = keccak256_native_cost::<R>(expected_length_in_bytes);
            resources.charge(&R::from_native(native_cost))?;

            if PROOF_ENV {
                let recomputed_hash = self.keccak256(buffered.code());

                if recomputed_hash != *hash {
                    return Err(internal_error!("Account hash mismatch").into());
                }
            } else {
                debug_assert!(self.keccak256(buffered.code()) == *hash);
            }

            if needs_artifacts {
                buffered.ensure_artifacts();
            }

            Ok(self
                .storage
                .entry(*hash)
                .or_insert(buffered)
                .as_executable())
        }
    }

    fn insert_verified_preimage(
        &mut self,
        hash: &Bytes32,
        preimage: &[&[u8]],
    ) -> Result<&'static [u8], SystemError> {
        let allocator = self.allocator.clone();
        let inserted = self
            .storage
            .entry(*hash)
            .or_insert_with(|| BytecodeWithArtifacts::new_in(preimage, allocator));

        Ok(inserted.as_executable().code())
    }
}

impl<R: Resources, A: Allocator + Clone> SnapshottableIo for BytecodeKeccakPreimagesStorage<R, A> {
    type StateSnapshot = NopSnapshotId;

    fn begin_new_tx(&mut self) {}

    fn finish_tx(&mut self) -> Result<(), InternalError> {
        Ok(())
    }

    fn start_frame(&mut self) -> Self::StateSnapshot {
        NopSnapshotId::new()
    }

    fn finish_frame(
        &mut self,
        _rollback_handle: Option<&Self::StateSnapshot>,
    ) -> Result<(), InternalError> {
        Ok(())
    }
}

impl<R: Resources, A: Allocator + Clone> PreimageCacheModel
    for BytecodeKeccakPreimagesStorage<R, A>
{
    type Resources = R;
    type PreimageRequest = PreimageRequestForUnknownLength;

    fn get_preimage<const PROOF_ENV: bool>(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        preimage_type: &Self::PreimageRequest,
        resources: &mut Self::Resources,
        oracle: &mut impl IOOracle,
    ) -> Result<&'static [u8], SystemError> {
        // we will NOT charge for preimages in here, but instead higher-level model should do it

        let PreimageRequestForUnknownLength { hash, .. } = preimage_type;

        // preimage type is not important in our case, we do not version them yet
        let cached = self.expose_preimage::<PROOF_ENV>(ee_type, hash, resources, oracle, false)?;

        Ok(cached.code())
    }

    fn record_preimage<const PROOF_ENV: bool>(
        &mut self,
        _ee_type: ExecutionEnvironmentType,
        preimage_type: &Self::PreimageRequest,
        resources: &mut Self::Resources,
        preimage: &[&[u8]],
    ) -> Result<&'static [u8], SystemError> {
        use crate::system_implementation::flat_storage_model::cost_constants::PREIMAGE_CACHE_SET_NATIVE_COST;
        use zk_ee::system::Computational;
        // we will NOT charge ergs for preimages in here, but instead higher-level model should do it
        resources.charge(&R::from_native(R::Native::from_computational(
            PREIMAGE_CACHE_SET_NATIVE_COST,
        )))?;

        let PreimageRequestForUnknownLength { hash, .. } = preimage_type;

        self.insert_verified_preimage(hash, preimage)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zk_ee::oracle::memory_io::host::{
        write_dynamic_bytes, NativeQuerierMemory, ReadQueryInput, ResponseBuffer, WriteQueryOutput,
    };
    use zk_ee::oracle::memory_io::MemoryOracle;
    use zk_ee::oracle::usize_serialization::{UsizeDeserializable, UsizeSerializable};
    use zk_ee::reference_implementations::{BaseResources, DecreasingNative};
    use zk_ee::system::Resource;

    type TestResources = BaseResources<DecreasingNative>;
    type TestPreimages = BytecodeKeccakPreimagesStorage<TestResources, Global>;

    /// Serves one bytecode: its length and its words. Counts the preimage queries.
    struct OneCodeOracle {
        code: Vec<u8>,
        hash: Bytes32,
        preimage_queries: usize,
        response: ResponseBuffer,
    }

    impl OneCodeOracle {
        fn new(code: Vec<u8>) -> Self {
            use crypto::MiniDigest;
            let hash = Bytes32::from_array(*crypto::sha3::Keccak256::digest(&code));
            Self {
                code,
                hash,
                preimage_queries: 0,
                response: ResponseBuffer::default(),
            }
        }
    }

    impl MemoryOracle for OneCodeOracle {
        fn send_query(&mut self, query_id: u32, input_word: usize) -> Result<(), InternalError> {
            // SAFETY: the test sends the queries from this process
            let memory = unsafe { NativeQuerierMemory::new() };
            assert_eq!(Bytes32::read_input(&memory, input_word)?, self.hash);
            let mut response = vec![];
            match query_id {
                ETHEREUM_BYTECODE_LENGTH_FROM_PREIMAGE_QUERY_ID => {
                    (self.code.len() as u32).write_output(&mut response)
                }
                ETHEREUM_BYTECODE_PREIMAGE_QUERY_ID => {
                    self.preimage_queries += 1;
                    write_dynamic_bytes(&self.code, &mut response);
                }
                _ => panic!("unexpected query {query_id:#x}"),
            }
            self.response.set(response)
        }

        zk_ee::memory_oracle_response_methods!(response);
    }

    impl IOOracle for OneCodeOracle {
        type RawIterator<'a> = core::iter::Empty<usize>;

        fn raw_query<'a, I: UsizeSerializable + UsizeDeserializable>(
            &'a mut self,
            query_type: u32,
            _input: &I,
        ) -> Result<Self::RawIterator<'a>, InternalError> {
            panic!("unexpected query {query_type:#x}")
        }
    }

    /// JUMPDEST at 0 and 4 (the one at 2 is a PUSH immediate), 9 bytes so the
    /// last word is a partial one
    const CODE: [u8; 9] = [0x5b, 0x60, 0x5b, 0x00, 0x5b, 0x00, 0x00, 0x00, 0x5b];

    fn expected_artifacts() -> Vec<u8> {
        let mut bitmap = vec![0usize; evm_interpreter::artifacts_byte_len(CODE.len()) / USIZE_SIZE];
        evm_interpreter::analyze_into(&CODE, &mut bitmap);
        bitmap.iter().flat_map(|w| w.to_ne_bytes()).collect()
    }

    /// A request that only observes the code (`EXTCODESIZE`) loads and verifies it but does not
    /// preprocess it; the first request that runs the code computes the artifacts in place, once.
    #[test]
    fn artifacts_are_computed_on_the_first_request_that_needs_them() {
        let mut preimages = TestPreimages::new_from_parts(Global);
        let mut oracle = OneCodeOracle::new(CODE.to_vec());
        let hash = oracle.hash;
        let mut resources = TestResources::FORMAL_INFINITE;

        let observed = preimages
            .get_executable_bytecode::<true>(
                ExecutionEnvironmentType::EVM,
                &hash,
                &mut resources,
                &mut oracle,
                false,
            )
            .expect("observing the code");
        assert_eq!(observed.bytecode, &CODE[..]);
        assert_eq!(observed.code_len as usize, CODE.len());
        assert_eq!(observed.artifacts_len, 0);
        assert!(!preimages.storage.get(&hash).unwrap().artifacts_ready());
        assert_eq!(oracle.preimage_queries, 1);

        let executable = preimages
            .get_executable_bytecode::<true>(
                ExecutionEnvironmentType::EVM,
                &hash,
                &mut resources,
                &mut oracle,
                true,
            )
            .expect("running the code");
        assert_eq!(oracle.preimage_queries, 1, "the code is loaded once");
        assert!(preimages.storage.get(&hash).unwrap().artifacts_ready());
        assert_eq!(executable.code(), &CODE[..]);
        let artifacts = expected_artifacts();
        assert_eq!(executable.artifacts_len as usize, artifacts.len());
        assert_eq!(
            &executable.bytecode[executable.bytecode.len() - artifacts.len()..],
            &artifacts[..]
        );
        let padding = executable.bytecode.len() - CODE.len() - artifacts.len();
        assert!(executable.bytecode[CODE.len()..CODE.len() + padding]
            .iter()
            .all(|b| *b == 0));

        // observing again keeps giving the artifacts, they are not recomputed
        let observed_again = preimages
            .get_executable_bytecode::<true>(
                ExecutionEnvironmentType::EVM,
                &hash,
                &mut resources,
                &mut oracle,
                false,
            )
            .expect("observing again");
        assert_eq!(observed_again.artifacts_len as usize, artifacts.len());
        assert_eq!(observed_again.bytecode, executable.bytecode);
    }

    /// Deployed code is recorded without the artifacts; a later run of it computes them
    #[test]
    fn recorded_code_gets_its_artifacts_when_run() {
        let mut preimages = TestPreimages::new_from_parts(Global);
        let mut oracle = OneCodeOracle::new(CODE.to_vec());
        let hash = oracle.hash;
        let mut resources = TestResources::FORMAL_INFINITE;

        let (head, tail) = CODE.split_at(5);
        let recorded = preimages
            .insert_verified_preimage(&hash, &[head, tail])
            .expect("recording");
        assert_eq!(recorded, &CODE[..]);
        assert!(!preimages.storage.get(&hash).unwrap().artifacts_ready());

        let executable = preimages
            .get_executable_bytecode::<true>(
                ExecutionEnvironmentType::EVM,
                &hash,
                &mut resources,
                &mut oracle,
                true,
            )
            .expect("running the recorded code");
        assert_eq!(oracle.preimage_queries, 0, "recorded code is not loaded");
        assert_eq!(executable.code(), &CODE[..]);
        let artifacts = expected_artifacts();
        assert_eq!(executable.artifacts_len as usize, artifacts.len());
        assert_eq!(
            &executable.bytecode[executable.bytecode.len() - artifacts.len()..],
            &artifacts[..]
        );
    }
}
