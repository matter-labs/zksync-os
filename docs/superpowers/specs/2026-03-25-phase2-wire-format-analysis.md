# Phase 2 Wire Format Migration: Detailed Analysis

## Current System (UsizeSerializable)

### How It Works

The guest and host communicate via a **bidirectional query protocol** over CSR registers. Each oracle query follows a strict write-then-read sequence:

```
Guest writes:  query_id (u32) → input_length (usize) → input_words (N × usize)
Guest reads:   response_length (usize) → response_words (M × usize)
```

Data is serialized via the `UsizeSerializable` / `UsizeDeserializable` traits, which convert Rust types into fixed-length sequences of `usize` values. The `SimpleOracleQuery` trait wraps this into a typed interface:

```rust
pub trait SimpleOracleQuery {
    const QUERY_ID: u32;
    type Input: UsizeSerializable + UsizeDeserializable;
    type Output: UsizeDeserializable;
}
```

On the host side, `ZkEENonDeterminismSource` buffers incoming writes, dispatches completed queries to registered `OracleQueryProcessor` implementations, and serves results back as iterators.

### Scale

| Category | Count |
|----------|-------|
| Types implementing UsizeSerializable | ~30 (12 primitives + 18+ complex) |
| Distinct query IDs | 57 |
| OracleQueryProcessor implementations | 18 |
| Architecture-specific branches (32/64-bit) | Throughout all serialization code |

### CSR Cost Per Query

| Phase | CSR ops |
|-------|---------|
| Write query ID | 1 |
| Write input length | 1 |
| Write input words | N (arch-dependent: u64 = 2 words on 32-bit, 1 on 64-bit) |
| Read response length | 1 |
| Read response words | M |
| **Total** | **3 + N + M** |

Examples:
- Storage slot query (key → value): ~15 CSR ops
- Block metadata (no input → ~300 words): ~302 CSR ops
- ModExp advice (pointer → large result): up to thousands

### 32/64-bit Bridging

The guest is 32-bit RISC-V, the host is 64-bit. All data crosses this boundary:
- Guest writes u32 values via CSR
- Host combines pairs into u64: `buffer.push((high << 32) | low)`
- Host results split u64 into two u32 halves: `high_half` cached for next read
- `UsizeSerializable` encodes u64 as 2 words on 32-bit, 1 word on 64-bit
- Every impl has `cfg_if!` branches for pointer width

### Strengths

1. **Zero-copy for fixed types**: U256, B160, Bytes32 serialize as direct word sequences — no encoding overhead, no heap allocation
2. **Compile-time size**: `USIZE_LEN` is a `const` — the compiler knows exact sizes, enabling stack allocation and loop unrolling
3. **No serde dependency**: Works without `serde` or `bincode` in the no_std guest binary
4. **Minimal guest-side code**: `CsrBasedIOOracle` is ~50 lines — just writes words and reads words
5. **RamPeek pattern**: Callable oracles read guest memory directly via pointers, avoiding large data copies through CSR

### Weaknesses

1. **Manual impls for every type**: Each new type needs hand-written `UsizeSerializable` + `UsizeDeserializable` with `cfg_if!` for both architectures. Error-prone and tedious.
2. **Architecture-coupled**: Every impl is 32/64-bit aware. Adding big-endian support would double the code.
3. **No schema evolution**: Adding a field to a serialized struct is a breaking change — there's no length prefix or field tags.
4. **DynUsizeIterator unsafe**: Returning owned data as iterators requires `unsafe` lifetime transmute via `DynUsizeIterator`. This is a correctness risk.
5. **Query dispatch overhead**: 57 query IDs, each with its own processor registration, dispatch via BTreeMap lookup.
6. **No error recovery**: If serialization is wrong, the guest/host protocol becomes desynchronized (reads garbage or hangs). No framing to re-sync.
7. **Duplicate oracle state**: Forward mode and proving mode create separate oracle instances that must produce identical results. `ReadWitnessSource` exists to enforce this, but it's an extra layer.

---

## Platform Approach (AirbenderCodecV0)

### How It Works

All input is **pre-encoded as a flat `[u32]` stream** before the VM starts. The guest reads values sequentially — no query IDs, no bidirectional protocol.

