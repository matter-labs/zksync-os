# Phase 1: Airbender Platform Integration — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Migrate zksync-os from direct riscv_transpiler/execution_utils dependencies to airbender-platform's build, runtime, and host APIs.

**Architecture:** The guest binary adopts `#[airbender::main]` and builds via `cargo airbender build`. The host side uses platform's `TranspilerRunner` and `CpuProver` via a thin `zksync_os_runner` facade. The test rig switches to a two-pass flow: forward run records `Vec<u32>`, transpiler run replays it. Wire format (UsizeSerializable) stays unchanged.

**Tech Stack:** Rust, airbender-platform (airbender-sdk, airbender-host), riscv32im-risc0-zkvm-elf target

**Spec:** `docs/superpowers/specs/2026-03-24-airbender-platform-integration-phase1-design.md`

---

### Task 1: Add workspace dependencies for airbender-platform

**Files:**
- Modify: `/root/zksync-os/Cargo.toml` (workspace dependencies section)

- [ ] **Step 1: Add airbender-platform workspace deps**

Add to `[workspace.dependencies]`:
```toml
airbender-sdk = { path = "../airbender-platform/crates/airbender-sdk" }
airbender-host = { path = "../airbender-platform/crates/airbender-host" }
airbender-guest = { path = "../airbender-platform/crates/airbender-guest" }
```

Keep existing `riscv_transpiler`, `execution_utils`, `prover_examples` for now — they'll be removed in later tasks after dependents are migrated.

- [ ] **Step 2: Verify workspace resolves**

Run: `cargo check -p forward_system 2>&1 | head -20`
Expected: compiles (no changes to existing crates yet)

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "chore: add airbender-platform workspace dependencies"
```

---

### Task 2: Add safe allocator init wrapper in `proof_running_system`

**Files:**
- Modify: `/root/zksync-os/proof_running_system/src/system/bootloader.rs`

The `#[airbender::main]` macro requires `fn(*mut usize, *mut usize)` but the current
`init_allocator` is `unsafe fn`. We need a safe wrapper. We also need to refactor
`run_proving` to not call `init_allocator` internally (avoiding double-init).

- [ ] **Step 1: Read the current `init_allocator` and `run_proving`**

Read `/root/zksync-os/proof_running_system/src/system/bootloader.rs` to understand
the current signatures and how `run_proving` uses `heap_start`/`heap_end`.

- [ ] **Step 2: Add safe wrapper**

Add below existing `init_allocator`:
```rust
/// Safe wrapper for use with `#[airbender::main(allocator_init = ...)]`.
pub fn init_allocator_safe(heap_start: *mut usize, heap_end: *mut usize) {
    unsafe { init_allocator(heap_start, heap_end) }
}
```

- [ ] **Step 3: Refactor `run_proving` to not init allocator**

`run_proving` currently takes `heap_start, heap_end` and calls `init_allocator`.
Remove the `init_allocator` call and the `heap_start`/`heap_end` parameters.
If heap boundaries are needed internally, use
`riscv_common::boot_sequence::heap_start()/heap_end()` or pass them differently.

Check all callers of `run_proving` — it's called from `zksync_os/src/main.rs`.
Update the call site to match the new signature.

- [ ] **Step 4: Verify compilation**

Run: `cargo check -p proof_running_system`
Expected: compiles

Note: changing `run_proving`'s signature will break `zksync_os/src/main.rs` until
Task 3 updates the call site. This is expected — Tasks 2 and 3 should be done
together before verifying the guest builds.

- [ ] **Step 5: Commit**

```bash
git add proof_running_system/
git commit -m "refactor(proof_running_system): add safe allocator wrapper, remove double-init from run_proving"
```

---

### Task 3: Migrate `zksync_os` guest binary to `#[airbender::main]`

