#!/usr/bin/env bash
# Sends the PoC workload to the local Besu node. Uses the well-known Hardhat dev key 0
# (funded in genesis). Writes the resulting block numbers to workload_blocks.txt.
set -euo pipefail
RPC=${RPC:-http://localhost:8545}
PK=0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80
FROM=0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266
OUT=contracts/out
bc() { jq -r .bytecode.object "$OUT/$1.sol/$1.json"; }

nonce=$(cast nonce $FROM --rpc-url $RPC)
token_addr=$(cast compute-address $FROM --nonce $nonce | awk '{print $NF}')
probe_addr=$(cast compute-address $FROM --nonce $((nonce+1)) | awk '{print $NF}')

# Block A: two deployments
tok_init=$(bc Token)$(cast abi-encode "f(uint256)" 1000000000000000000000000 | sed 's/^0x//')
h1=$(cast send --async --rpc-url $RPC --private-key $PK --nonce $nonce --create "$tok_init")
h2=$(cast send --async --rpc-url $RPC --private-key $PK --nonce $((nonce+1)) --create "$(bc Probe)")
cast receipt --rpc-url $RPC $h2 >/dev/null
blockA=$(cast receipt --rpc-url $RPC $h1 blockNumber)

# Block B: legacy + 1559 ETH transfers, token transfers to new and existing holders, probe call
n=$((nonce+2))
hs=()
hs+=($(cast send --async --rpc-url $RPC --private-key $PK --nonce $n --legacy 0x1111111111111111111111111111111111111111 --value 1ether))
hs+=($(cast send --async --rpc-url $RPC --private-key $PK --nonce $((n+1)) 0x2222222222222222222222222222222222222222 --value 2ether))
hs+=($(cast send --async --rpc-url $RPC --private-key $PK --nonce $((n+2)) $token_addr "transfer(address,uint256)" 0x3333333333333333333333333333333333333333 1000))
hs+=($(cast send --async --rpc-url $RPC --private-key $PK --nonce $((n+3)) $token_addr "transfer(address,uint256)" 0x3333333333333333333333333333333333333333 500))
hs+=($(cast send --async --rpc-url $RPC --private-key $PK --nonce $((n+4)) $probe_addr "record()" --gas-limit 500000))
for h in "${hs[@]}"; do cast receipt --rpc-url $RPC $h >/dev/null; done
blocksB=$(for h in "${hs[@]}"; do cast receipt --rpc-url $RPC $h blockNumber; done | sort -u | tr '\n' ' ')

# Block C: probe again (overwrites existing non-zero slots, reads BLOCKHASH of the previous block)
h=$(cast send --async --rpc-url $RPC --private-key $PK --nonce $((n+5)) $probe_addr "record()" --gas-limit 500000)
blockC=$(cast receipt --rpc-url $RPC $h blockNumber)

echo "token=$token_addr probe=$probe_addr"
echo "$blockA $blocksB $blockC" | tr ' ' '\n' | grep -v '^$' | sort -nu | tee workload_blocks.txt
