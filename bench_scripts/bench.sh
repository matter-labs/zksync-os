#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
RESULTS_DIR="$REPO_ROOT/bench_results"
BASELINE_DIR="$RESULTS_DIR/baseline"
CURRENT_DIR="$RESULTS_DIR/current"
BLOCKS_DIR="$REPO_ROOT/tests/instances/eth_runner/blocks"
# Use the first available block for quick mode
QUICK_BLOCK="$(ls "$BLOCKS_DIR" | head -1)"

FEATURES="rig/no_print,rig/cycle_marker"
# Osaka block fixtures need the latest-BPO blob schedule on the host. Prefer the
# current `fusaka-bpo-2` feature; fall back to the old `fusaka`/`fusaka-blobs`
# names so the A/B harness still works against a pre-rename merge-base.
if grep -q '^fusaka-bpo-2 = ' "$REPO_ROOT/tests/instances/eth_runner/Cargo.toml"; then
    FEATURES="$FEATURES,fusaka-bpo-2"
elif grep -q '^fusaka-blobs = ' "$REPO_ROOT/tests/instances/eth_runner/Cargo.toml"; then
    FEATURES="$FEATURES,fusaka,fusaka-blobs"
fi
PRECOMPILE_FEATURES="rig/no_print,precompiles/cycle_marker"
if grep -q "for-tests-benchmarking-pectra" "$REPO_ROOT/zksync_os/dump_bin.sh"; then
    FRI_PRECOMPILE_FEATURES="rig/no_print,system_hooks_tests/cycle_marker,system_hooks_tests/pectra"
else
    FRI_PRECOMPILE_FEATURES="rig/no_print,system_hooks_tests/cycle_marker"
fi
ETH_RUNNER_MANIFEST="$REPO_ROOT/tests/instances/eth_runner/Cargo.toml"
PRECOMPILE_MANIFEST="$REPO_ROOT/tests/instances/precompiles/Cargo.toml"
SYSTEM_HOOKS_MANIFEST="$REPO_ROOT/tests/instances/system_hooks/Cargo.toml"

usage() {
    cat <<'EOF'
Usage: bench_scripts/bench.sh <command> [args]

Commands:
  baseline               Build RISC-V binary, run all blocks + precompiles, save as baseline
  run                    Build RISC-V binary, run all blocks + precompiles, save as current
  quick                  Build RISC-V binary, run 1 block, compare against baseline
  compare                Compare saved baseline vs current results (no rebuild/re-run)
  flamegraph [path.svg]  Build RISC-V binary, run 1 block, produce flamegraph SVG

Results are saved to bench_results/ at the repo root.
EOF
    exit 1
}

build_riscv_binary() {
    echo "==> Building RISC-V test-crate benchmarking binary..."
    if grep -q "for-tests-benchmarking-pectra" "$REPO_ROOT/zksync_os/dump_bin.sh"; then
        (cd "$REPO_ROOT/zksync_os" && ./dump_bin.sh --type for-tests-benchmarking-pectra)
    else
        (cd "$REPO_ROOT/zksync_os" && ./dump_bin.sh --type for-tests-benchmarking)
    fi
    echo "==> Building RISC-V block-replay benchmarking binary..."
    (cd "$REPO_ROOT/zksync_os" && ./dump_bin.sh --type evm-replay-benchmarking)
}

