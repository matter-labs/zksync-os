# WordLayout IO Format Design

## Goal

Replace the `UsizeSerializable`/`UsizeDeserializable` oracle serialization system with a
u32 word-aligned format that achieves optimal proving performance (direct u32 stores from
CSR transport) uniformly for all types, with a single code path and no special cases.

## Architecture

The new `WordLayout` trait defines serialization directly in terms of u32 words. Every
field is padded to word boundaries, eliminating byte-level alignment issues. A derive macro
generates implementations automatically. The `IOOracle` trait uses `WordLayout` bounds
instead of `UsizeSerializable`/`UsizeDeserializable`. The `ProvingOracle` becomes a trivial
one-liner that passes `transport.read_word()` to `T::read_words()`.

## Starting point

Branch `draft-0.4.0`. This design replaces:

- `UsizeSerializable` / `UsizeDeserializable` traits and all impls
- `usize_serialization` module in `zk_ee`
- `SimpleOracleQuery` trait
- The current `IOOracle` trait bounds (serde-based on draft-0.4.0)
- `CsrBasedIOOracle` proving oracle implementation

## WordLayout trait

```rust
/// Word-aligned serialization for oracle IO. Every field is padded to u32
/// word boundaries. The format is architecture-independent (always u32).
pub trait WordLayout: Sized {
    /// Fixed word count, or None for variable-size types (Vec<T>).
    const WORD_COUNT: Option<usize>;

    /// Serialize to a sequence of u32 LE words.
    fn write_words(&self, write: &mut impl FnMut(u32));

    /// Deserialize from a sequence of u32 LE words.
    fn read_words(read: &mut impl FnMut() -> u32) -> Self;
}
```

Crate: `zk_ee`, module: `zk_ee::oracle::word_layout`.

## Encoding rules

| Type | Words | Encoding |
|------|-------|----------|
| `bool`, `u8`, `u16`, `u32` | 1 | Zero-extended to u32 |
| `u64` | 2 | Low word, high word |
| `[u8; N]` | ceil(N/4) | Byte-packed into LE u32 words, last word zero-padded |
| `[T; N]` (T != u8) | N * T::WORD_COUNT | Element-wise concatenation |
| `Vec<u8>` | 1 + ceil(len/4) | u32 length word + byte-packed data |
| `Vec<T>` (T != u8) | 1 + len * T::WORD_COUNT | u32 length word + elements |
| Struct (all fixed fields, repr(C), no sub-word fields) | sum of field word counts | Direct bulk read as u32 stores into memory |
| Struct (all fixed fields, has sub-word fields or no repr(C)) | sum of field word counts | Field-by-field read, each word-aligned |
| Struct (any dynamic field) | None | Field-by-field read |

`WORD_COUNT` is `Some(n)` when the word count is statically known (all fixed-size fields),
`None` when variable (contains Vec or similar).

## Primitive impls (hand-written)

```rust
impl WordLayout for () {
    const WORD_COUNT: Option<usize> = Some(0);
    fn write_words(&self, _: &mut impl FnMut(u32)) {}
    fn read_words(_: &mut impl FnMut() -> u32) -> Self {}
}

impl WordLayout for bool {
    const WORD_COUNT: Option<usize> = Some(1);
    fn write_words(&self, w: &mut impl FnMut(u32)) { w(*self as u32); }
    fn read_words(r: &mut impl FnMut() -> u32) -> Self { r() != 0 }
}

impl WordLayout for u8 {
    const WORD_COUNT: Option<usize> = Some(1);
    fn write_words(&self, w: &mut impl FnMut(u32)) { w(*self as u32); }
    fn read_words(r: &mut impl FnMut() -> u32) -> Self { r() as u8 }
}

impl WordLayout for u16 {
    const WORD_COUNT: Option<usize> = Some(1);
    fn write_words(&self, w: &mut impl FnMut(u32)) { w(*self as u32); }
    fn read_words(r: &mut impl FnMut() -> u32) -> Self { r() as u16 }
}

impl WordLayout for u32 {
    const WORD_COUNT: Option<usize> = Some(1);
    fn write_words(&self, w: &mut impl FnMut(u32)) { w(*self); }
    fn read_words(r: &mut impl FnMut() -> u32) -> Self { r() }
}

impl WordLayout for u64 {
    const WORD_COUNT: Option<usize> = Some(2);
    fn write_words(&self, w: &mut impl FnMut(u32)) {
        w(*self as u32);
        w((*self >> 32) as u32);
    }
    fn read_words(r: &mut impl FnMut() -> u32) -> Self {
        let lo = r() as u64;
        let hi = r() as u64;
        lo | (hi << 32)
    }
}
```

