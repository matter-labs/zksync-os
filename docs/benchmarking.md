# Benchmarking Reference

## Metric

Proving cost is proportional to **effective RISC-V cycles**:

```
effective_cycles = raw_risc_v_cycles
                 + 16 × blake_delegations    (id 1991)
                 + 4  × bigint_delegations   (id 1994)
                 + 4  × keccak_delegations   (id 1995)
```

`cycle_marker::print_cycle_markers()` computes this for the `process_block`
label. `bench_scripts/compare_bench.py`'s `Eff` column uses the same
weights and additionally adds `+1` per delegation of any other id. The
formula is the source of truth — when in doubt re-read
`cycle_marker/src/lib.rs::print_cycle_markers`.

Results are deterministic for the same RISC-V binary + input — no
averaging needed. Always rebuild the binary after touching any code that
ends up in `zksync_os` (`zksync_os/dump_bin.sh --type <type>`);
`bench_scripts/bench.sh` does this automatically.

## Running

`bench_scripts/bench.sh` wraps the full pipeline. Subcommands:
`baseline`, `quick`, `run`, `compare`, `flamegraph`. Read the script for
exact invocations. `cargo test --features rig/no_print` is preferred over
the underlying `cargo test` invocations when running tests directly.

For local proof-mode simulation set `ZKSYNC_RISC_V_RUN=true`; CI sets it
automatically.

## Data pipeline (env-var opt-ins)

The benchmark produces several artifacts, gated by env vars on the
forward-mode run (block bench or `precompiles` test crate):

| Env var | Producer | File layout |
|---|---|---|
| `MARKER_PATH` | `cycle_marker::print_cycle_markers` | `<path>.bench` — text format: `<label>: net cycles: <n>, net delegations: {id: count}` per marker, plus a global `Total delegations`. |
| `OPCODE_STATS_PATH` | `EvmOpcodeStatsTracer` | CSV: per-opcode gas + native stats with min/median/avg/max. |
| `OPCODE_SAMPLES_DIR` | `EvmOpcodeStatsTracer::dump_samples` | One `<OPCODE>.samples` file per opcode, `gas,native` per line in execution order. |
| `OPCODE_CYCLE_SAMPLES_DIR` | `cycle_marker` | One `<OPCODE>.cycles` file per opcode, raw RISC-V cycles per line in execution order. |
| `PRECOMPILE_STATS_PATH` | `PrecompileStatsTracer` | CSV: `name, address, count, avg_gas, median_gas, min_gas, max_gas, avg_native, median_native, min_native, max_native, native_per_gas`. |
| `PRECOMPILE_SAMPLES_DIR` | `PrecompileStatsTracer::dump_samples` | One `<precompile>.samples` file per precompile, `gas,native` per line. |
| `LABEL_CYCLE_SAMPLES_DIR` | `cycle_marker` | Per non-opcode label: `<label>.cycles` (raw) **and** `<label>.effective.cycles` (raw + delegation weights). |

All sample/cycle dump dirs use **append** mode — clean the dir between
runs (`rm -rf`) to avoid mixing.

### Effective vs raw cycles

Per-execution `<label>.cycles` files store raw RISC-V cycles only and
**undercount delegation-heavy work**. `<label>.effective.cycles` (same
formula as the `process_block` metric) is the correct input for
cycles/gas analysis of any label whose handler delegates (precompiles
like `ecrecover`/`modexp`/`bn254`, the `keccak` system function call,
account/storage-touching paths).

Opcode samples in `OPCODE_CYCLE_SAMPLES_DIR` currently dump raw only;
opcodes whose handlers delegate (`SHA3`, `SLOAD`/`SSTORE`,
`BALANCE`/`EXTCODE*`, `CALL` family, `CREATE`/`CREATE2`) are similarly
undercounted in `join_samples.py` output.

### Ecrecover intrinsic filter

