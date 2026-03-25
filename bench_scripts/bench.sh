#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
RESULTS_DIR="$REPO_ROOT/bench_results"
BASELINE_DIR="$RESULTS_DIR/baseline"
CURRENT_DIR="$RESULTS_DIR/current"
BLOCKS_DIR="$REPO_ROOT/tests/instances/eth_runner/blocks"
# Use the first available block for quick mode
QUICK_BLOCK="$(ls "$BLOCKS_DIR" | head -1)"

FEATURES="rig/no_print,rig/cycle_marker,rig/unlimited_native"
PRECOMPILE_FEATURES="rig/no_print,precompiles/cycle_marker,rig/unlimited_native"
ETH_RUNNER_MANIFEST="$REPO_ROOT/tests/instances/eth_runner/Cargo.toml"
PRECOMPILE_MANIFEST="$REPO_ROOT/tests/instances/precompiles/Cargo.toml"

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
    echo "==> Building RISC-V benchmarking binary..."
    (cd "$REPO_ROOT/zksync_os" && ./dump_bin.sh --type evm-replay-benchmarking)
}

run_block() {
    local block_dir="$1"
    local output_dir="$2"
    local blk
    blk="$(basename "$block_dir")"

    echo "==> Benchmarking block $blk..."
    ZKSYNC_RISC_V_RUN=true \
    MARKER_PATH="$output_dir/block_${blk}.bench" \
    cargo run --manifest-path "$ETH_RUNNER_MANIFEST" \
        --release -j 3 \
        --features "$FEATURES" \
        -- single-run --block-dir "$block_dir" --opcode-stats \
        > "$output_dir/block_${blk}.out" 2>&1
}

run_precompiles() {
    local output_dir="$1"

    echo "==> Benchmarking precompiles..."
    ZKSYNC_RISC_V_RUN=true \
    MARKER_PATH="$output_dir/precompiles.bench" \
    cargo test --manifest-path "$PRECOMPILE_MANIFEST" \
        --release -j 3 \
        --features "$PRECOMPILE_FEATURES" \
        -- test_precompiles \
        > "$output_dir/precompiles.out" 2>&1
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
    echo "==> Baseline saved to $BASELINE_DIR"
}

do_run() {
    mkdir -p "$CURRENT_DIR"
    build_riscv_binary
    run_all_blocks "$CURRENT_DIR"
    run_precompiles "$CURRENT_DIR"
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

    if [ -z "$pairs" ]; then
        echo "ERROR: No matching benchmark files found to compare."
        exit 1
    fi

    python3 "$REPO_ROOT/bench_scripts/compare_bench.py" "[${pairs}]"
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