```
Host encodes:  value → bincode → bytes → wire frame (length + BE u32 words) → append to stream
Guest reads:   length (u32) → payload words → bincode decode → typed value
```

Serialization uses `AirbenderCodecV0` (bincode v2 with `config::standard()`). Types must implement `serde::Serialize` + `serde::Deserialize`.

### Wire Framing

Each value is framed independently:
```
Word 0:        payload byte length (u32)
Words 1..N:    payload bytes in big-endian u32 chunks (zero-padded)
```

### CSR Cost Per Value

```
Total CSR reads = 1 (length) + ceil(encoded_size / 4)
```

| Type | Encoded size | CSR reads |
|------|-------------|-----------|
| u32 | 4 bytes | 2 |
| u64 | 8 bytes | 3 |
| bool | 1 byte | 2 |
| [u32; 8] | 32 bytes | 9 |
| Vec\<u8\> (100 bytes) | ~101 bytes | 27 |
| Complex struct (50 bytes) | ~50 bytes | 14 |

### Strengths

1. **Derive-based**: `#[derive(Serialize, Deserialize)]` — no manual serialization code. Adding a field is trivial.
2. **Schema evolution**: Bincode handles optional fields, enum variants. Versioned via `AIRBENDER_CODEC_V0` constant.
3. **No architecture coupling**: One encoding format, no 32/64-bit branching. Bincode handles endianness.
4. **Framing**: Each value has an explicit length prefix. Desynchronization is detectable.
5. **no_std compatible**: Bincode v2 + serde work in no_std with `alloc` only.
6. **Ecosystem standard**: Serde is the Rust serialization standard. Any type with serde derives works.
7. **No unsafe**: No `DynUsizeIterator`, no lifetime transmutes. Codec is pure safe Rust.
8. **Simpler host**: No query dispatch, no `OracleQueryProcessor` trait, no `BTreeMap` lookup. Just push values in order.

### Weaknesses

1. **Encoding overhead**: Bincode adds varint prefixes for lengths, enum tags. ~5-15% overhead over raw memory layout.
2. **Heap allocation**: Both encoding and decoding allocate `Vec<u8>`. In a RISC-V guest with limited heap, this matters.
3. **No random access**: Values must be read in push order. Can't ask "what's storage slot X?" — all data must be pre-computed.
4. **serde dependency**: Adds ~30KB to the guest binary (serde + bincode). All serialized types must derive serde traits.
5. **No RamPeek equivalent**: Can't read guest memory from host side during execution. The guest must serialize all operands explicitly. (Note: the user mentioned this will be added as a platform low-level API.)
6. **Sequential coupling**: Push order on host must exactly match read order on guest. One mismatch → total failure.
7. **Wire frame overhead**: 1 extra u32 per value (length prefix) + padding. For small values (bool, u32), this doubles the CSR cost vs. raw.

---

## Head-to-Head Comparison

### Performance

| Metric | Current (UsizeSerializable) | Platform (AirbenderCodecV0) |
|--------|---------------------------|----------------------------|
| CSR ops for u32 | 5 (3 protocol + 1 write + 1 read) | 2 (1 length + 1 data) |
| CSR ops for U256 | ~11 (3 + 4 write + 4 read) | 10 (1 length + 9 data) |
| CSR ops for storage query | ~15 (query + response) | ~24 (key frame + value frame, each with headers) |
| Encoding overhead | Zero (direct word copy) | ~5-15% (bincode varint + padding) |
| Heap allocation | None (iterator-based) | Yes (Vec\<u8\> per encode/decode) |
| 32/64 bridging cost | Every query (high_half caching) | None (fixed u32 words) |

**Verdict**: Current system has lower per-query CSR overhead for simple types because there's no framing. Platform has lower overhead for the overall session because there's no query dispatch protocol (no query ID, no input length, no response length). For a block execution with hundreds of queries, the total CSR count may be **comparable**.

The key difference: current system pays 3 CSR ops per query for protocol overhead. Platform pays 1 CSR op per value for length framing. If a "query" returns 1 value, platform wins (2 vs 5 CSR ops). If a "query" has complex multi-field input/output, current wins slightly (no per-field framing).

**In practice**: The dominant cost is the proving circuit, not CSR operations. Wire format performance is negligible.

### Code Simplification

