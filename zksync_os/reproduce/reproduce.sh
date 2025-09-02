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

FILES=(
    app.bin
    evm_replay.bin
    server_app.bin
    server_app_logging_enabled.bin
    multiblock_batch.bin
    multiblock_batch_logging_enabled.bin
)

for FILE in "${FILES[@]}"; do
    docker cp "$cid":/zksync_os/zksync_os/"$FILE" zksync_os/
    md5sum "zksync_os/$FILE"
done


docker rm -f "$cid" >/dev/null
