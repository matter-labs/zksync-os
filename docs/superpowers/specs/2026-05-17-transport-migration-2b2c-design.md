# Phase 2b+2c: Transport Migration — IOOracle to Airbender Platform

## Goal

Replace the custom usize-serialization-based oracle protocol with airbender-platform's
serde/bincode transport. All three execution passes (forward, prover input, proving)
use the same `IOOracle` trait with serde-based signatures. The proving pass reads
all responses — including callable oracle responses — from a pre-computed witness
built with `Inputs::push::<T>()`.

## Architecture

### IOOracle trait (serde-based)

Replaces the current usize-iterator-based trait:

```rust
pub trait IOOracle: 'static + Sized {
    fn query<I: Serialize, O: DeserializeOwned>(
        &mut self,
        query_type: u32,
        input: &I,
    ) -> Result<O, InternalError>;
}
```

Default convenience methods stay on the trait, calling `self.query()`:
- `query_with_empty_input::<O>(query_type)` — calls `self.query(query_type, &())`
- `try_begin_next_tx()` — calls `query_with_empty_input::<u32>()`, returns `Option<NonZeroU32>`
- `get_bytes_from_query(len_id, body_id, input)` — two sequential queries (length then body)

### Three implementations

**Forward mode** (`ZkEENonDeterminismSource`):

Dispatches to query processors by query_type. Input is serialized and passed to
the processor. Response bytes are deserialized to `O`.

```rust
fn query<I: Serialize, O: DeserializeOwned>(
    &mut self, query_type: u32, input: &I
) -> Result<O, _> {
    let input_bytes = AirbenderCodecV0::encode(input)?;
    let response_bytes = self.dispatch(query_type, &input_bytes);
    Ok(AirbenderCodecV0::decode(&response_bytes)?)
}
```

**Prover input mode** (`WitnessRecordingOracle<Inner: IOOracle>`):

Wraps any IOOracle. Delegates to inner, records every response via
`Inputs::push::<O>()`. Replaces `ReadWitnessSource`.

```rust
fn query<I: Serialize, O: DeserializeOwned>(
    &mut self, query_type: u32, input: &I
) -> Result<O, _> {
    let response: O = self.inner.query(query_type, input)?;
    self.inputs.push(&response)?;
    Ok(response)
}
```

Output is `Inputs` (airbender wire format: framed bincode u32 words).

**Proving mode** (`ProvingOracle`):

Ignores query_type and input. Reads next typed value from the pre-computed
witness stream via airbender guest transport.

```rust
fn query<I: Serialize, O: DeserializeOwned>(
    &mut self, _query_type: u32, _input: &I
) -> Result<O, _> {
    Ok(airbender_guest::read::<O>()?)
}
```

### OracleQueryProcessor trait

Changes from returning `Box<dyn ExactSizeIterator<Item = usize>>` to returning
serialized bytes:

```rust
pub trait OracleQueryProcessor {
    fn supported_query_ids(&self) -> Vec<u32>;
    fn process(&mut self, query_id: u32, input: &[u8]) -> Result<Vec<u8>, InternalError>;
}
```

Each of the 13 query processor implementations in `forward_system/src/run/query_processors/`
updates to deserialize input from bytes, serialize response to bytes.

The callable oracle processors (ArithmeticQuery, FieldOpsQuery,
BlobCommitmentAndProofQuery) lose the `memory: &dyn RamPeek` parameter — they only
run in forward/prover-input mode where they use `NativeArithmeticQuery` (reads
host process memory directly). The RISC-V variants (`ArithmeticQuery` with RamPeek)
are no longer needed for the main execution path.

### Witness format

Produced by pass 2 via `WitnessRecordingOracle`. Format is airbender `Inputs`:
each response is `push::<T>(value)` which encodes via `AirbenderCodecV0` (bincode v2)
and frames into u32 words (length prefix + big-endian payload).

Consumed by pass 3 via `runner.run(inputs.words())`. The guest reads sequentially
with `airbender_guest::read::<T>()`.

ALL responses go into the witness — both pre-computable (block metadata, tx data,
preimages, etc.) and callable oracle responses (modexp, division, field ops, blob KZG).
Callable oracle responses are computed in pass 2 using native processors and recorded
the same way as everything else.

### Call site changes

Call sites change minimally. The method name and type parameters change, but the
pattern stays the same:

```rust
// Before:
let metadata: BlockMetadataFromOracle = oracle.query_serializable(BLOCK_METADATA_QUERY_ID, &())?;

// After:
let metadata: BlockMetadataFromOracle = oracle.query(BLOCK_METADATA_QUERY_ID, &())?;
```

For `raw_query` call sites that use iterator-based access (preimages, tx data),
the response type changes to `Vec<u8>` or a domain-specific struct.

For callable oracle call sites that currently pass pointers:

```rust
// Before:
let quotient = oracle.raw_query(U256_DIV_REM_ADVICE_QUERY_ID, &(ptr as u32))?;

// After:
let response: DivRemResponse = oracle.query(U256_DIV_REM_ADVICE_QUERY_ID, &(ptr as u32))?;
```

