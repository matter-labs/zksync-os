# Transport Migration Plan 2: Consumer Migration & Cleanup

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace `IOOracle` with `SerdeIOOracle`, migrate all call sites, wire ProvingOracle and WitnessRecordingOracle into the execution pipeline, and delete the old usize-serialization code.

**Architecture:** Rename `SerdeIOOracle` → `IOOracle` (replacing the old trait), then fix all consumers crate by crate until everything compiles. The migration is a big-bang trait change followed by mechanical call site updates. Each task fixes one crate.

**Tech Stack:** Rust, serde, airbender-codec/guest/host

**Spec:** `docs/superpowers/specs/2026-05-17-transport-migration-2b2c-design.md`
**Prereq:** Plan 1 infrastructure (SerdeIOOracle, ProvingOracle, WitnessRecordingOracle, response types) already on branch `feat/serde-unconditional`.

---

## Migration Patterns Reference

These patterns apply across all tasks. Each task lists which files need which pattern.

### Pattern A: Simple query (was `query_serializable` or `query_with_empty_input`)
```rust
// Before:
let metadata: BlockMetadataFromOracle = oracle.query_serializable(BLOCK_METADATA_QUERY_ID, &())?;
let metadata: BlockMetadataFromOracle = oracle.query_with_empty_input(BLOCK_METADATA_QUERY_ID)?;

// After (identical — the new trait has the same convenience methods):
let metadata: BlockMetadataFromOracle = oracle.query(BLOCK_METADATA_QUERY_ID, &())?;
let metadata: BlockMetadataFromOracle = oracle.query_with_empty_input(BLOCK_METADATA_QUERY_ID)?;
```

### Pattern B: Raw iterator query (was `raw_query` returning usize iterator)
```rust
// Before:
let mut it = oracle.raw_query(QUERY_ID, &input)?;
let value = UsizeDeserializable::from_iter(&mut it)?;

// After — return typed value directly:
let value: ResponseType = oracle.query(QUERY_ID, &input)?;
```

### Pattern C: Byte buffer query (was `get_bytes_from_query` with length+body)
```rust
// Before (two queries — length then body):
let buffer = oracle.get_bytes_from_query(LEN_QUERY_ID, BODY_QUERY_ID, &(), allocator)?;

// After (single query returning bytes):
let bytes: Option<Vec<u8>> = oracle.query(BODY_QUERY_ID, &())?;
// or for non-optional:
let bytes: Vec<u8> = oracle.query(BODY_QUERY_ID, &())?;
```

### Pattern D: Preimage expose (was `expose_preimage` filling MaybeUninit buffer)
```rust
// Before:
let words_written = oracle.expose_preimage(QUERY_ID, &hash, &mut destination)?;

// After — read bytes, copy to buffer:
let preimage: Vec<u8> = oracle.query(QUERY_ID, &hash)?;
```
The caller's buffer management changes — instead of filling a `MaybeUninit<usize>` buffer,
it receives owned bytes. Each `expose_preimage` call site needs individual attention
since the buffer usage patterns differ.

### Pattern E: Callable oracle query (pointer-based, was `raw_query` with ptr)
```rust
// Before:
#[cfg(target_pointer_width = "32")]
let mut it = oracle.raw_query(ADVICE_QUERY_ID, &(ptr as u32))?;
#[cfg(target_pointer_width = "64")]
let mut it = oracle.raw_query(ADVICE_QUERY_ID, &(ptr as u64))?;
let q = read_limbs_from_oracle_response(&mut it);

// After — pointer still passed, response is typed:
#[cfg(target_pointer_width = "32")]
let response: DivRemResponse = oracle.query(ADVICE_QUERY_ID, &(ptr as u32))?;
#[cfg(target_pointer_width = "64")]
let response: DivRemResponse = oracle.query(ADVICE_QUERY_ID, &(ptr as u64))?;
let q = response.quotient;
```

