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

# Map app_name -> output filename for local copy
declare -A APPS=(
    [for_tests]="for_tests.bin"
    [evm_replay]="evm_replay.bin"
    [singleblock_batch]="singleblock_batch.bin"
    [singleblock_batch_logging_enabled]="singleblock_batch_logging_enabled.bin"
    [multiblock_batch]="multiblock_batch.bin"
    [multiblock_batch_logging_enabled]="multiblock_batch_logging_enabled.bin"
)

for APP_NAME in "${!APPS[@]}"; do
    FILE="${APPS[$APP_NAME]}"
    docker cp "$cid":/zksync_os/zksync_os/dist/"${APP_NAME}"/app.bin zksync_os/"$FILE"
    md5sum "zksync_os/$FILE"
done


docker rm -f "$cid" >/dev/null