**Files:**
- Modify: `/root/zksync-os/zksync_os/Cargo.toml`
- Modify: `/root/zksync-os/zksync_os/.cargo/config.toml`
- Rewrite: `/root/zksync-os/zksync_os/src/main.rs`
- Remove: `/root/zksync-os/zksync_os/src/asm/asm_reduced.S`
- Remove: `/root/zksync-os/zksync_os/src/memset.s`
- Remove: `/root/zksync-os/zksync_os/src/memcpy.s`
- Remove: `/root/zksync-os/zksync_os/src/trap_frame.rs`
- Remove: `/root/zksync-os/zksync_os/src/helper_reg_utils.rs`
- Remove: `/root/zksync-os/zksync_os/src/utils.rs`

- [ ] **Step 1: Update `Cargo.toml`**

Replace `riscv_common` dependency with `airbender-sdk`. Remove `heapless` —
it's only used by `quasi_uart.rs` which is replaced by `airbender-rt`'s UART.

```toml
[dependencies]
airbender = { package = "airbender-sdk", path = "../airbender-platform/crates/airbender-sdk", default-features = false }
proof_running_system = { path = "../proof_running_system", default-features = false }
crypto = { path = "../crypto", optional = true }
```

Note: use `default-features = false` on `airbender-sdk` to avoid the platform's
default `#[global_allocator]` conflicting with `proof_running_system`'s
`OptionalGlobalAllocator`. Check which sdk features need explicit enabling.

- [ ] **Step 2: Update `.cargo/config.toml`**

Replace with platform-matching config:
```toml
[build]
target = "riscv32im-risc0-zkvm-elf"
rustflags = [
  "-C", "target-feature=+m,-unaligned-scalar-mem,+relax",
  "-C", "link-arg=-Tmemory.x",
  "-C", "link-arg=-Tlink.x",
  "-C", "link-arg=--save-temps",
  "-C", "force-frame-pointers",
  "-C", "passes=lower-atomic",
  "--cfg", "getrandom_backend=\"custom\"",
]

[env]
CC = "clang"

[unstable]
build-std = ["alloc", "core", "panic_abort", "compiler_builtins"]
build-std-features = ["compiler-builtins-mem"]
```

Note: if build fails with missing std symbols, try adding `"std"` and `"proc_macro"`
to `build-std` (matching platform examples). Start without them.

- [ ] **Step 3: Rewrite `main.rs`**

Replace the ~245-line `main.rs` with:

```rust
#![no_std]
#![no_main]
#![allow(incomplete_features)]
#![feature(allocator_api)]

use proof_running_system::system::bootloader::{init_allocator_safe, run_proving};

mod csr {
    use airbender::rt::sys::{read_word, write_word};

    #[derive(Clone, Copy, Debug)]
    pub struct CSRBasedNonDeterminismSource;

    impl proof_running_system::io_oracle::NonDeterminismCSRSourceImplementation
        for CSRBasedNonDeterminismSource
    {
        #[inline(always)]
        fn csr_read_impl() -> usize {
            const {
                assert!(core::mem::size_of::<usize>() == core::mem::size_of::<u32>());
            }
            read_word() as usize
        }
        #[inline(always)]
        fn csr_write_impl(value: usize) {
            core::hint::black_box(write_word(value as u32))
        }
    }
}

pub use self::csr::CSRBasedNonDeterminismSource;

#[cfg(not(feature = "print_debug_info"))]
type LoggerTy = proof_running_system::zk_ee::system::NullLogger;

// TODO: for print_debug_info, use airbender::rt UART logger
#[cfg(feature = "print_debug_info")]
type LoggerTy = airbender::rt::uart::QuasiUart;

#[airbender::main(allocator_init = init_allocator_safe)]
fn main() -> [u32; 8] {
    run_proving::<CSRBasedNonDeterminismSource, LoggerTy>()
}
```

Key changes:
- `riscv_common::csr_read_word`/`csr_write_word` → `airbender::rt::sys::read_word`/`write_word`
- All boot code removed (handled by `airbender-rt`)
- All assembly files removed
- Panic handler, allocator, `_start_rust` all handled by the macro