---

## Tasks

### Task 1: Replace IOOracle trait with serde-based signature

The big-bang change. After this, nothing compiles until later tasks fix each crate.

**Files:**
- Modify: `zk_ee/src/oracle/mod.rs`
- Delete: `zk_ee/src/oracle/serde_oracle.rs` (content moves into mod.rs)
- Modify: `zk_ee/src/oracle/basic_queries.rs` (may need updates or deletion)

- [ ] **Step 1: Replace IOOracle trait body in `zk_ee/src/oracle/mod.rs`**

Remove the old trait (raw_query, expose_preimage, get_bytes_from_query, etc.) and
replace with the serde-based signature from `serde_oracle.rs`. Keep `IOResponder`
if still needed by forward_system. Remove imports of `UsizeSerializable`,
`UsizeDeserializable`, `MaybeUninit`, `UsizeAlignedByteBox`.

The new trait (rename `SerdeIOOracle` → `IOOracle`):

```rust
use crate::system::errors::internal::InternalError;
use core::num::NonZeroU32;
use serde::{de::DeserializeOwned, Serialize};

use self::query_ids::NEXT_TX_SIZE_QUERY_ID;

pub trait IOOracle: 'static + Sized {
    fn query<I: Serialize, O: DeserializeOwned + Serialize>(
        &mut self,
        query_type: u32,
        input: &I,
    ) -> Result<O, InternalError>;

    fn query_with_empty_input<O: DeserializeOwned + Serialize>(
        &mut self,
        query_type: u32,
    ) -> Result<O, InternalError> {
        self.query::<(), O>(query_type, &())
    }

    fn try_begin_next_tx(&mut self) -> Result<Option<NonZeroU32>, InternalError> {
        let size: u32 = self.query_with_empty_input(NEXT_TX_SIZE_QUERY_ID)?;
        Ok(NonZeroU32::new(size))
    }

    fn query_bytes<I: Serialize>(
        &mut self,
        query_type: u32,
        input: &I,
    ) -> Result<alloc::vec::Vec<u8>, InternalError> {
        self.query::<I, alloc::vec::Vec<u8>>(query_type, input)
    }
}
```

- [ ] **Step 2: Delete `serde_oracle.rs`**

Remove `zk_ee/src/oracle/serde_oracle.rs` and the `pub mod serde_oracle;` line
from `mod.rs`. The trait now lives directly in `mod.rs`.

- [ ] **Step 3: Update all imports of `SerdeIOOracle` to `IOOracle`**

These files import `SerdeIOOracle` and need to switch to `IOOracle`:
- `proof_running_system/src/proving_oracle.rs` — change `use zk_ee::oracle::serde_oracle::SerdeIOOracle` to `use zk_ee::oracle::IOOracle`
- `oracle_provider/src/witness_recording.rs` — same change

- [ ] **Step 4: Attempt to build zk_ee only**

```bash
cargo check -p zk_ee 2>&1 | head -30
```

Fix any compilation errors in zk_ee. The `system/mod.rs` file uses `raw_query_with_empty_input`,
`get_bytes_from_query`, etc. which no longer exist. These will be fixed in Task 2.
At this point zk_ee itself should compile (the system/mod.rs methods that used old
IOOracle methods will need updating).

- [ ] **Step 5: Commit (may not compile downstream yet)**

```bash
git add -u zk_ee/
git commit -m "feat(zk_ee)!: replace IOOracle with serde-based signature

BREAKING: IOOracle now uses serde Serialize/DeserializeOwned instead of
UsizeSerializable/UsizeDeserializable. All consumers must migrate."
```

---

### Task 2: Fix zk_ee system methods

The `System` struct in `zk_ee/src/system/mod.rs` has methods that use old IOOracle
methods (`raw_query_with_empty_input`, `get_bytes_from_query`). These need to use
the new `query`/`query_bytes` methods.