## Byte array impls

```rust
impl<const N: usize> WordLayout for [u8; N] {
    const WORD_COUNT: Option<usize> = Some(N.div_ceil(4));

    fn write_words(&self, w: &mut impl FnMut(u32)) {
        for chunk in self.chunks(4) {
            let mut buf = [0u8; 4];
            buf[..chunk.len()].copy_from_slice(chunk);
            w(u32::from_le_bytes(buf));
        }
    }

    fn read_words(r: &mut impl FnMut() -> u32) -> Self {
        let mut result = [0u8; N];
        let mut i = 0;
        while i < N {
            let word = r().to_le_bytes();
            let take = core::cmp::min(4, N - i);
            result[i..i + take].copy_from_slice(&word[..take]);
            i += 4;
        }
        result
    }
}
```

## Vec impl

Single impl for all `Vec<T: WordLayout>`. When `size_of::<T>() == 1` (u8, bool), uses
byte-packed encoding. Otherwise uses element-wise encoding.

```rust
impl<T: WordLayout> WordLayout for Vec<T> {
    const WORD_COUNT: Option<usize> = None;

    fn write_words(&self, w: &mut impl FnMut(u32)) {
        w(self.len() as u32);
        if core::mem::size_of::<T>() == 1 {
            // Byte-packed path for Vec<u8> etc.
            let bytes: &[u8] = unsafe {
                core::slice::from_raw_parts(self.as_ptr() as *const u8, self.len())
            };
            for chunk in bytes.chunks(4) {
                let mut buf = [0u8; 4];
                buf[..chunk.len()].copy_from_slice(chunk);
                w(u32::from_le_bytes(buf));
            }
        } else {
            for item in self.iter() {
                item.write_words(w);
            }
        }
    }

    fn read_words(r: &mut impl FnMut() -> u32) -> Self {
        let len = r() as usize;
        if core::mem::size_of::<T>() == 1 {
            // Byte-packed path
            let mut bytes = alloc::vec![0u8; len];
            let mut i = 0;
            while i < len {
                let word = r().to_le_bytes();
                let take = core::cmp::min(4, len - i);
                bytes[i..i + take].copy_from_slice(&word[..take]);
                i += 4;
            }
            unsafe { core::mem::transmute(bytes) }
        } else {
            (0..len).map(|_| T::read_words(r)).collect()
        }
    }
}
```

## Custom type impls

### Bytes32

Custom impl because its internal representation is `[usize; BYTES32_USIZE_SIZE]`
(architecture-dependent), but wire format is always 8 u32 words.

```rust
impl WordLayout for Bytes32 {
    const WORD_COUNT: Option<usize> = Some(8);

    fn read_words(r: &mut impl FnMut() -> u32) -> Self {
        let mut result = MaybeUninit::<Self>::uninit();
        let dst = result.as_mut_ptr() as *mut u32;
        for i in 0..8 {
            unsafe { dst.add(i).write(r()); }
        }
        unsafe { result.assume_init() }
    }

    fn write_words(&self, w: &mut impl FnMut(u32)) {
        let src = self as *const Self as *const u32;
        for i in 0..8 {
            w(unsafe { src.add(i).read() });
        }
    }
}
```

### U256 and B160

Foreign types (from `ruint`). Implement in `zk_ee` using limb access:

