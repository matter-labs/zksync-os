# Benchmarking Guide

## Overview

ZKsync OS performance is measured in **effective RISC-V cycles**, not wall-clock time. The proving cost is directly proportional to cycle count. A reduction in effective cycles = cheaper proving.

```
effective_cycles = raw_risc_v_cycles
                 + 16 × blake_delegations
                 + 4  × bigint_delegations
```

The repository currently uses two closely related metrics:
- `cycle_marker::print_cycle_markers()` and `zksync_os_runner::run_and_get_effective_cycles()` use the formula above.
- `bench_scripts/compare_bench.py` derives its `Eff` column from `.bench` files using the same Blake/BigInt weights, and also adds `+1` for every other delegation type recorded in the marker output.

## Quick Start

Use `bench_scripts/bench.sh` to run benchmarks. All subcommands that run benchmarks automatically rebuild the RISC-V binary first.

**Save a baseline** (do this once on the base branch):
```bash
bench_scripts/bench.sh baseline
```

**Quick check after making changes** (runs 1 block, compares against baseline):
```bash
bench_scripts/bench.sh quick
```

**Full benchmark run** (all blocks + precompiles):
```bash
bench_scripts/bench.sh run
```

**Compare full results against baseline:**
```bash
bench_scripts/bench.sh compare
```

**Generate a flamegraph** (identifies where RISC-V cycles are spent — use to find optimization targets). Produces both an SVG and a text summary (`.txt`) with self-cost and call stacks, suitable for automated analysis:
```bash
bench_scripts/bench.sh flamegraph              # default: bench_results/flamegraph.svg + .txt
bench_scripts/bench.sh flamegraph output.svg   # custom path (text summary at output.txt)
```

Results are saved to `bench_results/` (gitignored). Negative % in effective cycles = improvement.

## Prerequisites

One-time setup (in addition to the standard Rust toolchain):

```bash
rustup target add riscv32i-unknown-none-elf
cargo install cargo-binutils
rustup component add llvm-tools-preview rust-src
pip3 install matplotlib   # only needed for opcode frequency charts
```

## Interpreting Results

The comparison output is a markdown table with columns:
- **Base/Head Eff** — effective cycles (primary metric). Negative % = improvement.
- **Base/Head Raw** — raw RISC-V cycles excluding delegations.
- **Base/Head Blake** — number of Blake2 delegation calls.
- **Base/Head Bigint** — number of BigInt delegation calls.

`Base/Head Eff` is the comparison-script metric described above. Focus on that column when comparing two `.bench` files, and keep in mind it is slightly broader than the simulator-returned block effective value.

## How It Works

### Cycle Marker Framework

The `cycle_marker` crate provides macros to instrument code:

```rust
cycle_marker::wrap!("my_label", { /* measured code */ });
// or
cycle_marker::start!("my_label");
// ... code ...
cycle_marker::end!("my_label");
```

On RISC-V, these write to CSR `0x7ff`, signaling the simulator to record cycle counts. On the host (forward mode), labels are collected in thread-local storage for later pairing with simulator data.

The block-wide marker is `"run_prepared"` — this is what produces the overall effective cycle count.

### Feature Flags

| Feature | Scope | Effect |
|---------|-------|--------|
| `cycle_marker` | Multiple crates | Activates cycle measurement markers |
| `unlimited_native` | `basic_bootloader`, `forward_system` | Disables native resource limits so benchmarks don't hit gas ceilings |
| `benchmarking` | `proof_running_system`, `zksync_os` | Convenience: enables both `cycle_marker` + `unlimited_native` |
| `rig/no_print` | Test rig | Suppresses verbose execution logs |

### Benchmark Data Flow

1. Build RISC-V binary with `benchmarking` feature enabled
2. Run block replay through RISC-V simulator (`zksync_os_runner`)
3. Simulator records cycle counts at each CSR marker
4. `cycle_marker::print_cycle_markers()` computes effective cycles for the block-wide marker using Blake/BigInt delegation weights
5. Results written to file at `MARKER_PATH` (default: `markers.bench`)
6. Python scripts parse and compare `.bench` files, adding `+1` for other delegation types in the comparison report

### Output File Format

The `.bench` files produced by `cycle_marker` contain sections like:

```
=== Cycle markers:
run_prepared: net cycles: 12345678, net delegations: {1991: 100, 1994: 200}
some_inner_label: net cycles: 456789, net delegations: {1991: 50}
Total delegations: {1991: 100, 1994: 200}
==================
```

Delegation IDs: `1991` = Blake2, `1994` = BigInt.

## Manual Commands

The `bench_scripts/bench.sh` script wraps these commands. Use them directly only if you need finer control.

### Build the RISC-V Benchmarking Binary

```bash
cd zksync_os && ./dump_bin.sh --type evm-replay-benchmarking
```

### Run a Single Block

```bash
ZKSYNC_RISC_V_RUN=true \
MARKER_PATH=$(pwd)/result.bench \
cargo run --manifest-path tests/instances/eth_runner/Cargo.toml \
  --release -j 3 \
  --features rig/no_print,rig/cycle_marker,rig/unlimited_native \
  -- single-run --block-dir tests/instances/eth_runner/blocks/19299001 \
  --opcode-stats \
  > result.out
```