run_block() {
    local block_dir="$1"
    local output_dir="$2"
    local blk
    blk="$(basename "$block_dir")"

    # Ensure a clean slate for this block's samples/cycles/stats to avoid stale files
    local block_samples_dir="$output_dir/opcode_samples/block_${blk}"
    local block_cycles_dir="$output_dir/opcode_cycles/block_${blk}"
    local block_stats_path="$output_dir/opcode_stats/block_${blk}.csv"
    local precompile_stats_path="$output_dir/block_${blk}_precompile_stats.csv"
    local precompile_samples_dir="$output_dir/precompile_samples/block_${blk}"
    local precompile_cycles_dir="$output_dir/precompile_cycles/block_${blk}"

    rm -rf "$block_samples_dir" "$block_cycles_dir"
    rm -rf "$precompile_samples_dir" "$precompile_cycles_dir"
    rm -f "$block_stats_path" "$precompile_stats_path"

    mkdir -p "$output_dir/opcode_samples" "$output_dir/opcode_cycles" "$output_dir/opcode_stats"
    mkdir -p "$output_dir/precompile_samples" "$output_dir/precompile_cycles"

    echo "==> Benchmarking block $blk..."
    ZKSYNC_RISC_V_RUN=true \
    OPCODE_SAMPLES_DIR="$block_samples_dir" \
    OPCODE_CYCLE_SAMPLES_DIR="$block_cycles_dir" \
    OPCODE_STATS_PATH="$block_stats_path" \
    MARKER_PATH="$output_dir/block_${blk}.bench" \
    PRECOMPILE_STATS_PATH="$precompile_stats_path" \
    PRECOMPILE_SAMPLES_DIR="$precompile_samples_dir" \
    LABEL_CYCLE_SAMPLES_DIR="$precompile_cycles_dir" \
    cargo run --manifest-path "$ETH_RUNNER_MANIFEST" \
        --release -j 3 \
        --features "$FEATURES" \
        -- single-run --block-dir "$block_dir" --opcode-stats \
        > "$output_dir/block_${blk}.out" 2>&1
}

run_precompiles() {
    local output_dir="$1"

    # Use a dedicated sub-namespace so we don't clobber per-block dirs that
    # `run_all_blocks` already wrote under $output_dir/precompile_{samples,cycles}/block_*.
    local samples_dir="$output_dir/precompile_samples/test_precompiles"
    local cycles_dir="$output_dir/precompile_cycles/test_precompiles"

    # Clean only our subdir so per-block artifacts survive.
    rm -rf "$samples_dir" "$cycles_dir"
    mkdir -p "$samples_dir" "$cycles_dir"

    echo "==> Benchmarking precompiles..."
    ZKSYNC_RISC_V_RUN=true \
    MARKER_PATH="$output_dir/precompiles.bench" \
    PRECOMPILE_STATS_PATH="$output_dir/precompile_stats.csv" \
    PRECOMPILE_SAMPLES_DIR="$samples_dir" \
    LABEL_CYCLE_SAMPLES_DIR="$cycles_dir" \
    cargo test --manifest-path "$PRECOMPILE_MANIFEST" \
        --release -j 3 \
        --features "$PRECOMPILE_FEATURES" \
        -- test_precompiles \
        > "$output_dir/precompiles.out" 2>&1
}

run_fri_precompile() {
    local output_dir="$1"

    local samples_dir="$output_dir/precompile_samples/fri_precompile"
    local cycles_dir="$output_dir/precompile_cycles/fri_precompile"

    rm -rf "$samples_dir" "$cycles_dir"
    mkdir -p "$samples_dir" "$cycles_dir"
    rm -f "$output_dir/fri_precompile_stats.csv"

    echo "==> Benchmarking FRI precompile..."
    ZKSYNC_RISC_V_RUN=true \
    MARKER_PATH="$output_dir/fri_precompile.bench" \
    PRECOMPILE_STATS_PATH="$output_dir/fri_precompile_stats.csv" \
    PRECOMPILE_SAMPLES_DIR="$samples_dir" \
    LABEL_CYCLE_SAMPLES_DIR="$cycles_dir" \
    cargo test --manifest-path "$SYSTEM_HOOKS_MANIFEST" \
        --release -j 3 \
        --features "$FRI_PRECOMPILE_FEATURES" \
        -- fri_verifier_contract_returns_true_for_verified_proof \
        > "$output_dir/fri_precompile.out" 2>&1
}

