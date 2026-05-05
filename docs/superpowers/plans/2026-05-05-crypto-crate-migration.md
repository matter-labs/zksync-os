# Crypto Crate Migration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Migrate zksync-os from its local `crypto` crate to the upstream `airbender-crypto` crate, adding hook support for oracle-based EC field operations while preserving airbender-crypto's existing no-hook API.

**Architecture:** Two sequential PRs. PR1 adds the Secp256k1Hooks trait and `_with_hooks` function variants to airbender-crypto in the airbender-platform repo, plus visibility widening. PR2 switches zksync-os to depend on the upstream crate, deletes the local copy, and adapts the few hook-related call sites.

**Tech Stack:** Rust, Cargo workspaces, secp256k1 elliptic curve cryptography

---

## PR 1: airbender-crypto changes (in /root/airbender-platform)

### Task 1: Create the hooks module

**Files:**
- Create: `/root/airbender-platform/crates/airbender-crypto/src/secp256k1/hooks.rs`
- Modify: `/root/airbender-platform/crates/airbender-crypto/src/secp256k1/mod.rs`

- [ ] **Step 1: Create `hooks.rs` with the trait and default implementation**

```rust
// src/secp256k1/hooks.rs

pub trait Secp256k1Hooks {
    fn fe_sqrt_and_assign(&mut self, fe: &mut super::field::FieldElement) -> bool;
    fn fe_invert_and_assign(&mut self, fe: &mut super::field::FieldElement);
    fn scalar_invert_and_assign(&mut self, scalar: &mut super::scalars::Scalar);
}

pub struct DefaultSecp256k1Hooks;

impl Secp256k1Hooks for DefaultSecp256k1Hooks {
    #[inline(always)]
    fn fe_sqrt_and_assign(&mut self, fe: &mut super::field::FieldElement) -> bool {
        fe.sqrt_in_place()
    }

    #[inline(always)]
    fn fe_invert_and_assign(&mut self, fe: &mut super::field::FieldElement) {
        fe.invert_in_place()
    }

    #[inline(always)]
    fn scalar_invert_and_assign(&mut self, scalar: &mut super::scalars::Scalar) {
        scalar.invert_in_place()
    }
}
```

- [ ] **Step 2: Declare the hooks module in `mod.rs`**

In `/root/airbender-platform/crates/airbender-crypto/src/secp256k1/mod.rs`, add after the existing module declarations (after line 8 `mod scalars;`):

```rust
pub mod hooks;
```

- [ ] **Step 3: Verify it compiles**

Run: `cd /root/airbender-platform && cargo build -p airbender-crypto`
Expected: compiles successfully

- [ ] **Step 4: Commit**

```bash
cd /root/airbender-platform
git add crates/airbender-crypto/src/secp256k1/hooks.rs crates/airbender-crypto/src/secp256k1/mod.rs
git commit -m "feat(crypto): add Secp256k1Hooks trait for oracle-based field operations"
```

### Task 2: Widen visibility on FieldElement

**Files:**
- Modify: `/root/airbender-platform/crates/airbender-crypto/src/secp256k1/field/mod.rs`

The goal is to change all `pub(crate)` to `pub` on both `FieldElement` and `FieldElementConst` structs and all their methods/constants. The `FieldStorage` struct stays `pub(crate)` since it's only used internally.

- [ ] **Step 1: Widen FieldElementConst visibility**

In `field/mod.rs`, change all `pub(crate)` on `FieldElementConst` and its impl block to `pub`. Affected items (lines 41-115):
- `pub struct FieldElementConst(pub(crate) ...)` -> `pub struct FieldElementConst(pub ...)`
- Constants: `ZERO`, `ONE`
- Methods: `from_bytes_unchecked`, `mul`, `mul_int`, `square`, `add`, `invert`, `negate`, `normalize`, `to_storage`, `normalizes_to_zero`

- [ ] **Step 2: Widen FieldElement visibility**

In `field/mod.rs`, change all `pub(crate)` on `FieldElement` and its impl block to `pub`. Affected items (lines 118-260):
- `pub struct FieldElement(pub(crate) ...)` -> `pub struct FieldElement(pub ...)`
- Constants: `ZERO`, `ONE`, `BETA`
- Methods: `from_bytes_unchecked` (also remove `#[cfg(test)]`), `from_bytes`, `mul_in_place`, `mul_int_in_place`, `square_in_place`, `add_in_place`, `double_in_place`, `sub_in_place`, `add_int_in_place`, `invert_in_place`, `sqrt_in_place_unchecked`, `sqrt_in_place`, `negate_in_place`, `normalize_in_place`, `is_odd`, `normalizes_to_zero`, `to_bytes`, `to_storage` (also remove `#[cfg(test)]`)