**Files:**
- Modify: `zk_ee/src/system/mod.rs`
- Modify: `zk_ee/src/common_structs/da_commitment_scheme.rs`

- [ ] **Step 1: Update `try_begin_next_tx` in system/mod.rs**

The `try_begin_next_tx` method currently calls `oracle.raw_query_with_empty_input(TX_DATA_WORDS_QUERY_ID)`
and iterates usize words. Change to read `Vec<u8>` via `oracle.query_bytes`:

```rust
// The tx data read changes from usize iterator to Vec<u8>
let tx_data: Vec<u8> = self.io.oracle().query_bytes(TX_DATA_WORDS_QUERY_ID, &())?;
```

The buffer management changes — instead of filling a `MaybeUninit<usize>` buffer
word by word, copy the bytes. Read the full `try_begin_next_tx` method body and
rewrite it to work with `Vec<u8>` instead of usize iterators.

- [ ] **Step 2: Update `get_bytes_from_query` in system/mod.rs**

This was a two-step query (length then body). Replace with single query:

```rust
pub fn get_bytes_from_query(
    &mut self,
    _length_query_id: u32,
    body_query_id: u32,
) -> Result<Option<UsizeAlignedByteBox<S::Allocator>>, InternalError> {
    let bytes: Vec<u8> = self.io.oracle().query(body_query_id, &())?;
    if bytes.is_empty() {
        return Ok(None);
    }
    // Convert Vec<u8> to UsizeAlignedByteBox
    // ... (depends on UsizeAlignedByteBox API)
}
```

Note: If `UsizeAlignedByteBox` is tightly coupled to usize iterators, this type
may need to change or be replaced with `Vec<u8>` at call sites.

- [ ] **Step 3: Update DA commitment scheme query**

In `zk_ee/src/common_structs/da_commitment_scheme.rs`, update the oracle query call.

- [ ] **Step 4: Verify zk_ee compiles**

```bash
cargo check -p zk_ee
```

- [ ] **Step 5: Commit**

```bash
git add -u zk_ee/
git commit -m "fix(zk_ee): update system methods for serde-based IOOracle"
```

---

### Task 3: Fix basic_system

The largest set of call site migrations. Storage models, preimage caches, and
system functions all use IOOracle methods.

**Files to update (Pattern B — raw_query to typed query):**
- `basic_system/src/system_functions/u256_advice.rs` (4 call sites, Pattern E)
- `basic_system/src/system_functions/modexp/advice/bigint.rs` (3 call sites, Pattern E)
- `basic_system/src/system_functions/field_ops.rs` (2 call sites, Pattern A)

**Files to update (Pattern D — expose_preimage to query):**
- `basic_system/src/system_implementation/ethereum_storage_model/caches/preimage.rs` (3 call sites)
- `basic_system/src/system_implementation/ethereum_storage_model/persist_changes.rs` (2 call sites)
- `basic_system/src/system_implementation/flat_storage_model/preimage_cache.rs` (2 call sites)

**Files to update (Pattern A/B — simple queries):**
- `basic_system/src/system_implementation/flat_storage_model/simple_growable_storage.rs` (2 call sites)
- `basic_system/src/system_implementation/flat_storage_model/account_cache_entry.rs` (1 call site)
- `basic_system/src/system_implementation/system/io_subsystem.rs` (IOOracle bound)

- [ ] **Step 1: Update callable oracle call sites (u256_advice.rs)**

Apply Pattern E. Read the file, find all 4 `raw_query` calls, replace with typed
`oracle.query()` calls using `DivRemResponse` and `WideDivRemResponse` types.
Keep the `cfg(target_pointer_width)` branches for pointer construction.

- [ ] **Step 2: Update callable oracle call sites (modexp bigint.rs)**

Apply Pattern E. The modexp response is variable-length — use `ModexpResponse`
type which has `quotient: Vec<u64>` and `remainder: Vec<u64>`.