Check: does `airbender::rt::sys::write_word` take `u32` or `usize`? The platform's
`sys.rs` has `pub fn write_word(word: u32)`. Adjust the call accordingly.

For `LoggerTy` with `print_debug_info`: the platform's `airbender::rt::uart::QuasiUart`
implements `core::fmt::Write` but may not implement zksync-os's `Logger` trait.
If it doesn't, keep the local `quasi_uart.rs` and `heapless` dep for now, gated
behind `print_debug_info`. Make a definitive choice during implementation:
1. If `QuasiUart` can satisfy `Logger` trait → remove `quasi_uart.rs` + `heapless`
2. If not → keep `quasi_uart.rs` but update its CSR calls to use `airbender::rt::sys`

- [ ] **Step 4: Remove old files**

```bash
rm -f zksync_os/src/asm/asm_reduced.S
rm -f zksync_os/src/memset.s
rm -f zksync_os/src/memcpy.s
rm -f zksync_os/src/trap_frame.rs
rm -f zksync_os/src/helper_reg_utils.rs
rm -f zksync_os/src/utils.rs
```

Remove `quasi_uart.rs` if switched to platform UART; keep if Logger trait is
incompatible (see step 3 note).

- [ ] **Step 5: Verify guest builds**

Run from the `zksync_os/` directory:
```bash
cargo build --features "proving,for_tests" --release
```

Debug any compilation errors. Common issues:
- Missing `build-std` components — add `"std"`, `"proc_macro"` if needed
- Linker script path resolution — verify `memory.x`/`link.x` are found via `riscv_common`
- `init_allocator_safe` signature mismatch — adjust wrapper
- CSR function signatures (`read_word`/`write_word` argument types)

- [ ] **Step 6: Commit**

```bash
git add zksync_os/
git commit -m "feat(zksync_os): migrate guest binary to #[airbender::main]"
```

---

### Task 4: Migrate `dump_bin.sh` to use `cargo airbender build`

**Files:**
- Modify: `/root/zksync-os/zksync_os/dump_bin.sh`

- [ ] **Step 1: Install cargo-airbender locally**

```bash
cargo install --path /root/airbender-platform/crates/cargo-airbender
```

Verify: `cargo airbender --help`

- [ ] **Step 2: Rewrite `dump_bin.sh` internals**

Keep the `--type` interface. Replace `cargo build`/`cargo objcopy` with
`cargo airbender build`. Each type maps features to `cargo airbender build` args.

The tricky part: `cargo airbender build` outputs to `dist/<app-name>/app.{bin,elf,text}`.
Some types share the same app-name (e.g., `for-tests` and `for-tests-benchmarking`
both output as `for_tests`). Handle this by running `cargo airbender build` with
the `--app-name` flag.

```bash
#!/bin/sh
set -e

USAGE="Usage: $0 --type {singleblock-batch|...}"
TYPE=""

while [ "$#" -gt 0 ]; do
  case "$1" in
    --type)
      [ "$#" -ge 2 ] || { echo "Missing value for --type"; echo "$USAGE"; exit 2; }
      TYPE="$2"; shift 2 ;;
    *) echo "Unknown argument: $1"; echo "$USAGE"; exit 2 ;;
  esac
done

case "$TYPE" in
  for-tests)
    APP_NAME="for_tests"
    FEATURES="proving,for_tests"
    ;;
  for-tests-benchmarking)
    APP_NAME="for_tests"
    FEATURES="proving,for_tests,benchmarking"
    ;;
  for-tests-logging-enabled)
    APP_NAME="for_tests"
    FEATURES="proving,for_tests,print_debug_info"
    ;;
  singleblock-batch)
    APP_NAME="singleblock_batch"
    FEATURES="proving,production"
    ;;
  # ... etc for all types, matching existing feature sets
  *)
    echo "Invalid --type: $TYPE"; echo "$USAGE"; exit 1 ;;
esac

cargo airbender build \
  --app-name "$APP_NAME" \
  --profile release \
  -- --features "$FEATURES"

echo "Built [$TYPE] → dist/$APP_NAME/"
```