- [ ] **Step 3: Verify it compiles and tests pass**

Run: `cd /root/airbender-platform && cargo test -p airbender-crypto`
Expected: all tests pass

- [ ] **Step 4: Commit**

```bash
cd /root/airbender-platform
git add crates/airbender-crypto/src/secp256k1/field/mod.rs
git commit -m "feat(crypto): widen FieldElement visibility to pub for external hook consumers"
```

### Task 3: Widen visibility on Scalar

**Files:**
- Modify: `/root/airbender-platform/crates/airbender-crypto/src/secp256k1/scalars/mod.rs`
- Modify: `/root/airbender-platform/crates/airbender-crypto/src/secp256k1/scalars/invert.rs`

- [ ] **Step 1: Widen Scalar struct and method visibility**

In `scalars/mod.rs`, change all `pub(crate)` to `pub` on the `Scalar` struct and all its methods. Also remove `#[cfg(test)]` gates from items that are needed at runtime by zksync-os:

Items that need `#[cfg(test)]` removed AND `pub(crate)` -> `pub`:
- `ZERO` (line 38) — used in non-test code via `Secp256k1HooksWithOracle`
- `ONE` (line 40) — used in non-test `scalar_invert_and_assign`
- `from_bytes_unchecked` (line 47)
- `from_u128` (line 52)
- `from_be_hex` (line 58) — also remove the `#[allow(dead_code)]`
- `from_repr` (line 72)

Items that just need `pub(crate)` -> `pub`:
- `Scalar` struct (line 34): `pub struct Scalar(pub(crate) ...)` -> `pub struct Scalar(pub ...)`
- `from_signature` (line 62)
- `to_repr` (line 67)
- `from_k256_scalar` (line 78)
- `decompose` (line 82)
- `decompose_128` (line 87)
- `bits` (line 92)
- `bits_var` (line 96)
- `is_zero` (line 100)
- `negate_in_place` (line 104)

Also remove `#[cfg(test)]` from these trait impls that are needed by zksync-os at runtime:
- `impl core::ops::Sub for Scalar` (line 194) — used in `field_ops.rs` line 131: `t = t - Scalar::ONE`
- `impl core::ops::Neg for Scalar` (line 162) — required by `Sub` impl

The remaining `#[cfg(test)]`-gated trait impls (`Mul`, `Add`) can stay test-only unless needed.

- [ ] **Step 2: Widen `invert_in_place` visibility**

In `scalars/invert.rs` line 6, change:
```rust
pub(crate) fn invert_in_place(&mut self) {
```
to:
```rust
pub fn invert_in_place(&mut self) {
```

- [ ] **Step 3: Verify it compiles and tests pass**

Run: `cd /root/airbender-platform && cargo test -p airbender-crypto`
Expected: all tests pass

- [ ] **Step 4: Commit**

```bash
cd /root/airbender-platform
git add crates/airbender-crypto/src/secp256k1/scalars/mod.rs crates/airbender-crypto/src/secp256k1/scalars/invert.rs
git commit -m "feat(crypto): widen Scalar visibility to pub for external hook consumers"
```

### Task 4: Widen visibility on Affine and Jacobian

**Files:**
- Modify: `/root/airbender-platform/crates/airbender-crypto/src/secp256k1/points/mod.rs`
- Modify: `/root/airbender-platform/crates/airbender-crypto/src/secp256k1/points/affine.rs`
- Modify: `/root/airbender-platform/crates/airbender-crypto/src/secp256k1/points/jacobian.rs`

`Affine` is already `pub struct` but some of its methods are `pub(crate)`. `Jacobian` is `pub(crate)` and needs to stay internal (only used inside recover). But `decompress` and `to_affine` need to be `pub` for the `_with_hooks` variants.

- [ ] **Step 1: Widen Affine method visibility**

In `points/affine.rs`, change `pub(crate)` to `pub` on:
- `decompress` (line 135)
- `normalize_in_place` (line 171)
- `to_jacobian` (line 211)

Also remove `#[cfg(test)]` from `GENERATOR` (line 102-103) — it's used in airbender-crypto's own tests and may be useful externally.