- [ ] **Step 3: Update field_ops.rs**

Apply Pattern A. Use `FieldSqrtResponse` and `FieldInverseResponse`.

- [ ] **Step 4: Update preimage expose call sites**

Apply Pattern D. Each `expose_preimage` call becomes `oracle.query_bytes()` or
`oracle.query::<_, Vec<u8>>()`. The buffer filling logic changes from word-by-word
to byte copy. Read each call site carefully — they have different buffer
management patterns.

- [ ] **Step 5: Update storage query call sites**

Apply Pattern A/B. `query_serializable` → `query`, `raw_query` → `query`.

- [ ] **Step 6: Update IOOracle bounds on FullIO and storage models**

Every `O: IOOracle` bound stays the same (the trait name didn't change, just its
signature). But any function that constructs `UsizeSerializable` inputs needs
updating. Check the bounds on generic parameters.

- [ ] **Step 7: Remove UsizeSerializable/UsizeDeserializable impls from basic_system types**

Delete all `impl UsizeSerializable for X` and `impl UsizeDeserializable for X`
blocks in basic_system. These types already have serde derives from Phase 2a.

Key files:
- `basic_system/src/system_implementation/ethereum_storage_model/caches/account_properties.rs`
- `basic_system/src/system_implementation/flat_storage_model/simple_growable_storage.rs` (50+ impls)

- [ ] **Step 8: Verify basic_system compiles**

```bash
cargo check -p basic_system
```

- [ ] **Step 9: Commit**

```bash
git add -u basic_system/
git commit -m "fix(basic_system): migrate all IOOracle call sites to serde"
```

---

### Task 4: Fix basic_bootloader

**Files to update:**
- `basic_bootloader/src/bootloader/block_flow/zk/metadata_op.rs` (Pattern A)
- `basic_bootloader/src/bootloader/block_flow/zk/batch_data.rs` (Pattern B)
- `basic_bootloader/src/bootloader/block_flow/zk/post_tx_op/da_commitment_generator/blob_commitment_generator/commitment_and_proof_advice.rs` (Pattern E)
- `basic_bootloader/src/bootloader/block_flow/zk/post_tx_op/post_tx_op_proving_multiblock_batch.rs` (Pattern A)
- `basic_bootloader/src/bootloader/block_flow/zk/post_tx_op/post_tx_op_proving_singleblock_batch.rs` (Pattern A)
- `basic_bootloader/src/bootloader/block_flow/ethereum/block_header.rs` (Pattern C)
- `basic_bootloader/src/bootloader/block_flow/ethereum/post_tx_op_proving.rs` (Pattern C)
- `basic_bootloader/src/bootloader/block_flow/ethereum/post_tx_op_sequencing.rs` (Pattern C)
- `basic_bootloader/src/bootloader/transaction/mod.rs` (remove SimpleOracleQuery impls)

- [ ] **Step 1: Update block metadata query**

In `metadata_op.rs`, `oracle.query_with_empty_input(BLOCK_METADATA_QUERY_ID)` —
this already matches the new trait's method. Just verify the return type annotation.

- [ ] **Step 2: Update byte buffer queries (ethereum headers, withdrawals)**

Apply Pattern C. The `get_bytes_from_query` calls with length+body query IDs
become single `oracle.query_bytes(BODY_QUERY_ID, &input)` calls. The length
query ID is no longer needed.

- [ ] **Step 3: Update blob KZG commitment call site**

Apply Pattern E. Use existing `KZGCommitmentAndProof` type.

- [ ] **Step 4: Remove SimpleOracleQuery impls**

Delete `TxEncodingFormatQuery`, `TxFromQuery`, `HistoricalHashQuery` and all
other `SimpleOracleQuery` implementations. They're no longer needed — call sites
use `oracle.query()` directly.

- [ ] **Step 5: Remove UsizeSerializable/UsizeDeserializable impls**

