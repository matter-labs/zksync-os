# Runner flow

This section describes how the bootloader interacts with the execution environments to run contract code. This section is complemented by [Execution Environments](../execution_environments/execution_environments.md).

## Entrypoints

The bootloader implements (and uses) two entrypoints for code execution.

The first one is `run_till_completion` from the [`runner`](../../basic_bootloader/src/bootloader/runner.rs) module of the bootloader. This function implements the main execution loop given an initial call request, which is explained in the next section.

 The second one, [`run_single_interaction`](../../basic_bootloader/src/bootloader/run_single_interaction.rs), is just a simple wrapper over the previous to simplify external calls from the bootloader. It just adds the logic for starting and finishing the topmost execution frame and prepares the inputs for `run_till_completion`.

## Runner structure

The runner's responsibility is to coordinate calls into the execution environments. For this, the runner keeps a callstack of execution environment states and will be responsible of starting and finishing system frames. Frames are used to take snapshots for storage and memory to which the system can revert to in case of a failure.

The runner is implemented as an infinite loop that dispatches the call requests returned by the execution environment. As a reminder, these are used both for external calls and execution of constructor code for deployments.

The runner breaks out of the infinite loop after processing the completion of the initial request (when the callstack becomes empty).

### Call request

For the external call request, the bootloader needs to:

1. Read callee account, potentially charging for this access.
2. Perform some preparation for the call, such as calling into the EE to calculate resources to pass to callee.
3. Call into Execution Environment to perform EE-specific checks and logic. For example, for EVM this includes balance check for value transfer and, in case of deployment, nonce increase and address collision check. Up to this point, execution remains in caller's frame.
4. Create a new frame for the callee.
5. Perform value transfer, if any.
6. Create a new EE state and start executing it if there's any code to run. If the target is a special address (addresses used for precompiles or system contracts), the corresponding [System Hook](../system_hooks.md) is invoked instead.
7. Handle the returned preemption point, either recursively handling a new call request (for a nested call/deployment) or the completion of the original call.
