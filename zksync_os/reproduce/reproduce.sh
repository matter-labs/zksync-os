#!/bin/bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ZKSYNC_OS_DIR="$(dirname "$SCRIPT_DIR")"
REPO_ROOT="$(dirname "$ZKSYNC_OS_DIR")"

# `cargo airbender build --reproducible` passes `--locked` to cargo, so both
# the workspace root and the guest crate need a Cargo.lock present.
#
# The workspace root Cargo.lock is gitignored (library workspace), so we
# generate it here. It doesn't affect the guest binary.
#
# The guest zksync_os/Cargo.lock IS committed — we intentionally do NOT
# regenerate it so that builds are pinned to the committed dependency
# versions. If Cargo.toml changed without updating the lockfile, the
# --locked build will fail, which is the desired behavior.
#
# NOTE: the toolchain version must match rust-toolchain.toml.
cargo +nightly-2026-02-10 generate-lockfile --manifest-path "$REPO_ROOT/Cargo.toml"

cd "$ZKSYNC_OS_DIR"

TYPES=(
    for-tests
    evm-replay
    singleblock-batch
    singleblock-batch-logging-enabled
    multiblock-batch
    multiblock-batch-logging-enabled
)

for TYPE in "${TYPES[@]}"; do
    ./dump_bin.sh --type "$TYPE" --reproducible
done

# Copy dist/<app>/app.bin -> zksync_os/<app>.bin for backwards compatibility
# with downstream consumers (release workflow, zksync-era, etc.).
for TYPE in "${TYPES[@]}"; do
    APP="${TYPE//-/_}"
    cp -f "dist/${APP}/app.bin" "${APP}.bin"
    md5sum "${APP}.bin"
done
