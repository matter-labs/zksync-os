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

### Glamsterdam fixtures

Glamsterdam changes the block format (EIP-7928 block-level access lists, EIP-7843
slot number in the header), so its test data lives apart from the pre-Glamsterdam
blocks, under `glamsterdam_blocks/<block>/`, and has its own entry points. Each
fixture holds the raw JSON-RPC responses of a Geth node, written verbatim:
`block.json` (`eth_getBlockByNumber` with transactions), `witness.json`
(`debug_executionWitness`), `block_access_list.json` (`eth_getBlockAccessList`),
`raw_header.json` (`debug_getRawHeader`), `raw_block.json` (`debug_getRawBlock`) and
`raw_receipts.json` (`debug_getRawReceipts`).

```raw
# fetch 10 consecutive finalized blocks from a Geth node with the debug namespace
cargo run -p eth_runner --release --features rig/no_print -- glamsterdam-fetch --endpoint ENDPOINT --first-block 11861541 --count 10
# re-check the stored fixtures: header RLP and hash, block access list hash,
# access indices, parent header in the witness, transactions and receipts roots
cargo run -p eth_runner --release --features rig/no_print -- glamsterdam-check
```

The `glamsterdam_fixtures_are_consistent` test runs the same checks over every
stored fixture.

The Glamsterdam state consistency check
(`basic_bootloader::bootloader::block_flow::ethereum_glamsterdam::state_consistency`)
witnesses the header, its parent and the block access list, applies the access list
to the parent state with the stack MPT and compares the root with the header's,
without executing a transaction. It runs natively over the fixtures with:

```raw
cargo run -p eth_runner --release --features rig/no_print -- glamsterdam-state-consistency
```

(also the `glamsterdam_state_consistency_holds` test). The report ends with the byte
sizes of the initial account and slot value streams: the pre-block leaves as they come
out of the trie, each as `length byte || RLP item` (length zero for a missing leaf),
by address index and by slot position (the ranges witness these tables, linked by their hashes).

`glamsterdam-state-consistency-cycles` runs the same check on the RISC-V simulator with
the `glamsterdam_pre_execution` program (`binaries/build.sh`, see `binaries/README.md`;
`--app` selects another dist), checks that the program returns the hash of its linkage
output (`ethereum_glamsterdam::linkage::ProgramOutput` with a clear access mask) and
prints the cycles per block. The program builds for the standard
`riscv32i-unknown-none-elf` target without the M extension.

```raw
CARGO_WORKSPACE_DIR=$PWD/ cargo run -p eth_runner --release --features rig/no_print -- glamsterdam-state-consistency-cycles
```

A witness experiment (the access list as RLP bytes parsed lazily, versus word-aligned
entries RLP-encoded into the hasher while read) ran the whole check both ways on the
ten Sepolia fixtures: 463.8M against 464.1M cycles in total, within noise, with the
structured witness twice the size, so the RLP witness is the one used.

#### Transaction ranges

The second part of the Glamsterdam STF runs a range of transactions `start..end`
(transaction numbers, half-open) with the state the access list declares, checking every
transaction against its receipt and its state changes against the list (see
`basic_bootloader::bootloader::block_flow::ethereum_glamsterdam::tx_range`). A range
starting at 0 also runs the pre-transaction system calls (access index 0), a range ending
at the transaction count also runs the post-transaction operations (the withdrawals and
the request system calls, access index n+1): the phases go with the first and the last
transaction, so no range can cover the last transaction without them. The transaction
count is a witness of the range program; the pre-execution check proves it against the
transactions and receipts tries and the aggregation binds every range to it (every
lookup in the tries also applies a density rule with that count, see
`docs/ethereum_index_tries.md`). The transactions run on the third-generation EVM
interpreter (`evm_interpreter::v3`) with the two-dimensional gas of EIP-8037.

```raw
cargo run -p eth_runner --release --features rig/no_print -- glamsterdam-range
```