| Aspect | Current | Platform | Delta |
|--------|---------|----------|-------|
| Serialization trait impls | ~30 manual impls, ~800 lines | `#[derive]` on each type, ~0 lines | -800 lines |
| Architecture branching | `cfg_if!` in every impl | None | Eliminates entire category |
| DynUsizeIterator unsafe | 1 unsafe pattern used everywhere | Not needed | Removes unsafe |
| Query dispatch (OracleQueryProcessor) | 18 processors, BTreeMap dispatch | Not needed for proving path | Major simplification |
| ZkEENonDeterminismSource state machine | ~200 lines (buffer, bridging, dispatch) | Replaced by `QuasiUARTSource` | -200 lines |
| Guest IOOracle impl | ~50 lines (CsrBasedIOOracle) | `airbender::guest::read::<T>()` | -50 lines |
| Host query processors | 13 files in forward_system/query_processors | Remain for forward mode | No change |

**Total estimated code reduction**: ~1000-1200 lines removed from serialization + oracle dispatch.

**New code needed**: `serde` derives on ~30 types, possibly custom `Serialize`/`Deserialize` for a few complex types (U256, B160).

### Flexibility

| Aspect | Current | Platform |
|--------|---------|----------|
| Add new type | Write 2 trait impls + cfg_if | Add `#[derive(Serialize, Deserialize)]` |
| Add field to struct | Breaking change (fixed word count) | Non-breaking (bincode handles it) |
| Add new query type | New query ID + processor + SimpleOracleQuery impl | Just `push()` on host, `read()` on guest |
| Support new architecture | Double all impls for new pointer width | No change |
| Custom encoding | Full control | Limited to bincode's format |
| Schema versioning | None | Built-in via codec version |

### Migration Effort

#### What Needs to Change

**Guest side (proof_running_system + zk_ee)**:
1. Replace `CsrBasedIOOracle::raw_query()` with sequential `airbender::guest::read::<T>()` calls
2. Remove `SimpleOracleQuery` trait and all impls
3. Remove `UsizeSerializable`/`UsizeDeserializable` traits and all impls
4. Add `serde` derives to all oracle data types (~30 types)
5. Replace the query-response pattern in `basic_bootloader` with sequential reads

**Host side (oracle_provider + forward_system)**:
1. `ReadWitnessSource` changes to use `Inputs::push()` to record data
2. Forward mode keeps `OracleQueryProcessor` (still needs interactive resolution)
3. Recording format changes from raw `Vec<u32>` to platform's `Inputs` format
4. `ZkEENonDeterminismSource` state machine simplified or removed for proving path

**Callable oracles (modexp, field ops, blob KZG)**:
1. Guest must serialize operands explicitly instead of passing pointers
2. OR: use the upcoming platform RamPeek API to maintain current pattern
3. This is the biggest unknown — depends on platform API availability

**Test rig**:
1. Forward run recording switches to `Inputs::push()` format
2. Transpiler run input is already `&[u32]` (Phase 1)
3. Prover input format changes

#### Estimated Scope

| Area | Files | Effort |
|------|-------|--------|
| Serde derives on types | ~15 files in zk_ee, basic_system | Medium — mostly mechanical |
| Remove UsizeSerializable | ~5 files in zk_ee/oracle | Medium — delete + verify |
| Remove CsrBasedIOOracle | 1 file in proof_running_system | Small |
| Rewrite bootloader data acquisition | ~5 files in basic_bootloader | Large — core logic change |
| Update ReadWitnessSource | 1 file in oracle_provider | Medium |
| Update forward_system processors | 0 files (keep for forward mode) | None |
| Callable oracles | 3 files | Medium-Large (depends on RamPeek API) |
| Tests | ~10 files | Medium — update assertions |
| **Total** | ~40 files | **3-5 weeks estimated** |

### Risk Assessment

| Risk | Severity | Mitigation |
|------|----------|------------|
| Bincode format instability | Low | Pinned to bincode v2.0.x, versioned codec |
| Serde binary size increase | Medium | Measure before/after, ~30KB expected |
| RamPeek not available in platform | High | Block callable oracle migration until API exists |
| Sequential read order bugs | Medium | Extensive testing, CI verification |
| Performance regression | Low | CSR ops are negligible vs. proving cost |
| Forward/proving mode divergence | Medium | ReadWitnessSource ensures identical recordings |

