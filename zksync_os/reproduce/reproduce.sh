#!/bin/bash

# Make sure to run from the main zksync-os directory.

set -euo pipefail

# Set source date epoch for reproducible builds
SDE="$(git log -1 --format=%ct || echo 1700000000)"

# create a fresh docker
docker build \
  --build-arg SOURCE_DATE_EPOCH="$SDE" \
  --platform linux/amd64 \
  -t zksync-os-bin \
  -f zksync_os/reproduce/Dockerfile .

cid="$(docker create --platform=linux/amd64 zksync-os-bin)"

# Map of dist/<app>/app.bin -> flat output name for backwards compatibility
declare -A APPS=(
    [for_tests]=for_tests.bin
    [evm_replay]=evm_replay.bin
    [singleblock_batch]=singleblock_batch.bin
    [singleblock_batch_logging_enabled]=singleblock_batch_logging_enabled.bin
    [multiblock_batch]=multiblock_batch.bin
    [multiblock_batch_logging_enabled]=multiblock_batch_logging_enabled.bin
)

for APP in "${!APPS[@]}"; do
    OUT="${APPS[$APP]}"
    docker cp "$cid":/zksync_os/zksync_os/dist/"$APP"/app.bin zksync_os/"$OUT"
    md5sum "zksync_os/$OUT"
done

docker rm -f "$cid" >/dev/null