- `U256`: 8 words. Write/read as 4 u64 limbs (each 2 words).
- `B160`: 5 words. Write/read as 3 u64 limbs (6 words for 24 bytes) — but B160 is only
  20 bytes. Use 5 words: first 4 from the first 2 limbs, 5th is the low u32 of the third limb.

Actually, B160's current `UsizeSerializable` on draft-0.4.0 uses 3 usizes on 64-bit
(24 bytes, zero-padded). For the word-aligned format, the simplest encoding is 5 u32 words
(20 bytes packed) or 6 u32 words (3 u64 limbs). Use 6 words (3 u64 limbs) for simplicity
and alignment with the limb representation.

## Derive macro

### Crate location

New crate: `supporting_crates/word_layout_derive`, added to workspace members.
Re-exported via `zk_ee::oracle::word_layout::WordLayout` derive path.

### Derive rules

`#[derive(WordLayout)]` on a struct:

1. Compute `WORD_COUNT`: if all fields have `Some` word count, sum them. Otherwise `None`.
2. Generate `write_words`: call `field.write_words(w)` for each field in declaration order.
3. Generate `read_words`:
   - **Bulk path**: if struct is `repr(C)`, all fields are fixed-size, and no field has
     `WORD_COUNT == Some(1)` with a sub-word type (bool, u8, u16), then generate a direct
     bulk read: cast `MaybeUninit<Self>` to `*mut u32` and loop `N` stores. This produces
     code identical to today's RawWordReadable.
   - **Field-by-field path**: otherwise, read each field via `T::read_words(r)` and
     construct the struct. Every field starts at a word boundary because all prior fields
     consumed whole words.

### Bulk path detection

The derive macro inspects:
- All fields have `WORD_COUNT = Some(_)`
- No field is `bool`, `u8`, or `u16` (these are padded to 1 word in the format but occupy
  less than 4 bytes in memory, so bulk memcpy would read padding bytes into unrelated memory)

When these conditions are met, the struct qualifies for bulk read. The macro then checks
for `#[repr(C)]`:
- If `repr(C)` is present: emit the bulk store loop.
- If `repr(C)` is missing: emit a **compile error** telling the developer to add it.
  This prevents accidentally missing the optimization on eligible types.

```
error: WordLayout: DivRemResponse qualifies for bulk word read.
       Add #[repr(C)] to enable it.
```

Types that don't qualify (have sub-word fields or dynamic fields) use the field-by-field
path regardless of repr and never trigger this error.

### Examples

```rust
// Bulk path: repr(C), all fixed, no sub-word fields
#[repr(C)]
#[derive(WordLayout)]
struct DivRemResponse {
    quotient: [u64; 4],  // 8 words
}
// WORD_COUNT = Some(8), read_words = bulk 8-word store loop

// Field-by-field: has sub-word field
#[derive(WordLayout)]
struct InitialStorageSlotData {
    is_new_storage_slot: bool,  // 1 word (padded)
    initial_value: Bytes32,      // 8 words
}
// WORD_COUNT = Some(9), read_words = bool::read_words + Bytes32::read_words

// Dynamic: contains Vec
#[derive(WordLayout)]
struct ModexpResponse {
    quotient: Vec<u64>,   // dynamic
    remainder: Vec<u64>,  // dynamic
}
// WORD_COUNT = None, read_words = Vec::<u64>::read_words + Vec::<u64>::read_words
```

## IOOracle trait

```rust
pub trait IOOracle: 'static + Sized {
    fn query<I: WordLayout, O: WordLayout>(
        &mut self,
        query_type: u32,
        input: &I,
    ) -> Result<O, InternalError>;

    fn query_with_empty_input<O: WordLayout>(
        &mut self,
        query_type: u32,
    ) -> Result<O, InternalError> {
        self.query::<(), O>(query_type, &())
    }

    fn try_begin_next_tx(&mut self) -> Result<Option<NonZeroU32>, InternalError> {
        let size: u32 = self.query_with_empty_input(NEXT_TX_SIZE_QUERY_ID)?;
        Ok(NonZeroU32::new(size))
    }

    fn query_bytes<I: WordLayout>(
        &mut self,
        query_type: u32,
        input: &I,
    ) -> Result<alloc::vec::Vec<u8>, InternalError> {
        self.query::<I, alloc::vec::Vec<u8>>(query_type, input)
    }
}
```

