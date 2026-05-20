# WordLayout IO Format Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace UsizeSerializable/UsizeDeserializable and wincode-based oracle serialization with a u32 word-aligned WordLayout trait + derive macro, achieving optimal proving performance uniformly.

**Architecture:** New `WordLayout` trait defines serialization in u32 words. A derive proc-macro generates impls. IOOracle trait uses WordLayout bounds. ProvingOracle becomes a trivial one-liner. All types get optimal read/write performance without specialization or cfg gates.

**Tech Stack:** Rust nightly, proc-macro2/syn/quote for derive macro, no_std compatible.

**Starting branch:** `draft-0.4.0`

**Spec:** `docs/superpowers/specs/2026-05-20-word-layout-io-design.md`

**Key reference:** On draft-0.4.0, the IOOracle trait already uses wincode bounds (WincodeSerialize/WincodeDeserialize). OracleQueryProcessor::process takes `&[u8]` / returns `Vec<u8>` (wincode-encoded). UsizeSerializable is a separate abstraction used by the old CsrBasedIOOracle and for ZK layout computation. The new WordLayout replaces BOTH wincode and UsizeSerializable in the oracle path.

**Build/test commands:**
```bash
ZKSYNC_USE_CUDA_STUBS=true cargo check          # workspace check
ZKSYNC_USE_CUDA_STUBS=true cargo test            # default-members tests
ZKSYNC_USE_CUDA_STUBS=true cargo test -p zk_ee   # single crate
cd zksync_os && ./dump_bin.sh --type for-tests   # RISC-V binary
```

**Pre-existing build error:** `crypto::bigint_op_delegation_raw` in basic_system is unrelated to this work — ignore it.

---

### Task 1: Create word_layout_derive proc-macro crate

**Files:**
- Create: `supporting_crates/word_layout_derive/Cargo.toml`
- Create: `supporting_crates/word_layout_derive/src/lib.rs`
- Modify: `Cargo.toml` (workspace members)

This task creates the scaffolding. The derive macro will be implemented in Task 4 after the trait exists.

- [ ] **Step 1: Create Cargo.toml**

```toml
# supporting_crates/word_layout_derive/Cargo.toml
[package]
name = "word_layout_derive"
version = "0.1.0"
edition = "2021"

[lib]
proc-macro = true

[dependencies]
syn = { version = "2", features = ["full", "extra-traits"] }
quote = "1"
proc-macro2 = "1"
```

- [ ] **Step 2: Create stub lib.rs**

```rust
// supporting_crates/word_layout_derive/src/lib.rs
use proc_macro::TokenStream;

#[proc_macro_derive(WordLayout)]
pub fn derive_word_layout(input: TokenStream) -> TokenStream {
    TokenStream::new() // stub — implemented in Task 4
}
```

- [ ] **Step 3: Add to workspace members**

In `Cargo.toml`, add `"supporting_crates/word_layout_derive"` to the `members` and `default-members` arrays.

- [ ] **Step 4: Verify**

Run: `ZKSYNC_USE_CUDA_STUBS=true cargo check -p word_layout_derive`
Expected: compiles with no errors.

- [ ] **Step 5: Commit**

```bash
git add supporting_crates/word_layout_derive/ Cargo.toml
git commit -m "chore: scaffold word_layout_derive proc-macro crate"
```

---

### Task 2: Implement WordLayout trait + primitive impls

**Files:**
- Create: `zk_ee/src/oracle/word_layout/mod.rs`
- Create: `zk_ee/src/oracle/word_layout/primitives.rs`
- Create: `zk_ee/src/oracle/word_layout/arrays.rs`
- Create: `zk_ee/src/oracle/word_layout/vec.rs`
- Create: `zk_ee/src/oracle/word_layout/tests.rs`
- Modify: `zk_ee/src/oracle/mod.rs` (add module declaration)
- Modify: `zk_ee/Cargo.toml` (add word_layout_derive dependency)

- [ ] **Step 1: Create the trait definition**

Create `zk_ee/src/oracle/word_layout/mod.rs`:

```rust
extern crate alloc;

mod primitives;
mod arrays;
mod vec;

#[cfg(test)]
mod tests;

pub use word_layout_derive::WordLayout;

/// Word-aligned serialization for oracle IO. Every field is padded to u32
/// word boundaries. The format is architecture-independent (always u32).
pub trait WordLayout: Sized {
    /// Fixed word count, or None for variable-size types.
    const WORD_COUNT: Option<usize>;

    /// Serialize to a sequence of u32 LE words.
    fn write_words(&self, write: &mut impl FnMut(u32));

    /// Deserialize from a sequence of u32 LE words.
    fn read_words(read: &mut impl FnMut() -> u32) -> Self;
}
```

- [ ] **Step 2: Implement primitive impls**

Create `zk_ee/src/oracle/word_layout/primitives.rs`:

```rust
use super::WordLayout;

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

- [ ] **Step 3: Implement byte array impls**

Create `zk_ee/src/oracle/word_layout/arrays.rs`:

```rust
use super::WordLayout;

impl<const N: usize> WordLayout for [u8; N] {
    const WORD_COUNT: Option<usize> = Some(N.div_ceil(4));

    fn write_words(&self, w: &mut impl FnMut(u32)) {
        let mut i = 0;
        while i < N {
            let mut buf = [0u8; 4];
            let take = if N - i < 4 { N - i } else { 4 };
            buf[..take].copy_from_slice(&self[i..i + take]);
            w(u32::from_le_bytes(buf));
            i += 4;
        }
    }