- [ ] **Step 2: Widen Jacobian::to_affine visibility**

In `points/jacobian.rs` line 209, change:
```rust
pub(crate) fn to_affine(self) -> Affine {
```
to:
```rust
pub fn to_affine(self) -> Affine {
```

- [ ] **Step 3: Export Jacobian from points module**

In `points/mod.rs` line 6, change:
```rust
pub(crate) use jacobian::{Jacobian, JacobianConst};
```
to:
```rust
pub use jacobian::Jacobian;
pub(crate) use jacobian::JacobianConst;
```

- [ ] **Step 4: Verify it compiles and tests pass**

Run: `cd /root/airbender-platform && cargo test -p airbender-crypto`
Expected: all tests pass

- [ ] **Step 5: Commit**

```bash
cd /root/airbender-platform
git add crates/airbender-crypto/src/secp256k1/points/
git commit -m "feat(crypto): widen Affine and Jacobian visibility for hook variants"
```

### Task 5: Add `_with_hooks` variants for decompress and to_affine

**Files:**
- Modify: `/root/airbender-platform/crates/airbender-crypto/src/secp256k1/points/affine.rs`
- Modify: `/root/airbender-platform/crates/airbender-crypto/src/secp256k1/points/jacobian.rs`

- [ ] **Step 1: Add `decompress_with_hooks` and `set_xo_with_hooks` to Affine**

In `points/affine.rs`, add these methods to the `impl Affine` block (after the existing `set_xo` at line 169):

```rust
    pub fn decompress_with_hooks<H: super::super::hooks::Secp256k1Hooks>(
        x_bytes: &FieldBytes,
        y_is_odd: bool,
        hooks: &mut H,
    ) -> Option<Self> {
        #[allow(deprecated)]
        let len = x_bytes.len();
        debug_assert!(len == 32);

        #[allow(deprecated)]
        x_bytes.as_slice().try_into().ok().and_then(|x| {
            let x = FieldElement::from_bytes(x)?;
            let mut ret = Affine::DEFAULT;
            if ret.set_xo_with_hooks(&x, y_is_odd, hooks) {
                Some(ret)
            } else {
                None
            }
        })
    }

    fn set_xo_with_hooks<H: super::super::hooks::Secp256k1Hooks>(
        &mut self,
        x: &FieldElement,
        y_is_odd: bool,
        hooks: &mut H,
    ) -> bool {
        self.y = *x;
        self.y.square_in_place();
        self.y *= x;
        self.y += 7;

        let ret = hooks.fe_sqrt_and_assign(&mut self.y);
        self.y.normalize_in_place();

        if self.y.is_odd() != y_is_odd {
            self.y.negate_in_place(1);
        }

        self.x = *x;
        self.infinity = false;

        ret
    }
```

- [ ] **Step 2: Add `to_affine_with_hooks` to Jacobian**

In `points/jacobian.rs`, add this method to the `impl Jacobian` block (after `to_affine` ending at line 232):

```rust
    pub fn to_affine_with_hooks<H: super::super::hooks::Secp256k1Hooks>(
        self,
        hooks: &mut H,
    ) -> Affine {
        self.assert_verify();

        if self.is_infinity() {
            return Affine::INFINITY;
        }

        let mut zi = self.z;
        hooks.fe_invert_and_assign(&mut zi);

        let mut ret = Affine {
            x: zi,
            y: zi,
            infinity: false,
        };

        ret.x.square_in_place();
        ret.y *= ret.x;

        ret.x *= self.x;
        ret.y *= self.y;

        ret
    }
```

- [ ] **Step 3: Verify it compiles and tests pass**

Run: `cd /root/airbender-platform && cargo test -p airbender-crypto`
Expected: all tests pass

- [ ] **Step 4: Commit**

```bash
cd /root/airbender-platform
git add crates/airbender-crypto/src/secp256k1/points/
git commit -m "feat(crypto): add decompress_with_hooks and to_affine_with_hooks"
```

### Task 6: Add `recover_with_hooks` and `recover_with_context_and_hooks`

**Files:**
- Modify: `/root/airbender-platform/crates/airbender-crypto/src/secp256k1/recover.rs`
- Modify: `/root/airbender-platform/crates/airbender-crypto/src/secp256k1/mod.rs`

- [ ] **Step 1: Add the `_with_hooks` recovery functions**

In `recover.rs`, add these two functions after the existing `recover_with_context` function (after line 73):

