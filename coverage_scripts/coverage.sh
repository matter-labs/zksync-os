#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
RESULTS_DIR="$REPO_ROOT/coverage_results"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# Crates whose tests fail to compile locally or require unavailable resources:
#   evm_interpreter: no_std crate, tests use std::alloc::Global
#   basic_system: dev-dependency on reqwest pulls in openssl-sys
#   multiblock_batch_tests: requires pre-built RISC-V binary (for_tests.bin)
EXCLUDE_FROM_TESTS=(
    evm_interpreter
    basic_system
    multiblock_batch_tests
)

# Tests with pre-existing failures that must be skipped to avoid aborting
# the entire test suite. These are known issues on the dev branch:
#   has_upgrade_tx: stack overflow in basic_bootloader batch_data tests
SKIP_TESTS=(
    has_upgrade_tx
)

usage() {
    cat <<'EOF'
Usage: coverage_scripts/coverage.sh <command> [options]

Commands:
  summary            Collect coverage and show per-crate summary table
  html               Collect coverage and generate HTML report
  lcov               Collect coverage and generate LCOV file
  all                Collect coverage and generate all report formats
  report [format]    Re-generate report from last run (summary|html|lcov)

Options:
  --open             Open HTML report in browser after generating (html/all only)

Results are saved to coverage_results/ at the repo root.

Prerequisites:
  cargo install cargo-llvm-cov
  rustup component add llvm-tools-preview
EOF
    exit 1
}

check_prerequisites() {
    if ! cargo llvm-cov --version &>/dev/null; then
        echo "Error: cargo-llvm-cov is not installed."
        echo ""
        echo "Install with:"
        echo "  cargo install cargo-llvm-cov"
        echo "  rustup component add llvm-tools-preview"
        exit 1
    fi
}

build_exclude_args() {
    local args=()
    for pkg in "${EXCLUDE_FROM_TESTS[@]}"; do
        args+=(--exclude "$pkg")
    done
    echo "${args[@]}"
}

# Regex to exclude test infrastructure source files from coverage reports.
# These directories contain test harnesses and test instances, not production code.
# The pattern matches the directory component in both absolute and relative paths.
IGNORE_REGEX="/(tests|scripts)/"

collect_coverage() {
    mkdir -p "$RESULTS_DIR"
    local exclude_args
    exclude_args=$(build_exclude_args)

    echo "==> Cleaning previous coverage data..."
    cargo llvm-cov clean --workspace 2>/dev/null || true

    local skip_args=()
    for pattern in "${SKIP_TESTS[@]}"; do
        skip_args+=(--skip "$pattern")
    done

    echo "==> Running tests and collecting coverage data..."
    echo "    (excluded from tests: ${EXCLUDE_FROM_TESTS[*]})"
    echo "    (skipping tests: ${SKIP_TESTS[*]})"
    # --no-fail-fast: continue running tests even if some crates fail, so
    # coverage data from passing crates is still collected.
    # Note: we intentionally do NOT pass --features rig/no_print because
    # the workspace contains many crates that don't depend on 'rig', and
    # Cargo rejects unknown features when used with --workspace.
    # shellcheck disable=SC2086
    local test_exit=0
    cargo llvm-cov --no-report --no-fail-fast --workspace $exclude_args \
        -- "${skip_args[@]}" 2>&1 || test_exit=$?

    if [ "$test_exit" -ne 0 ]; then
        echo "==> Warning: test run exited with status $test_exit (some tests may have failed)."
        echo "    Coverage data from passing tests was still collected."
    else
        echo "==> Coverage data collected successfully."
    fi
}

report_summary() {
    echo "==> Generating per-crate coverage summary..."
    cargo llvm-cov report --json \
        --ignore-filename-regex "$IGNORE_REGEX" \
        > "$RESULTS_DIR/coverage.json"

    python3 "$SCRIPT_DIR/parse_coverage.py" \
        --workspace-root "$REPO_ROOT" \
        "$RESULTS_DIR/coverage.json"

    echo ""
    echo "JSON data: $RESULTS_DIR/coverage.json"
}

report_html() {
    echo "==> Generating HTML coverage report..."
    cargo llvm-cov report --html \
        --output-dir "$RESULTS_DIR/html" \
        --ignore-filename-regex "$IGNORE_REGEX"

    echo "HTML report: $RESULTS_DIR/html/index.html"
}

report_lcov() {
    echo "==> Generating LCOV coverage file..."
    cargo llvm-cov report --lcov \
        --output-path "$RESULTS_DIR/lcov.info" \
        --ignore-filename-regex "$IGNORE_REGEX"

    echo "LCOV file: $RESULTS_DIR/lcov.info"
}

OPEN_BROWSER=false
CMD="${1:-}"
shift || true

# Parse remaining flags
for arg in "$@"; do
    case "$arg" in
        --open) OPEN_BROWSER=true ;;
        summary|html|lcov) CMD_ARG="$arg" ;;
        *) echo "Unknown argument: $arg"; usage ;;
    esac
done

open_html_if_requested() {
    if [ "$OPEN_BROWSER" = true ] && [ -f "$RESULTS_DIR/html/index.html" ]; then
        if command -v xdg-open &>/dev/null; then
            xdg-open "$RESULTS_DIR/html/index.html"
        elif command -v open &>/dev/null; then
            open "$RESULTS_DIR/html/index.html"
        else
            echo "(Could not detect browser opener — open the HTML file manually)"
        fi
    fi
}

case "$CMD" in
    summary)
        check_prerequisites
        collect_coverage
        report_summary
        ;;
    html)
        check_prerequisites
        collect_coverage
        report_html
        open_html_if_requested
        ;;
    lcov)
        check_prerequisites
        collect_coverage
        report_lcov
        ;;
    all)
        check_prerequisites
        collect_coverage
        report_summary
        report_html
        report_lcov
        open_html_if_requested
        ;;
    report)
        check_prerequisites
        case "${CMD_ARG:-summary}" in
            summary) report_summary ;;
            html)
                report_html
                open_html_if_requested
                ;;
            lcov) report_lcov ;;
            *) echo "Unknown report format: ${CMD_ARG:-}"; usage ;;
        esac
        ;;
    *)
        usage
        ;;
esac
