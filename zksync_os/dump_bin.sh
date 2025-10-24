#!/bin/sh
set -e

USAGE="Usage: $0 --type {server|server-logging-enabled|debug-in-simulator|evm-replay|evm-replay-benchmarking|multiblock-batch|multiblock-batch-logging-enabled|evm-tester|for-tests}"
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

# Base features and output names
FEATURES="proving"

# Adjust for server modes
case "$TYPE" in
  server)
    FEATURES="$FEATURES,server"
    BIN_NAME="server_app.bin"
    ELF_NAME="server_app.elf"
    TEXT_NAME="server_app.text"
    ;;
  server-logging-enabled)
    FEATURES="$FEATURES,server,print_debug_info"
    BIN_NAME="server_app_logging_enabled.bin"
    ELF_NAME="server_app_logging_enabled.elf"
    TEXT_NAME="server_app_logging_enabled.text"
    ;;
  for-tests)
    FEATURES="$FEATURES,for_tests"
    BIN_NAME="for_tests.bin"
    ELF_NAME="for_tests.elf"
    TEXT_NAME="for_tests.text"
    ;;
  for-tests-logging-enabled)
    FEATURES="$FEATURES,for_tests,print_debug_info"
    BIN_NAME="for_tests.bin"
    ELF_NAME="for_tests.elf"
    TEXT_NAME="for_tests.text"
    ;;
  evm-replay)
    FEATURES="$FEATURES,eth_runner"
    BIN_NAME="evm_replay.bin"
    ELF_NAME="evm_replay.elf"
    TEXT_NAME="evm_replay.text"
    ;;
  evm-replay-benchmarking)
    FEATURES="$FEATURES,eth_runner,benchmarking"
    BIN_NAME="evm_replay.bin"
    ELF_NAME="evm_replay.elf"
    TEXT_NAME="evm_replay.text"
    ;;
  multiblock-batch)
    FEATURES="$FEATURES,multiblock-batch"
    BIN_NAME="multiblock_batch.bin"
    ELF_NAME="multiblock_batch.elf"
    TEXT_NAME="multiblock_batch.text"
    ;;
  multiblock-batch-logging-enabled)
    FEATURES="$FEATURES,multiblock-batch,print_debug_info"
    BIN_NAME="multiblock_batch_logging_enabled.bin"
    ELF_NAME="multiblock_batch_logging_enabled.elf"
    TEXT_NAME="multiblock_batch_logging_enabled.text"
    ;;
  evm-tester)
    FEATURES="$FEATURES,evm_tester"
    BIN_NAME="evm_tester.bin"
    ELF_NAME="evm_tester.elf"
    TEXT_NAME="evm_tester.text"
    ;;
  *)
    echo "Invalid --type: $TYPE"
    echo "$USAGE"
    exit 1
    ;;
esac

# Clean up only the artifacts for this mode
rm -f "$BIN_NAME" "$ELF_NAME" "$TEXT_NAME"

# Build
cargo build --features "$FEATURES" --release

# Produce and rename outputs
cargo objcopy --features "$FEATURES" --release -- -O binary "$BIN_NAME"
cargo objcopy --features "$FEATURES" --release -- -R .text "$ELF_NAME"
cargo objcopy --features "$FEATURES" --release -- -O binary --only-section=.text "$TEXT_NAME"

# Summary
echo "Built [$TYPE] with features: $FEATURES"
echo "→ $BIN_NAME"
echo "→ $ELF_NAME"
echo "→ $TEXT_NAME"
