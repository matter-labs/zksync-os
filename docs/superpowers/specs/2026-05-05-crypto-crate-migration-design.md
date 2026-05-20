# Crypto Crate Migration: `crypto` -> `airbender-crypto`

## Overview

Migrate zksync-os from its local `crypto` crate to the upstream `airbender-crypto` crate in `airbender-platform`. The two crates share a common origin and are ~95% identical. The main gap is that airbender-crypto removed the `Secp256k1Hooks` trait, which zksync-os needs for oracle-based field operations in proving mode.

## Approach

**Approach A — Duplicate functions.** Keep airbender-crypto's existing no-hook API intact. Add parallel `_with_hooks` functions that accept a `Secp256k1Hooks` trait parameter. The no-hook versions remain unchanged; the `_with_hooks` versions are new additions.

This was chosen over feature-gated hooks (Approach C) for simplicity. C can be layered on later if needed.

## Deliverables

Two PRs, merged sequentially:

1. **PR to `airbender-platform`**: add hooks support + visibility fixes to `airbender-crypto`
2. **PR to `zksync-os`**: switch dependency, delete local `crypto/` crate, adapt hook call sites

---

## PR 1: airbender-crypto Changes

### 1.1 Hooks Trait

Add `src/secp256k1/hooks.rs` with:

```rust
pub trait Secp256k1Hooks {
    fn fe_sqrt_and_assign(&mut self, fe: &mut FieldElement) -> bool;
    fn fe_invert_and_assign(&mut self, fe: &mut FieldElement);
    fn scalar_invert_and_assign(&mut self, scalar: &mut Scalar);
}

pub struct DefaultSecp256k1Hooks;
```

`DefaultSecp256k1Hooks` delegates to the existing `sqrt_in_place()`, `invert_in_place()` methods on `FieldElement` and `Scalar`. No method renames needed.

Declare `pub mod hooks` in `src/secp256k1/mod.rs`.

### 1.2 Duplicate Functions with Hooks

Add new functions alongside existing ones:

| Existing (unchanged)               | New                                                  |
|-------------------------------------|------------------------------------------------------|
| `recover(msg, sig, id)`            | `recover_with_hooks(msg, sig, id, hooks)`           |
| `recover_with_context(msg, sig, id, ctx)` | `recover_with_context_and_hooks(msg, sig, id, ctx, hooks)` |
| `Affine::decompress(bytes, is_odd)` | `Affine::decompress_with_hooks(bytes, is_odd, hooks)` |
| `Jacobian::to_affine()`            | `Jacobian::to_affine_with_hooks(hooks)`             |

The existing no-hook functions keep their current implementation (direct field ops). The `_with_hooks` variants route sqrt/invert/scalar_invert through the hooks trait.

### 1.3 Visibility Widening

Change all `pub(crate)` to `pub` on `FieldElement` and `Scalar` structs and all their methods/constants. These types are needed externally by zksync-os for implementing custom hooks (oracle-based field operations).

### 1.4 Testing

- Existing tests pass unchanged (no-hook API untouched).
- Add tests verifying `DefaultSecp256k1Hooks` produces identical results to calling methods directly.
- Port proptest-based recovery tests from zksync-os `crypto/tests/secp256k1.rs` that exercise hooks.

---

## PR 2: zksync-os Changes

### 2.1 Workspace Cargo.toml

Replace the local crypto crate with an upstream dependency using `package` renaming to avoid import changes:

```toml
crypto = { git = "https://github.com/matter-labs/airbender-platform", rev = "...", package = "airbender-crypto", default-features = false }
```

Add new workspace dependencies required by airbender-crypto's Keccak delegation:
- `common_constants` (from zksync-airbender)
- `seq-macro`

Remove `crypto` from workspace members list.

### 2.2 Delete Local `crypto/` Directory

The entire `/root/zksync-os/crypto/` directory is deleted. All functionality is now provided by the upstream crate.

### 2.3 Per-Crate Cargo.toml Updates

Each of the ~12 dependent crates switches from `crypto = { path = "../crypto", ... }` to `crypto = { workspace = true }` (or updates existing workspace reference). Feature flags (`forward`, `proving`, `secp256k1-static-context`, `testing`) remain the same.

Affected crates: `api`, `basic_bootloader`, `basic_system`, `callable_oracles`, `circuit_test_program`, `evm_interpreter`, `forward_system`, `proof_running_system`, `scripts`, `zk_ee`, `zksync_os`, `tests/rig`.

### 2.4 Call Site Changes

Only two files need code changes (hooks-related):

**`basic_system/src/system_functions/ecrecover.rs`**:
- Oracle path: `crypto::secp256k1::recover(msg, sig, id, hooks)` -> `crypto::secp256k1::recover_with_hooks(msg, sig, id, hooks)`
- Non-oracle path: `crypto::secp256k1::recover(msg, sig, id, &mut DefaultSecp256k1Hooks)` -> `crypto::secp256k1::recover(msg, sig, id)` (simpler, no hooks needed)

**`basic_system/src/system_functions/field_ops.rs`**:
- `impl crypto::secp256k1::hooks::Secp256k1Hooks for Secp256k1HooksWithOracle` — unchanged, trait now comes from airbender-crypto
- All test code using `DefaultSecp256k1Hooks` and `Secp256k1HooksWithOracle` — unchanged

All other `use crypto::` imports across the codebase work without modification due to the `package` rename.

### 2.5 Testing

- `cargo build` — workspace compiles
- `cargo test --workspace` — all tests pass
- `cargo clippy --all -- -D warnings` — clean
- Malicious oracle tests in `field_ops.rs` continue to verify hook correctness

---

## Out of Scope

- Keccak delegation improvements (airbender-crypto has `keccak_special5` — this is additive and already available after migration)
- `cfg` guard cleanup (`proving` + `fuzzing` -> `proving` only — already done in airbender-crypto)
- Further visibility tightening or API surface reduction
