# EVM Execution Environment

As the name suggests, the goal of this EE implementation is to make ZKsyncOS EVM-compatible.
The implementation can be found in [evm_interpreter](../../evm_interpreter/).

The EVM version we support currently is Osaka (Fusaka).

## Current divergences

- Keyless transactions may not work, more generally, we have additional cost due to pubdata.
- Deployment doesn’t fail if the storage for the deployed address is already used (when nonce is 0 and code is empty).
- When the block base fee is 0, then priority fee from transactions is ignored. That is, the gas price will also be 0 for every transaction.
- DIFFICULTY is mocked (returns 1), we don’t plan to support it
- EIP-4844 blob transactions (type 3) are not enabled in production. BLOBHASH always returns 0 (no blob hashes available). BLOBBASEFEE returns the value from block metadata.
- The EIP-7825 per-transaction gas cap is a chain-config parameter (`ChainConfig::max_tx_gas_limit`). The default matches the Fusaka value (`2^24`), so the default is non-divergent, but a chain may raise it above Ethereum's limit (it cannot be configured below).
- ZKsync OS is an L2 with no beacon chain or validator set, so Ethereum's
  consensus-layer block operations are not performed in production (they exist
  only in the Ethereum-equivalence test path):
  - Block withdrawals (EIP-4895) are not applied.
  - The parent beacon block root (EIP-4788) is not stored.
  - End-of-block execution-layer requests are not processed: withdrawal
    requests (EIP-7002), consolidation requests (EIP-7251), and deposit
    requests (EIP-6110).
