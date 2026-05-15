#!/usr/bin/env bash
# Render the PR-comment markdown for the bench workflow from base/head
# artifacts already flattened into the current directory.
#
# Usage: bench_scripts/render_pr_comment.sh <output.md>
#
# Inputs (current directory, written by `run_bench_side.sh` on each side):
#   base_block_<blk>.bench         head_block_<blk>.bench
#   base_block_<blk>_blobs.bench   head_block_<blk>_blobs.bench   (optional)
#   base_block_<blk>.out           head_block_<blk>.out
#   base_block_<blk>_blobs.out     head_block_<blk>_blobs.out     (optional)
#   base_precompiles.bench         head_precompiles.bench
#   opcode_samples/{base,head}_<blk>/   opcode_cycles/{base,head}_<blk>/
#   precompile_samples/head_<blk>/      precompile_cycles/head_<blk>/  (optional)
#   head_precompile_samples/            head_precompile_cycles/
#
# Output: writes the full PR-comment markdown to `<output.md>`.
#
# Sections (in order):
#   1. Block-level effective cycles      (`process_block` per (block, DA scheme))
#   2. Block-level sub-phases            (collapsed under <details>)
#   3. Precompiles test-crate bench      (collapsed under <details>)
#   4. Per-opcode gas/native diff
#   5. Per-opcode RISC-V cycles diff
#   6. Per-precompile per-execution ratios (collapsed under <details>)

set -euo pipefail

OUT="${1:?Usage: $0 <output.md>}"
mkdir -p "$(dirname "$OUT")"
: > "$OUT"

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"

# `add_pair LIST_VAR ENTRY` appends ENTRY to a comma-separated list held in
# the variable named by LIST_VAR (no separator added for the first entry).
add_pair() {
  local list_var="$1"; local entry="$2"
  if [ -z "${!list_var}" ]; then
    eval "$list_var=\$entry"
  else
    eval "$list_var=\"\${$list_var},\$entry\""
  fi
}

# Build three separate pair lists so the resulting PR comment has a clear
# top-level structure:
#   - Headline:   `process_block` per (block, DA scheme).
#   - Sub-phases: `system_init`, `run_tx_loop`, `da_commitment`,
#                 `state_commitment_update`, `blob_versioned_hash` — collapsed
#                 because they're noisy unless something regresses inside one.
#   - Precompiles bench: the synthetic test-crate workload — ~30 labels.
headline_pairs=""
subphase_pairs=""
# The default DA scheme run gets all four sub-phases. The BlobsZKsyncOS pass
# only differs in the post-tx-op stage, so we surface only the rows that
# actually change there.
subphase_symbols_keccak="system_init run_tx_loop da_commitment state_commitment_update"
subphase_symbols_blobs="da_commitment state_commitment_update blob_versioned_hash"

