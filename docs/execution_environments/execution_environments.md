# Execution Environments

Execution Environments (EEs) are ZKsyncOS's abstraction for a VM. They define their own bytecode format, interpreter, resource consumption and internal frame state. They are initialized and managed by the bootloader. When an EE reaches an exit state (call into another contract, deployment or return) it yields to the bootloader for it to handle it.

## Interface

ZKsyncOS provides an interface to define EEs and some concrete instances. We start by describing the key elements of this interface, which is located in this [directory](../../zk_ee/src/system/execution_environment/mod.rs).

### Launch parameters

An EE needs some initial data to be ran. This data includes:

- Resources for execution,
- Bytecode to be run,
- Call information (caller, callee, modifier),
- Call values (calldata, token value).

Execution environments offer a method to create a new instance (or frame) given some initial data. Once launched with this data, the EE will execute the bytecode until reaching a *preemption point*.

### Preemption points and their continuation

Preemption points are the possible states on which an EE will yield back to the bootloader. These are:

- External call request,
- Deployment request,
- External call completed, and
- Deployment completed.

The last two mark that the execution of the EE is done, and declare the resources returned and the execution result. For the first two, the EE expects the bootloader to do some preparation work (see [Runner flow](../bootloader/runner_flow.md) for more detail) and launch a new EE frame for the call/constructor execution. After this sub-frame is finished, the bootloader will continue the execution of the original EE frame, forwarding the result of the sub-frame.

Thus, execution environments have to provide methods for the bootloader to continue their execution after a call or deployment request. These methods need to take back the resources returned by the sub-call, handle its result and continue executing its own bytecode until reaching another preemption point.

### EE-specific functionality for the bootloader

Execution environments also provide methods that don't involve bytecode execution. Instead, they expose certain information that is specific to the EE type, or expose some element of the inner frame state. These include:

- Whether a call modifier is supported,
- Whether the context is static,
- How caller gas should be adjusted before passing them to the callee (think 63/64 rule for EVM),
- How to prepare for a deployment (init code checks, address derivation).

## Implementations

ZKsyncOS will include the following EEs:

- EVM: provides full native EVM-equivalence to ZKsync. Already implemented in [evm_interpreter](../../evm_interpreter/) and documented in the [EVM section](evm.md).
- WASM: allows ZKsync to support contracts written in any language that compiles to WASM (e.g. Rust). Already implemented in [iwasm_ee](../../iwasm_ee/).
- EraVM: provides backwards compatibility for migration of Era chains.
- Native RISC V: user-mode RISC V code execution unlocks highest proving performance due to not having any interpretation overhead.
