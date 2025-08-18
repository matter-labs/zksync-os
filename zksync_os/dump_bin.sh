#!/bin/sh
set -e

# Default mode
TYPE="default"

# Parse --type argument
while [ "$#" -gt 0 ]; do
  case "$1" in
    --type)
      TYPE="$2"
      shift 2
      ;;
    *)
      echo "Unknown argument: $1"
      echo "Usage: $0 [--type default|server|server-logging-enabled|evm-replay|benchmarking|evm-replay-benchmarking|debug-in-simulator|pectra|multiblock-batch|multiblock-batch-logging-enabled]"
      exit 1
      ;;
  esac
done

# Base features and output names
FEATURES="proving"
BIN_NAME="app.bin"
ELF_NAME="app.elf"
TEXT_NAME="app.text"

# Adjust for server modes
case "$TYPE" in
  server)
    FEATURES="$FEATURES,proof_running_system/unlimited_native,proof_running_system/wrap-in-batch"
    BIN_NAME="server_app.bin"
    ELF_NAME="server_app.elf"
    TEXT_NAME="server_app.text"
    ;;
  server-logging-enabled)
    FEATURES="$FEATURES,proof_running_system/unlimited_native,proof_running_system/wrap-in-batch,print_debug_info"
    BIN_NAME="server_app_logging_enabled.bin"
    ELF_NAME="server_app_logging_enabled.elf"
    TEXT_NAME="server_app_logging_enabled.text"
    ;;
  benchmarking)
    FEATURES="$FEATURES,proof_running_system/cycle_marker,proof_running_system/unlimited_native,proof_running_system/p256_precompile"
    BIN_NAME="app.bin"
    ELF_NAME="app.elf"
    TEXT_NAME="app.text"
    ;;
  debug-in-simulator)
    FEATURES="$FEATURES,print_debug_info,proof_running_system/cycle_marker,proof_running_system/unlimited_native,proof_running_system/p256_precompile"
    BIN_NAME="app_debug.bin"
    ELF_NAME="app_debug.elf"
    TEXT_NAME="app_debug.text"
    ;;
  evm-replay)
    FEATURES="$FEATURES,proof_running_system/unlimited_native,proof_running_system/disable_system_contracts,proof_running_system/prevrandao,proof_running_system/evm_refunds"
    BIN_NAME="evm_replay.bin"
    ELF_NAME="evm_replay.elf"
    TEXT_NAME="evm_replay.text"
    ;;
  evm-replay-benchmarking)
    FEATURES="$FEATURES,proof_running_system/unlimited_native,proof_running_system/disable_system_contracts,proof_running_system/cycle_marker,proof_running_system/prevrandao,proof_running_system/evm_refunds"
    BIN_NAME="evm_replay.bin"
    ELF_NAME="evm_replay.elf"
    TEXT_NAME="evm_replay.text"
    ;;
  pectra)
    FEATURES="$FEATURES,proof_running_system/pectra"
    BIN_NAME="app.bin"
    ELF_NAME="app.elf"
    TEXT_NAME="app.text"
    ;;
  multiblock-batch)
    FEATURES="$FEATURES,proof_running_system/unlimited_native,proof_running_system/multiblock-batch"
    BIN_NAME="multiblock_batch.bin"
    ELF_NAME="multiblock_batch.elf"
    TEXT_NAME="multiblock_batch.text"
    ;;
  multiblock-batch-logging-enabled)
    FEATURES="$FEATURES,proof_running_system/unlimited_native,proof_running_system/multiblock-batch,print_debug_info"
    BIN_NAME="multiblock_batch_logging_enabled.bin"
    ELF_NAME="multiblock_batch_logging_enabled.elf"
    TEXT_NAME="multiblock_batch_logging_enabled.text"
    ;;
  default)
    # leave defaults
    ;;
  *)
    echo "Invalid --type: $TYPE"
    echo "Valid types are: default, server, server-logging-enabled, evm-replay, benchmarking, evm-replay-benchmarking, debug-in-simulator, multiblock-batch"
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
