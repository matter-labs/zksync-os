# System hooks

System hooks are special functions that can be triggered by a call on a specific system address. The space for this special addresses is specified in the [bootloader](./bootloader/bootloader.md) configuration.

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
- Implementing system contracts: formal contracts that implement some system functionality, like Era's nonce holder. Needed to support EraVM.
  - L1 messenger system hook
  - L2 Base token system hook
  - Contract deployer system hook

## L1 messenger system hook

The L1 messenger system hook is responsible for sending messages to l1.
Users can call it using the special interface, input should be encoded as calldata for the `sendToL1(bytes)` method following solidity abi.

Implementation of the l1 messenger system hook does 2 things: decodes the input and records the message using the system method.

## L2 base token system hook

The l2 base token system implements only 2 methods: `withdraw(address)`, `withdrawWithMessage(address,bytes)`.

They needed to support Era VM like base token withdrawals.

## Contract deployer system hook

The contract deployer system hook implements only 1 method: `setBytecodeDetailsEVM(address,bytes32,uint32,bytes32)`.
It allows to set any deployed EVM bytecode to any address but can be called only by the special system address.

It accepts bytecode hash, bytecode length, and observable bytecode hash.
Please note that full bytecode will not be published in the pubdata.
We want to be able to perform upgrade with 1 tx, so we designed this method this way(by hashes + without pubdata) to fit into gas/calldata/pubdata limits.

It will be used only by protocol upgrade transactions, which are approved by governance.
Bytecodes will be published separately with Ethereum calldata.
