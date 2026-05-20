# Transport Migration Plan 1: Core Infrastructure

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Create the new serde-based IOOracle trait, ProvingOracle, WitnessRecordingOracle, and callable oracle response types — all as additive code alongside the existing usize-based system, compilable and independently testable.

**Architecture:** New trait `SerdeIOOracle` coexists with existing `IOOracle`. New implementations (`ProvingOracle`, `WitnessRecordingOracle`) implement the new trait. A `LegacyAdapter` wraps old `IOOracle` impls to satisfy the new trait for testing. Plan 2 will migrate consumers and delete the old code.

**Tech Stack:** Rust, serde, airbender-codec (bincode v2), airbender-guest (transport)

**Spec:** `docs/superpowers/specs/2026-05-17-transport-migration-2b2c-design.md`

---

## File Map

| File | Action | Responsibility |
|------|--------|----------------|
| `zk_ee/src/oracle/serde_oracle.rs` | Create | `SerdeIOOracle` trait definition |
| `zk_ee/Cargo.toml` | Modify | Add `airbender-codec` dependency |
| `zk_ee/src/oracle/mod.rs` | Modify | Add `pub mod serde_oracle;` |
| `basic_bootloader/src/bootloader/oracle_types.rs` | Create | Callable oracle response types (`DivRemResponse`, `ModexpResponse`, etc.) |
| `proof_running_system/src/proving_oracle.rs` | Create | `ProvingOracle` impl (reads from transport) |
| `proof_running_system/Cargo.toml` | Modify | Add `airbender-guest` dependency |
| `oracle_provider/src/witness_recording.rs` | Create | `WitnessRecordingOracle` impl |
| `oracle_provider/src/legacy_adapter.rs` | Create | Adapter from old `IOOracle` to `SerdeIOOracle` |
| `oracle_provider/Cargo.toml` | Modify | Add `airbender-host`, `airbender-codec` dependencies |

---

### Task 1: Define the SerdeIOOracle trait

**Files:**
- Create: `zk_ee/src/oracle/serde_oracle.rs`
- Modify: `zk_ee/src/oracle/mod.rs`
- Modify: `zk_ee/Cargo.toml`

- [ ] **Step 1: Add airbender-codec dependency to zk_ee**

In `zk_ee/Cargo.toml`, add under `[dependencies]`:

```toml
airbender-codec = { workspace = true }
```

Check that the workspace root `Cargo.toml` has `airbender-codec` defined. If not, add:

```toml
airbender-codec = { git = "https://github.com/matter-labs/airbender-platform", rev = "72cce091" }
```

- [ ] **Step 2: Create the SerdeIOOracle trait**

Create `zk_ee/src/oracle/serde_oracle.rs`:

```rust
use crate::system::errors::internal::InternalError;
use core::num::NonZeroU32;
use serde::{de::DeserializeOwned, Serialize};

use super::query_ids::NEXT_TX_SIZE_QUERY_ID;

pub trait SerdeIOOracle: 'static + Sized {
    fn query<I: Serialize, O: DeserializeOwned>(
        &mut self,
        query_type: u32,
        input: &I,
    ) -> Result<O, InternalError>;

    fn query_with_empty_input<O: DeserializeOwned>(
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

- [ ] **Step 3: Wire into oracle module**

In `zk_ee/src/oracle/mod.rs`, add after the existing `pub mod` declarations:

```rust
pub mod serde_oracle;
```

- [ ] **Step 4: Verify it compiles**

```bash
cargo check -p zk_ee
```

Expected: compiles with no errors.

- [ ] **Step 5: Commit**

```bash
git add zk_ee/src/oracle/serde_oracle.rs zk_ee/src/oracle/mod.rs zk_ee/Cargo.toml Cargo.toml
git commit -m "feat(zk_ee): add SerdeIOOracle trait

