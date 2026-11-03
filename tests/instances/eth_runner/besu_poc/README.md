# Besu (QBFT) execution proving PoC

Proves blocks of a local single-validator Besu QBFT chain with the Ethereum STF (`eth-stf`
binary). Consensus (QBFT seals) is not verified; the proof covers execution and state.

The chain runs every fork up to Prague from genesis, with a normal EIP-1559 base fee, no block
rewards and no blobs. Chain ID 2026.

## STF configuration

The Ethereum STF takes chain parameters at build time
(`basic_bootloader/src/bootloader/block_flow/ethereum/chain_config.rs`); defaults are mainnet.
Export these before building both `eth_runner` and `zksync_os/eth_stf.bin`:

```sh
export ZKSYNC_OS_ETH_CHAIN_ID=2026
export ZKSYNC_OS_ETH_PECTRA_BLOCK=0
export ZKSYNC_OS_ETH_MAX_EXTRA_DATA_LEN=1024   # QBFT extraData holds validators and seals
export ZKSYNC_OS_ETH_SYSTEM_CALLS=0            # Besu skips 4788/2935/7002/7251 on PoA chains
```

## Running

Requires Docker, foundry (`forge`, `cast`), `jq`, python3.

```sh
cd tests/instances/eth_runner/besu_poc
./start_besu.sh                       # generates the network on first run
(cd contracts && forge build)
./workload.sh                         # writes the block numbers to workload_blocks.txt
for b in $(cat workload_blocks.txt); do python3 -I make_witness.py $b blocks/$b; done

# build (with the env vars above)
(cd .. && cargo build --release --features rig/no_print,rig/unlimited_native,rig/eth_runner,rig/eth_stf)
(cd ../../../../zksync_os && ./dump_bin.sh --type eth-stf)

# forward + RISC-V run; the RISC-V output (8 LE u32 words) is the block hash
mkdir -p prover_inputs
OVERRIDE_ZKSYNC_OS_PATH=$(cd ../../../../zksync_os && pwd) RUST_LOG=info \
  ../target/release/eth_runner eth-run --block-dir blocks/107 \
  --witness-output-file prover_inputs/107_witness
```

`blocks/107` is a committed fixture (5 txs: legacy and EIP-1559 transfers, ERC20 transfers, a
call to `Probe.record()`), so the last step works without a node.

Proving uses the `tools/cli` of zksync-airbender v0.5.2, built with `nightly-2026-01-22`
(current nightly lacks `std::assert_matches`):

```sh
cli prove --bin zksync_os/eth_stf.bin --input-file prover_inputs/107_witness \
  --output-dir out --until final-recursion
cli verify-all --metadata out/metadata.json   # needs the include_verifiers feature
```

Block 107 is 4.7M cycles; base layer plus first recursion took 204 s on an M-series CPU with a
28 GB peak. The final layer (`--until final-proof`) needs 128 GB of RAM.

## Besu specifics

- QBFT block hash = keccak(header with the commit seals removed from extraData). The witness
  builder gives the STF seal-stripped headers, so the STF's keccak header hash equals the
  Besu block hash.
- Besu skips the EIP-4788/2935 system calls and the 7002/7251 request processors on PoA chains,
  hence `ZKSYNC_OS_ETH_SYSTEM_CALLS=0`.
- `debug_executionWitness` is Osaka-only in Besu (and needs Bonsai), so `make_witness.py`
  builds the witness from `prestateTracer`, `eth_getProof`, `eth_getCode` and
  `debug_getRawHeader`.
- COINBASE is the QBFT proposer and PREVRANDAO is QBFT's constant mixHash; both match the STF.

## Limitations

- The witness builder misses trie nodes needed when a deletion collapses a branch onto an
  untouched sibling (e.g. clearing one of two storage slots). The forward run then fails with
  a missing preimage; such blocks can't be proven until the builder fetches those nodes.
- One fork (Prague) only; no fork gating, `zeroBaseFee`/`fixedBaseFee`, block rewards or Clique.
- The STF doesn't check header `gas_used`/`blob_gas_used` or block-level gas/blob limits.