    fn read_words(r: &mut impl FnMut() -> u32) -> Self {
        let mut result = [0u8; N];
        let mut i = 0;
        while i < N {
            let word = r().to_le_bytes();
            let take = if N - i < 4 { N - i } else { 4 };
            result[i..i + take].copy_from_slice(&word[..take]);
            i += 4;
        }
        result
    }
}
```

Note: `[T; N]` for T != u8 cannot have a blanket impl due to coherence with `[u8; N]`. The derive macro handles arrays of non-u8 types by expanding element-wise in generated code. For arrays used in the codebase (like `[u64; 4]`), add explicit impls:

```rust
// Also in arrays.rs — impls for [u64; N] used by oracle types
impl<const N: usize> WordLayout for [u64; N] {
    const WORD_COUNT: Option<usize> = Some(N * 2);

    fn write_words(&self, w: &mut impl FnMut(u32)) {
        for val in self {
            val.write_words(w);
        }
    }

    fn read_words(r: &mut impl FnMut() -> u32) -> Self {
        let mut result = [0u64; N];
        for val in &mut result {
            *val = u64::read_words(r);
        }
        result
    }
}
```

- [ ] **Step 4: Implement Vec impl**

Create `zk_ee/src/oracle/word_layout/vec.rs`:

```rust
extern crate alloc;
use alloc::vec::Vec;
use super::WordLayout;

impl<T: WordLayout> WordLayout for Vec<T> {
    const WORD_COUNT: Option<usize> = None;

    fn write_words(&self, w: &mut impl FnMut(u32)) {
        w(self.len() as u32);
        if core::mem::size_of::<T>() == 1 {
            // Byte-packed path for Vec<u8>
            let bytes: &[u8] = unsafe {
                core::slice::from_raw_parts(self.as_ptr() as *const u8, self.len())
            };
            let mut i = 0;
            while i < bytes.len() {
                let mut buf = [0u8; 4];
                let take = core::cmp::min(4, bytes.len() - i);
                buf[..take].copy_from_slice(&bytes[i..i + take]);
                w(u32::from_le_bytes(buf));
                i += 4;
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
            let mut bytes = alloc::vec![0u8; len];
            let mut i = 0;
            while i < len {
                let word = r().to_le_bytes();
                let take = core::cmp::min(4, len - i);
                bytes[i..i + take].copy_from_slice(&word[..take]);
                i += 4;
            }
            // SAFETY: Vec<u8> and Vec<T> where T is 1 byte have same layout
            unsafe { core::mem::transmute(bytes) }
        } else {
            (0..len).map(|_| T::read_words(r)).collect()
        }
    }
}
```

- [ ] **Step 5: Write unit tests**

Create `zk_ee/src/oracle/word_layout/tests.rs`:

```rust
use super::WordLayout;
extern crate alloc;
use alloc::vec;
use alloc::vec::Vec;

fn roundtrip<T: WordLayout + PartialEq + core::fmt::Debug>(val: &T) -> T {
    let mut words = Vec::new();
    val.write_words(&mut |w| words.push(w));
    let mut iter = words.into_iter();
    T::read_words(&mut || iter.next().expect("not enough words"))
}

#[test]
fn primitives() {
    assert_eq!(roundtrip(&true), true);
    assert_eq!(roundtrip(&false), false);
    assert_eq!(roundtrip(&42u8), 42u8);
    assert_eq!(roundtrip(&1234u16), 1234u16);
    assert_eq!(roundtrip(&0xDEADBEEFu32), 0xDEADBEEFu32);
    assert_eq!(roundtrip(&0xDEADBEEF_CAFEBABEu64), 0xDEADBEEF_CAFEBABEu64);
    assert_eq!(roundtrip(&()), ());
}

#[test]
fn byte_arrays() {
    assert_eq!(roundtrip(&[1u8, 2, 3]), [1, 2, 3]);
    assert_eq!(roundtrip(&[1u8, 2, 3, 4]), [1, 2, 3, 4]);
    assert_eq!(roundtrip(&[1u8, 2, 3, 4, 5]), [1, 2, 3, 4, 5]);
    let big: [u8; 32] = core::array::from_fn(|i| i as u8);
    assert_eq!(roundtrip(&big), big);
}

#[test]
fn u64_arrays() {
    assert_eq!(roundtrip(&[1u64, 2, 3, 4]), [1u64, 2, 3, 4]);
}

#[test]
fn word_counts() {
    assert_eq!(<bool as WordLayout>::WORD_COUNT, Some(1));
    assert_eq!(<u32 as WordLayout>::WORD_COUNT, Some(1));
    assert_eq!(<u64 as WordLayout>::WORD_COUNT, Some(2));
    assert_eq!(<[u8; 3] as WordLayout>::WORD_COUNT, Some(1));
    assert_eq!(<[u8; 4] as WordLayout>::WORD_COUNT, Some(1));
    assert_eq!(<[u8; 5] as WordLayout>::WORD_COUNT, Some(2));
    assert_eq!(<[u8; 32] as WordLayout>::WORD_COUNT, Some(8));
    assert_eq!(<[u64; 4] as WordLayout>::WORD_COUNT, Some(8));
    assert_eq!(<Vec<u8> as WordLayout>::WORD_COUNT, None);
}

#[test]
fn vec_u8_byte_packed() {
    let val: Vec<u8> = vec![1, 2, 3, 4, 5];
    let result = roundtrip(&val);
    assert_eq!(result, val);
    // Verify word count: 1 (length) + 2 (5 bytes packed) = 3 words
    let mut words = Vec::new();
    val.write_words(&mut |w| words.push(w));
    assert_eq!(words.len(), 3);
}

#[test]
fn vec_u64() {
    let val: Vec<u64> = vec![0xAABBCCDD, 0x11223344];
    let result = roundtrip(&val);
    assert_eq!(result, val);
    // 1 (length) + 2*2 (two u64s) = 5 words
    let mut words = Vec::new();
    val.write_words(&mut |w| words.push(w));
    assert_eq!(words.len(), 5);
}
```

- [ ] **Step 6: Wire into zk_ee**

Add to `zk_ee/src/oracle/mod.rs`:
```rust
pub mod word_layout;
```

Add to `zk_ee/Cargo.toml` dependencies:
```toml
word_layout_derive = { path = "../supporting_crates/word_layout_derive" }
```

- [ ] **Step 7: Verify**

Run: `ZKSYNC_USE_CUDA_STUBS=true cargo test -p zk_ee --lib -- word_layout`
Expected: all tests pass.

- [ ] **Step 8: Commit**

```bash
git add zk_ee/src/oracle/word_layout/ zk_ee/src/oracle/mod.rs zk_ee/Cargo.toml
git commit -m "feat: implement WordLayout trait with primitive, array, and Vec impls"
```

---

### Task 3: Implement WordLayout for Bytes32, U256, B160

**Files:**
- Modify: `zk_ee/src/utils/bytes32.rs` (add WordLayout impl)
- Modify: `zk_ee/src/oracle/word_layout/mod.rs` (add foreign type impls for U256, B160)
- Modify: `zk_ee/src/oracle/word_layout/tests.rs` (add tests)

- [ ] **Step 1: Implement Bytes32**

In `zk_ee/src/utils/bytes32.rs`, add:

```rust
impl crate::oracle::word_layout::WordLayout for Bytes32 {
    const WORD_COUNT: Option<usize> = Some(8);

