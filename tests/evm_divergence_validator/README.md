# EVM Divergence Validator

CLI tool that checks whether a given scenario produces different results on zkSync OS vs standard EVM (REVM). Takes a JSON scenario describing Solidity contracts and transactions, compiles them, executes on both engines, and reports any state divergences.

## Use cases

- **Bug bounty triage** — quickly validate whether a reported scenario actually produces an EVM divergence, without writing a Rust test case.
- **AI audit feedback loop** — structured JSON output with exit codes enables automated agents to generate scenarios, run them, and iterate.

## Prerequisites

- [Foundry](https://getfoundry.sh/) (`forge`) must be on PATH for Solidity compilation.

## Usage

```bash
cargo run -p evm_divergence_validator -- scenario.json

# or build once and run directly
cargo build -p evm_divergence_validator --release
./target/release/evm-divergence-validator scenario.json
```

### Exit codes

| Code | Meaning |
|------|---------|
| 0 | Match — zkSync OS and REVM produce identical state |
| 1 | Divergence found |
| 2 | Error (bad input, compilation failure, etc.) |

## Scenario format

```json
{
  "contracts": {
    "MyContract": {
      "source": "// SPDX-License-Identifier: MIT\npragma solidity ^0.8.20;\ncontract MyContract { uint256 public x; function set(uint256 v) public { x = v; } }"
    },
    "Other": {
      "file": "contracts/Other.sol"
    }
  },
  "accounts": {
    "alice": { "balance": "1000000000000000000" }
  },
  "block": {
    "basefee": 1000,
    "gas_limit": 30000000,
    "timestamp": 1700000000
  },
  "steps": [
    {
      "type": "deploy",
      "contract": "MyContract",
      "from": "alice",
      "gas": 5000000
    },
    {
      "type": "call",
      "to": "$deployed:0",
      "from": "alice",
      "function": "set(uint256)",
      "args": [42],
      "gas": 1000000
    }
  ]
}
```

### Contracts

Each entry maps a contract name to its source. Two options:

| Field | Description |
|-------|-------------|
| `source` | Inline Solidity source code |
| `file` | Path to a `.sol` file, relative to the scenario JSON |

The contract name must match the Solidity `contract` name (forge uses it to locate the artifact).

### Accounts

Named accounts with a pre-funded `balance` (in wei, decimal or `0x`-prefixed hex). Any `from` name referenced in steps that isn't listed here gets a default balance sufficient for gas.

### Block (optional)

| Field | Default |
|-------|---------|
| `basefee` | 1000 |
| `gas_limit` | 30000000 |
| `timestamp` | 1700000000 |

### Steps

All steps execute as transactions within a single block. Two types:

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

The `to` field accepts:
- `"$deployed:N"` — address from the Nth deploy step (0-indexed)
- A named account from `accounts`
- A hex address like `"0x1234..."`

## Output

Structured JSON to stdout:

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

On divergence (`status: "divergence"`), the `error` field contains details about which storage slots or account fields differ between zkSync OS and REVM.

## What it checks

The tool uses the existing REVM consistency checker from `tests/revm_runner/`, which compares end-of-block state diffs:

- Storage slot changes (per address, per slot)
- Account nonce changes
- Account balance changes
- Deployed bytecode changes

## Limitations

- All steps run in a single block. Multi-block scenarios are not yet supported.
- Gas comparison is approximate due to zkSync OS double gas accounting (EVM gas + native resources).
- Events/logs and return data are not compared (only final state).
- The REVM side uses `zksync-os-revm` (adapted REVM), not vanilla REVM, so it accounts for known zkSync-specific behaviors (precompile differences, fee distribution, etc.).