- [ ] **Step 3: Test the build**

```bash
cd zksync_os && ./dump_bin.sh --type for-tests
ls -la dist/for_tests/
```

Expected: `app.bin`, `app.elf`, `app.text`, `manifest.toml` in `dist/for_tests/`.

- [ ] **Step 4: Commit**

```bash
git add zksync_os/dump_bin.sh
git commit -m "chore(zksync_os): migrate dump_bin.sh to cargo airbender build"
```

---

### Task 5: Update artifact path resolution in test rig

**Files:**
- Modify: `/root/zksync-os/tests/rig/src/chain.rs` (function `get_zksync_os_path`)

- [ ] **Step 1: Update `get_zksync_os_path`**

Change path resolution to point at `dist/<app>/app.{ext}`:

```rust
fn get_zksync_os_path(app_name: &Option<String>, extension: &str) -> PathBuf {
    let app = app_name.as_deref().unwrap_or("for_tests");
    let filename = format!("app.{extension}");
    let zksync_os_path = std::env::var("OVERRIDE_ZKSYNC_OS_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(std::env::var("CARGO_WORKSPACE_DIR").unwrap())
                .join("zksync_os")
                .join("dist")
                .join(app)
        });
    zksync_os_path.join(filename)
}
```

- [ ] **Step 2: Build the binary and run a test**

```bash
cd zksync_os && ./dump_bin.sh --type for-tests
cd .. && cargo test -p unit --features rig/no_print -- --test-threads=1 2>&1 | tail -20
```

Expected: tests find binaries at new paths and pass.

- [ ] **Step 3: Commit**

```bash
git add tests/rig/src/chain.rs
git commit -m "fix(rig): update binary path resolution for cargo airbender build dist layout"
```

---

### Task 6: Rewrite `zksync_os_runner` as facade over `airbender-host`

**Files:**
- Modify: `/root/zksync-os/zksync_os_runner/Cargo.toml`
- Rewrite: `/root/zksync-os/zksync_os_runner/src/lib.rs`

- [ ] **Step 1: Update `Cargo.toml`**

Replace `riscv_transpiler` and `common_constants` with `airbender-host`:

```toml
[dependencies]
airbender-host = { workspace = true }
cycle_marker = { path = "../cycle_marker", optional = true }

[features]
cycle_marker = ["dep:cycle_marker"]
flamegraph = []
```

Remove `riscv_transpiler`, `common_constants` dependencies.

- [ ] **Step 2: Rewrite `lib.rs`**

```rust
use std::path::{Path, PathBuf};
use airbender_host::program::Program;
use airbender_host::runner::{Runner, ExecutionResult, FlamegraphConfig};

pub fn run(
    dist_dir: PathBuf,
    cycles: usize,
    input_words: &[u32],
) -> [u32; 8] {
    run_and_get_effective_cycles(dist_dir, cycles, input_words).0
}

pub fn run_and_get_effective_cycles(
    dist_dir: PathBuf,
    cycles: usize,
    input_words: &[u32],
) -> ([u32; 8], Option<u64>) {
    run_inner(dist_dir, cycles, input_words, None)
}

pub fn run_default_with_flamegraph_path(
    dist_dir: PathBuf,
    sym_path: PathBuf,
    cycles: usize,
    input_words: &[u32],
    flamegraph_path: Option<PathBuf>,
) -> ([u32; 8], Option<u64>) {
    let flamegraph = flamegraph_path.map(|output| FlamegraphConfig {
        output,
        sampling_rate: 1,
        inverse: false,
        elf_path: Some(sym_path),
    });
    run_inner(dist_dir, cycles, input_words, flamegraph)
}

fn run_inner(
    dist_dir: PathBuf,
    cycles: usize,
    input_words: &[u32],
    flamegraph: Option<FlamegraphConfig>,
) -> ([u32; 8], Option<u64>) {
    println!("ZK RISC-V transpiler is starting");

    let program = Program::load(&dist_dir)
        .unwrap_or_else(|e| panic!("failed to load program from {}: {e}", dist_dir.display()));

    let mut builder = program.transpiler_runner()
        .with_cycles(cycles);

    if let Some(fg) = flamegraph {
        builder = builder.with_flamegraph(fg);
    }

    let runner = builder.build()
        .expect("failed to build transpiler runner");

    let result = runner.run(input_words)
        .expect("transpiler execution failed");

    let output = result.receipt.output;

    #[allow(unused_mut)]
    let mut effective_cycles = Some(result.cycles_executed as u64);

    #[cfg(feature = "cycle_marker")]
    {
        if let Some(cm) = result.cycle_markers {
            effective_cycles = cycle_marker::print_cycle_markers(cm);
        }
    }

    (output, effective_cycles)
}

// Helper for run_and_get_effective_cycles_from_bytes if still needed
pub fn run_and_get_effective_cycles_from_bytes(
    img_bytes: &[u8],
    text_bytes: &[u8],
    cycles: usize,
    input_words: &[u32],
) -> ([u32; 8], Option<u64>) {
    // For backward compat: create a temp dist dir, or use low-level API
    // This may need to use TranspilerRunnerBuilder directly instead of Program
    todo!("implement if needed, or remove callers")
}
```