Callable oracle call sites **keep passing pointers** — a u32/u64 serializes trivially
via serde, no large data copies. The `cfg(target_pointer_width)` branches stay.
In forward/prover-input mode, the processor receives the pointer and reads operands
from host process memory. In proving mode, the input is ignored entirely (response
is pre-computed). Serializing large operands (e.g., modexp bigints) through the
oracle would be expensive and unnecessary.

### Response types for callable oracles

New serde-serializable response types:

| Query | Response type | Fields |
|-------|--------------|--------|
| U256_DIV_REM | `DivRemResponse` | `quotient: [u64; 4]` |
| U256_WIDE_DIV_REM | `WideDivRemResponse` | `quotient: ([u64; 4], [u64; 4])` |
| MODEXP | `ModexpResponse` | `quotient: Vec<u64>, remainder: Vec<u64>` |
| FIELD_OPS (sqrt) | `(Bytes32, bool)` | result + sign flag |
| FIELD_OPS (inverse) | `Bytes32` | result |
| BLOB_KZG | `KZGCommitmentAndProof` | `commitment: [u8; 48], proof: [u8; 48]` |

### Deletions (Phase 2c)

- `UsizeSerializable` trait + all ~30 impls
- `UsizeDeserializable` trait + all ~30 impls
- `SimpleOracleQuery` trait + all impls
- `CsrBasedIOOracle` + `CsrBasedIOOracleIterator`
- `NonDeterminismCSRSourceImplementation` trait
- `DynUsizeIterator` (unsafe lifetime transmute)
- `ExactSizeChain`, `ExactSizeChainN` (iterator composition helpers)
- `ReadWitnessSource` (replaced by `WitnessRecordingOracle`)
- `QueryBuffer` and 32/64-bit bridging logic in `ZkEENonDeterminismSource`
- `usize_serialization/` module in zk_ee
- `query_ids.rs` constants (query IDs still exist but the dispatch namespace is simplified)
- RISC-V callable oracle variants (`ArithmeticQuery`, `FieldOpsQuery`,
  `BlobCommitmentAndProofQuery` with RamPeek) — only native variants are needed

### What stays

- `IOOracle` trait (new serde-based signature)
- `IOResponder` trait (if still used by forward system)
- Query IDs (used by forward-mode dispatch)
- `OracleQueryProcessor` trait (new byte-based signature)
- All 13 query processor implementations (updated signatures)
- Native callable oracle processors
- `DISCONNECT_ORACLE_QUERY_ID` (control signal, returns `()`)
- `UART_QUERY_ID` handling (debug logging, side-effect only)

## Pass execution flow (after migration)

```
Pass 1: Forward (native, in-process)
  oracle = ZkEENonDeterminismSource [IOOracle, serde-based]
  processors return serialized bytes
  -> BlockOutput

Pass 2: Prover input (native, in-process)
  oracle = WitnessRecordingOracle<ZkEENonDeterminismSource>
  records every response via Inputs::push::<O>()
  -> Inputs (airbender wire format)
  -> BlockOutput (asserted identical to pass 1)

Pass 3: RISC-V proving (transpiler)
  oracle = ProvingOracle [IOOracle, reads from witness]
  runner.run(inputs.words())
  guest reads via airbender_guest::read::<T>() internally
  -> [u32; 8] proof output
```

## Known optimization opportunities

**Guest-side allocation**: `airbender_guest::read::<T>()` allocates a `Vec<u8>` per
read via `read_framed_bytes_with()`. For the RISC-V guest this is the hot path.
Optimization options (deferred):
- Reusable scratch buffer in `ProvingOracle`
- CSR-backed bincode Reader (zero-alloc for fixed-size types)
- Fixed-size fast path skipping the length word

**Host-side processor serialization**: `OracleQueryProcessor::process()` allocates
`Vec<u8>` per response. Less critical since host runs natively. Can optimize with
pre-sized buffers if profiling shows it matters.

## Crates affected

| Crate | Changes |
|-------|---------|
| `zk_ee` | IOOracle trait, delete usize_serialization module, delete SimpleOracleQuery |
| `proof_running_system` | Delete CsrBasedIOOracle, add ProvingOracle |
| `oracle_provider` | Rewrite ZkEENonDeterminismSource (serde-based dispatch), delete ReadWitnessSource, add WitnessRecordingOracle |
| `basic_system` | Update IOOracle call sites, delete UsizeSerializable impls |
| `basic_bootloader` | Update IOOracle call sites, delete UsizeSerializable impls, new callable oracle response types |
| `callable_oracles` | Delete RISC-V variants, update native variants to byte-based interface |
| `forward_system` | Update 13 query processors to byte-based returns |
| `zksync_os` (RISC-V binary) | Use ProvingOracle instead of CsrBasedIOOracle |
| `zksync_os_runner` | Pass `Inputs.words()` instead of `Vec<u32>` witness |
| `tests/rig` | Update test harness for new witness format |
