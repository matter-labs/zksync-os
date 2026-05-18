# block_reexecutor

Re-executes a single L2 block against RPC state, validates transaction outputs against RPC receipts, and writes call traces.

## What it does

- Loads block data and metadata from RPC (or from disk cache).
- Executes transactions with `RpcValueOracleFactory`.
- Compares execution results against RPC receipts:
  - success/failure status
  - gas used
  - logs count/content
- Runs REVM on the same block context/state.
- Writes two trace files in geth call tracer format (`CallFrame[]`).

## Run

```bash
RUST_LOG=info cargo run -p block_reexecutor -- --endpoint <rpc> --block-hash <block_hash>
```

### Run With Predefined Transactions

Use state/block metadata from `--block-number`, but execute transactions loaded from a JSON file:

```bash
RUST_LOG=info cargo run -p block_reexecutor -- \
  --endpoint <rpc> \
  --block-number <block_number> \
  --transactions-file <path/to/transactions.json>
```

`transactions.json` must contain RLP tx payload + signer, with tx bytes encoded as hex string:

```json
[
  {
    "tx": "0x02f86e82853901843b...",
    "signer": "0x1111111111111111111111111111111111111111",
    "hash": "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
  }
]
```

Also accepted for compatibility:

- `{"rlp":"0x...", "signer":"0x...", "hash":"0x..."}`
- `{"Rlp":["0x...", "0x..."], "hash":"0x..."}`
- previous `EncodedTx` JSON with byte arrays

The tool decodes every RLP payload into an Ethereum transaction for REVM tracing and uses the same `EncodedTx::Rlp` values for block reexecution.
If `hash` is provided for predefined txs, execution results are validated against RPC receipts matched by hash.
If RPC returns `null` for any provided hash, the tool logs it and skips receipt checks.

## Output files

- `tracer_output_<block_number>.json`
- `revm_call_trace_<block_number>.json`

## Cache files

All cache files are stored under:

- `.cache/block_reexecutor/`

If you need a full refetch, delete the cache.