Delete usize trait impls from:
- `basic_bootloader/src/bootloader/transaction/mod.rs` (TxEncodingFormat)
- `basic_bootloader/src/bootloader/block_flow/zk/.../commitment_and_proof_advice.rs` (KZGCommitmentAndProof)
- Any other bootloader types with usize trait impls

- [ ] **Step 6: Verify basic_bootloader compiles**

```bash
cargo check -p basic_bootloader
```

- [ ] **Step 7: Commit**

```bash
git add -u basic_bootloader/
git commit -m "fix(basic_bootloader): migrate IOOracle call sites to serde"
```

---

### Task 5: Update oracle_provider

**Files:**
- Modify: `oracle_provider/src/lib.rs` — rewrite `ZkEENonDeterminismSource` to impl new IOOracle
- Delete: `ReadWitnessSource` (replaced by `WitnessRecordingOracle`)
- Modify: `oracle_provider/src/witness_recording.rs` — update `SerdeIOOracle` → `IOOracle`

- [ ] **Step 1: Rewrite ZkEENonDeterminismSource impl IOOracle**

The new impl dispatches to `OracleQueryProcessor` using serde bytes:

```rust
impl IOOracle for ZkEENonDeterminismSource {
    fn query<I: Serialize, O: DeserializeOwned + Serialize>(
        &mut self,
        query_type: u32,
        input: &I,
    ) -> Result<O, InternalError> {
        if query_type == DISCONNECT_ORACLE_QUERY_ID {
            self.is_connected_to_external_oracle = false;
        }
        if !self.is_connected_to_external_oracle {
            // Return default/empty — or handle per query type
        }
        let input_bytes = AirbenderCodecV0::encode(input)
            .map_err(|_| internal_error!("failed to encode query input"))?;
        let processor = self.get_processor(query_type)?;
        let response_bytes = processor.process(query_type, &input_bytes)?;
        AirbenderCodecV0::decode(&response_bytes)
            .map_err(|_| internal_error!("failed to decode query response"))
    }
}
```

- [ ] **Step 2: Update OracleQueryProcessor trait**

```rust
pub trait OracleQueryProcessor {
    fn supported_query_ids(&self) -> Vec<u32>;
    fn process(&mut self, query_id: u32, input: &[u8]) -> Result<Vec<u8>, InternalError>;
}
```

- [ ] **Step 3: Delete old code from oracle_provider**

Remove:
- `ReadWitnessSource` struct and impl
- `QueryBuffer` struct
- `NonDeterminismCSRSource` impl for `ZkEENonDeterminismSource`
- The `read_impl`/`write_impl` state machine (32/64-bit bridging)
- `high_half`, `iterator_len_to_indicate`, `current_iterator` fields

- [ ] **Step 4: Update WitnessRecordingOracle**

Change `SerdeIOOracle` → `IOOracle` in `witness_recording.rs`.

- [ ] **Step 5: Delete legacy_adapter.rs**

No longer needed.

- [ ] **Step 6: Verify oracle_provider compiles**

```bash
cargo check -p oracle_provider
```

- [ ] **Step 7: Commit**

```bash
git add -u oracle_provider/
git commit -m "fix(oracle_provider): rewrite for serde-based IOOracle

ZkEENonDeterminismSource uses serde encode/decode. ReadWitnessSource
replaced by WitnessRecordingOracle. QueryBuffer and 32/64-bit
bridging removed."
```

---

### Task 6: Update forward_system query processors

Each of the 13 query processors needs to change from the old
`OracleQueryProcessor` interface (returning `Box<dyn ExactSizeIterator<Item = usize>>`)
to the new byte-based interface (returning `Result<Vec<u8>, InternalError>`).