```rust
#[cfg(feature = "secp256k1-static-context")]
pub fn recover_with_hooks<H: super::hooks::Secp256k1Hooks>(
    message: &crate::k256::Scalar,
    signature: &crate::k256::ecdsa::Signature,
    recovery_id: &crate::k256::ecdsa::RecoveryId,
    hooks: &mut H,
) -> Result<Affine, Secp256k1Err> {
    use super::context::ECRECOVER_CONTEXT;

    recover_with_context_and_hooks(message, signature, recovery_id, &ECRECOVER_CONTEXT, hooks)
}

pub fn recover_with_context_and_hooks<H: super::hooks::Secp256k1Hooks>(
    message: &crate::k256::Scalar,
    signature: &crate::k256::ecdsa::Signature,
    recovery_id: &crate::k256::ecdsa::RecoveryId,
    context: &ECMultContext,
    hooks: &mut H,
) -> Result<Affine, Secp256k1Err> {
    let (mut sigr, mut sigs) = Scalar::from_signature(signature);
    let message = Scalar::from_k256_scalar(*message);

    let mut brx = sigr.to_repr();

    if recovery_id.is_x_reduced() {
        match <U256 as FieldBytesEncoding<Secp256k1>>::decode_field_bytes(&brx)
            .checked_add(&Secp256k1::ORDER)
            .into_option()
        {
            Some(restored) => {
                brx = <U256 as FieldBytesEncoding<Secp256k1>>::encode_field_bytes(&restored);
            }
            None => return Err(Secp256k1Err::OperationOverflow),
        }
    }

    let is_odd = recovery_id.is_y_odd();
    let x = Affine::decompress_with_hooks(&brx, is_odd, hooks)
        .ok_or(Secp256k1Err::InvalidParams)?;

    let xj = x.to_jacobian();

    hooks.scalar_invert_and_assign(&mut sigr);
    sigs *= sigr;

    sigr *= message;
    sigr.negate_in_place();

    let mut pk = ecmult(&xj, &sigs, &sigr, context).to_affine_with_hooks(hooks);
    pk.normalize_in_place();

    if pk.is_infinity() {
        return Err(Secp256k1Err::RecoveredInfinity);
    }

    Ok(pk)
}
```

- [ ] **Step 2: Export the new functions from `mod.rs`**

In `secp256k1/mod.rs`, add after the existing re-exports (after line 17 `pub use recover::recover_with_context;`):

```rust
pub use recover::recover_with_context_and_hooks;

#[cfg(feature = "secp256k1-static-context")]
pub use recover::recover_with_hooks;
```

- [ ] **Step 3: Verify it compiles and tests pass**

Run: `cd /root/airbender-platform && cargo test -p airbender-crypto`
Expected: all tests pass

- [ ] **Step 4: Commit**

```bash
cd /root/airbender-platform
git add crates/airbender-crypto/src/secp256k1/recover.rs crates/airbender-crypto/src/secp256k1/mod.rs
git commit -m "feat(crypto): add recover_with_hooks for oracle-based ecrecover"
```

### Task 7: Add tests for hooks

**Files:**
- Modify: `/root/airbender-platform/crates/airbender-crypto/src/secp256k1/recover.rs` (tests module)

- [ ] **Step 1: Add test verifying hooks-based recovery matches hookless recovery**

In `recover.rs`, add a new test to the existing `mod tests` block (after the last test):

```rust
    #[cfg(feature = "secp256k1-static-context")]
    #[test]
    fn recover_with_default_hooks_matches_recover() {
        use super::hooks::DefaultSecp256k1Hooks;
        use k256::ecdsa::{RecoveryId, Signature};
        use {
            k256::elliptic_curve::ops::Reduce,
            k256::{ecdsa::hazmat::bits2field, Scalar},
        };

        let digest = [
            56, 209, 138, 203, 103, 210, 92, 139, 185, 148, 39, 100, 182, 47, 24, 225, 112,
            84, 246, 106, 129, 123, 212, 41, 84, 35, 173, 249, 237, 152, 135, 62,
        ];
        let r = digest;
        let s = [
            120, 157, 29, 212, 35, 210, 95, 7, 114, 210, 116, 141, 96, 247, 228, 184, 27,
            177, 77, 8, 110, 186, 142, 142, 142, 251, 109, 207, 248, 164, 174, 2,
        ];

        let signature = Signature::from_scalars(r, s).unwrap();
        let recovery_id = RecoveryId::try_from(0u8).unwrap();
        let message = <Scalar as Reduce<k256::U256>>::reduce_bytes(
            &bits2field::<k256::Secp256k1>(&digest).unwrap(),
        );

        let without_hooks = super::recover(&message, &signature, &recovery_id).unwrap();
        let with_hooks = super::recover_with_hooks(
            &message,
            &signature,
            &recovery_id,
            &mut DefaultSecp256k1Hooks,
        )
        .unwrap();

        assert_eq!(without_hooks, with_hooks);
    }
```