join_precompile_samples_run() {
    local output_dir="$1"

    local pairs=()
    local bench_args=()

    # Test-crate cycle bench (test_precompiles). Lives in its own subdir so
    # it doesn't collide with the per-block subdirs that share the parent.
    local tc_samples="$output_dir/precompile_samples/test_precompiles"
    local tc_cycles="$output_dir/precompile_cycles/test_precompiles"
    if [ -d "$tc_samples" ] && [ -d "$tc_cycles" ]; then
        pairs+=("$tc_samples" "$tc_cycles")
        if [ -f "$output_dir/precompiles.bench" ]; then
            bench_args+=(--bench-file "$output_dir/precompiles.bench")
        else
            bench_args+=(--bench-file /dev/null)
        fi
    fi

    local fri_samples="$output_dir/precompile_samples/fri_precompile"
    local fri_cycles="$output_dir/precompile_cycles/fri_precompile"
    if [ -d "$fri_samples" ] && [ -d "$fri_cycles" ]; then
        pairs+=("$fri_samples" "$fri_cycles")
        if [ -f "$output_dir/fri_precompile.bench" ]; then
            bench_args+=(--bench-file "$output_dir/fri_precompile.bench")
        else
            bench_args+=(--bench-file /dev/null)
        fi
    fi

    # Per-block eth_runner bench (real workloads).
    for dir in "$BLOCKS_DIR"/*/; do
        local blk
        blk="$(basename "$dir")"
        local p_samples="$output_dir/precompile_samples/block_${blk}"
        local p_cycles="$output_dir/precompile_cycles/block_${blk}"
        local p_bench="$output_dir/block_${blk}.bench"
        if [ -d "$p_samples" ] && [ -d "$p_cycles" ]; then
            pairs+=("$p_samples" "$p_cycles")
            if [ -f "$p_bench" ]; then
                bench_args+=(--bench-file "$p_bench")
            else
                bench_args+=(--bench-file /dev/null)
            fi
        fi
    done

    if [ ${#pairs[@]} -ge 2 ]; then
        echo "==> Joining precompile per-execution samples (${#pairs[@]} dirs across $((${#pairs[@]} / 2)) sources)..."
        python3 "$REPO_ROOT/bench_scripts/join_precompile_samples.py" \
            "${pairs[@]}" \
            "${bench_args[@]}" \
            --out-dir "$output_dir/precompile_joined" \
            --summary \
            > "$output_dir/precompile_joined_summary.txt" 2>&1 || true
    fi
}

run_all_blocks() {
    local output_dir="$1"
    for dir in "$BLOCKS_DIR"/*/; do
        run_block "$dir" "$output_dir"
    done
}

do_baseline() {
    mkdir -p "$BASELINE_DIR"
    build_riscv_binary
    run_all_blocks "$BASELINE_DIR"
    run_precompiles "$BASELINE_DIR"
    run_fri_precompile "$BASELINE_DIR"
    join_precompile_samples_run "$BASELINE_DIR"
    echo "==> Baseline saved to $BASELINE_DIR"
}

do_run() {
    mkdir -p "$CURRENT_DIR"
    build_riscv_binary
    run_all_blocks "$CURRENT_DIR"
    run_precompiles "$CURRENT_DIR"
    run_fri_precompile "$CURRENT_DIR"
    join_precompile_samples_run "$CURRENT_DIR"
    echo "==> Results saved to $CURRENT_DIR"
}

