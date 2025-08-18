# Execution Environments

Execution Environments (EEs) are ZKsyncOS's abstraction for a VM. They define their own bytecode format, interpreter, resource consumption and internal frame state. They are initialized and managed by the bootloader. When an EE reaches an exit state (call into another contract, deployment or return) it yields to the bootloader for it to handle it.

## Interface

ZKsyncOS provides an interface to define EEs and some concrete instances. We start by describing the key elements of this interface, which is located in this [directory](../../zk_ee/src/system/execution_environment/mod.rs).

Execution environments offer methods to:

- Create an empty instance (or frame),
- Perform initial checks on a new instance given the launch state, as well as apply some init logic (e.g. bumping the caller's nonce during deployment),
- Actually launch the EE execution, which runs until reaching a *preemption point*, and
- Continue after the system handles the preemption.

### Launch parameters

An EE needs some initial data to be ran. This data includes:

- Resources for execution,
- Call information (caller, callee, modifier),
- Call values (calldata, token value).

### Preemption points and their continuation

Preemption points are the possible states on which an EE will yield back to the bootloader. There are two of these:

- Call request, and
- Execution completed.

The first one is used both for external calls and constructor execution in deployments.
The EE expects the bootloader to do some preparation work (see [Runner flow](../bootloader/runner_flow.md) for more detail) and launch a new EE frame for the call/constructor execution. After this sub-frame is finished, the bootloader will continue the execution of the original EE frame, forwarding the result of the sub-frame. The second one marks that the execution of the EE is done, and declare the resources returned and the execution result.

Thus, execution environments have to provide a method for the bootloader to continue their execution after a call request. This method needs to take back the resources returned by the sub-call, handle its result and continue executing its own bytecode until reaching another preemption point.

### A note on deployments

Deployments are mostly handled by the EE, which calls into the runner to execute the constructor code. For example, the EVM EE keeps an internal flag in it's state to distinguish between external calls and deployments. This EE calls into the system to deploy the code produced by the constructor once the execution is completed but before yielding to the bootloader.

### EE-specific functionality for the bootloader

Execution environments also provide methods that don't involve bytecode execution. Instead, they expose certain information that is specific to the EE type, or expose some element of the inner frame state.
For now, the only such method is a helper to calculate resources to be passed from caller to callee according to EE-specific rules.

## Implementations

ZKsyncOS will include the following EEs:

- EVM: provides full native EVM-equivalence to ZKsync. Already implemented in [evm_interpreter](../../evm_interpreter/) and documented in the [EVM section](evm.md).
- WASM: allows ZKsync to support contracts written in any language that compiles to WASM (e.g. Rust). Already implemented in [iwasm_ee](../../iwasm_ee/).
- EraVM: provides backwards compatibility for migration of Era chains.
- Native RISC V: user-mode RISC V code execution unlocks highest proving performance due to not having any interpretation overhead.
