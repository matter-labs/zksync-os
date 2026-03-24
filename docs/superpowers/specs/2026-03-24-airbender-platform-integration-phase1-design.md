# Phase 1: Airbender Platform Integration

## Context

ZKsync OS currently integrates with airbender v2 at the crate level — directly
depending on `riscv_transpiler`, `execution_utils`, and `prover_examples`.
Airbender-platform provides a higher-level SDK wrapping these crates into a
standardized build, execution, and proving pipeline.

This spec covers **Phase 1**: adopting the platform's build tooling, runtime, and
host APIs while keeping the current wire format (UsizeSerializable) unchanged.

Phase 2 (out of scope here) will replace the wire format with
`AirbenderCodecV0` (serde + bincode v2) and restructure the oracle query
dispatch pattern.

## Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Guest entry point | `#[airbender::main]` | All custom boot code is identical to `airbender-rt`; eliminates ~200 lines |
| Target triple | `riscv32im-risc0-zkvm-elf` | Match platform conventions |
| Build tool | `cargo airbender build` via `dump_bin.sh` wrapper | Keeps CI scripts unchanged |
| Runner | Platform `TranspilerRunner` behind `zksync_os_runner` facade | Minimizes caller churn |
| Prover | Platform `CpuProver` | Replaces manual `execution_utils` wiring |
| Wire format | Keep `UsizeSerializable` (Phase 1) | Decouple from serialization rewrite |
| Forward mode | Stays native with interactive oracles | Forward run records `Vec<u32>` via `ReadWitnessSource`; proving replays it |
| Witness tracing | Dropped | Non-essential benchmarking utility |

## 1. Guest Binary Migration (`zksync_os`)

### 1.1 Entry Point

Replace the manual boot sequence with `#[airbender::main]`:

```rust
#![no_std]
#![no_main]
#![feature(allocator_api)]

use proof_running_system::system::bootloader::run_proving;

#[airbender::main(allocator_init = proof_running_system::system::bootloader::init_allocator_safe)]
fn main() -> [u32; 8] {
    run_proving::<CSRBasedNonDeterminismSource, LoggerTy>()
}

mod csr {
    // CSRBasedNonDeterminismSource — unchanged from current impl
}
```

Note: `#[airbender::main]` generates a call to `airbender::guest::commit(output)`,
which writes output to registers x10-x17 and exits. This replaces the current
manual `zksync_os_finish_success(&output)` call. The Phase 2 scope item
"Adopt `airbender::guest::commit()`" is therefore addressed incidentally by Phase 1.

### 1.2 What Gets Removed

- `asm_reduced.S` — provided by `riscv_common` via `airbender-rt`
- `memset.s`, `memcpy.s` — provided by `riscv_common`
- Manual `load_to_ram` / ROM-to-RAM section loading — `riscv_common::boot_sequence::init()`
- `NullAllocator`, `OptionalGlobalAllocator`, `#[global_allocator]` — `airbender-rt` talc allocator
- `_start_rust`, `_setup_interrupts`, `_machine_start_trap_rust` — `airbender-rt`
- `eh_personality`, panic handler — `riscv_common`
- `trap_frame.rs`, `helper_reg_utils.rs` — dead code / provided by runtime
- `heapless` dependency — verify if still needed; remove if not

### 1.3 `run_proving` Refactor

Currently `run_proving()` takes `heap_start, heap_end` and calls `init_allocator()`
internally. Since `airbender-rt` handles allocator init before `main()`, two changes
are needed:

1. Remove the internal `init_allocator()` call from `run_proving` to avoid
   double-initialization.
2. Change `run_proving` signature to drop `heap_start, heap_end` parameters.
   If heap boundaries are still needed internally, use
   `riscv_common::boot_sequence::heap_start()/heap_end()`.

### 1.4 `init_allocator` Signature

The `#[airbender::main]` macro calls `airbender::rt::start_with_allocator_init(init_fn, ...)`
which expects `fn(*mut usize, *mut usize)`. The current `init_allocator` is
`unsafe fn`. A safe wrapper must be provided:

```rust
pub fn init_allocator_safe(heap_start: *mut usize, heap_end: *mut usize) {
    unsafe { init_allocator(heap_start, heap_end) }
}
```

### 1.5 Dependencies

```toml
[dependencies]
airbender = { package = "airbender-sdk", path = "..." }
proof_running_system = { path = "../proof_running_system", default-features = false }
crypto = { path = "../crypto", optional = true }
# riscv_common removed — comes transitively via airbender-rt
# heapless removed — verify first
```

### 1.6 Cargo Config

`cargo airbender build` handles the target triple and most rustflags internally.
The `.cargo/config.toml` in `zksync_os/` should match the platform's guest
convention (see `examples/fibonacci/guest/.cargo/config.toml`):

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

Note: the `build-std` list should NOT include `std` or `proc_macro` for a no_std
guest binary. Verify against the platform's conventions during implementation —
if `cargo airbender build` overrides `build-std` internally, this config may be
unnecessary.

### 1.7 QuasiUART / Logging

The `print_debug_info` feature currently uses a custom `QuasiUART`. Options:
- Switch to `airbender-rt`'s UART (functionally identical)
- Keep custom impl temporarily if there are feature-flag complications

Recommendation: switch to `airbender-rt`'s UART since the implementations are
identical.

## 2. Build System Migration

### 2.1 `dump_bin.sh` Becomes a Wrapper

`dump_bin.sh` keeps its `--type` interface but internally calls
`cargo airbender build`:

```bash
case "$TYPE" in
  for-tests)
    cargo airbender build \
      --app-name for_tests \
      --profile release \
      -- --features "proving,for_tests"
    ;;
  for-tests-benchmarking)
    cargo airbender build \
      --app-name for_tests \
      --profile release \
      -- --features "proving,for_tests,benchmarking"
    ;;
  # ... etc for each type
esac
```

Each type maps to the same feature set as today. Artifact output lands in
`dist/<app-name>/app.{bin,text,elf}` plus `manifest.toml`.

`cargo airbender build` produces the manifest with SHA-256 checksums and a
codec version stamp ("v0"). In Phase 1, the guest does not actually use
`AirbenderCodecV0` — the codec field in the manifest is semantically meaningless
but will be enforced by `Program::load()` on the host side. This is acceptable;
Phase 2 will make the codec usage real.

### 2.2 Artifact Path Resolution

`tests/rig/src/chain.rs:get_zksync_os_path()` updates to resolve from `dist/`:

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

### 2.3 CI Prerequisites

All CI workflows that build the RISC-V binary add:

```yaml
- name: Install cargo-airbender
  run: cargo install cargo-airbender --locked
```

Or use a pre-built binary for faster CI. The `dump_bin.sh` calls remain unchanged
in CI scripts.

### 2.4 Reproducible Build Pipeline

`release-binaries.yml` uses `zksync_os/reproduce/Dockerfile` which calls
`dump_bin.sh`. The Dockerfile must be updated to:
- Install `cargo-airbender`
- Use the `riscv32im-risc0-zkvm-elf` target (replacing `riscv32i-unknown-none-elf`)
- `dump_bin.sh` still works since it wraps `cargo airbender build`

This is security-sensitive — verify the reproducible build hash is deterministic
with the new toolchain.

## 3. Replace `zksync_os_runner`

### 3.1 New API Surface

The crate becomes a thin facade over `airbender-host`:

```rust
/// Run the RISC-V binary with pre-recorded input words.
pub fn run(
    dist_dir: PathBuf,
    cycles: usize,
    input_words: &[u32],
) -> [u32; 8]

/// Run and return effective cycles (with cycle markers if enabled).
pub fn run_and_get_effective_cycles(
    dist_dir: PathBuf,
    cycles: usize,
    input_words: &[u32],
) -> ([u32; 8], Option<u64>)

/// Run with optional flamegraph profiling.
pub fn run_with_flamegraph(
    dist_dir: PathBuf,
    cycles: usize,
    input_words: &[u32],
    flamegraph_config: Option<FlamegraphConfig>,
) -> ([u32; 8], Option<u64>)
```

