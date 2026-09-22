#!/bin/sh
set -e

USAGE="Usage: $0 --type {singleblock-batch|singleblock-batch-logging-enabled|debug-in-simulator|evm-replay|evm-replay-benchmarking|evm-replay-benchmarking-fusaka|multiblock-batch|multiblock-batch-logging-enabled|evm-tester|for-tests|for-tests-benchmarking|for-tests-logging-enabled|eth-stf|eth-stf-fusaka|eth-stf-fusaka-debug} [--reproducible]"
TYPE=""
REPRODUCIBLE=""
EXTRA_CARGO_ARGS=""

# Parse arguments
while [ "$#" -gt 0 ]; do
  case "$1" in
    --type)
      [ "$#" -ge 2 ] || { echo "Missing value for --type"; echo "$USAGE"; exit 2; }
      TYPE="$2"
      shift 2
      ;;
    --reproducible)
      REPRODUCIBLE="--reproducible --workspace-root .."
      shift
      ;;
    *)
      echo "Unknown argument: $1"
      echo "$USAGE"
      exit 2
      ;;
  esac
done

# Base features
# `EXTRA_FEATURES=a,b` adds crate features to every type (e.g. `bytereverse_delegation`)
FEATURES="proving${EXTRA_FEATURES:+,$EXTRA_FEATURES}"

# Adjust for server modes
case "$TYPE" in
  singleblock-batch)
    FEATURES="$FEATURES,production"
    APP_NAME="singleblock_batch"
    ;;
  singleblock-batch-logging-enabled)
    FEATURES="$FEATURES,production,print_debug_info"
    APP_NAME="singleblock_batch_logging_enabled"
    ;;
  multiblock-batch)
    FEATURES="$FEATURES,production,multiblock-batch"
    APP_NAME="multiblock_batch"
    ;;
  multiblock-batch-logging-enabled)
    FEATURES="$FEATURES,production,multiblock-batch,print_debug_info"
    APP_NAME="multiblock_batch_logging_enabled"
    ;;
  for-tests)
    FEATURES="$FEATURES,for_tests"
    APP_NAME="for_tests"
    ;;
  for-tests-benchmarking)
    FEATURES="$FEATURES,for_tests,benchmarking"
    APP_NAME="for_tests"
    ;;
  for-tests-logging-enabled)
    FEATURES="$FEATURES,for_tests,print_debug_info"
    APP_NAME="for_tests"
    ;;
  evm-replay)
    FEATURES="$FEATURES,eth_runner"
    APP_NAME="evm_replay"
    ;;
  eth-stf)
    FEATURES="$FEATURES,eth_runner,eth_stf"
    APP_NAME="eth_stf"
    ;;
  eth-stf-fusaka)
    # `eth-stf` with the BPO2 blob-count schedule (`fusaka-bpo-2`), required to
    # replay post-BPO2 mainnet blocks (the Ethproofs flow). The host side must be
    # built with `--features rig/eth_stf,fusaka-bpo-2` to match.
    FEATURES="$FEATURES,eth_runner,eth_stf,fusaka-bpo-2"
    APP_NAME="eth_stf"
    ;;
  eth-stf-fusaka-debug)
    # Same program as `eth-stf-fusaka`, but the release build keeps full DWARF
    # debug info (for every crate) so `dist/eth_stf_debug/app.elf` can symbolize
    # transpiler flamegraphs (`eth_runner ethproofs-flamegraph`). The raw
    # `app.bin` / `app.text` are unaffected by debug info, but the dist is kept
    # separate so profiling always uses a self-consistent bin + ELF pair.
    FEATURES="$FEATURES,eth_runner,eth_stf,fusaka-bpo-2"
    APP_NAME="eth_stf_debug"
    EXTRA_CARGO_ARGS='--config debug_symbols.toml'
    ;;
  evm-replay-benchmarking)
    FEATURES="$FEATURES,eth_runner,benchmarking"
    APP_NAME="evm_replay"
    ;;
  evm-replay-benchmarking-fusaka)
    # Adds `fusaka-bpo-2` (the BPO2 blob-count schedule) on top of
    # `evm-replay-benchmarking` so the proving binary can replay post-BPO Osaka
    # blocks (BPO blob base fee + higher blob count).
    # NOTE: the literal string `evm-replay-benchmarking-fusaka` is used as a
    # `grep -q` fallback target by `.github/workflows/bench.yml` — if this
    # case label is renamed, update the workflow too.
    FEATURES="$FEATURES,eth_runner,benchmarking,fusaka-bpo-2"
    APP_NAME="evm_replay"
    ;;
  evm-tester)
    FEATURES="$FEATURES,evm_tester"
    APP_NAME="evm_tester"
    ;;
  "")
    echo "Missing --type argument"
    echo "$USAGE"
    exit 2
    ;;
  *)
    echo "Invalid --type: $TYPE"
    echo "$USAGE"
    exit 1
    ;;
esac

DIST_DIR="dist/$APP_NAME"

# Clean up previous artifacts for this app
rm -rf "$DIST_DIR"

# Build via cargo airbender — outputs go to dist/<APP_NAME>/app.{bin,elf,text} + manifest.toml
# shellcheck disable=SC2086 # EXTRA_CARGO_ARGS is intentionally word-split
cargo airbender build --app-name "$APP_NAME" --release $REPRODUCIBLE -- --features "$FEATURES" $EXTRA_CARGO_ARGS

# Summary
echo "Built [$TYPE] with features: $FEATURES"
echo "-> $DIST_DIR/app.bin"
echo "-> $DIST_DIR/app.elf"
echo "-> $DIST_DIR/app.text"
echo "-> $DIST_DIR/manifest.toml"
