# EVM Divergence Validator

CLI tool that checks whether a given scenario produces different results on ZKsync OS vs standard EVM (REVM). Takes a YAML (or JSON) scenario describing Solidity contracts and transactions, compiles them, executes on both engines, and reports any state divergences.

## Use cases

- **Bug bounty triage** — quickly validate whether a reported scenario actually produces an EVM divergence, without writing a Rust test case.
- **AI audit feedback loop** — structured JSON output with exit codes enables automated agents to generate scenarios, run them, and iterate.

## Prerequisites

- [Foundry](https://getfoundry.sh/) (`forge`) must be on PATH for Solidity compilation (not needed for `send_raw` steps with pre-compiled bytecode).
- The validator must run with the P-256 precompile enabled. This crate enables `forward_system/production` in `Cargo.toml`, so the standard commands below include it. Do not disable this feature when preparing a bug bounty PoC.

## Usage

```bash
# Build and run from the crate directory
cd tests/evm_divergence_validator
cargo run -- path/to/scenario.yaml

# or build once and run directly
cargo build --release
./target/release/evm-divergence-validator scenario.yaml
```

Note: this crate is excluded from the workspace (to avoid feature leakage), so `cargo run -p` from the repo root won't work. Run from its own directory instead.

Accepts `.yaml`/`.yml` (recommended) and `.json` files.

### Exit codes

| Code | Meaning |
|------|---------|
| 0 | Match — ZKsync OS and REVM produce identical state |
| 1 | Divergence found |
| 2 | Error (bad input, compilation failure, etc.) |

## Scenario format

```yaml
contracts:
  MyContract:
    source: |
      // SPDX-License-Identifier: MIT
      pragma solidity ^0.8.20;

      contract MyContract {
          uint256 public x;
          function set(uint256 v) public { x = v; }
      }
  Other:
    file: contracts/Other.sol

accounts:
  alice:
    balance: "1000000000000000000"

block:
  basefee: 1000
  gas_limit: 30000000
  timestamp: 1700000000

steps:
  - type: deploy
    contract: MyContract
    from: alice
    gas: 5000000

  - type: call
    to: "$deployed:0"
    from: alice
    function: "set(uint256)"
    args: [42]
    gas: 1000000
```

### Contracts

Each entry maps a contract name to its definition. Three modes:

| Fields | Description |
|--------|-------------|
| `source` | Inline Solidity source code (compiled via `forge build`) |
| `file` | Path to a `.sol` file, relative to the scenario file (compiled via `forge build`) |
| `bytecode` + `address` | Raw runtime bytecode predeployed at the given address (no compilation) |

For Solidity contracts, the name must match the Solidity `contract` name (forge uses it to locate the artifact).

Predeployed bytecode example:

```yaml
contracts:
  Store:
    bytecode: "0x60003560005500"
    address: "0x0000000000000000000000000000000000000101"
```

Steps can reference predeployed contracts by name in the `to` field (e.g. `to: Store`).

The `contracts` section can be omitted entirely when using only `send_raw` steps.

### Accounts

Named accounts with a pre-funded `balance` (in wei, decimal or `0x`-prefixed hex). Any `from` name referenced in steps that isn't listed here gets a default balance sufficient for gas.

### Block (optional)

| Field | Default |
|-------|---------|
| `basefee` | 1000 |
| `gas_limit` | 30000000 |
| `timestamp` | 1700000000 |

### Steps

All steps execute as transactions within a single block. Three types:

**deploy** — deploys a compiled contract via CREATE.

| Field | Required | Description |
|-------|----------|-------------|
| `contract` | yes | Name from the `contracts` map |
| `from` | yes | Sender account name |
| `args` | no | Constructor arguments as JSON values |
| `gas` | no | Gas limit (default: 5000000) |
| `value` | no | ETH value in wei |

**call** — calls a deployed contract.

| Field | Required | Description |
|-------|----------|-------------|
| `to` | yes | Target (see below) |
| `from` | yes | Sender account name |
| `function` | yes | Solidity function signature, e.g. `"transfer(address,uint256)"` |
| `args` | no | Function arguments as JSON values |
| `gas` | no | Gas limit (default: 5000000) |
| `value` | no | ETH value in wei |

**send_raw** — sends raw bytecode/calldata directly. Use for cases unreachable via Solidity.

| Field | Required | Description |
|-------|----------|-------------|
| `to` | no | Target (omit for CREATE) |
| `from` | yes | Sender account name |
| `data` | no | Raw hex data (deployment bytecode or calldata) |
| `gas` | no | Gas limit (default: 5000000) |
| `value` | no | ETH value in wei |

The `to` field accepts:
- `"$deployed:N"` — address from the Nth deploy/send_raw(CREATE) step (0-indexed)
- A named account from `accounts`
- A hex address like `"0x1234..."`

## Output

Structured JSON to stdout. Status is one of `"match"`, `"divergence"`, or `"error"`.

```json
{
  "status": "match",
  "steps": [
    {
      "description": "deploy MyContract -> 0xF65D...",
      "success": true,
      "gas_used": 132939
    },
    {
      "description": "call set(uint256) on 0xF65D...",
      "success": true,
      "gas_used": 43718
    }
  ]
}
```

On divergence, the `error` field contains details about which storage slots, account fields, return data, or event logs differ.

## What it checks

The tool uses the existing REVM consistency checker from `tests/revm_runner/`, which compares:

**End-of-block state diffs:**
- Storage slot changes (per address, per slot)
- Account nonce changes
- Account balance changes
- Deployed bytecode changes

**Per-transaction execution results** (when `independent_gas` is enabled):
- Success/revert outcome
- Return data
- Event logs (address, topics, data)

## Design notes

- All steps run in a single block. Multi-block scenarios are not yet supported.
- The validator preinstalls the L1 Messenger and L2 Base Token bytecode at their canonical ZKsync OS system addresses before applying scenario-specific state.
- The tool enables `unlimited_native` and `independent_gas`, so ZKsync OS gas accounting follows standard EVM rules and is not overridden from ZKsync OS to REVM. The validator reports `gas_used` per step, but does not treat per-transaction gas differences as a separate divergence check — gas differences surface through balance diffs in the state comparison.
- The REVM side uses `zksync-os-revm` (adapted REVM), which accounts for ZKsync-specific behaviors (precompile differences, fee distribution, etc.). This is intentional — divergences caught here are real bugs, not known differences.