---

## Recommendation

### Phasing Strategy

**Phase 2a: Add serde derives (non-breaking)**
- Add `#[derive(Serialize, Deserialize)]` to all oracle types alongside existing `UsizeSerializable`
- Both systems work simultaneously
- No runtime behavior change
- Validates serde compatibility with all types

**Phase 2b: Switch proving path to platform codec**
- Guest binary switches to `airbender::guest::read::<T>()` for reading oracle data
- `ReadWitnessSource` switches to `Inputs::push()` format for recording
- Forward mode keeps `UsizeSerializable` + interactive oracles (unchanged)
- Callable oracles: wait for platform RamPeek API

**Phase 2c: Clean up**
- Remove `UsizeSerializable`/`UsizeDeserializable` traits
- Remove `SimpleOracleQuery`, `DynUsizeIterator`
- Remove `CsrBasedIOOracle`
- Remove 32/64-bit bridging code from `ZkEENonDeterminismSource`

### Key Blocker

The callable oracles (modexp, field ops, blob KZG) that use `RamPeek` are the **critical path**. These oracles read large operands directly from guest memory via pointers. Without the platform's RamPeek API, the guest would need to serialize up to 32MB of data through CSR (thousands of CSR reads for a single modexp call). This is unacceptable for performance.

**Wait for the platform RamPeek API before starting Phase 2b.**

---

## Platform Codec Improvement Opportunities

Independent of the zksync-os migration, the airbender-platform codec itself can be
improved. These are suggestions for the platform team.

### 1. Eliminate heap allocation on every `read::<T>()`

**Problem**: Every `read::<T>()` call allocates a `Vec<u8>` via
`read_framed_bytes_with()` to buffer the framed bytes, then passes the slice to
bincode for decoding. For a block execution with hundreds of reads, that's
hundreds of short-lived allocations in the RISC-V guest.

**Fix**: Implement bincode's `Reader` trait for a CSR-backed reader with a small
inline buffer (4 bytes — one u32 word):

```rust
struct CsrReader {
    buf: [u8; 4],
    pos: usize,
    len: usize,
}

impl bincode::de::read::Reader for CsrReader {
    fn read(&mut self, bytes: &mut [u8]) -> Result<(), DecodeError> {
        // Fill bytes from buf, refill from CSR read_word() as needed
    }
}
```

Then decode directly: `bincode::decode_from_reader::<T, _, _>(csr_reader, config)`
— zero heap allocation for fixed-size types.

**Caveat**: bincode's `decode_from_reader` uses the `Decode` trait, not serde's
`Deserialize`. Types would need `#[derive(bincode::Decode)]` or a serde compat
path. If that's too invasive, a simpler approach: use a stack-allocated scratch
buffer (e.g., `[u8; 256]`) for small types, falling back to heap for large ones.

**Impact**: High. Eliminates the dominant allocation hotspot in the guest read path.
**Effort**: High. Requires either dual trait support or switching from serde to
bincode-native Decode.

### 2. Use fixed-int encoding instead of varint

**Problem**: `config::standard()` uses varint encoding. A u32 value encodes as
1-5 bytes depending on magnitude. For ZK workloads where most values are
full-width (U256 components, storage keys, hashes), varint adds overhead — the
values are almost always large, so the varint prefix byte is pure waste.

Varint encoding sizes:
- u < 251: 1 byte
- 251 ≤ u < 2^16: 3 bytes
- 2^16 ≤ u < 2^32: 5 bytes (vs. 4 bytes fixed)
- 2^32 ≤ u < 2^64: 9 bytes (vs. 8 bytes fixed)

**Fix**: Switch to `config::standard().with_fixed_int_encoding()`:
- u32 is always 4 bytes, u64 is always 8 bytes
- No varint parsing overhead
- For hash/crypto-heavy data (the dominant case in zksync-os), this is both
  faster and often smaller

**Trade-off**: Slightly larger for payloads dominated by small integers (counters,
booleans). But zksync-os data is overwhelmingly U256/B160/Bytes32.

**Versioning**: This changes the wire format. Should be a new codec version (V1)
coexisting with V0 via the manifest codec field.