Note: The API changes from taking `impl NonDeterminismCSRSource` to `&[u32]`.
Callers will be updated in subsequent tasks.

- [ ] **Step 3: Verify compilation**

```bash
cargo check -p zksync_os_runner
```

Expected: compiles (callers not yet updated, but the crate itself should build).

- [ ] **Step 4: Commit**

```bash
git add zksync_os_runner/
git commit -m "feat(zksync_os_runner): rewrite as facade over airbender-host TranspilerRunner"
```

---

### Task 7: Update `cycle_marker` to use platform types

**Files:**
- Modify: `/root/zksync-os/cycle_marker/Cargo.toml`
- Modify: `/root/zksync-os/cycle_marker/src/lib.rs`

- [ ] **Step 1: Update `Cargo.toml`**

Replace `riscv_transpiler` with `airbender-host`:

```toml
[dependencies]
airbender-host = { workspace = true, optional = true }

[features]
use_riscv_transpiler = ["airbender-host"]
```

- [ ] **Step 2: Update re-exports and `print_cycle_markers`**

In `lib.rs`, replace:
```rust
#[cfg(feature = "use_riscv_transpiler")]
pub use riscv_transpiler::cycle::{CycleMarker, CycleMarkerHooks, Mark};
```

With:
```rust
#[cfg(feature = "use_riscv_transpiler")]
pub use airbender_host::cycle_marker::{CycleMarker, Mark};
```

Remove `CycleMarkerHooks` re-export — the platform handles hook setup internally.

Update `print_cycle_markers` signature: it currently takes `CycleMarker` from
`riscv_transpiler::cycle`. The platform's `airbender_host::cycle_marker::CycleMarker`
has the same shape (`markers: Vec<Mark>`, `delegation_counter: HashMap<u32, u64>`).
Verify field names match and adjust if needed.

- [ ] **Step 3: Update guest-side marker emission**

In `cycle_marker/src/lib.rs`, the `start()` and `end()` functions emit CSR
assembly on RISC-V. Replace with platform's guest API:

```rust
pub fn start(_label: &'static str) {
    #[cfg(target_arch = "riscv32")]
    {
        airbender_guest::cycle::marker();
    }
    #[cfg(not(target_arch = "riscv32"))]
    LABELS.with_borrow_mut(|v| v.push(Label::Start(_label)))
}

pub fn end(_label: &'static str) {
    #[cfg(target_arch = "riscv32")]
    {
        airbender_guest::cycle::marker();
    }
    #[cfg(not(target_arch = "riscv32"))]
    LABELS.with_borrow_mut(|v| v.push(Label::End(_label)))
}
```

Add `airbender-guest` as an optional dependency (for RISC-V target only).