Serde-based oracle trait that will replace the usize-iterator-based
IOOracle. Coexists with the old trait during migration."
```

---

### Task 2: Define callable oracle response types

**Files:**
- Create: `basic_bootloader/src/bootloader/oracle_types.rs`
- Modify: `basic_bootloader/src/bootloader/mod.rs`

- [ ] **Step 1: Create response types**

Create `basic_bootloader/src/bootloader/oracle_types.rs`:

```rust
use serde::{Deserialize, Serialize};
use zk_ee::utils::Bytes32;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DivRemResponse {
    pub quotient: [u64; 4],
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WideDivRemResponse {
    pub quotient_lo: [u64; 4],
    pub quotient_hi: [u64; 4],
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModexpResponse {
    pub quotient: alloc::vec::Vec<u64>,
    pub remainder: alloc::vec::Vec<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FieldSqrtResponse {
    pub result: Bytes32,
    pub is_valid: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FieldInverseResponse {
    pub result: Bytes32,
}
```

Note: `KZGCommitmentAndProof` already exists and already has serde derives (from Phase 2a).

- [ ] **Step 2: Wire into bootloader module**

In `basic_bootloader/src/bootloader/mod.rs`, add:

```rust
pub mod oracle_types;
```

- [ ] **Step 3: Verify it compiles**

```bash
cargo check -p basic_bootloader
```

Note: may fail due to pre-existing `crypto::bigint_op_delegation_raw` error.
Try: `cargo check -p basic_bootloader 2>&1 | grep -v bigint_op_delegation`
to confirm no NEW errors from our changes.

- [ ] **Step 4: Commit**

```bash
git add basic_bootloader/src/bootloader/oracle_types.rs basic_bootloader/src/bootloader/mod.rs
git commit -m "feat(basic_bootloader): add serde response types for callable oracles

DivRemResponse, WideDivRemResponse, ModexpResponse, FieldSqrtResponse,
FieldInverseResponse — used by the new SerdeIOOracle-based proving path."
```

---

### Task 3: Implement ProvingOracle

**Files:**
- Create: `proof_running_system/src/proving_oracle.rs`
- Modify: `proof_running_system/src/lib.rs`
- Modify: `proof_running_system/Cargo.toml`

- [ ] **Step 1: Add airbender-guest dependency**

In `proof_running_system/Cargo.toml`, add under `[dependencies]`:

```toml
airbender-guest = { workspace = true, default-features = false }
```

- [ ] **Step 2: Create ProvingOracle**

Create `proof_running_system/src/proving_oracle.rs`:

```rust
use airbender_guest::input::{read_with, GuestError};
use airbender_guest::transport::Transport;
use serde::de::DeserializeOwned;
use serde::Serialize;
use zk_ee::oracle::serde_oracle::SerdeIOOracle;
use zk_ee::system::errors::internal::InternalError;
use zk_ee::internal_error;

pub struct ProvingOracle<T: Transport> {
    transport: T,
}

impl<T: Transport> ProvingOracle<T> {
    pub fn new(transport: T) -> Self {
        Self { transport }
    }
}

impl<T: Transport + 'static> SerdeIOOracle for ProvingOracle<T> {
    fn query<I: Serialize, O: DeserializeOwned>(
        &mut self,
        _query_type: u32,
        _input: &I,
    ) -> Result<O, InternalError> {
        read_with::<O>(&mut self.transport).map_err(|e| internal_error!("proving oracle read failed"))
    }
}
```

- [ ] **Step 3: Wire into crate**

In `proof_running_system/src/lib.rs`, add:

```rust
pub mod proving_oracle;
```

- [ ] **Step 4: Verify it compiles**

```bash
cargo check -p proof_running_system
```

- [ ] **Step 5: Write a unit test**

Add to `proof_running_system/src/proving_oracle.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use airbender_codec::{AirbenderCodec, AirbenderCodecV0};
    use airbender_core::wire::frame_words_from_bytes;
    use airbender_guest::transport::MockTransport;

    fn mock_oracle_with<T: serde::Serialize>(values: &[&T]) -> ProvingOracle<MockTransport> {
        let mut words = Vec::new();
        for value in values {
            let bytes = AirbenderCodecV0::encode(*value).expect("encode");
            let framed = frame_words_from_bytes(&bytes).expect("frame");
            words.extend(framed);
        }
        ProvingOracle::new(MockTransport::new(words))
    }

    #[test]
    fn reads_sequential_typed_values() {
        let mut oracle = mock_oracle_with(&[&42u32, &true, &0xDEADBEEFu64]);

        let v1: u32 = oracle.query(0, &()).unwrap();
        let v2: bool = oracle.query(0, &()).unwrap();
        let v3: u64 = oracle.query(0, &()).unwrap();

        assert_eq!(v1, 42);
        assert_eq!(v2, true);
        assert_eq!(v3, 0xDEADBEEF);
    }

    #[test]
    fn ignores_query_type_and_input() {
        let mut oracle = mock_oracle_with(&[&99u32]);

        // Different query_type and non-trivial input — should be ignored
        let result: u32 = oracle.query(0x40070000, &(1u32, 2u32, 3u32)).unwrap();
        assert_eq!(result, 99);
    }
}
```

- [ ] **Step 6: Run tests**

```bash
cargo test -p proof_running_system -- proving_oracle
```

Expected: 2 tests pass.

- [ ] **Step 7: Commit**

```bash
git add proof_running_system/src/proving_oracle.rs proof_running_system/src/lib.rs proof_running_system/Cargo.toml
git commit -m "feat(proof_running_system): add ProvingOracle

Implements SerdeIOOracle by reading pre-computed values from an
airbender Transport. Ignores query_type and input — all responses
come from the witness stream."
```

---

### Task 4: Implement WitnessRecordingOracle

**Files:**
- Create: `oracle_provider/src/witness_recording.rs`
- Modify: `oracle_provider/src/lib.rs`
- Modify: `oracle_provider/Cargo.toml`

- [ ] **Step 1: Add dependencies**

In `oracle_provider/Cargo.toml`, add:

```toml
airbender-host = { workspace = true }
airbender-codec = { workspace = true }
serde = { workspace = true, default-features = false, features = ["derive"] }
```

- [ ] **Step 2: Create WitnessRecordingOracle**

Create `oracle_provider/src/witness_recording.rs`:

```rust
use airbender_host::Inputs;
use serde::de::DeserializeOwned;
use serde::Serialize;
use zk_ee::oracle::serde_oracle::SerdeIOOracle;
use zk_ee::system::errors::internal::InternalError;
use zk_ee::internal_error;

pub struct WitnessRecordingOracle<O: SerdeIOOracle> {
    inner: O,
    inputs: Inputs,
}

impl<O: SerdeIOOracle> WitnessRecordingOracle<O> {
    pub fn new(inner: O) -> Self {
        Self {
            inner,
            inputs: Inputs::new(),
        }
    }

    pub fn into_inputs(self) -> (O, Inputs) {
        (self.inner, self.inputs)
    }
}

impl<O: SerdeIOOracle> SerdeIOOracle for WitnessRecordingOracle<O> {
    fn query<I: Serialize, R: DeserializeOwned>(
        &mut self,
        query_type: u32,
        input: &I,
    ) -> Result<R, InternalError> {
        let response: R = self.inner.query(query_type, input)?;
        self.inputs
            .push(&response)
            .map_err(|e| internal_error!("witness recording failed"))?;
        Ok(response)
    }
}
```

- [ ] **Step 3: Wire into crate**

In `oracle_provider/src/lib.rs`, add:

```rust
pub mod witness_recording;
```

- [ ] **Step 4: Verify it compiles**

```bash
cargo check -p oracle_provider
```

- [ ] **Step 5: Write a roundtrip test**

Add to `oracle_provider/src/witness_recording.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use airbender_codec::{AirbenderCodec, AirbenderCodecV0};
    use airbender_core::wire::frame_words_from_bytes;
    use airbender_guest::input::read_with;
    use airbender_guest::transport::MockTransport;
    use zk_ee::oracle::serde_oracle::SerdeIOOracle;

    struct FixedOracle {
        values: Vec<Vec<u8>>,
        cursor: usize,
    }

    impl FixedOracle {
        fn new(values: Vec<Vec<u8>>) -> Self {
            Self { values, cursor: 0 }
        }
    }

    impl SerdeIOOracle for FixedOracle {
        fn query<I: Serialize, O: DeserializeOwned>(
            &mut self,
            _query_type: u32,
            _input: &I,
        ) -> Result<O, InternalError> {
            let bytes = &self.values[self.cursor];
            self.cursor += 1;
            AirbenderCodecV0::decode(bytes)
                .map_err(|_| zk_ee::internal_error!("decode failed"))
        }
    }

    fn fixed_oracle_with<T: Serialize>(values: &[T]) -> FixedOracle {
        let encoded: Vec<Vec<u8>> = values
            .iter()
            .map(|v| AirbenderCodecV0::encode(v).expect("encode"))
            .collect();
        FixedOracle::new(encoded)
    }

    #[test]
    fn roundtrip_recording_and_replay() {
        // Record
        let inner = fixed_oracle_with(&[42u32, 99u32, 7u32]);
        let mut recorder = WitnessRecordingOracle::new(inner);

        let v1: u32 = recorder.query(0, &()).unwrap();
        let v2: u32 = recorder.query(0, &()).unwrap();
        let v3: u32 = recorder.query(0, &()).unwrap();

        assert_eq!((v1, v2, v3), (42, 99, 7));

        // Extract witness
        let (_inner, inputs) = recorder.into_inputs();
        let witness_words = inputs.words().to_vec();

        // Replay via MockTransport (simulates ProvingOracle path)
        let mut transport = MockTransport::new(witness_words);
        let r1: u32 = read_with(&mut transport).unwrap();
        let r2: u32 = read_with(&mut transport).unwrap();
        let r3: u32 = read_with(&mut transport).unwrap();

        assert_eq!((r1, r2, r3), (42, 99, 7));
    }
}
```

- [ ] **Step 6: Run tests**

```bash
cargo test -p oracle_provider -- witness_recording
```

Expected: 1 test passes — confirming the record-then-replay roundtrip works.

- [ ] **Step 7: Commit**

```bash
git add oracle_provider/src/witness_recording.rs oracle_provider/src/lib.rs oracle_provider/Cargo.toml
git commit -m "feat(oracle_provider): add WitnessRecordingOracle

Wraps any SerdeIOOracle and records every response via
airbender Inputs::push(). Roundtrip test confirms witness can
be replayed through MockTransport."
```

---

### Task 5: Implement LegacyAdapter

This adapter wraps an old `IOOracle` impl and presents the new `SerdeIOOracle`
interface. It's temporary — used for testing and for the incremental migration
in Plan 2. It does double-serialization (serde → usize → serde) which is
acceptable as a transitional shim.

**Files:**
- Create: `oracle_provider/src/legacy_adapter.rs`
- Modify: `oracle_provider/src/lib.rs`

- [ ] **Step 1: Create the adapter**

Create `oracle_provider/src/legacy_adapter.rs`:

```rust
use airbender_codec::{AirbenderCodec, AirbenderCodecV0};
use serde::de::DeserializeOwned;
use serde::Serialize;
use zk_ee::oracle::serde_oracle::SerdeIOOracle;
use zk_ee::oracle::usize_serialization::{UsizeDeserializable, UsizeSerializable};
use zk_ee::oracle::IOOracle;
use zk_ee::system::errors::internal::InternalError;
use zk_ee::internal_error;

pub struct LegacyAdapter<O: IOOracle> {
    pub inner: O,
}

impl<O: IOOracle> LegacyAdapter<O> {
    pub fn new(inner: O) -> Self {
        Self { inner }
    }
}
```

Note: A full generic implementation of `SerdeIOOracle for LegacyAdapter<O>` is
complex because the old `raw_query` requires `UsizeSerializable` bounds that
serde types don't have. The adapter is best used for specific query types where
the input/output types are known. For the migration, each query processor will
be migrated individually — the adapter serves as a bridge for mixed old/new code.

- [ ] **Step 2: Wire into crate**

In `oracle_provider/src/lib.rs`, add:

```rust
pub mod legacy_adapter;
```

- [ ] **Step 3: Verify it compiles**

```bash
cargo check -p oracle_provider
```

- [ ] **Step 4: Commit**

```bash
git add oracle_provider/src/legacy_adapter.rs oracle_provider/src/lib.rs
git commit -m "feat(oracle_provider): add LegacyAdapter scaffold

Bridge between old IOOracle and new SerdeIOOracle interfaces.
Will be used during incremental migration in Plan 2."
```

---

### Task 6: Integration test — full record-replay with response types

**Files:**
- Modify: `oracle_provider/src/witness_recording.rs` (add test)

- [ ] **Step 1: Write integration test with callable oracle response types**

Add to the `tests` module in `oracle_provider/src/witness_recording.rs`:

```rust
    use basic_bootloader::bootloader::oracle_types::{
        DivRemResponse, ModexpResponse, FieldSqrtResponse,
    };
    use zk_ee::utils::Bytes32;

    #[test]
    fn roundtrip_mixed_types_including_callable_oracle_responses() {
        let block_metadata_value = 42u32;
        let div_rem = DivRemResponse {
            quotient: [1, 2, 3, 4],
        };
        let modexp = ModexpResponse {
            quotient: vec![0xAA, 0xBB],
            remainder: vec![0xCC],
        };
        let field_sqrt = FieldSqrtResponse {
            result: Bytes32::zero(),
            is_valid: true,
        };

        // Record all responses
        let inner = fixed_oracle_with_any(&[
            &block_metadata_value as &dyn erased_serde::Serialize,
            &div_rem,
            &modexp,
            &field_sqrt,
        ]);
        let mut recorder = WitnessRecordingOracle::new(inner);

        let _: u32 = recorder.query(0x40070000, &()).unwrap();
        let _: DivRemResponse = recorder.query(0x40050030, &(0u32,)).unwrap();
        let _: ModexpResponse = recorder.query(0x40050010, &(0u32,)).unwrap();
        let _: FieldSqrtResponse = recorder.query(0x40050011, &(0u32,)).unwrap();

        // Replay
        let (_, inputs) = recorder.into_inputs();
        let mut transport = MockTransport::new(inputs.words().to_vec());

        let r1: u32 = read_with(&mut transport).unwrap();
        let r2: DivRemResponse = read_with(&mut transport).unwrap();
        let r3: ModexpResponse = read_with(&mut transport).unwrap();
        let r4: FieldSqrtResponse = read_with(&mut transport).unwrap();

        assert_eq!(r1, 42);
        assert_eq!(r2.quotient, [1, 2, 3, 4]);
        assert_eq!(r3.quotient, vec![0xAA, 0xBB]);
        assert_eq!(r3.remainder, vec![0xCC]);
        assert!(r4.is_valid);
    }
```

Note: The `fixed_oracle_with_any` helper needs to handle heterogeneous types.
This can be done by pre-encoding each value to bytes and storing them:

```rust
    fn fixed_oracle_with_any(values: &[&dyn erased_serde::Serialize]) -> FixedOracle {
        let encoded: Vec<Vec<u8>> = values
            .iter()
            .map(|v| {
                let bytes = AirbenderCodecV0::encode_erased(*v).expect("encode");
                bytes
            })
            .collect();
        FixedOracle::new(encoded)
    }
```

If `erased_serde` is not available, use a simpler approach — pre-encode each
value individually and pass as `Vec<Vec<u8>>`:

```rust
    fn encode_value<T: Serialize>(v: &T) -> Vec<u8> {
        AirbenderCodecV0::encode(v).expect("encode")
    }

    #[test]
    fn roundtrip_mixed_types_including_callable_oracle_responses() {
        let inner = FixedOracle::new(vec![
            encode_value(&42u32),
            encode_value(&DivRemResponse { quotient: [1, 2, 3, 4] }),
            encode_value(&ModexpResponse {
                quotient: vec![0xAA, 0xBB],
                remainder: vec![0xCC],
            }),
            encode_value(&FieldSqrtResponse {
                result: Bytes32::zero(),
                is_valid: true,
            }),
        ]);
        let mut recorder = WitnessRecordingOracle::new(inner);

        let _: u32 = recorder.query(0x40070000, &()).unwrap();
        let _: DivRemResponse = recorder.query(0x40050030, &(0u32,)).unwrap();
        let _: ModexpResponse = recorder.query(0x40050010, &(0u32,)).unwrap();
        let _: FieldSqrtResponse = recorder.query(0x40050011, &(0u32,)).unwrap();

        let (_, inputs) = recorder.into_inputs();
        let mut transport = MockTransport::new(inputs.words().to_vec());

        let r1: u32 = read_with(&mut transport).unwrap();
        let r2: DivRemResponse = read_with(&mut transport).unwrap();
        let r3: ModexpResponse = read_with(&mut transport).unwrap();
        let r4: FieldSqrtResponse = read_with(&mut transport).unwrap();

        assert_eq!(r1, 42);
        assert_eq!(r2.quotient, [1, 2, 3, 4]);
        assert_eq!(r3.quotient, vec![0xAA, 0xBB]);
        assert_eq!(r3.remainder, vec![0xCC]);
        assert!(r4.is_valid);
    }
```

- [ ] **Step 2: Run tests**

```bash
cargo test -p oracle_provider -- roundtrip_mixed_types
```

Expected: passes.

- [ ] **Step 3: Commit**

```bash
git add oracle_provider/src/witness_recording.rs
git commit -m "test(oracle_provider): roundtrip test with callable oracle response types

Verifies that DivRemResponse, ModexpResponse, FieldSqrtResponse
can be recorded via WitnessRecordingOracle and replayed through
MockTransport — the full witness pipeline."
```

---

## Checkpoint

At this point:
- `SerdeIOOracle` trait exists in `zk_ee`
- `ProvingOracle` reads from airbender transport (tested)
- `WitnessRecordingOracle` records to airbender `Inputs` (tested)
- `LegacyAdapter` scaffold exists for bridging old/new during migration
- Callable oracle response types exist with serde derives
- Roundtrip test proves the full record→replay pipeline works
- **All existing code still compiles and works unchanged**

Plan 2 will: replace `IOOracle` with `SerdeIOOracle`, migrate all call sites,
migrate query processors, wire into the guest binary and test rig, delete old code.