do_quick() {
    if [ ! -d "$BASELINE_DIR" ]; then
        echo "ERROR: No baseline found. Run 'bench_scripts/bench.sh baseline' first."
        exit 1
    fi

    mkdir -p "$CURRENT_DIR"
    build_riscv_binary
    run_block "$BLOCKS_DIR/$QUICK_BLOCK" "$CURRENT_DIR"

    echo ""
    echo "==> Quick comparison (block $QUICK_BLOCK):"
    python3 "$REPO_ROOT/bench_scripts/compare_bench.py" \
        "[(\"block_${QUICK_BLOCK}\", \"$BASELINE_DIR/block_${QUICK_BLOCK}.bench\", \"$CURRENT_DIR/block_${QUICK_BLOCK}.bench\", \"process_block\")]"
    echo ""
    python3 "$REPO_ROOT/bench_scripts/compare_opcode_stats.py" \
        "$BASELINE_DIR/block_${QUICK_BLOCK}.out" "$CURRENT_DIR/block_${QUICK_BLOCK}.out" \
        --sample-dirs \
        "$BASELINE_DIR/opcode_samples/block_${QUICK_BLOCK}" "$CURRENT_DIR/opcode_samples/block_${QUICK_BLOCK}" \
        2>/dev/null || true
    python3 "$REPO_ROOT/bench_scripts/compare_opcode_cycles.py" \
        "$BASELINE_DIR/block_${QUICK_BLOCK}.bench" "$CURRENT_DIR/block_${QUICK_BLOCK}.bench" \
        --gas-stats "$BASELINE_DIR/block_${QUICK_BLOCK}.out" "$CURRENT_DIR/block_${QUICK_BLOCK}.out" \
        --sample-dirs \
        "$BASELINE_DIR/opcode_samples/block_${QUICK_BLOCK}" "$BASELINE_DIR/opcode_cycles/block_${QUICK_BLOCK}" \
        "$CURRENT_DIR/opcode_samples/block_${QUICK_BLOCK}" "$CURRENT_DIR/opcode_cycles/block_${QUICK_BLOCK}" \
        2>/dev/null || true
}

