# ZKsync OS ETH Runner

This module contains a tool to run real Ethereum mainnet blocks on top of ZKsync OS. **This tool is a WIP, it has known bugs.**

On a high-level, this tool works in the following way:

1. Creates the initial state on which to run the block. Note that, as ZKsync OS uses a different state tree than Ethereum, we have to create an equivalent tree. To avoid having to recreate the full Ethereum tree, we just take the projection described by the accounts/slots accessed during the block. This way, we can construct a minimal tree over which the block application is equivalent to that in Ethereum. Note that we randomize the positions in the tree that leaves are inserted into, to have a realistic Merkle proof cost.

2. It runs all of the block's transactions over the constructed pre-state.

3. It performs checks to ensure EVM compatibility:

    - Transaction success
    - Logs equality
    - Gas consumption (note, ZKsync OS does not support gas refunds, so we check equivalence up-to refunds).
    - Storage writes: we compare the state diff produced by ZKsync OS to that of Ethereum extensionally.

## How to run

The tool has two modes: `single-run` and `live-run`. The former takes as argument the block data in JSON format. The second one just takes an RPC endpoint for an archive node with the debug section enabled and fetches the traces directly. The latter also can run a given range of blocks.

### Single run

From the root of the project, run:

```raw
RUST_LOG=eth_runner=info cargo run -p eth_runner --release --features rig/no_print -- single-run --block-dir tests/instances/eth_runner/blocks/22244135 --randomized
```

This will run the example block committed to the repo (22244135). Some more example blocks can be found in https://github.com/antoniolocascio/ethereum-block-examples.

### Live run

From the root of the projects, run:

```raw
RUST_LOG=eth_runner=info cargo run -p eth_runner --release --features rig/no_print  -- live-run --start-block 19299000 --end-block 19299005 --endpoint ENDPOINT --db ../db
```

This command will fetch blocks in the range [19299000, 19299005] from the Ethereum archive node `ENDPOINT`. It creates a local database to cache some RPC information.

### Prover input generation

Both subcommands have an optional parameter `--witness-output-dir` that expects a directory to dump the witness for the block

### Ethproofs (prove a block with the `eth_stf` program)

The `ethproofs-*` subcommands fetch a mainnet block plus its execution witness
(`debug_executionWitness`) from a Reth node, record the ZKsync OS prover input for
it, prove it with `airbender-host` and optionally submit the proof to Ethproofs.
They need the `eth_stf` distribution. Post-BPO2 mainnet blocks (anything recent) use
the BPO2 blob schedule, so build the guest with `zksync_os/dump_bin.sh --type eth-stf-fusaka`
and the host side with `--features rig/eth_stf,fusaka-bpo-2`; otherwise blob transactions
fail validation (`BlobElementIsNotSupported`) and the transactions root diverges.

The prover backend is chosen at compile time: `--features gpu` for real GPU proofs,
`--features proving` for (very slow) CPU proofs, otherwise the transpiler-backed dev
prover (no real proof, useful to exercise the flow locally).

```raw
# record the prover input for one block (and optionally store it)
cargo run -p eth_runner --release --features rig/eth_stf,fusaka-bpo-2 -- ethproofs-run --block-number 25990253 --reth-endpoint ENDPOINT --witness-output-dir ../witnesses --dump-dir tests/instances/eth_runner/25990253
# prove a stored prover input
cargo run -p eth_runner --release --features gpu,rig/eth_stf,fusaka-bpo-2 -- prove-with-witness --witness-input ../witnesses/25990253_witness.bincode
# prove live blocks and submit them (every `block_mod`-th block, offset `prover_id`)
cargo run -p eth_runner --release --features gpu,rig/eth_stf,fusaka-bpo-2 -- ethproofs-with-proofs --reth-endpoint ENDPOINT --auth-token TOKEN --cluster-id ID --block-mod 100 --prover-id 0
# same, without submission
cargo run -p eth_runner --release --features gpu,rig/eth_stf,fusaka-bpo-2 -- ethproofs-with-proofs-no-submission --reth-endpoint ENDPOINT
```

### Profiling the Ethereum STF

`ethproofs-collect` stores consecutive blocks (walking down from the confirmed head and
skipping blocks the STF cannot replay yet) together with their execution witnesses and the
recorded prover input:

```raw
cargo run -p eth_runner --release --features rig/eth_stf,fusaka-bpo-2 -- ethproofs-collect --reth-endpoint ENDPOINT --count 16 --output-dir tests/instances/eth_runner/blocks
```

Build the STF with full debug info (`dist/eth_stf_debug`, a self-consistent bin/ELF pair;
see `zksync_os/debug_symbols.toml`) and profile a collected block on the transpiler:

```raw
(cd zksync_os && ./dump_bin.sh --type eth-stf-fusaka-debug)
cargo run -p eth_runner --release --features rig/eth_stf,fusaka-bpo-2 -- ethproofs-flamegraph --block-dir tests/instances/eth_runner/blocks/25990540 --output tests/instances/eth_runner/flamegraphs/25990540.svg --sampling-rate 100
# reverse stack order (leaf functions at the root)
cargo run -p eth_runner --release --features rig/eth_stf,fusaka-bpo-2 -- ethproofs-flamegraph --block-dir tests/instances/eth_runner/blocks/25990540 --output 25990540.inverse.svg --inverse
# top-down (inclusive) and bottom-up (self time) tables over one or many flamegraphs
python3 tests/instances/eth_runner/scripts/analyze_flamegraphs.py --top 30 tests/instances/eth_runner/flamegraphs/*.svg
```

The profiler replays the oracle responses recorded by the native run, so it only works while
the guest asks for exactly the same hints. A guest that does not (e.g. a build where ecrecover
ends up inverting another `z`) fails the hint check; pass `--live-oracle` to answer its
queries with the real oracle instead, and `--app` to pick its distribution.

`ethproofs-compare-oracles` runs the same block twice on the transpiler, first with the
real oracle (`ZkEENonDeterminismSource`, which parses the execution witness and answers
every guest query on the fly) and then with a replay source that serves the recorded word
stream and ignores guest writes, and reports the setup and execution time of each:

```raw
cargo run -p eth_runner --release --features rig/eth_stf,fusaka-bpo-2 -- ethproofs-compare-oracles --block-dir tests/instances/eth_runner/blocks/25990540 --runs 3
```
