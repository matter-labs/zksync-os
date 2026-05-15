#!/usr/bin/env bash
# Run the base- or head-side benchmark passes for the PR bench workflow.
#
# Usage: bench_scripts/run_bench_side.sh <base|head>
#
# Reads/writes from the repo root (run via `cd $REPO_ROOT && ...`). For each
# fixture in `tests/instances/eth_runner/blocks/*`:
#   - Pass 1: default DA scheme (BlobsAndPubdataKeccak256), full
#             instrumentation (opcodes + precompiles + cycle markers).
#   - Pass 2: BlobsZKsyncOS DA scheme — cycle markers only (skipped if the
#             merge-base predates `BENCH_DA_SCHEME` plumbing).
# Then runs the synthetic test-crate precompile workload once.
#
# Output file prefixes are `<side>_` (so the workflow's bench-base and
# bench-head jobs upload disjoint artifact sets).
#
# `PRECOMPILE_STATS_PATH`, `PRECOMPILE_SAMPLES_DIR`, and
# `LABEL_CYCLE_SAMPLES_DIR` are best-effort on the base side: they're
# consumed by `PrecompileStatsTracer` and the label-cycle dump path, both
# introduced on the PR. When the merge-base predates that instrumentation
# the env vars are ignored, no `base_block_${blk}_precompile_stats.csv` is
# produced, and `compare_precompile_stats.py` skips rows for which a base
# CSV is missing — same pattern as the `for-tests-benchmarking-pectra` /
# `BENCH_DA_SCHEME` fallbacks below.

set -euo pipefail

SIDE="${1:?Usage: $0 <base|head>}"
case "$SIDE" in
  base|head) ;;
  *) echo "side must be 'base' or 'head', got '$SIDE'" >&2; exit 2 ;;
esac

# Profile/feature/test-filter selection adapts to the checked-out tree:
#   - `bench-fast` profile: introduced on the PR; merge-base may lack it,
#     in which case we fall back to `--release`.
#   - `precompiles/pectra` feature + extra test functions: introduced
#     together with `for-tests-benchmarking-pectra`; if the proving binary
#     type is unavailable, the proving binary can't run BLS/BLAKE2F/KZG
#     vectors, so we drop those tests + the feature.
if grep -q "bench-fast" Cargo.toml; then
  PROFILE="--profile bench-fast"
else
  PROFILE="--release"
fi
if grep -q "for-tests-benchmarking-pectra" zksync_os/dump_bin.sh; then
  PRECOMPILES_FEATURES="rig/no_print,precompiles/cycle_marker,precompiles/pectra,rig/unlimited_native"
  PRECOMPILES_TESTS="test_precompiles test_pectra_precompiles test_kzg_regression"
else
  PRECOMPILES_FEATURES="rig/no_print,precompiles/cycle_marker,rig/unlimited_native"
  PRECOMPILES_TESTS="test_precompiles"
fi

EVM_FEATURES="rig/no_print,rig/cycle_marker,rig/unlimited_native"

for dir in tests/instances/eth_runner/blocks/*; do
  blk=$(basename "$dir")

  # Pass 1: default DA scheme (BlobsAndPubdataKeccak256) — full
  # instrumentation. Head writes opcode stats CSV as a supplementary
  # artifact; base skips it because nothing downstream consumes a
  # base-side opcode CSV (the per-opcode compare reads stdout `.out`
  # files, not the CSV).
  env_args=(
    OPCODE_SAMPLES_DIR="$(pwd)/opcode_samples/${SIDE}_${blk}"
    OPCODE_CYCLE_SAMPLES_DIR="$(pwd)/opcode_cycles/${SIDE}_${blk}"
    MARKER_PATH="$(pwd)/${SIDE}_block_${blk}.bench"
    PRECOMPILE_STATS_PATH="$(pwd)/${SIDE}_block_${blk}_precompile_stats.csv"
    PRECOMPILE_SAMPLES_DIR="$(pwd)/precompile_samples/${SIDE}_${blk}"
    LABEL_CYCLE_SAMPLES_DIR="$(pwd)/precompile_cycles/${SIDE}_${blk}"
  )
  if [ "$SIDE" = "head" ]; then
    env_args+=(OPCODE_STATS_PATH="$(pwd)/${SIDE}_block_${blk}_opcode_stats.csv")
  fi
  env "${env_args[@]}" \
    cargo run --manifest-path tests/instances/eth_runner/Cargo.toml $PROFILE \
      --features "$EVM_FEATURES" \
      -- single-run --block-dir "$dir" --opcode-stats \
      > "${SIDE}_block_${blk}.out"

  # Pass 2: BlobsZKsyncOS DA scheme. Only the post-tx-op stage differs
  # (the tx loop is identical to pass 1), so we capture ONLY the cycle
  # markers here — no opcode/precompile dumps.
  if grep -q "BENCH_DA_SCHEME" tests/instances/eth_runner/src/single_run.rs; then
    BENCH_DA_SCHEME=blobs_zksync_os \
    MARKER_PATH="$(pwd)/${SIDE}_block_${blk}_blobs.bench" \
      cargo run --manifest-path tests/instances/eth_runner/Cargo.toml $PROFILE \
        --features "$EVM_FEATURES" \
        -- single-run --block-dir "$dir" \
        > "${SIDE}_block_${blk}_blobs.out"
  fi
done

# Test-crate precompile workload. Test name substring filters (Rust's
# harness matches if ANY substring matches):
#   test_precompiles        — 114 core precompile vectors (TESTS)
#   test_pectra_precompiles — 6 BLAKE2F + BLS12-381 vectors (PECTRA_TESTS,
#                             gated by `precompiles/pectra` feature)
#   test_kzg_regression     — 1 KZG / point_evaluation vector (KZG_TESTS)
# `test_p256` (781 P256 vectors) is `#[ignore = "Too long for CI"]`; would
# need `--include-ignored` to run, which significantly lengthens CI. Tracked
# as a follow-up coverage gap.
MARKER_PATH="$(pwd)/${SIDE}_precompiles.bench" \
PRECOMPILE_STATS_PATH="$(pwd)/${SIDE}_precompile_stats.csv" \
PRECOMPILE_SAMPLES_DIR="$(pwd)/${SIDE}_precompile_samples" \
LABEL_CYCLE_SAMPLES_DIR="$(pwd)/${SIDE}_precompile_cycles" \
  cargo test $PROFILE --features "$PRECOMPILES_FEATURES" -p precompiles \
    -- --test-threads=1 $PRECOMPILES_TESTS