do_compare() {
    if [ ! -d "$BASELINE_DIR" ]; then
        echo "ERROR: No baseline found. Run 'bench_scripts/bench.sh baseline' first."
        exit 1
    fi
    if [ ! -d "$CURRENT_DIR" ]; then
        echo "ERROR: No current results found. Run 'bench_scripts/bench.sh run' first."
        exit 1
    fi

    local pairs=""
    for dir in "$BLOCKS_DIR"/*/; do
        local blk
        blk="$(basename "$dir")"
        local base_file="$BASELINE_DIR/block_${blk}.bench"
        local head_file="$CURRENT_DIR/block_${blk}.bench"
        if [ -f "$base_file" ] && [ -f "$head_file" ]; then
            if [ -n "$pairs" ]; then
                pairs="${pairs},"
            fi
            pairs="${pairs}(\"block_${blk}\", \"${base_file}\", \"${head_file}\", \"process_block\")"
        fi
    done

    local base_precompiles="$BASELINE_DIR/precompiles.bench"
    local head_precompiles="$CURRENT_DIR/precompiles.bench"
    if [ -f "$base_precompiles" ] && [ -f "$head_precompiles" ]; then
        if [ -n "$pairs" ]; then
            pairs="${pairs},"
        fi
        pairs="${pairs}(\"precompiles\", \"${base_precompiles}\", \"${head_precompiles}\")"
    fi

    local base_fri_precompile="$BASELINE_DIR/fri_precompile.bench"
    local head_fri_precompile="$CURRENT_DIR/fri_precompile.bench"
    if [ -f "$base_fri_precompile" ] && [ -f "$head_fri_precompile" ]; then
        if [ -n "$pairs" ]; then
            pairs="${pairs},"
        fi
        pairs="${pairs}(\"fri_precompile\", \"${base_fri_precompile}\", \"${head_fri_precompile}\")"
    fi

    if [ -z "$pairs" ]; then
        echo "ERROR: No matching benchmark files found to compare."
        exit 1
    fi

    python3 "$REPO_ROOT/bench_scripts/compare_bench.py" "[${pairs}]"
    echo ""

    local stats_args=()
    local cycle_args=()
    local gas_args=()
    local stats_sample_args=()
    local cycle_sample_args=()
    for dir in "$BLOCKS_DIR"/*/; do
        local blk
        blk="$(basename "$dir")"
        if [ -f "$BASELINE_DIR/block_${blk}.out" ] && [ -f "$CURRENT_DIR/block_${blk}.out" ]; then
            stats_args+=("$BASELINE_DIR/block_${blk}.out" "$CURRENT_DIR/block_${blk}.out")
            gas_args+=("$BASELINE_DIR/block_${blk}.out" "$CURRENT_DIR/block_${blk}.out")
            stats_sample_args+=(
                "$BASELINE_DIR/opcode_samples/block_${blk}"
                "$CURRENT_DIR/opcode_samples/block_${blk}"
            )
        fi
        if [ -f "$BASELINE_DIR/block_${blk}.bench" ] && [ -f "$CURRENT_DIR/block_${blk}.bench" ]; then
            cycle_args+=("$BASELINE_DIR/block_${blk}.bench" "$CURRENT_DIR/block_${blk}.bench")
            cycle_sample_args+=(
                "$BASELINE_DIR/opcode_samples/block_${blk}"
                "$BASELINE_DIR/opcode_cycles/block_${blk}"
                "$CURRENT_DIR/opcode_samples/block_${blk}"
                "$CURRENT_DIR/opcode_cycles/block_${blk}"
            )
        fi
    done

    if [ ${#stats_args[@]} -gt 0 ]; then
        python3 "$REPO_ROOT/bench_scripts/compare_opcode_stats.py" \
            "${stats_args[@]}" --sample-dirs "${stats_sample_args[@]}" \
            2>/dev/null || true
    fi
    if [ ${#cycle_args[@]} -gt 0 ]; then
        python3 "$REPO_ROOT/bench_scripts/compare_opcode_cycles.py" \
            "${cycle_args[@]}" --gas-stats "${gas_args[@]}" --sample-dirs "${cycle_sample_args[@]}" \
            2>/dev/null || true
    fi
    # Aggregate per-precompile stats across test-crate + all block benchmarks.
    local precompile_stats_args=()
    if [ -f "$BASELINE_DIR/precompile_stats.csv" ] && [ -f "$CURRENT_DIR/precompile_stats.csv" ]; then
        precompile_stats_args+=(
            "$BASELINE_DIR/precompile_stats.csv"
            "$CURRENT_DIR/precompile_stats.csv"
        )
    fi
    if [ -f "$BASELINE_DIR/fri_precompile_stats.csv" ] && [ -f "$CURRENT_DIR/fri_precompile_stats.csv" ]; then
        precompile_stats_args+=(
            "$BASELINE_DIR/fri_precompile_stats.csv"
            "$CURRENT_DIR/fri_precompile_stats.csv"
        )
    fi
    for dir in "$BLOCKS_DIR"/*/; do
        local blk
        blk="$(basename "$dir")"
        local base_csv="$BASELINE_DIR/block_${blk}_precompile_stats.csv"
        local head_csv="$CURRENT_DIR/block_${blk}_precompile_stats.csv"
        if [ -f "$base_csv" ] && [ -f "$head_csv" ]; then
            precompile_stats_args+=("$base_csv" "$head_csv")
        fi
    done
    if [ ${#precompile_stats_args[@]} -ge 2 ]; then
        python3 "$REPO_ROOT/bench_scripts/compare_precompile_stats.py" \
            "${precompile_stats_args[@]}" \
            2>/dev/null || true
    fi
}

do_flamegraph() {
    local output_svg="${1:-$RESULTS_DIR/flamegraph.svg}"
    local output_txt="${output_svg%.svg}.txt"
    mkdir -p "$(dirname "$output_svg")"
    build_riscv_binary

    echo "==> Generating flamegraph for block $QUICK_BLOCK..."
    ZKSYNC_RISC_V_RUN=true \
    cargo run --manifest-path "$ETH_RUNNER_MANIFEST" \
        --release -j 3 \
        --features "$FEATURES" \
        -- single-run --block-dir "$BLOCKS_DIR/$QUICK_BLOCK" \
        --flamegraph "$output_svg"

    echo "==> Generating text summary..."
    python3 "$REPO_ROOT/bench_scripts/parse_flamegraph.py" "$output_svg" "$output_txt"

    echo "==> Flamegraph saved to $output_svg"
    echo "==> Text summary saved to $output_txt"
}

# --- Main ---

[ $# -lt 1 ] && usage

case "$1" in
    baseline)   do_baseline ;;
    run)        do_run ;;
    quick)      do_quick ;;
    compare)    do_compare ;;
    flamegraph) do_flamegraph "${2:-}" ;;
    *)          usage ;;
esac
