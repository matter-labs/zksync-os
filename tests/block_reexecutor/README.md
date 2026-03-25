# Block Re-executor

CLI tool for re-executing historical blocks and simulating transactions against real chain state via an external RPC endpoint (e.g. zksync-os-server).

## Prerequisites

Build the workspace from the repository root:

```bash
cargo build -p block_reexecutor
```

A running RPC endpoint is required (defaults to `http://localhost:8545`).

## Usage

The tool has two subcommands: **replay** and **simulate**.

### `replay` — Re-execute a block's transactions

Fetches a block from the RPC, re-executes all its transactions via the forward system, and verifies outputs against RPC receipts.

```bash
# By block hash (recommended — unambiguous)
cargo run -p block_reexecutor -- replay --block-hash 0xabc123...

# By block number (resolves to hash via canonical chain history)
cargo run -p block_reexecutor -- replay --block-number 12345

# Custom endpoint
cargo run -p block_reexecutor -- replay --endpoint http://my-node:8545 --block-hash 0xabc123...
```

### `simulate` — Simulate predefined transactions against a block's state

Uses a block's pre-state as the starting point and executes transactions from a JSON file instead of from the block itself. Useful for testing arbitrary transactions against real historical state.

```bash
cargo run -p block_reexecutor -- simulate --block-hash 0xabc123... --transactions-file txs.json

# Also works with block number
cargo run -p block_reexecutor -- simulate --block-number 12345 --transactions-file txs.json
```

### Block identification

- `--block-hash` is the recommended way to identify blocks — it is unambiguous and reorg-safe.
- `--block-number` resolves to a hash via RPC using canonical chain history. A warning is printed since the same block number may refer to different blocks after a reorg.

Exactly one of `--block-hash` or `--block-number` must be provided.

## Transactions file format

The `simulate` subcommand accepts a JSON file containing an array of transactions. Several formats are supported:

```json
[
  { "rlp": "0x02f8...", "signer": "0xAbC...", "hash": "0xDEF..." },
  { "tx": "0x02f8...", "signer": "0xAbC..." },
  { "Rlp": ["0x02f8...", "0xAbC..."], "hash": "0xDEF..." }
]
```

Fields:
- `rlp` / `tx` / `Rlp`: RLP-encoded transaction bytes (hex string or `[bytes, signer]` tuple)
- `signer`: transaction signer address
- `hash` (optional): expected transaction hash for receipt verification against RPC

## Supported transaction types

The tool supports all ZKsync OS transaction types:

| Type | ID | Description |
|------|----|-------------|
| Legacy / EIP-2930 / EIP-1559 / EIP-4844 | 0x00–0x03 | Standard Ethereum transactions |
| L1 | 0x7f | L1-originated transactions |
| Upgrade | 0x7e | Protocol upgrade transactions |
| Service | 0x7d | System-internal service transactions |

## Caching

The tool caches fetched data under `.cache/block_reexecutor/` to speed up repeated runs:

- **Block parameters** (block data, metadata, receipts, historical block hashes)
- **Oracle data** (storage slot values, preimage hashes)

Stale cache entries are automatically detected and refetched.

## Output

Each run produces:
- `tracer_output_<block_number>.json` — call trace from forward system execution
- `revm_call_trace_<block_number>.json` — call trace from REVM execution

In `replay` mode, transaction outputs are verified against RPC receipts (status, gas used, logs).
