#!/bin/bash

set -e

APP_NAME="app_native_blake"
DIST_DIR="dist/$APP_NAME"
rm -rf "$DIST_DIR"
mkdir -p "$DIST_DIR"
cargo objcopy --release -- -O binary "$DIST_DIR/app.bin"
cargo objcopy --release -- -O binary --only-section=.text "$DIST_DIR/app.text"

APP_NAME="app_extended_delegation_blake"
DIST_DIR="dist/$APP_NAME"
rm -rf "$DIST_DIR"
mkdir -p "$DIST_DIR"
cargo objcopy --release --features single_round_with_control -- -O binary "$DIST_DIR/app.bin"
cargo objcopy --release --features single_round_with_control -- -O binary --only-section=.text "$DIST_DIR/app.text"