    fn read_words(r: &mut impl FnMut() -> u32) -> Self {
        let mut result = core::mem::MaybeUninit::<Self>::uninit();
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

- [ ] **Step 2: Implement U256 and B160**

In `zk_ee/src/oracle/word_layout/mod.rs`, add a `foreign_types` module or add directly:

```rust
// U256 = [u64; 4] internally, repr(transparent). 8 words.
impl WordLayout for ruint::aliases::U256 {
    const WORD_COUNT: Option<usize> = Some(8);

    fn write_words(&self, w: &mut impl FnMut(u32)) {
        for limb in self.as_limbs() {
            limb.write_words(w);
        }
    }

    fn read_words(r: &mut impl FnMut() -> u32) -> Self {
        let limbs: [u64; 4] = core::array::from_fn(|_| u64::read_words(r));
        Self::from_limbs(limbs)
    }
}

// B160 = [u64; 3] internally (24 bytes, only 20 meaningful). 6 words.
impl WordLayout for ruint::aliases::B160 {
    const WORD_COUNT: Option<usize> = Some(6);

    fn write_words(&self, w: &mut impl FnMut(u32)) {
        for limb in self.as_limbs() {
            limb.write_words(w);
        }
    }

    fn read_words(r: &mut impl FnMut() -> u32) -> Self {
        let limbs: [u64; 3] = core::array::from_fn(|_| u64::read_words(r));
        Self::from_limbs(limbs)
    }
}
```

- [ ] **Step 3: Add tests**

In `tests.rs`, add:

```rust
use crate::utils::Bytes32;

#[test]
fn bytes32_roundtrip() {
    let val = Bytes32::from_array(core::array::from_fn(|i| i as u8));
    let result = roundtrip(&val);
    assert_eq!(val.as_u8_array_ref(), result.as_u8_array_ref());
    let mut words = Vec::new();
    val.write_words(&mut |w| words.push(w));
    assert_eq!(words.len(), 8);
}

#[test]
fn u256_roundtrip() {
    use ruint::aliases::U256;
    let val = U256::from(0xDEADBEEF_CAFEBABE_u64);
    assert_eq!(roundtrip(&val), val);
}

#[test]
fn b160_roundtrip() {
    use ruint::aliases::B160;
    let val = B160::from(0x1234567890ABCDEFu64);
    assert_eq!(roundtrip(&val), val);
}
```

- [ ] **Step 4: Verify and commit**

Run: `ZKSYNC_USE_CUDA_STUBS=true cargo test -p zk_ee --lib -- word_layout`

```bash
git add zk_ee/
git commit -m "feat: implement WordLayout for Bytes32, U256, B160"
```

---

### Task 4: Implement derive macro

**Files:**
- Modify: `supporting_crates/word_layout_derive/src/lib.rs`
- Create: `supporting_crates/word_layout_derive/tests/derive_tests.rs` (integration test)

The derive macro must handle three cases:
1. **Bulk path**: all fixed-size fields, repr(C), no sub-word fields → direct u32 store loop
2. **Field-by-field path**: fixed or mixed fields, sub-word fields present or no repr(C) → per-field read/write
3. **Compile error**: qualifies for bulk but missing repr(C)

Sub-word types are: `bool`, `u8`, `u16`. The macro detects these by checking field type names.

- [ ] **Step 1: Implement the derive macro**

Replace `supporting_crates/word_layout_derive/src/lib.rs` with the full implementation. The macro must:

1. Parse the struct with `syn::DeriveInput`
2. Extract field names and types
3. Check for `#[repr(C)]` in attributes
4. Determine if each field type is sub-word (`bool`, `u8`, `u16`) by checking the type path's last segment
5. Determine if struct qualifies for bulk path (no sub-word fields, no obviously-dynamic types like `Vec`)
6. Generate appropriate `WordLayout` impl

Key generated code patterns:

For **WORD_COUNT**: sum field word counts. If any field is `Vec<_>`, emit `None`. Otherwise emit `Some(sum)` using const blocks:
```rust
const WORD_COUNT: Option<usize> = const {
    // If all fields have known word counts, compute sum
    // Otherwise None
};
```

Since the derive macro can't evaluate `T::WORD_COUNT` at macro expansion time (it's a const, resolved later), use this pattern:
```rust
const WORD_COUNT: Option<usize> = {
    match (<Field1Type as WordLayout>::WORD_COUNT,
           <Field2Type as WordLayout>::WORD_COUNT) {
        (Some(a), Some(b)) => Some(a + b),
        _ => None,
    }
};
```

For **write_words** (always field-by-field):
```rust
fn write_words(&self, w: &mut impl FnMut(u32)) {
    self.field1.write_words(w);
    self.field2.write_words(w);
}
```

For **read_words**:
- Bulk path (repr(C), qualifies):
```rust
fn read_words(r: &mut impl FnMut() -> u32) -> Self {
    const N: usize = match Self::WORD_COUNT {
        Some(n) => n,
        None => unreachable!(),
    };
    let mut result = core::mem::MaybeUninit::<Self>::uninit();
    let dst = result.as_mut_ptr() as *mut u32;
    for i in 0..N {
        unsafe { dst.add(i).write(r()); }
    }
    unsafe { result.assume_init() }
}
```

- Field-by-field path:
```rust
fn read_words(r: &mut impl FnMut() -> u32) -> Self {
    Self {
        field1: <Field1Type as WordLayout>::read_words(r),
        field2: <Field2Type as WordLayout>::read_words(r),
    }
}
```

- Compile error (qualifies for bulk but no repr(C)):
```rust
compile_error!("WordLayout: StructName qualifies for bulk word read. Add #[repr(C)] to enable it.");
```

- [ ] **Step 2: Write integration tests**

The proc-macro crate needs a separate test that uses the derive. Create a test in the `zk_ee` crate since it already depends on the derive:

Add to `zk_ee/src/oracle/word_layout/tests.rs`:

```rust
use crate::oracle::word_layout::WordLayout;

// Test: bulk path (repr(C), all fixed, no sub-word)
#[repr(C)]
#[derive(Debug, PartialEq, WordLayout)]
struct BulkStruct {
    a: u64,
    b: [u64; 4],
}

#[test]
fn derive_bulk_struct() {
    assert_eq!(BulkStruct::WORD_COUNT, Some(10));
    let val = BulkStruct { a: 42, b: [1, 2, 3, 4] };
    assert_eq!(roundtrip(&val), val);
}

// Test: field-by-field (has sub-word field)
#[derive(Debug, PartialEq, WordLayout)]
struct FieldByFieldStruct {
    flag: bool,
    value: u64,
}

#[test]
fn derive_field_by_field() {
    assert_eq!(FieldByFieldStruct::WORD_COUNT, Some(3)); // 1 + 2
    let val = FieldByFieldStruct { flag: true, value: 999 };
    assert_eq!(roundtrip(&val), val);
}

// Test: dynamic (contains Vec)
#[derive(Debug, PartialEq, WordLayout)]
struct DynamicStruct {
    data: Vec<u64>,
}

#[test]
fn derive_dynamic() {
    assert_eq!(DynamicStruct::WORD_COUNT, None);
    let val = DynamicStruct { data: vec![1, 2, 3] };
    assert_eq!(roundtrip(&val), val);
}
```

- [ ] **Step 3: Verify and commit**

Run: `ZKSYNC_USE_CUDA_STUBS=true cargo test -p zk_ee --lib -- word_layout`

```bash
git add supporting_crates/word_layout_derive/ zk_ee/src/oracle/word_layout/tests.rs
git commit -m "feat: implement WordLayout derive macro with bulk and field-by-field paths"
```

---

### Task 5: Add WordLayout to all oracle response types

**Files:**
- Modify: `basic_system/src/oracle_types.rs`
- Modify: `basic_bootloader/src/bootloader/oracle_types.rs`
- Modify: `basic_bootloader/src/bootloader/block_flow/zk/post_tx_op/da_commitment_generator/blob_commitment_generator/commitment_and_proof_advice.rs` (KZGCommitmentAndProof)
- Modify: `basic_system/Cargo.toml`, `basic_bootloader/Cargo.toml` (add word_layout_derive dep via zk_ee re-export)

These are the callable oracle response types. Each needs `#[derive(WordLayout)]` and appropriate `#[repr(C)]` where eligible.

- [ ] **Step 1: Add WordLayout to basic_system oracle types**

In `basic_system/src/oracle_types.rs`, add `#[derive(WordLayout)]` to each type. Types that qualify for bulk read and already have `repr(C)` just add the derive. Types without `repr(C)` that qualify: add `repr(C)`. Types with Vec fields or sub-word fields get field-by-field path automatically.

```rust
// DivRemResponse: [u64; 4] = 8 words. Qualifies for bulk.
#[repr(C)]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, WordLayout)]
pub struct DivRemResponse {
    pub quotient: [u64; 4],
}

// WideDivRemResponse: [u64; 4] × 2 = 16 words. Qualifies for bulk.
#[repr(C)]
#[derive(Clone, Debug, Serialize, Deserialize, WordLayout)]
pub struct WideDivRemResponse {
    pub quotient_lo: [u64; 4],
    pub quotient_hi: [u64; 4],
}

// ModexpResponse: Vec<u64> fields. Dynamic, field-by-field.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, WordLayout)]
pub struct ModexpResponse {
    pub quotient: Vec<u64>,
    pub remainder: Vec<u64>,
}

// FieldSqrtResponse: has bool (sub-word). Field-by-field.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, WordLayout)]
pub struct FieldSqrtResponse {
    pub result: Bytes32,
    pub is_valid: bool,
}

// FieldInverseResponse: Bytes32 only = 8 words. Qualifies for bulk.
#[repr(C)]
#[derive(Clone, Debug, Serialize, Deserialize, WordLayout)]
pub struct FieldInverseResponse {
    pub result: Bytes32,
}
```

- [ ] **Step 2: Mirror in basic_bootloader oracle types**

Apply identical changes to `basic_bootloader/src/bootloader/oracle_types.rs`.

- [ ] **Step 3: Add WordLayout to KZGCommitmentAndProof**

In `commitment_and_proof_advice.rs`:

```rust
#[repr(C, align(8))]
#[derive(WordLayout)]  // add this
pub struct KZGCommitmentAndProof {
    pub commitment: [u8; 48],  // 12 words byte-packed
    pub proof: [u8; 48],       // 12 words byte-packed
}
// Total: 24 words. But [u8; 48] uses byte-packing, not word-per-element.
// The derive macro sees [u8; 48] which has WordLayout impl via [u8; N].
// This struct has no sub-word scalar fields (arrays don't count), so it
// qualifies for bulk IF the byte-packed array layout matches repr(C) memory.
// Actually, [u8; 48] in memory is 48 bytes, but WordLayout encodes it as
// 12 words (48 bytes packed). With repr(C), the struct is 96 bytes in memory
// and 24 words in WordLayout. The layouts match, so bulk path works.
```

Note: the derive macro needs to recognize `[u8; N]` as NOT a sub-word type for bulk-path eligibility purposes. `[u8; N]` is byte-packed (not padded per element), so its in-memory layout (N bytes) matches its word layout (ceil(N/4) * 4 bytes, zero-padded). For repr(C) structs, this is correct as long as there's no padding between fields. For `KZGCommitmentAndProof { [u8; 48], [u8; 48] }` with repr(C): field 1 at offset 0 (48 bytes), field 2 at offset 48 (48 bytes). Total 96 bytes = 24 words. Bulk read works.

- [ ] **Step 4: Ensure Cargo.toml dependencies**

Each crate that uses `#[derive(WordLayout)]` needs access to the macro. Since `zk_ee` re-exports it (`pub use word_layout_derive::WordLayout`), crates that depend on `zk_ee` can use `use zk_ee::oracle::word_layout::WordLayout;`. No additional Cargo.toml changes needed if the derive is re-exported.

However, proc-macro re-exports require the `proc_macro_derive` to be in scope. The crates `basic_system` and `basic_bootloader` already depend on `zk_ee`. Add `use zk_ee::oracle::word_layout::WordLayout;` to each oracle_types.rs.

- [ ] **Step 5: Verify and commit**

Run: `ZKSYNC_USE_CUDA_STUBS=true cargo check -p basic_system -p basic_bootloader`

```bash
git add basic_system/ basic_bootloader/ 
git commit -m "feat: derive WordLayout for all oracle response types"
```

---

### Task 6: Add WordLayout to system types

**Files:**
- Modify: `zk_ee/src/storage_types/initial_storage_slot_data.rs`
- Modify: `zk_ee/src/storage_types/storage_address.rs`
- Modify: `zk_ee/src/system/metadata/zk_metadata.rs` (BlockMetadataFromOracle, BlockHashes)
- Modify: `zk_ee/src/common_structs/proof_data.rs`
- Modify: `zk_ee/src/common_structs/state_root_view.rs` (trait bounds)
- Modify: `zk_ee/src/execution_environment_type.rs`
- Modify: `zk_ee/src/types_config/mod.rs` (trait bounds)
- Modify: `basic_system/src/system_implementation/flat_storage_model/simple_growable_storage.rs` (FlatStorageLeaf, FlatStorageCommitment, LeafProof, proof types)
- Modify: `basic_system/src/system_implementation/ethereum_storage_model/caches/account_properties.rs` (EthereumAccountProperties)

These are all the types that currently implement UsizeSerializable/UsizeDeserializable. Each gets a WordLayout derive or manual impl. Generic types (those parameterized by IOTypes) need manual impls since the derive macro can't handle arbitrary trait-bounded generics.

- [ ] **Step 1: Simple types in zk_ee**

Add `#[derive(WordLayout)]` or manual impls to:

- `ExecutionEnvironmentType`: manual impl (enum, maps to u8)
- `Bytes32`: already done in Task 3
- `BlockHashes`: manual impl (wraps `[U256; 256]`, needs array-of-U256 handling)
- `BlockMetadataFromOracle`: manual impl (complex struct with B160 field)

For `BlockHashes([U256; 256])`: since it's a newtype, implement manually:
```rust
impl WordLayout for BlockHashes {
    const WORD_COUNT: Option<usize> = Some(256 * 8); // 256 U256s, each 8 words
    fn write_words(&self, w: &mut impl FnMut(u32)) {
        for hash in &self.0 { hash.write_words(w); }
    }
    fn read_words(r: &mut impl FnMut() -> u32) -> Self {
        Self(core::array::from_fn(|_| U256::read_words(r)))
    }
}
```

For `BlockMetadataFromOracle`: manual impl listing all fields.

- [ ] **Step 2: Generic types in zk_ee**

These types are generic over `IOTypes: SystemIOTypesConfig`. The derive macro can't handle them directly. Implement manually with trait bounds:

- `StorageAddress<IOTypes>`: manual impl with bounds on Address, StorageKey
- `InitialStorageSlotData<IOTypes>`: manual impl with bounds on StorageValue
- `ProofData<SR>`: manual impl with bounds on SR

Example for `InitialStorageSlotData`:
```rust
impl<IOTypes: SystemIOTypesConfig> WordLayout for InitialStorageSlotData<IOTypes>
where
    IOTypes::StorageValue: WordLayout,
{
    const WORD_COUNT: Option<usize> = match (
        <bool as WordLayout>::WORD_COUNT,
        <IOTypes::StorageValue as WordLayout>::WORD_COUNT,
    ) {
        (Some(a), Some(b)) => Some(a + b),
        _ => None,
    };
    // field-by-field read/write
}
```

- [ ] **Step 3: Update trait bounds in types_config**

In `zk_ee/src/types_config/mod.rs`, add `WordLayout` bounds alongside existing `UsizeSerializable` bounds on `StorageValue`, `Address`, `StorageKey`, etc. (During migration, both bounds coexist. Old bounds are removed in Task 10.)

In `zk_ee/src/common_structs/state_root_view.rs`, add `WordLayout` bound to `StateRootView` trait.

In `zk_ee/src/system/io.rs`, add `WordLayout` bound to `IOStateCommitment`.

In `storage_models/src/common_structs/traits/storage_model.rs`, add `WordLayout` bound to `StorageCommitment`.

- [ ] **Step 4: Types in basic_system**

Add `#[derive(WordLayout)]` to:
- `FlatStorageLeaf<N>`: has Bytes32 + Bytes32 + u64. No sub-word fields. Add `repr(C)` → bulk path.
- `FlatStorageCommitment<N>`: has Bytes32 + u64. Add `repr(C)` → bulk path.
- `EthereumAccountProperties`: has u64 + U256 + Bytes32 + Bytes32. Add `repr(C)` → bulk path.

Manual impls for generic proof types:
- `LeafProof<N, H, A>`: contains `Box<[Bytes32; N], A>` — manual impl needed. Read index (u64), leaf (FlatStorageLeaf), path (N Bytes32 values read into Box).
- `ExistingReadProof<N, H, A>`: wraps LeafProof — manual impl.
- `ValueAtIndexProof<N, H, A>`: wraps ExistingReadProof — manual impl.
- `NewReadProof<N, H, A>`, `NewWriteProof<N, H, A>`: manual impls.

- [ ] **Step 5: Verify and commit**

Run: `ZKSYNC_USE_CUDA_STUBS=true cargo check -p zk_ee -p basic_system`

```bash
git add zk_ee/ basic_system/ storage_models/
git commit -m "feat: add WordLayout to all system and storage types"
```

---

### Task 7: Update IOOracle trait and oracle infrastructure

**Files:**
- Modify: `zk_ee/src/oracle/mod.rs` (IOOracle trait bounds)
- Modify: `proof_running_system/src/proving_oracle.rs` (rewrite ProvingOracle)
- Modify: `oracle_provider/src/witness_recording.rs` (rewrite WitnessRecordingOracle)
- Modify: `oracle_provider/src/lib.rs` (update ZkEENonDeterminismSource, OracleQueryProcessor)
- Modify: `proof_running_system/src/lib.rs` (remove feature flags for specialization)

This is the breaking change. After this task, all downstream code must use WordLayout.

- [ ] **Step 1: Update IOOracle trait**

In `zk_ee/src/oracle/mod.rs`, change the trait bounds:

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

Remove `WincodeSerialize`, `WincodeDeserialize` trait aliases and the `RawWordReadable` trait.

- [ ] **Step 2: Rewrite ProvingOracle**

In `proof_running_system/src/proving_oracle.rs`:

```rust
use airbender_guest::transport::Transport;
use zk_ee::oracle::word_layout::WordLayout;
use zk_ee::system::errors::internal::InternalError;

pub struct ProvingOracle<T: Transport> {
    transport: T,
}

impl<T: Transport> ProvingOracle<T> {
    pub fn new(transport: T) -> Self {
        Self { transport }
    }
}

impl<T: Transport + 'static> zk_ee::oracle::IOOracle for ProvingOracle<T> {
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

Delete: `ReadDispatch` trait, `RawWordReadable` imports, `read_raw`, `read_wincode`, `WordReader` usage.

Remove from `proof_running_system/src/lib.rs`: `#![feature(min_specialization)]`, `#![feature(rustc_attrs)]`.

- [ ] **Step 3: Rewrite WitnessRecordingOracle**

In `oracle_provider/src/witness_recording.rs`:

```rust
use zk_ee::oracle::word_layout::WordLayout;
use zk_ee::oracle::IOOracle;
use zk_ee::system::errors::internal::InternalError;

pub struct WitnessRecordingOracle<O: IOOracle> {
    inner: O,
    witness_words: Vec<u32>,
}

impl<O: IOOracle> WitnessRecordingOracle<O> {
    pub fn new(inner: O) -> Self {
        Self { inner, witness_words: Vec::new() }
    }

    pub fn into_witness(self) -> (O, Vec<u32>) {
        (self.inner, self.witness_words)
    }
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

- [ ] **Step 4: Update OracleQueryProcessor and ZkEENonDeterminismSource**

In `oracle_provider/src/lib.rs`:

Change `OracleQueryProcessor::process` to work with u32 words:

```rust
pub trait OracleQueryProcessor {
    fn supported_query_ids(&self) -> Vec<u32>;
    fn supports_query_id(&self, query_id: u32) -> bool { ... }
    fn process(
        &mut self,
        query_id: u32,
        input: &[u32],
        memory: &dyn RamPeek,
    ) -> Result<Vec<u32>, InternalError>;
}
```

Update `ZkEENonDeterminismSource::query` to encode input via `WordLayout::write_words` into `Vec<u32>`, pass to processor, decode response via `WordLayout::read_words` from the returned `Vec<u32>`.

- [ ] **Step 5: Verify (expect many downstream errors)**

Run: `ZKSYNC_USE_CUDA_STUBS=true cargo check -p proof_running_system -p oracle_provider`
Expected: these crates compile. Downstream crates will have errors (fixed in Tasks 8-9).

- [ ] **Step 6: Commit**

```bash
git add zk_ee/ proof_running_system/ oracle_provider/
git commit -m "feat: update IOOracle to use WordLayout bounds, rewrite oracle implementations"
```

---

### Task 8: Migrate all query processors and call sites

**Files:**
- Modify: `callable_oracles/src/arithmetic/mod.rs` (ArithmeticQuery, NativeArithmeticQuery)
- Modify: `callable_oracles/src/field_hints/mod.rs` (FieldOpsQuery, NativeFieldOpsQuery)
- Modify: `callable_oracles/src/blob_kzg_commitment/mod.rs`
- Modify: All 13 query processors in `forward_system/src/run/query_processors/`
- Modify: All query call sites (see list from exploration)

Each query processor currently uses `wincode::serialize` / `wincode::deserialize` internally. Change to `WordLayout::write_words` / `WordLayout::read_words` with `Vec<u32>`.

Each call site that uses `SimpleOracleQuery::get()` changes to direct `oracle.query(QUERY_ID, &input)` calls. The `SimpleOracleQuery` trait is deleted.

This is the largest task — it touches many files but each change is mechanical:
- Replace `wincode::serialize(&val)` with `let mut words = Vec::new(); val.write_words(&mut |w| words.push(w));`
- Replace `wincode::deserialize(&bytes)` with building a word iterator and calling `T::read_words`
- Replace `SomeQuery::get(oracle, &input)` with `oracle.query(SOME_QUERY_ID, &input)`

- [ ] **Step 1: Update callable_oracles**

For each query processor, change `process()` to take `&[u32]` / return `Vec<u32>`. Update internal encoding/decoding to use WordLayout.

- [ ] **Step 2: Update forward_system query processors**

Same pattern for all 13 processors in `forward_system/src/run/query_processors/`.

- [ ] **Step 3: Migrate SimpleOracleQuery call sites**

Replace all `SomeQuery::get(oracle, &input)` with `oracle.query(QUERY_ID, &input)`. Files:
- `basic_system/src/system_implementation/caches/generic_pubdata_aware_plain_storage.rs`
- `basic_system/src/system_implementation/ethereum_storage_model/caches/account_cache.rs`
- `basic_system/src/system_implementation/flat_storage_model/simple_growable_storage.rs`
- `basic_bootloader/src/bootloader/block_flow/zk/post_tx_op/post_tx_op_proving_*.rs` (disconnect queries)
- `basic_bootloader/src/bootloader/block_flow/ethereum/post_tx_op_proving.rs`
- `basic_bootloader/src/bootloader/transaction/mod.rs`
- `basic_bootloader/src/bootloader/block_flow/ethereum/block_hashes_cache.rs`
- `forward_system/src/run/query_processors/zk_proof_data.rs`
- `forward_system/src/run/query_processors/read_tree.rs`
- `forward_system/src/run/query_processors/read_storage.rs`

- [ ] **Step 4: Verify full workspace**

Run: `ZKSYNC_USE_CUDA_STUBS=true cargo check`
Fix any remaining errors.

- [ ] **Step 5: Commit**

```bash
git add callable_oracles/ forward_system/ basic_system/ basic_bootloader/
git commit -m "feat: migrate all query processors and call sites to WordLayout"
```

---

### Task 9: Migrate test infrastructure

**Files:**
- Modify: `tests/instances/unit/src/malicious_oracle.rs`
- Modify: `tests/instances/unit/src/initial_slot_regression.rs`
- Modify: `tests/block_reexecutor/src/rpc_oracle.rs`
- Modify: `tests/rig/` (test framework oracle mocks)
- Modify: `tests/fuzzer/` (fuzz targets with UsizeSerializable bounds)

- [ ] **Step 1: Update test oracle mocks**

Each test mock that implements `IOOracle` or `SimpleOracleQuery` needs updating to use WordLayout bounds. Replace wincode/serde serialization in test oracles with WordLayout write/read.

- [ ] **Step 2: Update fuzz targets**

Remove `UsizeSerializable + UsizeDeserializable` bounds from fuzz target helper functions, replace with `WordLayout`.

- [ ] **Step 3: Verify tests pass**

Run: `ZKSYNC_USE_CUDA_STUBS=true cargo test`

- [ ] **Step 4: Commit**

```bash
git add tests/
git commit -m "test: migrate test infrastructure to WordLayout"
```

---

### Task 10: Delete old code

**Files:**
- Delete: `zk_ee/src/oracle/usize_serialization/` (entire module)
- Delete: `zk_ee/src/oracle/simple_oracle_query.rs`
- Delete: `zk_ee/src/utils/exact_size_chain.rs` and `exact_size_chain_n.rs` (if only used by UsizeSerializable)
- Modify: `zk_ee/src/oracle/mod.rs` (remove old module declarations, WincodeSerialize/Deserialize aliases, RawWordReadable)
- Modify: `zk_ee/src/types_config/mod.rs` (remove UsizeSerializable bounds)
- Modify: `zk_ee/src/common_structs/state_root_view.rs` (remove UsizeSerializable bounds)
- Modify: `zk_ee/src/system/io.rs` (remove UsizeSerializable bounds)
- Modify: `storage_models/src/common_structs/traits/storage_model.rs` (remove UsizeSerializable bounds)
- Remove all `UsizeSerializable`/`UsizeDeserializable` impls from every file that had them
- Remove wincode dependency from oracle path crates (keep if used elsewhere)

- [ ] **Step 1: Remove old trait definitions and modules**

Delete the `usize_serialization` directory and `simple_oracle_query.rs`. Remove their module declarations from `zk_ee/src/oracle/mod.rs`.

- [ ] **Step 2: Remove old impls from all types**

Go through every file that implemented `UsizeSerializable` or `UsizeDeserializable` and remove those impl blocks. Files (from exploration):
- `zk_ee/src/utils/bytes32.rs`
- `zk_ee/src/execution_environment_type.rs`
- `zk_ee/src/system/metadata/zk_metadata.rs`
- `zk_ee/src/storage_types/initial_storage_slot_data.rs`
- `zk_ee/src/storage_types/storage_address.rs`
- `zk_ee/src/common_structs/proof_data.rs`
- `basic_system/src/system_implementation/flat_storage_model/simple_growable_storage.rs`
- `basic_system/src/system_implementation/ethereum_storage_model/caches/account_properties.rs`

- [ ] **Step 3: Remove old trait bounds**

Remove `UsizeSerializable + UsizeDeserializable` bounds from:
- `zk_ee/src/types_config/mod.rs`
- `zk_ee/src/common_structs/state_root_view.rs`
- `zk_ee/src/system/io.rs`
- `storage_models/src/common_structs/traits/storage_model.rs`

- [ ] **Step 4: Remove wincode and serde from oracle path**

Remove `WincodeSerialize`, `WincodeDeserialize` trait aliases from `zk_ee/src/oracle/mod.rs`. Remove wincode `SchemaRead`/`SchemaWrite` impls from oracle types (they can keep serde for JSON debugging). Remove `wincode` dependency from `proof_running_system/Cargo.toml` and `oracle_provider/Cargo.toml` if no longer needed.

- [ ] **Step 5: Remove ExactSizeChain utilities**

Check if `zk_ee/src/utils/exact_size_chain.rs` and `exact_size_chain_n.rs` are used anywhere outside UsizeSerializable. If not, delete them.

- [ ] **Step 6: Clean up feature flags**

Remove from `zk_ee/src/lib.rs`: `#![feature(min_specialization)]`, `#![feature(rustc_attrs)]` if no longer needed.
Same for `basic_system/src/lib.rs`, `basic_bootloader/src/lib.rs`.

- [ ] **Step 7: Full verification**

```bash
ZKSYNC_USE_CUDA_STUBS=true cargo check
ZKSYNC_USE_CUDA_STUBS=true cargo test
cd zksync_os && ./dump_bin.sh --type for-tests
```

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "chore: delete UsizeSerializable, SimpleOracleQuery, and wincode oracle path"
```

---

### Task 11: Final integration testing and benchmarking

**Files:** No code changes — verification only.

- [ ] **Step 1: Run full default-members test suite**

```bash
ZKSYNC_USE_CUDA_STUBS=true cargo test
```

- [ ] **Step 2: Build RISC-V binary**

```bash
cd zksync_os && ./dump_bin.sh --type for-tests
```

- [ ] **Step 3: Run rig tests with RISC-V simulation (if CI-like environment)**

```bash
ZKSYNC_USE_CUDA_STUBS=true ZKSYNC_RISC_V_RUN=true cargo test -p transactions
```

- [ ] **Step 4: Run clippy**

```bash
ZKSYNC_USE_CUDA_STUBS=true cargo clippy --all -- -D warnings
```

- [ ] **Step 5: Run fmt**

```bash
cargo fmt
```

- [ ] **Step 6: Final commit if any fixes**

```bash
git add -A
git commit -m "fix: address test and lint issues from WordLayout migration"
```