**Impact**: Medium. Removes varint overhead on crypto-heavy types.
**Effort**: Low. Single config change + version bump.

### 3. Skip framing for fixed-size types

**Problem**: Every value gets a 4-byte length prefix, even when the size is known
at compile time. Reading a `u32` costs 2 CSR reads (length + data) instead of 1.
For small fixed-size types, framing overhead is 50-300%.

**Fix**: Add a `read_fixed::<T>()` variant where `T: FixedSize` provides
`const SIZE: usize`. Skip the length word entirely:

```rust
pub fn read_fixed<T: FixedSize + DeserializeOwned>() -> T {
    // Read ceil(T::SIZE / 4) words directly — no length prefix
    let mut bytes = [0u8; T::SIZE];
    for chunk in bytes.chunks_mut(4) {
        let word = transport.read_word().to_be_bytes();
        chunk.copy_from_slice(&word[..chunk.len()]);
    }
    AirbenderCodecV0::decode(&bytes)
}
```

For zksync-os, the most frequently read types (U256, B160, Bytes32, storage keys)
are all fixed-size. This halves the CSR cost for small types.

**Impact**: Medium. Saves 1 CSR read per fixed-size value.
**Effort**: Medium. New trait + API variant.

### 4. Batch reads for structured queries

**Problem**: Reading related values separately (e.g., storage key then value)
means 2 frames with 2 length prefixes. The old query model sent everything as
one query.

**Fix**: Group related values into structs or tuples and read as a single frame:

```rust
// One frame, one decode, one length prefix
let query_result: (StorageKey, StorageValue) = read()?;
```

This is already possible with tuples and structs — the key is to design the data
layout so related values are grouped. This is a design guideline, not a code
change.

**Impact**: Medium. Fewer frames, fewer CSR reads.
**Effort**: Low. Design choice during migration.

### 5. Switch wire framing to little-endian

**Problem**: Wire framing uses big-endian u32 words (`u32::from_be_bytes`), but
RISC-V is little-endian. Every word requires a byte swap (single instruction but
adds up over thousands of reads).

**Fix**: Switch `frame_words_from_bytes` and `read_framed_bytes_with` to use
`u32::from_le_bytes` / `u32::to_le_bytes`. The transpiler's `QuasiUARTSource`
already operates on native-endian u32 words.

**Impact**: Low-Medium. Removes one micro-op per word.
**Effort**: Low. Two-line change in wire.rs.

### 6. Eliminate host-side double allocation in `push()`

**Problem**: `Inputs::push(&value)` performs:
1. `encode_to_vec()` → allocates `Vec<u8>`
2. `frame_words_from_bytes()` → allocates `Vec<u32>`
3. `self.words.extend()` → copies into final buffer

Two temporary allocations per push.

**Fix**: Implement a custom bincode `Writer` that encodes directly into the u32
word stream:
1. First pass: compute encoded size (bincode has `encoded_size()`)
2. Write length word to `self.words`
3. Encode directly into word-aligned buffer via custom Writer

Eliminates both intermediate allocations.

**Impact**: Medium. Removes 2 allocations per push (host-side only, less critical
than guest).
**Effort**: Medium. Custom bincode Writer implementation.

### Summary Table

| Improvement | Impact | Effort | Blocks on |
|------------|--------|--------|-----------|
| Fixed-int encoding (V1 codec) | Medium | Low | Nothing |
| Batch reads via structs | Medium | Low (design choice) | Nothing |
| LE wire framing | Low-Medium | Low | Nothing |
| Skip framing for fixed-size | Medium | Medium | Nothing |
| CSR Reader (no heap alloc) | High | High | bincode Decode vs serde |
| Direct word encoding on host | Medium | Medium | Nothing |

Quick wins: fixed-int encoding and struct batching. Big win: CSR Reader.

---

### Bottom Line

The migration is worthwhile for code quality (eliminates ~1000 lines of manual serialization, removes unsafe code, eliminates architecture coupling) but has negligible performance impact. The main risk is the RamPeek dependency. Start with Phase 2a (serde derives) which is risk-free and validates the approach.

The platform codec itself can be improved independently — fixed-int encoding and
LE wire framing are low-effort changes that benefit all platform users, not just
zksync-os.
