# System

The *System* is passed to all EEs by the bootloader. It provides access to [memory](./memory.md), [IO](./io/io.md) and [oracles](./io/oracles.md). Oracles are the interface through which the system accesses outside information, they are described in more detail in their documentation page.

The System struct is generic over memory, IO, oracles and allocators and provides little else besides access to those and block metadata. The *Concrete system implementations* are two instances of those components to result in full system implementations, one for the sequencer and the other for the prover (as anticipated in the [Overview](../overview.md#running-environments)).

## System struct

We start by describing the [system struct](../../zk_ee/src/system/mod.rs).

For convenience, the system is parameterized with `S: SystemTypes`, which bundles all the types the system is generic over.

### Limited capabilities

`SystemTypes` only requires `IO` to be an `IOSubsystem`. To access the all io functionality, `IOSubsystemExt` is required.
The weaker version is meant to be passed to Execution environments, while the bootloader may use the stronger version of the trait by requiring `S::IO: IOSubsystemExt`.

### Resources and IO types configuration

The system is generic over how it represents computational resources. The system provides a reference implementation for [basic resources](../../zk_ee/src/reference_implementations/mod.rs) that models resources as an amount of *ergs*, which are consumed as gas in Ethereum.

It is also generic over what types are used for IO. These include addresses, storage keys and values, balances, etc. The system implements an [instance](../../zk_ee/src/types_config/mod.rs) for Ethereum-like systems with the expected Ethereum types.

The system doesn't really have to be generic over these types because many parts of the codebase only work if the types are the `EthereumLikeTypes`. However, this way
we can at least be sure that the places that don't require `EthereumLikeTypes` don't need to be edited if a type changes.

```rust
pub trait EthereumLikeTypes:
    SystemTypes<IOTypes = EthereumIOTypesConfig, Resources = BaseResources> {}
```

### System functions

The system also describes a set of pure functions that should be part of any system implementation. These are mostly cryptographic primitives and are described in the [`SystemFunctions`](../../zk_ee/src/system/base_system_functions.rs) trait. Most of them are used to implement precompiles as [System Hooks](../system_hooks.md).

### IO and Memory subsystems

Most of the functionality of the system is related to IO or memory. These two functionalities are defined as their own subsystems.
The [IO subsystem](./io/io.md) and [memory subsystem](memory.md) have their own documentation pages.

### User-facing methods

- Start and finish io-frames (frames without separate memory, like near-calls in Era),
- Query the block metadata from the oracle.

If you have access to a System with the *Ext versions of the subsystems available, you can also use the following methods.

- Start and finish global execution frames (memory and io),
- Query the oracle to read the next transaction,
- Deploy bytecode,
- Initialize the system from the oracle,
- Finalize the system after the block processing is done.

## Implementations of the system's parts

The system functions are implemented in [this directory](../../basic_system/src/system_functions/). Some of them had to be implemented from scratch or forked due to the need to not use the global allocator in proof running mode.

These parts differ between forward-running and proving:

- Allocator, described in the [`memory` section](memory.md).
- Stack implementations.
- IOOracle, described in the [`oracle` section](./io/oracles.md).
- Logger, a simple trait to log data about the system execution, omitted in proving.