for dir in tests/instances/eth_runner/blocks/*; do
  blk=$(basename "$dir")
  python3 "$REPO_ROOT/bench_scripts/parse_opcodes.py" "base_block_${blk}.out" "bench_results/base_block_${blk}.csv" "bench_results/base_block_${blk}.png"
  python3 "$REPO_ROOT/bench_scripts/parse_opcodes.py" "head_block_${blk}.out" "bench_results/head_block_${blk}.csv" "bench_results/head_block_${blk}.png"
  add_pair headline_pairs "(\"block_${blk} (keccak DA)\", \"base_block_${blk}.bench\", \"head_block_${blk}.bench\", \"process_block\")"
  for sym in $subphase_symbols_keccak; do
    add_pair subphase_pairs "(\"block_${blk} (keccak DA)\", \"base_block_${blk}.bench\", \"head_block_${blk}.bench\", \"${sym}\")"
  done
  # When the merge-base predates `BENCH_DA_SCHEME` plumbing the bench-base
  # job emits no blobs `.bench` file. Fall back to comparing the head's
  # blobs file against itself so the absolute values are still visible in
  # the PR comment (deltas will read 0% — fine until the next PR cycles
  # after merge).
  if [ -f "head_block_${blk}_blobs.bench" ]; then
    if [ -f "base_block_${blk}_blobs.bench" ]; then
      base_blob="base_block_${blk}_blobs.bench"
    else
      base_blob="head_block_${blk}_blobs.bench"
    fi
    add_pair headline_pairs "(\"block_${blk} (blobs DA)\", \"${base_blob}\", \"head_block_${blk}_blobs.bench\", \"process_block\")"
    for sym in $subphase_symbols_blobs; do
      add_pair subphase_pairs "(\"block_${blk} (blobs DA)\", \"${base_blob}\", \"head_block_${blk}_blobs.bench\", \"${sym}\")"
    done
  fi
done

precompiles_pair="(\"precompiles\", \"base_precompiles.bench\", \"head_precompiles.bench\")"

# Section 1: headline.
{
  echo "## Block-level effective cycles"
  echo ""
  python3 "$REPO_ROOT/bench_scripts/compare_bench.py" --no-title "[${headline_pairs}]"
  echo ""
} >> "$OUT"

# Section 2: sub-phases (collapsed).
{
  echo "<details><summary>Block-level sub-phases</summary>"
  echo ""
  python3 "$REPO_ROOT/bench_scripts/compare_bench.py" --no-title --sort-by-symbol "[${subphase_pairs}]"
  echo ""
  echo "</details>"
  echo ""
} >> "$OUT"

# Section 3: precompiles test-crate bench (collapsed).
{
  echo "<details><summary>Precompiles test-crate bench (synthetic workload, all labels)</summary>"
  echo ""
  python3 "$REPO_ROOT/bench_scripts/compare_bench.py" --no-title "[${precompiles_pair}]"
  echo ""
  echo "</details>"
  echo ""
} >> "$OUT"

# Sections 4 + 5: per-opcode. Each sub-script emits nothing when nothing
# moved, so the section header may have no rows under it.
stats_args=""
cycle_args=""
gas_args=""
stats_sample_args=""
cycle_sample_args=""
for dir in tests/instances/eth_runner/blocks/*; do
  blk=$(basename "$dir")
  stats_args="$stats_args base_block_${blk}.out head_block_${blk}.out"
  cycle_args="$cycle_args base_block_${blk}.bench head_block_${blk}.bench"
  gas_args="$gas_args base_block_${blk}.out head_block_${blk}.out"
  stats_sample_args="$stats_sample_args $(pwd)/opcode_samples/base_${blk} $(pwd)/opcode_samples/head_${blk}"
  cycle_sample_args="$cycle_sample_args $(pwd)/opcode_samples/base_${blk} $(pwd)/opcode_cycles/base_${blk} $(pwd)/opcode_samples/head_${blk} $(pwd)/opcode_cycles/head_${blk}"
done

{
  echo "## Per-opcode"
  if ! python3 "$REPO_ROOT/bench_scripts/compare_opcode_stats.py" $stats_args \
       --sample-dirs $stats_sample_args; then
    echo ""
    echo "_Per-opcode gas/native diff generation failed; see CI logs._"
  fi
  if ! python3 "$REPO_ROOT/bench_scripts/compare_opcode_cycles.py" $cycle_args \
       --gas-stats $gas_args \
       --sample-dirs $cycle_sample_args; then
    echo ""
    echo "_Per-opcode cycles diff generation failed; see CI logs._"
  fi
} >> "$OUT"

# Section 6: per-execution precompile ratios.
# `--bench-file` and `--opcode-samples-dir` are positionally matched to
# the (tracer_dir, cycles_dir) pairs passed to `join_precompile_samples.py`.
# `--opcode-samples-dir` provides gas/native for synthetic precompile
# entries (currently `keccak` sourced from `SHA3.samples`).
join_pairs="head_precompile_samples head_precompile_cycles"
join_bench_args=""
join_opcode_args=""
if [ -f head_precompiles.bench ]; then
  join_bench_args="--bench-file head_precompiles.bench"
fi
# Test-crate run dumps opcode samples to a flat dir; per-block runs dump
# under opcode_samples/head_${blk}. Pass the flat dir as the first
# --opcode-samples-dir to align with the first join pair.
if [ -d head_precompile_samples ]; then
  join_opcode_args="--opcode-samples-dir opcode_samples"
fi
for dir in tests/instances/eth_runner/blocks/*; do
  blk=$(basename "$dir")
  if [ -d "precompile_samples/head_${blk}" ] && [ -d "precompile_cycles/head_${blk}" ]; then
    join_pairs="$join_pairs precompile_samples/head_${blk} precompile_cycles/head_${blk}"
    if [ -f "head_block_${blk}.bench" ]; then
      join_bench_args="$join_bench_args --bench-file head_block_${blk}.bench"
    else
      join_bench_args="$join_bench_args --bench-file /dev/null"
    fi
    if [ -d "opcode_samples/head_${blk}" ]; then
      join_opcode_args="$join_opcode_args --opcode-samples-dir opcode_samples/head_${blk}"
    else
      join_opcode_args="$join_opcode_args --opcode-samples-dir /dev/null"
    fi
  fi
done

{
  echo ""
  echo "## Per-precompile"
  echo ""
  echo "<details><summary>Per-precompile per-execution ratios (head)</summary>"
  echo ""
  echo '```'
  if ! python3 "$REPO_ROOT/bench_scripts/join_precompile_samples.py" $join_pairs $join_bench_args $join_opcode_args --summary; then
    echo "(per-execution ratios generation failed; see CI logs)"
  fi
  echo '```'
  echo ""
  echo "</details>"
} >> "$OUT"