runs every fixture as one whole-block range on the host; `--parts N` splits every block
into N ranges of roughly equal cumulative gas (the receipts' running total) and
aggregates their outputs (`tx_range::aggregation`): the ranges must tile the block's
transactions in order with the proven count, run on the pre-execution check's header and
initial value tables (by hash), link their gas sums, the union of the accounts and slots
they accessed must be the whole access list, so that a list with entries nothing
accessed is rejected. The header's requests hash is checked by the last range itself.
Of the request types only the EIP-6110 deposit requests come out of the logs (the
withdrawal, consolidation and builder requests are dequeued from their system contracts'
storage by the post-transaction system calls, which that range runs), so their hashing is
carried across the ranges as a sha256 state (`tx_range::requests::Sha256Accumulator`,
the hasher's words and buffer as 104 bytes): a range witnesses the state after the
transactions before it (unverified, like its start gas; the first range starts from the
state after the type byte), absorbs the deposit events of its own transactions once they
ran, and publishes both states. The last range finishes the hash, adds the system
request hashes and compares with the header. The programs' public outputs
(`linkage::ProgramOutput`) carry the transaction count, the transaction marks
`tx_start_mark..tx_end_mark` (`0..count` for the pre-execution check) and the two hasher
states (all zero for the pre-execution check), so the aggregation program sees that the
states chain from the head through every range and that the range ending the block, the
one that checked the requests hash, is the last one. The deposit contract is a chain
parameter (`deposit_contract_address(chain_id)`: mainnet, Sepolia, Holesky) (also the
`whole_block_ranges_run_the_fixtures`, `split_ranges_agree_with_the_whole_block` and
`subranges_aggregate_to_the_block` tests).

`glamsterdam-self-consistency [--parts N]` is the self-consistency check of the two
programs over the stored fixtures: the pre-execution check and every range run natively
first with the oracle's responses recorded as the RISC-V guest reads them, then on the
simulator from the recorded words alone, and each program's public output must be the
hash of the linkage output of its native run; the ranges run from the initial value
tables of the one native pre-execution run and all outputs are aggregated.
`glamsterdam-follow --endpoint URL [--first-block N] [--count N] [--parts N] [--keep]`
is its long-running form over live Sepolia: it fetches every finalized block as it comes
into `glamsterdam_follow/<block>/`, checks it, and keeps going. A failing or panicking
block does not stop it: its fixture is kept with `failure.txt`, `initial_values.bin` and
`ranges.txt` (one `start..end` transaction range per line) next to it for a replay, and
its number goes to `diverging_blocks.txt`; the
cycles and gas of every block and range are appended to `blocks.csv` and `ranges.csv`
in the work directory, with what the block exercised (blob and set-code transactions,
creations, withdrawals, deposit requests, the non-empty system request types) as the
last columns of `blocks.csv`. Both commands print one line per stage of every block
(fetch, native run and simulator run of the pre-execution check and of each range, with
the cycles, the recorded words and the timings, then the aggregation), a `features:` line
and a summary line per block, and the follower's `progress:` line carries the running
coverage (how many blocks had blobs, deposit requests, each request type, ...), so a run
is followed from its log:

```bash
cargo run -p eth_runner --release --features rig/no_print -- glamsterdam-follow --endpoint "$GLAMSTERDAM_ENDPOINT" --parts 3 2>&1 | tee glamsterdam_follow.log
```

The ignored tests `fixtures_are_self_consistent` and `sepolia_blocks_are_self_consistent`
(endpoint from `GLAMSTERDAM_ENDPOINT`, block count from `GLAMSTERDAM_BLOCKS`) run the
same two checks.

Two findings of the first live runs are worth knowing: the transactions and receipts tries
are rebuilt from the RPC data before a block runs and must reproduce the header's roots
(the header is the trusted value, the rebuilt nodes only serve the trie descents), which
caught an RLP length bug for items of 64 KiB and more; and a failed creation transaction
refilled the NEW_ACCOUNT state gas of its target twice (the frame's restore repays the
spill, and the root frame credited it again), caught by a reverted and by a halted
creation. The chain matches the EIP-8037 text: the charge is refilled on a revert and
burned with the execution gas on a halt.

`glamsterdam-range-cycles` does the same on
the RISC-V simulator with the `glamsterdam_tx_range` program (`binaries/build.sh`, see
`binaries/README.md`), also with `--parts`, checks that every program returns the hash
of its linkage output (`ethereum_glamsterdam::linkage::ProgramOutput`, the public
output every Glamsterdam program publishes and the aggregation links), and prints the
cycles per range and per block.
`glamsterdam-trace-diff --block-dir DIR --tx N --trace FILE` diffs our EVM steps of one
transaction (depth, program counter, gas) against the node's `debug_traceTransaction`
struct logs saved in `FILE`, to find where an execution diverges.