Key change: `impl NonDeterminismCSRSource` parameter is replaced by `&[u32]`
(pre-recorded input words). Internally, the facade:

1. Loads `Program::load(dist_dir)` — validates manifest, checksums, codec version
2. Builds `TranspilerRunner` via `program.transpiler_runner().with_cycles(cycles).build()`
3. Calls `runner.run(input_words)` — internally creates `QuasiUARTSource`
4. Extracts output from `ExecutionResult::receipt.output`

### 3.2 Cycle Markers

Uses the platform's cycle marker API (PR #32, merged). The facade integrates
cycle marker collection via the platform's `TranspilerRunner` cycle marker
support and feeds results to `cycle_marker::print_cycle_markers()`.

### 3.3 Dependencies

```toml
[dependencies]
airbender-host = { path = "..." }
cycle_marker = { path = "../cycle_marker", optional = true }

# riscv_transpiler, common_constants — removed
```

### 3.4 `simulate_witness_tracing`

Dropped. Was a benchmarking utility using `SimpleSnapshotter` for witness
generation throughput measurement. Not needed for correctness. Can be re-added
as a platform feature later.

## 4. Test Rig Changes (`tests/rig`)

### 4.1 Two-Pass Execution Flow

The current test rig runs the RISC-V simulation by wrapping a **fresh** oracle
in `ReadWitnessSource` and passing it to the runner as a `NonDeterminismCSRSource`.
The runner executes interactively, recording reads as a side effect.

With the platform model, the runner only accepts `&[u32]`. The test rig must
adopt a **two-pass** approach:

**Pass 1 — Prover-input forward run (already exists):** At `chain.rs:876`, the
test rig already runs a "prover-input" forward execution wrapping the oracle in
`ReadWitnessSource`. This produces `prover_input_forward: Vec<u32>` — the
complete sequence of oracle read values.

**Pass 2 — RISC-V transpiler run (changed):** Instead of wrapping a new oracle
in `ReadWitnessSource` and passing it to `zksync_os_runner::run()`, pass the
`prover_input_forward` words from pass 1:

```rust
let (proof_output, block_effective) = zksync_os_runner::run_and_get_effective_cycles(
    get_zksync_os_dist_dir(&app),
    1 << 36,
    &prover_input_forward,  // pre-recorded words from pass 1
);
```

This is correct because the existing code at `chain.rs:1063` already asserts
`prover_input_forward == proof_input` (where `proof_input` is the recording from
the RISC-V run). They produce identical sequences by construction — both execute
the same deterministic bootloader code with the same oracle data.

**Simplification:** The second `ReadWitnessSource` wrapping (line 971) and the
equivalence assertion (line 1063) can be removed since they're redundant. The
CSR dump feature (`CSR_READS_DUMP` env var) uses the same `prover_input_forward`
data.

### 4.2 Prover Integration

`run_prover()` currently wires up `execution_utils` manually. Replaced by:

```rust
fn run_prover(input_words: &[u32]) {
    // Uses zksync_os_runner facade which wraps CpuProver
    zksync_os_runner::prove(dist_dir, input_words);
}
```

Or uses `airbender-host`'s `CpuProver` directly.

### 4.3 Dependencies

```toml
# Remove:
riscv_transpiler = ...
execution_utils = ...
prover_examples = ...

# Add:
airbender-host = { path = "..." }
```

## 5. Cycle Marker Migration

### 5.1 Guest Side

Replace direct CSR assembly with platform's guest API:

```rust
// Before:
core::arch::asm!("csrrw x0, 0x7ff, x0")

// After:
airbender::guest::cycle::mark()
```

Note: verify the exact API path from PR #32. If the guest API is at a different
path, adjust accordingly.

### 5.2 Host Side

`cycle_marker` crate re-exports switch from `riscv_transpiler::cycle::*` to
platform's cycle marker types. `print_cycle_markers()` adapts to consume
the platform's `CycleMarkerResult` type from `airbender-host::cycle_marker`.

