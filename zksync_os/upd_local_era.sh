#!/bin/sh
set -e

# Default: do recompile/dump
RECOMPILE=true

# Parse flags
while [ "$#" -gt 0 ]; do
  case "$1" in
    --no-recompile)
      RECOMPILE=false
      shift
      ;;
    *)
      echo "Unknown argument: $1"
      echo "Usage: $0 [--no-recompile]"
      exit 1
      ;;
  esac
done

TARGET_DIR="../../zksync-era"

# 1. Optionally regenerate the server binaries
if [ "$RECOMPILE" = "true" ]; then
  echo "Regenerating server binaries…"

  echo " → ./dump_bin.sh --type singleblock-batch"
  ./dump_bin.sh --type singleblock-batch

  echo " → ./dump_bin.sh --type singleblock-batch-logging-enabled"
  ./dump_bin.sh --type singleblock-batch-logging-enabled
fi

# 2. Verify target directory exists
if [ ! -d "$TARGET_DIR" ]; then
  echo "Error: target directory '$TARGET_DIR' does not exist."
  exit 1
fi

# 3. Copy singleblock_batch app.bin → app.bin
SINGLEBLOCK_BIN="dist/singleblock_batch/app.bin"
if [ ! -f "$SINGLEBLOCK_BIN" ]; then
  echo "Error: source file '$SINGLEBLOCK_BIN' not found."
  exit 1
fi
cp -f "$SINGLEBLOCK_BIN" "$TARGET_DIR/app.bin"
echo "Copied $SINGLEBLOCK_BIN → $TARGET_DIR/app.bin"

# 4. Copy singleblock_batch_logging_enabled app.bin → app_logging_enabled.bin
LOGGING_BIN="dist/singleblock_batch_logging_enabled/app.bin"
if [ ! -f "$LOGGING_BIN" ]; then
  echo "Error: source file '$LOGGING_BIN' not found."
  exit 1
fi
cp -f "$LOGGING_BIN" "$TARGET_DIR/app_logging_enabled.bin"
echo "Copied $LOGGING_BIN → $TARGET_DIR/app_logging_enabled.bin"

# 5. Done
echo "All specified binaries have been replaced in '$TARGET_DIR'."