- [ ] **Step 2: Run tests**

Run: `cd /root/airbender-platform && cargo test -p airbender-crypto`
Expected: all tests pass, including the new `recover_with_default_hooks_matches_recover`

- [ ] **Step 3: Commit**

```bash
cd /root/airbender-platform
git add crates/airbender-crypto/src/secp256k1/recover.rs
git commit -m "test(crypto): verify hooks-based recovery matches standard recovery"
```

### Task 8: Run full checks and open draft PR

**Files:** none (CI/quality checks only)

- [ ] **Step 1: Run clippy**

Run: `cd /root/airbender-platform && cargo clippy -p airbender-crypto -- -D warnings`
Expected: no warnings

- [ ] **Step 2: Run fmt**

Run: `cd /root/airbender-platform && cargo fmt -p airbender-crypto`
Expected: no changes (or apply formatting)

- [ ] **Step 3: Run full test suite**

Run: `cd /root/airbender-platform && cargo test -p airbender-crypto`
Expected: all tests pass

- [ ] **Step 4: Push branch and open draft PR**

```bash
cd /root/airbender-platform
git push -u origin <branch-name>
gh pr create --draft --title "feat(crypto): add Secp256k1Hooks for oracle-based EC field operations" --body "$(cat <<'EOF'
# What

Add hook support to airbender-crypto's secp256k1 module for oracle-based field operations (sqrt, invert, scalar invert). This enables zksync-os to migrate from its local crypto crate to the upstream airbender-crypto.

## Why

zksync-os needs the hooks mechanism to route expensive field operations through an oracle in proving mode, where the prover provides hints and the verifier checks them. This is a prerequisite for the crypto crate migration.

## Changes

- Add `Secp256k1Hooks` trait and `DefaultSecp256k1Hooks` implementation
- Add `_with_hooks` variants: `recover_with_hooks`, `recover_with_context_and_hooks`, `decompress_with_hooks`, `to_affine_with_hooks`
- Widen visibility of `FieldElement`, `Scalar`, `Affine`, `Jacobian` from `pub(crate)` to `pub`
- Existing no-hook API is completely unchanged

## Checklist

- [x] PR title corresponds to the body of PR (we generate changelog entries from PRs).
- [x] Tests for the changes have been added / updated.
- [x] Documentation comments have been added / updated.
- [x] Code has been formatted via `cargo fmt` and `cargo clippy`.
EOF
)"
```

---

## PR 2: zksync-os changes (in /root/zksync-os)

### Task 9: Update workspace Cargo.toml

**Files:**
- Modify: `/root/zksync-os/Cargo.toml`

- [ ] **Step 1: Add `crypto` as workspace dependency pointing to airbender-platform**

In the `[workspace.dependencies]` section (after the existing airbender-platform entries around line 75), add:

```toml
crypto = { git = "https://github.com/matter-labs/airbender-platform", rev = "<PR1-merge-rev>", package = "airbender-crypto", default-features = false }
```

Also add the new transitive dependencies needed by airbender-crypto's Keccak delegation:

```toml
common_constants = { git = "https://github.com/matter-labs/zksync-airbender", rev = "24a74ace", default-features = false }
seq-macro = { version = "0.3.6", default-features = false }
```

- [ ] **Step 2: Remove `crypto` from workspace members**

In the `[workspace]` `members` list (line 9), remove:
```toml
    "crypto",
```

Also remove from the `exclude` list (line 48):
```toml
    "crypto/src/blake2s/test_program",
```

- [ ] **Step 3: Verify it parses**

Run: `cd /root/zksync-os && cargo metadata --no-deps 2>&1 | head -5`
Expected: no parse errors (may have resolution errors until per-crate Cargo.toml is updated)

- [ ] **Step 4: Commit**

```bash
cd /root/zksync-os
git add Cargo.toml
git commit -m "chore: add airbender-crypto as workspace dependency"
```