- [ ] **Step 4: Verify compilation**

```bash
cargo check -p cycle_marker
cargo check -p cycle_marker --features use_riscv_transpiler
```

- [ ] **Step 5: Commit**

```bash
git add cycle_marker/
git commit -m "feat(cycle_marker): switch to airbender-platform cycle marker types"
```

---

### Task 8: Update test rig to two-pass flow and new runner API

**Files:**
- Modify: `/root/zksync-os/tests/rig/Cargo.toml`
- Modify: `/root/zksync-os/tests/rig/src/chain.rs`

This is the most complex task. The test rig must switch from passing a
`NonDeterminismCSRSource` to the runner, to passing `&[u32]` input words.

- [ ] **Step 1: Update `Cargo.toml`**

Remove `riscv_transpiler` dependency (check all features that reference it).
Keep `zksync_os_runner` dependency.

For `e2e_proving` feature: remove `execution_utils` and `prover_examples` deps.
The prover will be invoked through `zksync_os_runner` or `airbender-host` directly.

Add `airbender-host` dependency for the `e2e_proving` feature if needed.

- [ ] **Step 2: Refactor `execute_block` RISC-V simulation**

In `chain.rs`, find the block starting at ~line 969 (`if do_riscv_run {`).

Current flow:
```rust
let copy_source = ReadWitnessSource::new(oracle);
let items = copy_source.get_read_items();
// ... pass copy_source to runner
let proof_input = items.borrow().iter().copied().collect::<Vec<u32>>();
assert_eq!(prover_input_forward, proof_input);
```

New flow — use `prover_input_forward` from the prover-input run (line 876):
```rust
let proof_output = if do_riscv_run {
    let dist_dir = get_zksync_os_dist_dir(&app);
    let now = std::time::Instant::now();

    let (proof_output, block_effective) = if flamegraph_output.is_some() {
        let sym_path = get_zksync_os_sym_path(&app);
        zksync_os_runner::run_default_with_flamegraph_path(
            dist_dir,
            sym_path,
            1 << 36,
            &prover_input_forward,
            flamegraph_output,
        )
    } else {
        zksync_os_runner::run_and_get_effective_cycles(
            dist_dir,
            1 << 36,
            &prover_input_forward,
        )
    };

    // CSR dump uses prover_input_forward directly
    if let Ok(output_csr) = std::env::var("CSR_READS_DUMP") {
        let mut file = File::create(&output_csr).expect("should create file");
        for num in prover_input_forward.iter() {
            write!(file, "{num:08X}").expect("Failed to write to file");
        }
    }

    // ... rest of output checking
};
```

Add a helper:
```rust
fn get_zksync_os_dist_dir(app_name: &Option<String>) -> PathBuf {
    let app = app_name.as_deref().unwrap_or("for_tests");
    std::env::var("OVERRIDE_ZKSYNC_OS_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(std::env::var("CARGO_WORKSPACE_DIR").unwrap())
                .join("zksync_os")
                .join("dist")
                .join(app)
        })
}
```

Remove the redundant `assert_eq!(prover_input_forward, proof_input)` — they're
the same data now.

- [ ] **Step 3: Update `run_prover` for `e2e_proving`**

Replace the manual `execution_utils` wiring with platform's `CpuProver` via
`zksync_os_runner` facade or directly via `airbender-host`:

```rust
#[cfg(feature = "e2e_proving")]
fn run_prover(input_words: &[u32]) {
    use airbender_host::program::Program;
    use airbender_host::prover::Prover;

    let dist_dir = get_zksync_os_dist_dir(&None);
    let program = Program::load(&dist_dir).expect("failed to load program");
    let prover = program.cpu_prover()
        .with_cycles(1 << 24)
        .build()
        .expect("failed to build prover");

    let result = prover.prove(input_words)
        .expect("proving failed");

    println!("Proving complete: {} cycles", result.cycles);
}
```

- [ ] **Step 4: Update the multiblock execute_block if it exists**

