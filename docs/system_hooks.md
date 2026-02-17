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
  - P256
- Implementing system functionality needed for ZKsync operations:
  - L1 messenger system hook
  - Set bytecode on address system hook
  - Mint base token system hook (temporary, to be replaced)

## L1 messenger system hook

The L1 messenger system hook (at address `0x7001`) is responsible for sending messages to L1.
It can only be called by the L1 messenger system contract at address `0x8008`.
The input should be the ABI-encoded parameters: sender address and message bytes.

Implementation of the L1 messenger system hook decodes the input and records the message using the system method.

## Set bytecode on address system hook

The set bytecode on address system hook (at address `0x7002`) allows setting deployed EVM bytecode to any address.
It can only be called by the Contract Deployer system contract at address `0x8006`.

The hook accepts the following ABI-encoded parameters:
- `address` - target address to set bytecode on (32 bytes, ABI padded)
- `bytes32` - bytecode hash
- `uint256` - bytecode length
- `bytes32` - observable bytecode hash

Key features:
- Enforces EIP-158 by rejecting bytecode longer than 24576 bytes
- Used exclusively for protocol upgrades approved by governance
- Does not publish full bytecode in pubdata to fit within gas/calldata limits
- Bytecodes are published separately via Ethereum calldata

## Mint base token system hook

The mint base token system hook (at address `0x7100`) allows minting base tokens.
It can only be called by the L2 base token contract at address `0x800a`.
The calldata must be exactly 32 bytes containing the amount to mint (as uint256).