**Files (each processor):**
- `forward_system/src/run/query_processors/block_metadata.rs`
- `forward_system/src/run/query_processors/tx_data.rs`
- `forward_system/src/run/query_processors/generic_preimage.rs`
- `forward_system/src/run/query_processors/read_tree.rs`
- `forward_system/src/run/query_processors/read_storage.rs`
- `forward_system/src/run/query_processors/zk_proof_data.rs`
- `forward_system/src/run/query_processors/da_commitment_scheme.rs`
- `forward_system/src/run/query_processors/ethereum_header.rs`
- `forward_system/src/run/query_processors/ethereum_cl.rs`
- `forward_system/src/run/query_processors/ethereum_initial_account_state.rs`
- `forward_system/src/run/query_processors/ethereum_initial_storage_slot_value.rs`
- `forward_system/src/run/query_processors/uart_print.rs`
- `forward_system/src/run/query_processors/mod.rs`

**Also:**
- `forward_system/src/run/mod.rs` — oracle setup
- `forward_system/src/system/system_types/mod.rs` — type aliases
- `forward_system/src/system/bootloader.rs` — run functions

- [ ] **Step 1: Update each processor**

Migration pattern for each processor:

```rust
// Before:
impl OracleQueryProcessor for BlockMetadataResponder {
    fn process_buffered_query(
        &mut self, query_id: u32, query: Vec<usize>, _memory: &dyn RamPeek,
    ) -> Box<dyn ExactSizeIterator<Item = usize> + 'static + Send + Sync> {
        // ... compute response, return as usize iterator
    }
}

// After:
impl OracleQueryProcessor for BlockMetadataResponder {
    fn process(&mut self, query_id: u32, input: &[u8]) -> Result<Vec<u8>, InternalError> {
        let response = self.block_metadata;
        AirbenderCodecV0::encode(&response)
            .map_err(|_| internal_error!("encode failed"))
    }
}
```

Each processor: decode input from bytes (if it has input), compute response,
encode response to bytes.

- [ ] **Step 2: Update callable oracle processors**

The callable oracle processors in `callable_oracles/` need the same treatment.
These keep reading from host memory via raw pointers (native variants only).
The RISC-V variants (`ArithmeticQuery`, `FieldOpsQuery`, `BlobCommitmentAndProofQuery`
with `RamPeek`) can be deleted — they're only needed for on-the-fly processing
which we're not using.

- [ ] **Step 3: Update oracle setup in forward_system/src/run/mod.rs**

The `generate_proof_input` and `run_block` functions set up the oracle with
processors. These need to use `WitnessRecordingOracle` instead of `ReadWitnessSource`.

- [ ] **Step 4: Update system types**

In `forward_system/src/system/system_types/mod.rs`:
- `ProverInputSystem` changes from `ForwardSystemTypes<ReadWitnessSource, true>` to
  `ForwardSystemTypes<WitnessRecordingOracle<ZkEENonDeterminismSource>, true>`

- [ ] **Step 5: Verify forward_system compiles**

```bash
cargo check -p forward_system
```

- [ ] **Step 6: Commit**

```bash
git add -u forward_system/ callable_oracles/
git commit -m "fix(forward_system): migrate query processors to byte-based interface"
```

---

### Task 7: Update proof_running_system and guest binary

**Files:**
- Modify: `proof_running_system/src/system/mod.rs` — use ProvingOracle
- Modify: `proof_running_system/src/system/bootloader.rs` — use ProvingOracle
- Delete: `proof_running_system/src/io_oracle/mod.rs` — CsrBasedIOOracle no longer needed
- Modify: `zksync_os/src/main.rs` — use ProvingOracle with CsrTransport
- Modify: `proof_running_system/src/proving_oracle.rs` — rename SerdeIOOracle → IOOracle

- [ ] **Step 1: Update ProvingOracle to use IOOracle (renamed from SerdeIOOracle)**

In `proving_oracle.rs`, change `use zk_ee::oracle::serde_oracle::SerdeIOOracle`
to `use zk_ee::oracle::IOOracle`. Change `impl SerdeIOOracle` to `impl IOOracle`.

