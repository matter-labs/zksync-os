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
RUST_LOG=eth_runner=info cargo run -p eth_runner --release --features rig/no_print,rig/unlimited_native -- single-run --block-dir tests/instances/eth_runner/blocks/22244135 --randomized
```

This will run the example block committed to the repo (22244135). Some more example blocks can be found in https://github.com/antoniolocascio/ethereum-block-examples.

### Live run

From the root of the projects, run:

```raw
RUST_LOG=eth_runner=info cargo run -p eth_runner --release --features rig/no_print,rig/unlimited_native  -- live-run --start-block 19299000 --end-block 19299005 --endpoint ENDPOINT --db ../db
```

This command will fetch blocks in the range [19299000, 19299005] from the Ethereum archive node `ENDPOINT`. It creates a local database to cache some RPC information.

### Prover input generation

Both subcommands have an optional parameter `--witness-output-dir` that expects a directory to dump the witness for the block
