# Contract code size

`ChainConfig.large_contracts_enabled` selects the EVM code limits for a batch:

| Setting | Deployed code | Initcode, including constructor arguments |
| --- | --- | --- |
| `false` (default) | 24,576 bytes (24 KiB) | 49,152 bytes (48 KiB) |
| `true` | 65,536 bytes (64 KiB) | 131,072 bytes (128 KiB) |

The enabled limits follow [EIP-7954](https://eips.ethereum.org/EIPS/eip-7954).
The option changes only size admission: gas schedules, native-resource pricing,
and memory budgets are unchanged. The existing byte/word-based charges still
apply to the actual code length. Initcode is always limited to twice the
selected deployed-code limit; there is no independent initcode setting.

Transaction deployment validation, CREATE/CREATE2, constructor output, and both
bytecode-setting hooks use the same config. Existing code remains executable if
the option is later disabled. Configuration is fixed within a batch.

## Memory bounds

The [preimage cache](io/caches.md) admits actual retained bytes, including code,
jumpdest artifacts, alignment, and per-entry overhead, before allocating them.
Its 128 MiB transaction budget and 256 MiB block budget do not depend on this
option. Resource exhaustion and L1 postprocessing retain their existing behavior.
In particular, zero-length EXTCODECOPY still loads the target code and consumes
the cache budget.

## Encoding and activation

The option is appended after `l1_tx_filtering_enabled` in the chain-config oracle
serialization as a boolean. Its commitment is appended to `ChainConfig::hash`
as a 32-byte word whose last byte is 0 or 1. The batch public-input layout stays
unchanged; the chain-config hash changes even when the option is disabled.
Settlement-side config hashing must use the same six-word encoding at the
protocol upgrade. Update the shared golden vectors together.

Older JSON state dumps without the field default to `false`. The oracle wire
encoding requires the new field; it is not backward compatible with an older
proving binary.
