# EVM Execution Environment

As the name suggests, the goal of this EE implementation is to make ZKsyncOS EVM-compatible.
The implementation can be found in [evm_interpreter](../../evm_interpreter/).

The EVM version we support currently is Cancun.

## Current divergences

- Keyless transactions may not work, more generally, we have additional cost due to pubdata.
- Deployment doesn’t fail if the storage for the deployed address is already used (when nonce is 0 and code is empty).
- We use the L2Tx type to encode transactions, which encodes nonces as u32. This means that, in practice, violate EIP 2681, even if ZKsync OS internally doesn’t.
- When the block base fee is 0, then priority fee from transactions is ignored. That is, the gas price will also be 0 for every transaction.
- DIFFICULTY is mocked (returns 1), we don’t plan to support it
- Blake2F precompile is not supported
