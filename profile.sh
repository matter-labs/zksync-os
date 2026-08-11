#!/bin/bash

# set -euo pipefail

# ─── Help & Usage ─────────────────────────────────────────────────────────────

usage() {
  cat <<EOF
Usage: $0 <label> [OPTIONS]

Profiles peak heap memory usage of the end-to-end run_base_system test using heaptrack,
then produces a flamegraph-compatible stack trace for visualization.

Has to be run from the root folder of the repo.

Arguments:
  <label>     Label used for output files (e.g. "baseline", "my-branch")

Output files:
  heaptrack-<label>.zst     Raw heaptrack recording
  flamegraph-<label>.txt    Demangled stack data, ready for speedscope
  test-<label>.log          Logs from run_base_system test

Workflow:
  1. Builds the test binary in release mode with e2e_proving feature
  2. Records heap allocations via heaptrack
  3. Extracts peak-memory flamegraph stacks and demangles Rust symbols

Visualization:
  Upload flamegraph-<label>.txt to https://speedscope.app

Dependencies:
  cargo, jq, heaptrack, rustfilt

Examples:
  $0 baseline
  $0 after-my-optimization
EOF
}

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
  usage
  exit 0
fi

if [[ -z "${1:-}" ]]; then
  echo "Error: test name argument is required." >&2
  echo "Run '$0 --help' for usage." >&2
  exit 1
fi

LABEL="$1"

# ─── Dependency checks ────────────────────────────────────────────────────────

check_deps() {
  local missing=()
  for cmd in cargo jq heaptrack heaptrack_print rustfilt; do
    if ! command -v "$cmd" &>/dev/null; then
      missing+=("$cmd")
    fi
  done

  if [[ ${#missing[@]} -gt 0 ]]; then
    echo "Error: missing required dependencies: ${missing[*]}" >&2
    echo "Install them and re-run. See --help for the full list." >&2
    exit 1
  fi
}

check_deps

# ─── Environment ──────────────────────────────────────────────────────────────

export RUST_MIN_STACK=33554432
export ZKSYNC_RISC_V_RUN=true
export CI=true
export CARGO_WORKSPACE_DIR
CARGO_WORKSPACE_DIR="$(pwd)"

OUTPUT_FILE="heaptrack-${LABEL}"
FLAMEGRAPH_FILE="flamegraph-${LABEL}.txt"
TEST_LOG="test-${LABEL}.log"
STACKS_TMP=$(mktemp /tmp/heaptrack-stacks-XXXXXX.txt)
trap 'rm -f "$STACKS_TMP"' EXIT

# ─── Step 1: Build test binary ────────────────────────────────────────────────

echo "==> Building test binary (release, e2e_proving)..."
TEST_BIN=$(
  cargo test --release \
    -p transactions \
    run_base_system \
    --features rig/e2e_proving \
    --no-run \
    --message-format=json \
    2>/dev/null \
  | jq -r 'select(.executable != null) | .executable' \
  | head -1
)

if [[ -z "$TEST_BIN" ]]; then
  echo "Error: failed to locate test binary. Check that the build succeeded." >&2
  exit 1
fi

echo "    Binary: $TEST_BIN"

# ─── Step 2: Record heap profile ──────────────────────────────────────────────

echo "==> Recording heap profile -> ${OUTPUT_FILE}.zst (test logs: ${TEST_LOG})..."
heaptrack --record-only -o "$OUTPUT_FILE" \
  "$TEST_BIN" --exact run_base_system --nocapture \
  2>&1 > "$TEST_LOG"

RECORDING="${OUTPUT_FILE}.zst"
if [[ ! -f "$RECORDING" ]]; then
  echo "Error: heaptrack did not produce expected output file '${RECORDING}'." >&2
  exit 1
fi

# ─── Step 3: Extract & demangle flamegraph stacks ─────────────────────────────

echo "==> Extracting peak-memory flamegraph stacks..."
heaptrack_print "$RECORDING" -F "$STACKS_TMP" --flamegraph-cost-type peak >/dev/null

echo "==> Demangling Rust symbols -> ${FLAMEGRAPH_FILE} ..."
rustfilt < "$STACKS_TMP" > "$FLAMEGRAPH_FILE"

if [[ ! -s "$FLAMEGRAPH_FILE" ]]; then
  echo "Warning: flamegraph output is empty — the recording may have no heap data." >&2
fi

# ─── Done ─────────────────────────────────────────────────────────────────────

echo ""
echo "Done. Peak-memory flamegraph saved to: ${FLAMEGRAPH_FILE}"
echo "Visualize at: https://speedscope.app"