### Task 10: Update per-crate Cargo.toml files

**Files:**
- Modify: `/root/zksync-os/basic_system/Cargo.toml`
- Modify: `/root/zksync-os/basic_bootloader/Cargo.toml`
- Modify: `/root/zksync-os/callable_oracles/Cargo.toml`
- Modify: `/root/zksync-os/evm_interpreter/Cargo.toml`
- Modify: `/root/zksync-os/zk_ee/Cargo.toml`
- Modify: `/root/zksync-os/forward_system/Cargo.toml`
- Modify: `/root/zksync-os/proof_running_system/Cargo.toml`
- Modify: `/root/zksync-os/api/Cargo.toml`
- Modify: `/root/zksync-os/scripts/Cargo.toml`
- Modify: `/root/zksync-os/tests/rig/Cargo.toml`
- Modify: `/root/zksync-os/circuit_test_program/Cargo.toml`
- Modify: `/root/zksync-os/zksync_os/Cargo.toml`

- [ ] **Step 1: Update each crate's crypto dependency**

For each crate, replace the local path dependency with a workspace reference. The feature forwarding stays the same since airbender-crypto has the same features.

For most crates, change:
```toml
crypto = { path = "../crypto", default-features = false }
```
to:
```toml
crypto = { workspace = true }
```

Special cases:
- `basic_system/Cargo.toml` has two crypto entries (one under `[dependencies]` with `secp256k1-static-context`, one under `[dev-dependencies]` with `secp256k1-static-context` and `testing`). Change both:
  - `[dependencies]`: `crypto = { workspace = true, features = ["secp256k1-static-context"] }`
  - `[dev-dependencies]`: `crypto = { workspace = true, features = ["secp256k1-static-context", "testing"] }`
- `circuit_test_program/Cargo.toml`: `crypto = { workspace = true, features = ["proving"] }` — note this crate is excluded from workspace, so it cannot use `workspace = true`. Instead use the full git dep:
  ```toml
  crypto = { git = "https://github.com/matter-labs/airbender-platform", rev = "<PR1-merge-rev>", package = "airbender-crypto", default-features = false, features = ["proving"] }
  ```
- `zksync_os/Cargo.toml`: also excluded from workspace, same treatment as circuit_test_program:
  ```toml
  crypto = { git = "https://github.com/matter-labs/airbender-platform", rev = "<PR1-merge-rev>", package = "airbender-crypto", default-features = false, optional = true }
  ```
- `tests/rig/Cargo.toml`: `crypto = { workspace = true }`

- [ ] **Step 2: Verify workspace resolves**

Run: `cd /root/zksync-os && cargo check 2>&1 | tail -20`
Expected: may fail on missing files (crypto/ deleted next), but dependency resolution should work

- [ ] **Step 3: Commit**

```bash
cd /root/zksync-os
git add basic_system/Cargo.toml basic_bootloader/Cargo.toml callable_oracles/Cargo.toml evm_interpreter/Cargo.toml zk_ee/Cargo.toml forward_system/Cargo.toml proof_running_system/Cargo.toml api/Cargo.toml scripts/Cargo.toml tests/rig/Cargo.toml circuit_test_program/Cargo.toml zksync_os/Cargo.toml
git commit -m "chore: switch per-crate crypto deps to workspace reference"
```

### Task 11: Delete local crypto crate

**Files:**
- Delete: `/root/zksync-os/crypto/` (entire directory)

- [ ] **Step 1: Remove the crypto directory**

```bash
cd /root/zksync-os && rm -rf crypto/
```

- [ ] **Step 2: Commit**

```bash
cd /root/zksync-os
git add -A crypto/
git commit -m "chore: remove local crypto crate (replaced by airbender-crypto)"
```

### Task 12: Update call sites — callable_oracles

**Files:**
- Modify: `/root/zksync-os/callable_oracles/src/field_hints/impls.rs`

The local crypto crate had methods named `sqrt_in_place_inner` and `invert_in_place_inner`. In airbender-crypto, these are `sqrt_in_place` and `invert_in_place`.

- [ ] **Step 1: Update method names in impls.rs**

In `callable_oracles/src/field_hints/impls.rs`:

Line 19 — change:
```rust
    let is_quadratic_residue = candidate.sqrt_in_place_inner();
```
to:
```rust
    let is_quadratic_residue = candidate.sqrt_in_place();
```