- [ ] **Step 2: Update ProofRunningSystemTypes**

In `proof_running_system/src/system/mod.rs`, change the oracle type from
`CsrBasedIOOracle<I>` to `ProvingOracle<CsrTransport>` (or generic over Transport).

- [ ] **Step 3: Update run_proving in bootloader.rs**

Change from creating `CsrBasedIOOracle::<I>::init()` to `ProvingOracle::new(CsrTransport)`.
Remove the `I: NonDeterminismCSRSourceImplementation` generic parameter.

- [ ] **Step 4: Update guest binary (zksync_os/src/main.rs)**

Remove the `CSRBasedNonDeterminismSource` impl. The main function becomes:

```rust
#[airbender::main(allocator_init = init_allocator)]
fn main() -> [u32; 8] {
    run_proving::<LoggerTy>()
}
```

Where `run_proving` no longer takes a generic CSR source — it creates a
`ProvingOracle<CsrTransport>` internally.

- [ ] **Step 5: Delete CsrBasedIOOracle**

Delete `proof_running_system/src/io_oracle/mod.rs` and its module declaration.
Delete `NonDeterminismCSRSourceImplementation` trait and all related code.

- [ ] **Step 6: Verify proof_running_system compiles**

```bash
cargo check -p proof_running_system
```

- [ ] **Step 7: Commit**

```bash
git add -u proof_running_system/ zksync_os/
git commit -m "feat(proof_running_system): switch to ProvingOracle

Guest binary now uses ProvingOracle<CsrTransport> instead of
CsrBasedIOOracle. All oracle responses read from pre-computed
witness via airbender transport."
```

---

### Task 8: Update test rig and zksync_os_runner

**Files:**
- Modify: `tests/rig/src/chain.rs` — use WitnessRecordingOracle, pass Inputs to runner
- Modify: `zksync_os_runner/src/lib.rs` — accept `Inputs.words()` (already accepts `&[u32]`)

- [ ] **Step 1: Update test rig chain.rs**

In the `run_inner` method:
- Pass 2 changes from `ReadWitnessSource::new(oracle)` to
  `WitnessRecordingOracle::new(oracle)`
- Witness extraction changes from `oracle.get_read_items().borrow().clone()`
  to `recorder.into_inputs().1.words().to_vec()`
- Pass 3 stays as `runner.run(&witness_words)` — the format is just different

- [ ] **Step 2: Update forward_system run functions**

`generate_proof_input` and related functions in `forward_system/src/run/mod.rs`
that return `Vec<u32>` need to return `Inputs` instead (or `Vec<u32>` via
`inputs.words().to_vec()`).

- [ ] **Step 3: Run the test suite**

```bash
cargo test -p rig --features rig/no_print
```

This is the key validation — if the test rig works end-to-end (forward pass →
witness recording → RISC-V simulation → output comparison), the migration is correct.

- [ ] **Step 4: Commit**

```bash
git add -u tests/rig/ zksync_os_runner/ forward_system/
git commit -m "feat(rig): wire WitnessRecordingOracle into test pipeline

Test rig now records witness via airbender Inputs and replays
through ProvingOracle in RISC-V simulation."
```

---

### Task 9: Delete old code

**Files to delete:**
- `zk_ee/src/oracle/usize_serialization/` (entire directory)
- `zk_ee/src/oracle/simple_oracle_query.rs`
- `zk_ee/src/oracle/basic_queries.rs`
- `proof_running_system/src/io_oracle/` (entire directory, if not already deleted)
- `oracle_provider/src/legacy_adapter.rs`

