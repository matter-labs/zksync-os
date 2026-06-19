# System hooks

System hooks are special functions that can be triggered by a call on a specific system address. The space for these special addresses is specified in the [bootloader](./bootloader/bootloader.md) configuration.

System hooks have two distinct use cases:

- Implementing precompiles à la EVM. We currently support the following precompiles at their EVM addresses:
  - ecrecover
  - sha256
  - ripemd-160
  - identity
  - modexp
  - ecadd
  - ecmul
  - ecpairing
  - blake2f
  - point evaluation (KZG, EIP-4844)
  - BLS12-381 (EIP-2537):
    - G1 addition
    - G2 addition
    - G1 multi-scalar multiplication
    - G2 multi-scalar multiplication
    - pairing check
    - map field element to G1
    - map field element to G2
  - P256
- Implementing Gateway-only precompiles:
  - FRI proof verification (`0x7003`, behind `fri_precompile`, disabled in default builds) — see the [FRI precompile design](./fri_precompile.md)
- Implementing system functionality needed for ZKsync operations:
  - L1 messenger system hook
  - Set bytecode on address system hook
  - Contract deployer system hook (temporary for backward compatibility)
  - Mint base token system hook (used only for system-level mints)

## L1 messenger system hook

The L1 messenger system hook (at address `0x7001`) is responsible for sending messages to L1.
It can only be called by the L1 messenger system contract at address `0x8008`.
The input should be the ABI-encoded parameters: sender address and message bytes.

Implementation of the L1 messenger system hook decodes the input and records the message using the system method.
Calls from any other caller are treated as calls to an empty account: success with empty returndata and no side effects.

## Set bytecode on address system hook

The set bytecode on address system hook (at address `0x7002`) allows setting deployed EVM bytecode to any address.
It can only be called by the Contract Deployer system contract at address `0x8006`
or directly by the ComplexUpgrader system contract at address `0x800f`.

The hook accepts the following ABI-encoded parameters:
- `address` - target address to set bytecode on (32 bytes, ABI padded)
- `bytes32` - bytecode hash
- `uint256` - bytecode length
- `bytes32` - observable bytecode hash

Key features:
- Enforces EIP-170 by rejecting bytecode longer than 24576 bytes
- Used exclusively for protocol upgrades approved by governance
- Does not publish full bytecode in pubdata to fit within gas/calldata limits
- Bytecodes are published separately via Ethereum calldata
- Calls from unauthorized callers are treated as calls to an empty account: success with empty returndata, no writes, and no EVM gas burn.

## Mint base token system hook

The mint base token system hook (at address `0x7100`) allows minting base tokens.
It can only be called by the L2 base token contract at address `0x800a`.
The calldata must be exactly 32 bytes containing the amount to mint (as uint256).

## Contract deployer system hook

This hook is temporary needed for backward compatibility to not break existing upgrade flow.

The contract deployer system hook is installed on the ContractDeployer address `0x8006`.
It implements only 1 method: `setBytecodeDetailsEVM(address,bytes32,uint32,bytes32)`.
It allows setting any deployed EVM bytecode to any address, but only when called by the ComplexUpgrader system address `0x800f`.

It accepts bytecode hash, bytecode length, and observable bytecode hash.
Please note that full bytecode will not be published in the pubdata.
We want to be able to perform upgrade with 1 tx, so we designed this method this way (by hashes + without pubdata) to fit into gas/calldata/pubdata limits.

It will be used only by protocol upgrade transactions, which are approved by governance.
Bytecodes will be published separately with Ethereum calldata.
Calls from unauthorized callers are treated as calls to an empty account: success with empty returndata, no writes, and no EVM gas burn.

## FRI precompile (Gateway-only)

The FRI precompile (at address `0x0000000000000000000000000000000000007003`)
lets contracts ask whether a specific `statement_versioned_hash` is in the
**current transaction's verified-statements list**, which is populated
during `FriProofTx` validation.

The precompile is a pure membership check on tx-scoped state. It does
not itself run the FRI verifier and it does not re-derive any hash —
the verification happens in the server and during sequencing, only transactions
with valid FRI proofs are sequenced, so the precompile checks if the statement 
versioned hash was supplied in the transaction.

Support for this hook is compiled only when the default-off Cargo
feature `fri_precompile` is enabled. Production and audit builds are
expected to leave that feature disabled.

### Registration

- If `fri_precompile` is disabled, `add_fri_proof_verification_hook`
  is a no-op and the address is unregistered.
- If `fri_precompile` is enabled, the hook is registered only when
  `system.get_chain_config().fri_proof_verification_enabled() == true`.
- When unregistered, the address behaves like an empty account
  (success with empty returndata, no side effects, no EVM gas burn).

### Interface

- **Calldata:** exactly **32 bytes** containing the
  `statement_versioned_hash`.
- **Value:** must be zero. A non-zero `value` returns failure.
- **Bad length:** any calldata length other than 32 returns failure.
- **Output:** 32-byte ABI-encoded `bool` — `0x00..01` if the hash is in
  the current tx-scoped list, `0x00..00` if it is not. Missing sidecar
  data or verifier rejection is handled before EVM execution in
  admission and proving paths; those cases reject the tx rather than
  making the precompile return `false`.

### Lifecycle

The verified-hash list is populated by the bootloader's `FriProofTx`
validator before EVM execution begins and cleared at tx end. The
precompile is the only way for EVM code to observe it.

See [FRI precompile design](./fri_precompile.md) for the full flow.
