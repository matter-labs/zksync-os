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
#   1. Block-level effective cycles      (`process_block` aggregated across all
#                                         block fixtures per DA scheme; the
#                                         per-block breakdown is under <details>)
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
# Aggregate variant of the headline: same data relabeled so all blocks of a
# DA scheme collapse to one summed row (see `compare_bench.py --aggregate`).
# Keeps the top-level comment to two rows regardless of fixture count; the
# per-block breakdown goes under a spoiler.
headline_agg_pairs=""
subphase_pairs=""
# The default DA scheme run gets all four sub-phases. The BlobsZKsyncOS pass
# only differs in the post-tx-op stage, so we surface only the rows that
# actually change there.
subphase_symbols_keccak="system_init run_tx_loop da_commitment state_commitment_update"
subphase_symbols_blobs="da_commitment state_commitment_update blob_versioned_hash"

for dir in tests/instances/eth_runner/blocks/*; do
  blk=$(basename "$dir")
  # When this PR changes the fixture set, the base (merge-base) side ran a
  # different set of blocks and produced no artifacts for the new ones.
  # Synthesize the missing base artifacts from head so the comparison renders
  # with 0% deltas and visible absolute values (same philosophy as the blobs
  # `.bench` fallback below); real deltas appear once the fixtures land on the
  # base branch.
  for suf in .out .bench; do
    if [ ! -e "base_block_${blk}${suf}" ] && [ -e "head_block_${blk}${suf}" ]; then
      cp "head_block_${blk}${suf}" "base_block_${blk}${suf}"
    fi
  done
  for d in opcode_samples opcode_cycles; do
    if [ ! -d "${d}/base_${blk}" ] && [ -d "${d}/head_${blk}" ]; then
      cp -r "${d}/head_${blk}" "${d}/base_${blk}"
    fi
  done
  python3 "$REPO_ROOT/bench_scripts/parse_opcodes.py" "base_block_${blk}.out" "bench_results/base_block_${blk}.csv" "bench_results/base_block_${blk}.png"
  python3 "$REPO_ROOT/bench_scripts/parse_opcodes.py" "head_block_${blk}.out" "bench_results/head_block_${blk}.csv" "bench_results/head_block_${blk}.png"
  add_pair headline_pairs "(\"block_${blk} (keccak DA)\", \"base_block_${blk}.bench\", \"head_block_${blk}.bench\", \"process_block\")"
  add_pair headline_agg_pairs "(\"all blocks (keccak DA)\", \"base_block_${blk}.bench\", \"head_block_${blk}.bench\", \"process_block\")"
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
    add_pair headline_agg_pairs "(\"all blocks (blobs DA)\", \"${base_blob}\", \"head_block_${blk}_blobs.bench\", \"process_block\")"
    for sym in $subphase_symbols_blobs; do
      add_pair subphase_pairs "(\"block_${blk} (blobs DA)\", \"${base_blob}\", \"head_block_${blk}_blobs.bench\", \"${sym}\")"
    done
  fi
done

precompiles_pair="(\"precompiles\", \"base_precompiles.bench\", \"head_precompiles.bench\")"
fri_precompile_pair="(\"fri_precompile\", \"base_fri_precompile.bench\", \"head_fri_precompile.bench\")"

# Each sub-script emits nothing when no row moved between base and head.
# We capture into a tmpfile and skip the surrounding header/<details>
# entirely when the body is empty, so reviewers don't see "## Per-opcode"
# followed by silence on a PR that didn't touch any opcode.
emit_section() {
  # emit_section <body_file> <header_lines...>
  # Appends header_lines + body to $OUT only when body_file is non-empty.
  local body="$1"; shift
  if [ -s "$body" ]; then
    for line in "$@"; do printf '%s\n' "$line" >> "$OUT"; done
    cat "$body" >> "$OUT"
    printf '\n' >> "$OUT"
  fi
}
emit_details_section() {
  # emit_details_section <body_file> <summary>
  # Wraps non-empty body in <details><summary>…</summary>; same skip rule.
  local body="$1"; local summary="$2"
  if [ -s "$body" ]; then
    {
      printf '<details><summary>%s</summary>\n\n' "$summary"
      cat "$body"
      printf '\n</details>\n\n'
    } >> "$OUT"
  fi
}

# Section 1: headline. Show one aggregate `process_block` row per DA scheme
# (summed across all block fixtures) in the top-level table, and the full
# per-block breakdown under a spoiler so the comment stays readable as the
# fixture set grows.
headline_agg_body=$(mktemp)
python3 "$REPO_ROOT/bench_scripts/compare_bench.py" --no-title --aggregate "[${headline_agg_pairs}]" > "$headline_agg_body"
emit_section "$headline_agg_body" "## Block-level effective cycles" "" "_Totals across all block fixtures (\`process_block\`). Per-block breakdown below._" ""
rm -f "$headline_agg_body"

headline_body=$(mktemp)
python3 "$REPO_ROOT/bench_scripts/compare_bench.py" --no-title "[${headline_pairs}]" > "$headline_body"
emit_details_section "$headline_body" "Per-block effective cycles"
rm -f "$headline_body"

# Section 2: sub-phases (collapsed).
subphase_body=$(mktemp)
python3 "$REPO_ROOT/bench_scripts/compare_bench.py" --no-title --sort-by-symbol "[${subphase_pairs}]" > "$subphase_body"
emit_details_section "$subphase_body" "Block-level sub-phases"
rm -f "$subphase_body"

# Section 3: precompiles test-crate bench (collapsed).
precompiles_body=$(mktemp)
python3 "$REPO_ROOT/bench_scripts/compare_bench.py" --no-title "[${precompiles_pair}]" > "$precompiles_body"
emit_details_section "$precompiles_body" "Precompiles test-crate bench (synthetic workload, all labels)"
rm -f "$precompiles_body"

# Section 3a: FRI precompile contract/sidecar bench (collapsed).
fri_precompile_body=$(mktemp)
if [ -f base_fri_precompile.bench ] && [ -f head_fri_precompile.bench ]; then
  python3 "$REPO_ROOT/bench_scripts/compare_bench.py" --no-title "[${fri_precompile_pair}]" > "$fri_precompile_body"
fi
emit_details_section "$fri_precompile_body" "FRI precompile bench (FriProofTx + sidecar + contract call)"
rm -f "$fri_precompile_body"

# Section 3b: pubdata bytes per block (keccak-DA bench files; pubdata is
# invariant to the DA scheme — the keccak file just happens to be where
# `single_run.rs` appends `pubdata_bytes: N`). `compare_pubdata.py`
# self-suppresses when no block's value changed between base and head,
# so no emit_section wrapper is needed.
pubdata_pairs=""
for dir in tests/instances/eth_runner/blocks/*; do
  blk=$(basename "$dir")
  if [ -z "$pubdata_pairs" ]; then
    pubdata_pairs="(\"block_${blk}\", \"base_block_${blk}.bench\", \"head_block_${blk}.bench\")"
  else
    pubdata_pairs="${pubdata_pairs},(\"block_${blk}\", \"base_block_${blk}.bench\", \"head_block_${blk}.bench\")"
  fi
done
python3 "$REPO_ROOT/bench_scripts/compare_pubdata.py" "[${pubdata_pairs}]" >> "$OUT"

# Section 4: per-opcode. Two sub-scripts each emit nothing when nothing
# moved; we suppress the "## Per-opcode" header when both are silent.
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

stats_body=$(mktemp)
if ! python3 "$REPO_ROOT/bench_scripts/compare_opcode_stats.py" $stats_args \
     --sample-dirs $stats_sample_args > "$stats_body"; then
  printf '\n_Per-opcode gas/native diff generation failed; see CI logs._\n' > "$stats_body"
fi
cycles_body=$(mktemp)
if ! python3 "$REPO_ROOT/bench_scripts/compare_opcode_cycles.py" $cycle_args \
     --gas-stats $gas_args \
     --sample-dirs $cycle_sample_args > "$cycles_body"; then
  printf '\n_Per-opcode cycles diff generation failed; see CI logs._\n' > "$cycles_body"
fi
if [ -s "$stats_body" ] || [ -s "$cycles_body" ]; then
  {
    printf '## Per-opcode\n'
    cat "$stats_body"
    cat "$cycles_body"
  } >> "$OUT"
fi
rm -f "$stats_body" "$cycles_body"

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
if [ -d head_fri_precompile_samples ] && [ -d head_fri_precompile_cycles ]; then
  join_pairs="$join_pairs head_fri_precompile_samples head_fri_precompile_cycles"
  if [ -f head_fri_precompile.bench ]; then
    join_bench_args="$join_bench_args --bench-file head_fri_precompile.bench"
  else
    join_bench_args="$join_bench_args --bench-file /dev/null"
  fi
  join_opcode_args="$join_opcode_args --opcode-samples-dir /dev/null"
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

precompile_body=$(mktemp)
if ! python3 "$REPO_ROOT/bench_scripts/join_precompile_samples.py" $join_pairs $join_bench_args $join_opcode_args --summary > "$precompile_body"; then
  printf '(per-execution ratios generation failed; see CI logs)\n' > "$precompile_body"
fi
if [ -s "$precompile_body" ]; then
  {
    printf '## Per-precompile\n\n'
    printf '<details><summary>Per-precompile per-execution ratios (head)</summary>\n\n'
    printf '```\n'
    cat "$precompile_body"
    printf '```\n\n</details>\n'
  } >> "$OUT"
fi
rm -f "$precompile_body"
