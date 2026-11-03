#!/usr/bin/env bash
# Starts a single-validator Besu QBFT node on :8545, generating the network on first run.
# The generated network (networkFiles/, including the validator key) and the database (data/)
# are not tracked.
set -euo pipefail
cd "$(dirname "$0")"
IMAGE=hyperledger/besu:26.11.0

if [ ! -f networkFiles/genesis.json ]; then
  docker run --rm -v "$PWD":/poc $IMAGE operator generate-blockchain-config \
    --config-file=/poc/qbftConfigFile.json --to=/poc/networkFiles --private-key-file-name=key
  mkdir -p data && cp networkFiles/keys/*/key data/key
fi

docker rm -f besu-poc >/dev/null 2>&1 || true
docker run -d --name besu-poc -p 8545:8545 -v "$PWD":/poc $IMAGE \
  --data-path=/poc/data --genesis-file=/poc/networkFiles/genesis.json \
  --data-storage-format=BONSAI --bonsai-limit-trie-logs-enabled=false --sync-mode=FULL \
  --rpc-http-enabled --rpc-http-host=0.0.0.0 --rpc-http-api=ETH,NET,QBFT,DEBUG,TXPOOL,WEB3 \
  --host-allowlist='*' --min-gas-price=0