Omit `--opcode-stats` when only block-level cycle benchmarks are needed — it adds per-opcode tracing overhead.

Available blocks: `19299001`, `22244135`, `23292836` (in `tests/instances/eth_runner/blocks/`).

### Run Precompile Benchmarks

```bash
ZKSYNC_RISC_V_RUN=true \
MARKER_PATH=$(pwd)/precompiles.bench \
cargo test --release -j 3 \
  --features rig/no_print,precompiles/cycle_marker,rig/unlimited_native \
  -p precompiles -- test_precompiles
```

### Compare Results

```bash
python3 bench_scripts/compare_bench.py \
  '[("block_19299001", "base.bench", "head.bench", "process_block")]'
```

### Generate Flamegraph

```bash
ZKSYNC_RISC_V_RUN=true \
cargo run --manifest-path tests/instances/eth_runner/Cargo.toml \
  --release -j 3 \
  --features rig/no_print,rig/cycle_marker,rig/unlimited_native \
  -- single-run --block-dir tests/instances/eth_runner/blocks/19299001 \
  --flamegraph block_19299001.svg

# Convert SVG to text summary (self-cost + call stacks)
python3 bench_scripts/parse_flamegraph.py block_19299001.svg block_19299001.txt
```

### Parse Opcode Statistics

```bash
python3 bench_scripts/parse_opcodes.py result.out opcodes.csv opcodes.png
```

### Per-Opcode Benchmarking

The benchmark flow collects per-opcode gas, native resource, and RISC-V cycle stats. The forward-mode run uses `EvmOpcodeStatsTracer` (enabled via `--opcode-stats`) to record gas/native per opcode execution (with min/max/median). The RISC-V run records per-opcode cycles via `cycle_marker` opcode markers.

**Quick run with all data:**
```bash
OPCODE_SAMPLES_DIR=$(pwd)/samples \
OPCODE_CYCLE_SAMPLES_DIR=$(pwd)/cycle_samples \
OPCODE_STATS_PATH=$(pwd)/opcode_stats.csv \
bash bench_scripts/bench.sh quick
```

The `.out` file contains the per-opcode stats table (gas/native with min/max/median). The `.bench` file contains per-opcode cycle stats. Setting the env vars also dumps per-execution sample files for detailed analysis.

**Join per-execution samples to get actual cycles/gas ratios:**
```bash
python3 bench_scripts/join_samples.py samples/ cycle_samples/ --summary --out-dir joined/
```

Produces per-execution `(gas, native, cycles, cycles/gas, native/gas)` CSVs per opcode and a summary table with p50/p95/p99/max ratios.

**Visualize:**
```bash
python3 bench_scripts/visualize_opcode_stats.py joined/ --out-dir charts/
```

Produces: total cycle consumption bar chart, sorted cycles/gas ratio curves per opcode, and per-opcode detail plots with percentile annotations.

**Compare per-opcode stats between base and head:**
```bash
python3 bench_scripts/compare_opcode_stats.py base_block.out head_block.out "label"
```

Outputs a compact diff table only when gas/native stats change. Used by CI to add opcode stats diffs to PR comments.

## CI Integration

The CI workflow (`.github/workflows/bench.yml`) runs the full comparison automatically on every PR. It:
1. Checks out the merge-base of the PR
2. Builds RISC-V binaries and runs all block + precompile benchmarks
3. Checks out the PR branch and repeats
4. Posts a comparison table as a PR comment (cycle benchmarks + per-opcode stats diff if changed)

## Important Notes

- Always rebuild the RISC-V binary (`dump_bin.sh`) after code changes — the binary is a static artifact. The `bench_scripts/bench.sh` script does this automatically.
- `unlimited_native` must be enabled for benchmarking to prevent transactions from hitting native gas limits mid-block.
- The `for-tests` binary type is for functional tests; `evm-replay-benchmarking` is for performance measurement.
- Results are deterministic for the same binary + input. No need for multiple runs or statistical averaging.

## Key Files

| Path | Description |
|------|-------------|
| `bench_scripts/bench.sh` | Convenience script for running benchmarks |
| `cycle_marker/src/lib.rs` | Cycle marker macros and effective cycle calculation |
| `zksync_os/dump_bin.sh` | RISC-V binary build script with type selection |
| `zksync_os_runner/src/lib.rs` | RISC-V simulator runner, returns effective cycles |
| `tests/instances/eth_runner/` | Real Ethereum block replay binary |
| `tests/instances/eth_runner/blocks/` | Benchmark block fixtures |
| `tests/instances/precompiles/` | Precompile benchmark tests |
| `bench_scripts/compare_bench.py` | Compares base vs head `.bench` files |
| `bench_scripts/parse_flamegraph.py` | Converts flamegraph SVG to text summary with self-cost and call stacks |
| `bench_scripts/parse_opcodes.py` | Parses opcode frequency from simulator output |
| `bench_scripts/compare_opcode_stats.py` | Compares per-opcode gas/native stats between base and head |
| `bench_scripts/join_samples.py` | Joins per-execution tracer + cycle samples, computes ratios |
| `bench_scripts/visualize_opcode_stats.py` | Generates charts from joined per-execution data |
| `forward_system/src/system/tracers/evm_opcode_stats.rs` | Per-opcode gas/native stats tracer |
| `.github/workflows/bench.yml` | CI benchmarking pipeline |