## ProvingOracle

```rust
pub struct ProvingOracle<T: Transport> {
    transport: T,
}

impl<T: Transport + 'static> IOOracle for ProvingOracle<T> {
    #[inline(always)]
    fn query<I: WordLayout, O: WordLayout>(
        &mut self,
        _query_type: u32,
        _input: &I,
    ) -> Result<O, InternalError> {
        Ok(O::read_words(&mut || self.transport.read_word()))
    }
}
```

No specialization, no dispatch traits, no WordReader.

## WitnessRecordingOracle

```rust
pub struct WitnessRecordingOracle<O: IOOracle> {
    inner: O,
    witness_words: Vec<u32>,
}

impl<O: IOOracle> IOOracle for WitnessRecordingOracle<O> {
    fn query<I: WordLayout, R: WordLayout>(
        &mut self,
        query_type: u32,
        input: &I,
    ) -> Result<R, InternalError> {
        let response: R = self.inner.query(query_type, input)?;
        response.write_words(&mut |w| self.witness_words.push(w));
        Ok(response)
    }
}
```

## ZkEENonDeterminismSource (forward oracle)

Dispatches to query processors. Encodes input via `WordLayout::write_words` into a
`Vec<u32>`, passes to the query processor. The processor returns a typed response.
The oracle serializes the response via `WordLayout::write_words` for any recording,
and returns it.

Query processors continue to receive/return typed data. The WordLayout
serialization happens at the oracle boundary, not inside processors.

## What gets deleted from draft-0.4.0

| Deleted | Replacement |
|---------|-------------|
| `UsizeSerializable` trait + all impls | `WordLayout` trait + derive |
| `UsizeDeserializable` trait + all impls | `WordLayout` trait + derive |
| `usize_serialization` module | `word_layout` module |
| `SimpleOracleQuery` trait | Direct `WordLayout` bounds on `IOOracle::query` |
| `CsrBasedIOOracle` | `ProvingOracle` (trivial impl) |
| `ExactSizeChain` utility (used by UsizeSerializable) | Not needed |
| Per-element bounds checking in iterators | Not needed (const word counts) |

## Wire format comparison

For `InitialStorageSlotData { bool, Bytes32 }`:

| Format | Words | Alignment issues |
|--------|-------|-----------------|
| Old (UsizeSerializable, riscv32) | 9 (1 + 8) | None (all word-aligned) |
| Wincode (current transport-migration) | 8.25 → 9 packed | Bool at byte 0, Bytes32 at byte 1 (unaligned) |
| New (WordLayout) | 9 (1 + 8) | None (all word-aligned) |

The new format matches the old format's word count but with u32 words instead of
architecture-dependent usize, and with derive macro support instead of manual impls.

## Performance characteristics

Every fixed-size type achieves the same performance as the current `RawWordReadable`
optimization: a tight loop of `transport.read_word() → u32 store`. No wincode, no
WordReader, no byte buffering, no alignment issues.

For variable-size types (Vec), there is one heap allocation per Vec (unavoidable). The
elements are read word-by-word with no framework overhead.

Call-site optimizations like the modexp streaming read (writing directly into BigintRepr
instead of constructing ModexpResponse) remain possible and natural: the caller reads
individual u64 values via `u64::read_words` in a loop. No cfg gates needed — the word
sequence is the same whether read as one ModexpResponse or as individual fields.

## Testing strategy

- Unit tests for each primitive WordLayout impl: roundtrip write/read.
- Unit tests for derive macro: struct with fixed fields, struct with sub-word fields,
  struct with Vec fields.
- Wire format compatibility test: verify word sequences match expected encoding.
- Integration: rig tests (forward + proving) exercise the full oracle path.
- Benchmark: compare against draft-0.4.0 baseline to verify no regression.
