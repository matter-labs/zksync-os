#!/bin/sh
set -e

USAGE="Usage: $0 --type {singleblock-batch|singleblock-batch-logging-enabled|debug-in-simulator|evm-replay|evm-replay-benchmarking|eth-stf|multiblock-batch|multiblock-batch-logging-enabled|evm-tester|for-tests|for-tests-benchmarking|for-tests-logging-enabled}"
TYPE=""

# Parse --type argument
while [ "$#" -gt 0 ]; do
  case "$1" in
    --type)
      [ "$#" -ge 2 ] || { echo "Missing value for --type"; echo "$USAGE"; exit 2; }
      TYPE="$2"
      shift 2
      ;;
    *)
      echo "Unknown argument: $1"
      echo "$USAGE"
      exit 2
      ;;
  esac
done

# Base features
FEATURES="proving"

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
  evm-replay-benchmarking)
    FEATURES="$FEATURES,eth_runner,benchmarking"
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
cargo airbender build --app-name "$APP_NAME" --release -- --features "$FEATURES"

# Summary
echo "Built [$TYPE] with features: $FEATURES"
echo "-> $DIST_DIR/app.bin"
echo "-> $DIST_DIR/app.elf"
echo "-> $DIST_DIR/app.text"
echo "-> $DIST_DIR/manifest.toml"
