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
  
  echo " → ./dump_bin.sh --type server"
  ./dump_bin.sh --type server

  echo " → ./dump_bin.sh --type server-logging-enabled"
  ./dump_bin.sh --type server-logging-enabled
fi

# 2. Verify target directory exists
if [ ! -d "$TARGET_DIR" ]; then
  echo "Error: target directory '$TARGET_DIR' does not exist."
  exit 1
fi

# 3. Copy server_app.bin → app.bin
if [ ! -f server_app.bin ]; then
  echo "Error: source file 'server_app.bin' not found."
  exit 1
fi
cp -f server_app.bin "$TARGET_DIR/app.bin"
echo "Copied server_app.bin → $TARGET_DIR/app.bin"

# 4. Copy server_app_logging_enabled.bin → app_logging_enabled.bin
if [ ! -f server_app_logging_enabled.bin ]; then
  echo "Error: source file 'server_app_logging_enabled.bin' not found."
  exit 1
fi
cp -f server_app_logging_enabled.bin "$TARGET_DIR/app_logging_enabled.bin"
echo "Copied server_app_logging_enabled.bin → $TARGET_DIR/app_logging_enabled.bin"

# 5. Done
echo "All specified binaries have been replaced in '$TARGET_DIR'."