Check if there's a separate multiblock execution path in `chain.rs` that also
uses the old runner API. Search for other `zksync_os_runner::run` calls and
update them similarly.

- [ ] **Step 5: Build and run tests**

```bash
cd zksync_os && ./dump_bin.sh --type for-tests
cd .. && cargo test -p unit --features rig/no_print -- --test-threads=1 2>&1 | tail -30
```

Expected: tests pass with new two-pass flow.

- [ ] **Step 6: Commit**

```bash
git add tests/rig/
git commit -m "feat(rig): switch to two-pass flow with pre-recorded input words"
```

---

### Task 9: Update `tests/instances/multiblock_batch` imports

**Files:**
- Modify: `/root/zksync-os/tests/instances/multiblock_batch/Cargo.toml`
- Modify: `/root/zksync-os/tests/instances/multiblock_batch/src/lib.rs`

- [ ] **Step 1: Check current imports**

Read the file and find all `riscv_transpiler` and `zksync_os_runner` usage.
The crate uses `QuasiUARTSource` from `riscv_transpiler` and calls `zksync_os_runner`
with the old API.

- [ ] **Step 2: Update imports and API calls**

Replace `riscv_transpiler::abstractions::non_determinism::QuasiUARTSource` usage.
Since the multiblock test likely creates a `QuasiUARTSource` from recorded words
and passes it to the runner, update to pass `&[u32]` directly to the new runner API.

- [ ] **Step 3: Update Cargo.toml**

Remove `riscv_transpiler` dep. Keep `zksync_os_runner`.

- [ ] **Step 4: Verify compilation**

```bash
cargo check -p multiblock_batch
```

- [ ] **Step 5: Commit**

```bash
git add tests/instances/multiblock_batch/
git commit -m "fix(multiblock_batch): update to new zksync_os_runner API"
```

---

### Task 10: Remove old workspace dependencies

**Files:**
- Modify: `/root/zksync-os/Cargo.toml`

- [ ] **Step 1: Remove workspace deps that are no longer needed**

Remove from `[workspace.dependencies]`:
- `execution_utils` — replaced by platform's `CpuProver`
- `prover_examples` — replaced by platform prover APIs

`riscv_transpiler` **must stay** — `oracle_provider` and `callable_oracles` still
depend on it for the `RamPeek` trait. It will be removed when the platform
exposes `RamPeek` as a low-level API.

Verify with: `grep -r "execution_utils\|prover_examples" --include="Cargo.toml" | grep -v "target/" | grep -v "fuzzer/"`

- [ ] **Step 2: Verify full workspace build**

```bash
cargo check --workspace
```

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "chore: remove unused riscv_transpiler/execution_utils/prover_examples workspace deps"
```

---

### Task 11: Update `crypto` dev-dependency

**Files:**
- Modify: `/root/zksync-os/crypto/Cargo.toml`

- [ ] **Step 1: Check `riscv_transpiler` usage in crypto**

```bash
grep -n "riscv_transpiler" crypto/Cargo.toml crypto/src/**/*.rs
```

If it's only a dev-dependency used in tests, update the import path or remove
if tests can use `oracle_provider::RamPeek` instead.

- [ ] **Step 2: Update and verify**

```bash
cargo test -p crypto 2>&1 | tail -10
```

- [ ] **Step 3: Commit**

```bash
git add crypto/
git commit -m "fix(crypto): update riscv_transpiler dev-dependency"
```

---

### Task 12: Update fuzzer wrappers

**Files:**
- Modify: `/root/zksync-os/tests/fuzzer/fuzz/wrappers/callable_oracles_forward/Cargo.toml`
- Modify: `/root/zksync-os/tests/fuzzer/fuzz/wrappers/crypto_proving/Cargo.toml`
- Modify: `/root/zksync-os/tests/fuzzer/fuzz/wrappers/crypto_forward/Cargo.toml`
- Modify: `/root/zksync-os/tests/fuzzer/fuzz/wrappers/oracle_provider_forward/Cargo.toml`

- [ ] **Step 1: Update `riscv_transpiler` references**

These are excluded from the workspace. Replace `riscv_transpiler` git deps
with either `airbender-host` or keep `riscv_transpiler` if only used for `RamPeek`.

- [ ] **Step 2: Verify each builds**

```bash
cargo check --manifest-path tests/fuzzer/fuzz/wrappers/callable_oracles_forward/Cargo.toml
# ... etc
```

- [ ] **Step 3: Commit**

```bash
git add tests/fuzzer/
git commit -m "fix(fuzzer): update airbender dependency references in fuzzer wrappers"
```

---

### Task 13: Update CI workflows

**Files:**
- Modify: `/root/zksync-os/.github/workflows/ci.yml`
- Modify: `/root/zksync-os/.github/workflows/bench.yml`
- Modify: `/root/zksync-os/.github/workflows/evm_terster_proof_run.yml`
- Modify: `/root/zksync-os/.github/workflows/fuzz.yml`
- Modify: `/root/zksync-os/.github/workflows/release-binaries.yml`
- Modify: `/root/zksync-os/zksync_os/reproduce/Dockerfile` (if exists)

- [ ] **Step 1: Add `cargo-airbender` install step to all workflows that build RISC-V**

Add before the RISC-V compile step:
```yaml
- name: Install cargo-airbender
  run: cargo install cargo-airbender --locked --git https://github.com/matter-labs/airbender-platform