### 5.3 `cycle_marker/Cargo.toml`

```toml
[dependencies]
airbender-host = { path = "...", optional = true }
# riscv_transpiler removed

[features]
use_riscv_transpiler = ["airbender-host"]  # rename this feature later
```

## 6. Workspace Dependency Changes

### 6.1 New Dependencies

```toml
[workspace.dependencies]
airbender-sdk = { path = "../airbender-platform/crates/airbender-sdk" }
airbender-host = { path = "../airbender-platform/crates/airbender-host" }
```

### 6.2 Removed Dependencies

```toml
# Removed from workspace:
riscv_transpiler = ...    # replaced by platform abstractions
execution_utils = ...     # replaced by CpuProver
prover_examples = ...     # replaced by platform prover APIs
```

### 6.3 Transitional Dependencies

`oracle_provider` and `callable_oracles` currently depend on `riscv_transpiler`
only for the `RamPeek` trait. These keep the direct dep temporarily until the
platform exposes `RamPeek` as a low-level API, then switch to importing from
`airbender-host`.

## 7. Affected Crates Summary

| Crate | Change Level | Key Changes |
|-------|-------------|-------------|
| `zksync_os` | **Heavy** | `#[airbender::main]`, remove boot code, new target triple |
| `zksync_os_runner` | **Heavy** | Rewrite as facade over `airbender-host` |
| `proof_running_system` | **Medium** | Remove `init_allocator` from `run_proving`, adjust heap access, add safe wrapper |
| `tests/rig` | **Medium** | Two-pass flow, pass `&[u32]` to runner, replace prover wiring |
| `cycle_marker` | **Medium** | Switch to platform cycle marker types |
| `tests/instances/multiblock_batch` | **Light** | Update `riscv_transpiler` imports (`QuasiUARTSource`), update runner calls |
| `oracle_provider` | **Light** | `RamPeek` import path may change |
| `callable_oracles` | **Light** | Same — `RamPeek` import path |
| `crypto` | **Light** | Dev-dependency on `riscv_transpiler` — update import path |
| `forward_system` | **None** | Unchanged in Phase 1 |

### 7.1 Excluded Crates (Out of Workspace)

These crates are excluded from the workspace but need updating to avoid breakage
when building independently:

- `tests/fuzzer/fuzz/wrappers/callable_oracles_forward` — `riscv_transpiler` dep
- `tests/fuzzer/fuzz/wrappers/crypto_proving` — same
- `tests/fuzzer/fuzz/wrappers/crypto_forward` — same
- `tests/fuzzer/fuzz/wrappers/oracle_provider_forward` — same

These should be updated alongside the main migration but are lower priority
since they're excluded from `cargo test --workspace`.

## 8. CI Workflow Updates

All workflows calling `dump_bin.sh`:
- `ci.yml` — add `cargo install cargo-airbender`
- `bench.yml` — same
- `evm_tester_proof_run.yml` — same
- `fuzz.yml` — same
- `release-binaries.yml` — update `Dockerfile` (see section 2.4)

Binary path references in CI scripts remain unchanged since `dump_bin.sh`
interface is preserved.

## 9. Out of Scope (Phase 2)

- Replace `UsizeSerializable`/`UsizeDeserializable` with serde + `AirbenderCodecV0`
- Replace `CsrBasedIOOracle` with `airbender::guest::read::<T>()`
- Remove query dispatch pattern (`OracleQueryProcessor`, query IDs)
- Restructure `forward_system` oracle architecture

Note: `airbender::guest::commit()` for output is adopted in Phase 1 incidentally
via `#[airbender::main]` macro expansion.

## 10. Success Criteria

- `cargo airbender build --project zksync_os` produces working binaries
- All workspace tests pass: `cargo test --workspace`
- RISC-V simulation tests pass: `ZKSYNC_RISC_V_RUN=true cargo test -p <instance>`
- `e2e_proving` passes with platform's `CpuProver`
- Benchmark CI passes with cycle markers via platform API
- Reproducible build pipeline produces deterministic hashes
- No regressions in existing test coverage