Every L2 transaction invokes `ecrecover` internally for signature
verification. `bench_scripts/join_precompile_samples.py --bench-file`
strips the first `ecrecover` cycle marker per `process_transaction`
boundary and keeps only subsequent (precompile-target) ecrecovers.

Positional heuristic assumption: every tx has **exactly one** intrinsic
ecrecover before any user code. Holds for the current mainnet block
fixtures; does **not** hold for L1→L2 priority ops, EIP-7702 set-code
authority recovery, or `eth_call`. Replace with a dedicated marker label
(`ecrecover_intrinsic`) before adding fixtures that violate the
assumption.

## Comparison scripts

- `compare_bench.py` — base/head `.bench` diff; produces the headline
  effective-cycles table.
- `compare_opcode_stats.py` — diff per-opcode gas/native stats.
- `compare_opcode_cycles.py` — diff per-opcode RISC-V cycles + cycles/gas
  ratios.
- `compare_precompile_stats.py` — diff per-precompile gas/native stats;
  emits a head-only spoiler when base lacks instrumentation.
- `join_samples.py` — per-opcode per-execution join (gas,native,cycles).
- `join_precompile_samples.py` — per-precompile per-execution join;
  prefers `<label>.effective.cycles` and falls back to raw with a stderr
  note + summary header indicating which kind was used.
- `cycles_per_native_report.py` — local-only ad-hoc tool. Given one or
  more `(samples_dir, cycles_dir)` pairs from prior bench runs,
  computes per-execution `cycles / native` ratios per opcode and per
  precompile and writes a Markdown report (median / p95 / max). Useful
  for spotting opcodes or precompiles whose native budget is out of
  step with their cycle cost. Not wired into the CI comment.

## CI

`.github/workflows/bench.yml` runs the full pipeline on each PR:
checkout merge-base → bench-base, checkout head → bench-head, then
`compare` step composes a comparison comment from
`compare_*` and `join_*` script outputs. Script failures are surfaced
via explicit `_… failed; see CI logs._` markers in the PR comment
rather than silently dropping tables.

## Key files

| Path | Description |
|------|-------------|
| `cycle_marker/src/lib.rs` | Cycle marker macros, effective-cycle formula, per-execution dumps |
| `zksync_os/dump_bin.sh` | RISC-V binary build script; `--type` selects feature combo |
| `zksync_os_runner/src/lib.rs` | RISC-V simulator runner |
| `bench_scripts/bench.sh` | Convenience wrapper for end-to-end runs |
| `bench_scripts/compare_bench.py` | base/head `.bench` cycles diff |
| `bench_scripts/compare_opcode_stats.py` | base/head opcode gas/native diff |
| `bench_scripts/compare_opcode_cycles.py` | base/head opcode cycles + cycles/gas diff |
| `bench_scripts/compare_precompile_stats.py` | base/head precompile gas/native diff |
| `bench_scripts/cycles_per_native_report.py` | local-only `cycles/native` per-opcode + per-precompile ratio report (median / p95 / max) |
| `bench_scripts/join_samples.py` | Per-opcode per-execution join |
| `bench_scripts/join_precompile_samples.py` | Per-precompile per-execution join (effective-preferring) |
| `bench_scripts/parse_flamegraph.py` | Flamegraph SVG → text summary |
| `bench_scripts/visualize_opcode_stats.py` | Charts from joined per-execution data |
| `forward_system/src/system/tracers/evm_opcode_stats.rs` | Per-opcode gas/native tracer |
| `forward_system/src/system/tracers/precompile_stats.rs` | Per-precompile gas/native tracer |
| `forward_system/src/system/tracers/pair.rs` | Combinator for running two tracers together |
| `tests/instances/eth_runner/` | Block replay binary; consumes blocks from `blocks/` |
| `tests/instances/precompiles/` | Precompile benchmark test crate |
| `.github/workflows/bench.yml` | CI pipeline |