**Files to clean up:**
- `zk_ee/src/oracle/mod.rs` — remove `pub mod usize_serialization`, `pub mod simple_oracle_query`, `pub mod basic_queries`
- Remove all remaining `UsizeSerializable`/`UsizeDeserializable` trait impls across the workspace
- Remove `DynUsizeIterator` and `ExactSizeChain` utilities
- Remove `UsizeAlignedByteBox` if no longer used
- `callable_oracles/` — delete RISC-V variants (ArithmeticQuery, FieldOpsQuery, BlobCommitmentAndProofQuery with RamPeek)

- [ ] **Step 1: Delete usize_serialization module**

```bash
rm -rf zk_ee/src/oracle/usize_serialization/
```

Remove `pub mod usize_serialization;` from `zk_ee/src/oracle/mod.rs`.

- [ ] **Step 2: Delete SimpleOracleQuery and basic_queries**

```bash
rm zk_ee/src/oracle/simple_oracle_query.rs
rm zk_ee/src/oracle/basic_queries.rs
```

Remove their `pub mod` declarations from `mod.rs`.

- [ ] **Step 3: Delete remaining UsizeSerializable impls across workspace**

Grep for `UsizeSerializable` and `UsizeDeserializable` across the workspace.
Delete every remaining `impl` block. Key locations:
- `zk_ee/src/utils/bytes32.rs`
- `zk_ee/src/system/metadata/zk_metadata.rs`
- `zk_ee/src/execution_environment_type.rs`
- `zk_ee/src/common_structs/proof_data.rs`
- `zk_ee/src/storage_types/storage_address.rs`
- `zk_ee/src/storage_types/initial_storage_slot_data.rs`
- `callable_oracles/src/lib.rs`

- [ ] **Step 4: Delete RISC-V callable oracle variants**

In `callable_oracles/`:
- Delete `ArithmeticQuery` (keep `NativeArithmeticQuery`)
- Delete `FieldOpsQuery` (keep `NativeFieldOpsQuery`)
- Delete `BlobCommitmentAndProofQuery` (keep `NativeBlobCommitmentAndProofQuery`)
- Delete the `utils/evaluate.rs` RamPeek-based memory reading helpers (if only used by deleted variants)

- [ ] **Step 5: Delete legacy_adapter.rs**

```bash
rm oracle_provider/src/legacy_adapter.rs
```

Remove `pub mod legacy_adapter;` from `oracle_provider/src/lib.rs`.

- [ ] **Step 6: Clean up unused imports and dependencies**

Run clippy across affected crates to find dead code:

```bash
cargo clippy -p zk_ee -p basic_system -p basic_bootloader -p oracle_provider -p callable_oracles -- -D warnings
```

- [ ] **Step 7: Full workspace build and test**

```bash
cargo check -p zk_ee -p basic_system -p basic_bootloader -p oracle_provider -p forward_system -p proof_running_system -p callable_oracles
cargo test -p oracle_provider -p proof_running_system
```

- [ ] **Step 8: Commit**

```bash
git add -u
git commit -m "chore: delete old usize serialization code

Removes UsizeSerializable/UsizeDeserializable traits and all impls,
SimpleOracleQuery, CsrBasedIOOracle, usize_serialization module,
RISC-V callable oracle variants, ReadWitnessSource, QueryBuffer,
LegacyAdapter, and all 32/64-bit bridging code."
```

---

## Checkpoint

After all tasks:
- `IOOracle` trait is serde-based
- All call sites use `oracle.query()` / `oracle.query_with_empty_input()` / `oracle.query_bytes()`
- Forward mode: `ZkEENonDeterminismSource` dispatches to byte-based processors
- Prover input mode: `WitnessRecordingOracle` records via `Inputs::push()`
- Proving mode: `ProvingOracle` reads from airbender transport
- Guest binary uses `ProvingOracle<CsrTransport>`
- Test rig uses full pipeline (forward → recording → RISC-V replay)
- All old usize serialization code deleted (~1000+ lines removed)
- No `UsizeSerializable`, `UsizeDeserializable`, `SimpleOracleQuery`, `CsrBasedIOOracle` remain