Line 30 — change:
```rust
    el.invert_in_place_inner();
```
to:
```rust
    el.invert_in_place();
```

- [ ] **Step 2: Commit**

```bash
cd /root/zksync-os
git add callable_oracles/src/field_hints/impls.rs
git commit -m "fix: update field method names for airbender-crypto API"
```

### Task 13: Update call sites — ecrecover

**Files:**
- Modify: `/root/zksync-os/basic_system/src/system_functions/ecrecover.rs`

- [ ] **Step 1: Update the oracle path to use `recover_with_hooks`**

In `ecrecover.rs`, the `ecrecover_inner` function (lines 108-151) currently calls `crypto::secp256k1::recover` with hooks for both paths. Update it to use the hookless API for the non-oracle path and `recover_with_hooks` for the oracle path.

Change lines 128-141 from:
```rust
    let res = match oracle {
        Some(oracle) => crypto::secp256k1::recover(
            &message,
            &signature,
            &recovery_id,
            &mut Secp256k1HooksWithOracle::new(oracle),
        ),
        None => crypto::secp256k1::recover(
            &message,
            &signature,
            &recovery_id,
            &mut DefaultSecp256k1Hooks,
        ),
    };
```
to:
```rust
    let res = match oracle {
        Some(oracle) => crypto::secp256k1::recover_with_hooks(
            &message,
            &signature,
            &recovery_id,
            &mut Secp256k1HooksWithOracle::new(oracle),
        ),
        None => crypto::secp256k1::recover(
            &message,
            &signature,
            &recovery_id,
        ),
    };
```

Also remove the now-unused import on line 3:
```rust
use crypto::secp256k1::hooks::DefaultSecp256k1Hooks;
```

- [ ] **Step 2: Verify it compiles**

Run: `cd /root/zksync-os && cargo check -p basic_system`
Expected: compiles

- [ ] **Step 3: Commit**

```bash
cd /root/zksync-os
git add basic_system/src/system_functions/ecrecover.rs
git commit -m "fix: use recover_with_hooks for oracle path in ecrecover"
```

### Task 14: Build and test the full workspace

**Files:** none (verification only)

- [ ] **Step 1: Build the workspace**

Run: `cd /root/zksync-os && cargo build`
Expected: compiles successfully

- [ ] **Step 2: Run clippy**

Run: `cd /root/zksync-os && cargo clippy --all -- -D warnings`
Expected: no warnings. Fix any issues that arise.

- [ ] **Step 3: Run fmt**

Run: `cd /root/zksync-os && cargo fmt`
Expected: no changes (or apply formatting)

- [ ] **Step 4: Run workspace tests**

Run: `cd /root/zksync-os && cargo test --workspace`
Expected: all tests pass

- [ ] **Step 5: Commit any fixes**

If clippy/fmt required changes, commit them:
```bash
cd /root/zksync-os
git add -A
git commit -m "chore: fix clippy and formatting after crypto migration"
```

### Task 15: Open draft PR for zksync-os

**Files:** none (PR only)

- [ ] **Step 1: Push branch and open draft PR**

```bash
cd /root/zksync-os
git push -u origin <branch-name>
gh pr create --draft --title "feat: migrate crypto crate to airbender-crypto" --body "$(cat <<'EOF'
## What ❔

Migrate from the local `crypto` crate to the upstream `airbender-crypto` from `airbender-platform`. The local crate is deleted and replaced with a workspace dependency using `package = "airbender-crypto"` aliased as `crypto` to avoid renaming imports.

## Why ❔

Consolidate cryptographic primitives in a single upstream crate (`airbender-crypto`) instead of maintaining a local fork. The upstream crate now includes hook support for oracle-based EC field operations needed by proving mode.

## Changes

- Add `crypto` workspace dependency pointing to `airbender-crypto` via git
- Delete local `crypto/` directory
- Update all per-crate Cargo.toml to use workspace dependency
- Update `callable_oracles` field hints: `_inner` method renames (`sqrt_in_place_inner` → `sqrt_in_place`)
- Update `ecrecover`: use `recover_with_hooks` for oracle path, hookless `recover` for non-oracle path

## Is this a breaking change?
- [ ] Yes
- [x] No

## Checklist

- [x] PR title corresponds to the body of PR (we generate changelog entries from PRs).
- [x] Tests for the changes have been added / updated.
- [x] Documentation comments have been added / updated.
- [x] Code has been formatted.
EOF
)"
```