```

Or if a release binary exists:
```yaml
- name: Install cargo-airbender
  run: cargo install cargo-airbender --locked
```

- [ ] **Step 2: Update target triple in CI**

Any step that runs `rustup target add riscv32i-unknown-none-elf` should change to
`riscv32im-risc0-zkvm-elf`. Note: this may be a custom target that doesn't need
`rustup target add` — `cargo airbender build` with `build-std` handles it.
Remove the `rustup target add` line if not needed.

- [ ] **Step 3: Update Dockerfile for reproducible builds**

If `/root/zksync-os/zksync_os/reproduce/Dockerfile` exists, update it to install
`cargo-airbender` and remove references to `riscv32i-unknown-none-elf`.

- [ ] **Step 4: Commit**

```bash
git add .github/ zksync_os/reproduce/
git commit -m "ci: add cargo-airbender install, update target triple"
```

---

### Task 14: Full integration verification

- [ ] **Step 1: Build all binaries**

```bash
cd zksync_os
./dump_bin.sh --type for-tests
./dump_bin.sh --type evm-replay-benchmarking
```

- [ ] **Step 2: Run workspace tests**

```bash
cargo test --workspace --features rig/no_print 2>&1 | tail -30
```

- [ ] **Step 3: Run RISC-V simulation tests**

```bash
ZKSYNC_RISC_V_RUN=true cargo test -p unit --features rig/no_print -- --test-threads=1 2>&1 | tail -30
```

- [ ] **Step 4: Run clippy**

```bash
cargo clippy --all -- -D warnings
```

- [ ] **Step 5: Fix any remaining issues and commit**

Stage only the specific files that changed (do NOT use `git add -A`):
```bash
git add <changed-files>
git commit -m "fix: resolve integration issues from platform migration"
```

---

## Task Dependency Graph

```
Task 1 (workspace deps)
  ↓
Task 2 (init_allocator refactor)
  ↓
Task 3 (guest binary migration)
  ↓
Task 4 (dump_bin.sh)
  ↓
Task 5 (path resolution)
  ↓
Task 6 (zksync_os_runner facade)  ←  Task 7 (cycle_marker) [parallel]
  ↓
Task 8 (test rig two-pass flow)
  ↓
Task 9 (multiblock_batch)
  ↓
Task 10 (remove old deps)  ←  Task 11 (crypto)  ←  Task 12 (fuzzer) [parallel]
  ↓
Task 13 (CI workflows)
  ↓
Task 14 (integration verification)
```

Tasks 6 and 7 can be done in parallel. Tasks 10, 11, 12 can be done in parallel.
