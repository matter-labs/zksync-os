#!/bin/bash

# Make sure to run from the main zksync-os directory.

set -e  # Exit on any error

# create a fresh docker
docker build -t zksync-os-bin  -f zksync_os/reproduce/Dockerfile .

docker create --name zksync-os-bin zksync-os-bin

FILES=(
    app.bin
)

for FILE in "${FILES[@]}"; do
    docker cp zksync-os-bin:/zksync_os/zksync_os/$FILE zksync_os/
    md5sum zksync_os/$FILE
done


docker rm zksync-os-bin