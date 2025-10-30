# Double resource accounting

As introduced in the [Overview](./overview.md), ZKsyncOS implements a double resource accounting model, where both an Execution Environment resource (EVM gas, for instance) and native resource are tracked.

The interface to interact with resources can be found in [resources.rs](../zk_ee/src/system/resources.rs).

## Execution Environment resource: Ergs

The EE resource is called Ergs, and corresponds to a model for computation performed by Execution Environments. The current ZKsyncOS design assumes every EE will use essentially the same resource for this, but potentially with a different conversion rate. For example, a unit of EVM gas is currently equivalent to 256 ergs, while a unit of EraVM gas might use a smaller constant for this conversion.

The gas-related fields of ZKsyncOS transactions are interpreted as referring to EVM gas. In general, any reference to the word "gas" in the codebase or docs refer to EVM gas.

Ergs passed to a new EE frame during a call/deployment can be limited as in EVM, in which case the caller will keep the rest of the ergs available.

## Native resource

The native resource models the offchain cost of processing a transaction. Currently, this is dominated by proving and publishing data. A good intuition for it is "how many RISC-V cycles it takes to prove a given computation".

If a transaction execution runs out of native resources, the entire transaction is reverted. If the same happens during transaction validation, the transaction is considered invalid.

The native resources are passed fully from frame to frame, a call cannot set a limit on how much of it the callee can spend.

### Native resource limit

In order to avoid having to use a new custom transaction format, the native resource limit (or native limit) is derived from the transaction's gas limit and fee. That is, the native limit can be incremented by either increasing the gas limit or the gas price of the transaction (using priority fee). The idea of using both these fields is to allow users re-use presigned transactions with a fixed EVM-computed gas limit.

However, if a wallet uses the standard `eth_estimateGas` RPC, the gas estimation will assume no priority fee is added and will add enough gas to make sure the transaction has enough native resource to go through.

More precisely, the logic for native resource limit derivation and charging is:

Let:

- `gasPrice` be the transaction's gas price (base fee + priority fee).
- `nativePrice` be a constant set by the operator, reflecting the "cost of processing a single cycle". Note: for L1->L2 transactions we use a code constant instead of one provided by operator.
- `gasLimit` be the transaction's (EVM) gas limit.

First we define the ratio between EVM gas and native resource as:
  `nativePerGas := gasPrice/nativePrice`
Note: for call simulation we use a constant for it, as gasPrice might be set to 0.

Next we define the limit for the native resource as:
  `nativeLimit := gasLimit * nativePerGas`

Then we process the transaction, charging both Ergs for EE execution and native resource for any kind of computation (EE, bootloader or system work).

If execution doesn't run out of native resources, we first charge for pubdata from native resource.
Then we compute the difference between the implicit gas used derived from native resource consumption and the gas used by EEs from the ergs used. We call this value `deltaGas`.
  `deltaGas := (nativeUsed / nativePerGas) - gasUsed`

If `deltaGas > 0`, we add it to `gasUsed` and charge it from ergs. This ensures that gas estimation will include additional gas to cover for native resources using just base fee. We expect the base fee to be enough to cover most transactions without the need of additional gas.

Finally, any remaining gas left is refunded as usual.
