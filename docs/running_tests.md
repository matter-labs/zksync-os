# Tests

We have three types of integration testing for ZKsyncOS:

- [Instances](../tests/instances/): hand-written transaction tests, benchmarks and test vectors for precompiles. Are written using our [testing rig](../tests/rig/).
- [Fuzzing](../tests/fuzzer/): fuzzing for several parts of the system, including transaction processing.
- [EVM tester](https://github.com/matter-labs/era-evm-tester/): tool to run the EF EVM tests on ZKsyncOS.

Detailed instructions for building ZKsyncOS can be found in the [README](../README.md).

## Instances and rig

The testing rig provides methods to simplify the writing of tests. The main abstraction here is [`Chain`](../tests/rig/src/chain.rs), which represent an in-memory state of the chain. A test can specify predeployed contracts and balance/nonce for given addresses. After preparation of the initial chain state, the test has to define the transactions to be executed. Finally, the test calls `run_block` to execute it.

### Concrete system implementation entry points

The `run_block` function will do two things. First, it will call the `run_forward` entrypoint of the forward running system. This is just a concrete instance of the `run_prepared` function from the [bootloader](./bootloader/bootloader.md). After this, it will do a *proof run*, which means that it will do the same run but on the proof running system. This works in the following way:

- [`proof_running_system`](../proof_running_system/) defines the concrete instantiation needed for proving, including a wrapper called `run_proving` to run the bootloader's `run_prepared`.
- [`zksync_os`](../zksync_os/src/main.rs) defines the main function in the RISC-V binary that will be proven as a call to `run_proving`.
- [`zksync_os_runner`](../zksync_os_runner/src/lib.rs) defines a `run` function that takes the RISC-V binary and the initial oracle (non-determinism source) and simulates the execution of that binary. This is done using our [`risc_v_simulator`](https://github.com/matter-labs/zksync-airbender/tree/main/risc_v_simulator).  This is the method called by the testing rig for the proof run.

Note that this flow isn't running the actual prover, just simulating the execution that will be proved.
To actually compute the proof after both runs, the `e2e_proving` feature needs to be enabled.

## Fuzzing

We employ two approaches to fuzz testing coverage-guide fuzzing for low-level primitives and metamorphic differential fuzzing.

The first approach is implemented in the [`tests/fuzzer`](../tests/fuzzer/) directory. 
It includes fuzz targets for core cryptographic functions, system infrastructure, precompiles and Bootloader's entry points.

Metamorphic Fuzzing for Ethereum Test Cases. 
The second approach leverages metamorphic testing to generate random test cases by mutating existing Ethereum test vectors.
This includes both:
 - Metamorphic transformations (preserving semantic correctness under changes)
 - Randomized mutations guided by code coverage.

At present, we use `libFuzzer` to apply both random and metamorphic mutations.
The resulting test cases are then executed using [EVM tester](https://github.com/matter-labs/era-evm-tester/)
to validate correctness and detect panics or discrepancies.

Both fuzzing strategies are executed on a daily basis as part of our testing pipeline:
 - Fuzzing of the primitives run daily using [GitHub Actions](../.github/workflows/fuzz.yml)) 
 - Metamorphic testing run continuously in our cloud-based test infrastructure for deeper, longer fuzzing sessions.

## EVM tester

The [EVM tester](https://github.com/matter-labs/era-evm-tester/) is a tool for parsing and running the [EVM test suite](https://github.com/ethereum/tests/tree/) compiled by the Ethereum Foundation.

To run it on ZKsyncOS, you first need to clone the tester and switch to the `zk-ee` branch. Next, you can modify the Cargo.toml file from the `evm_tester` crate to make it use a local version of ZKsyncOS, otherwise it will fetch it from github.

Once prepared, the command to run all the tests is:

```raw
cargo run --bin evm-tester --features zksync_os_forward_system/no_print --release -- --environment=ZKOS --path ethereum-tests/GeneralStateTests/
```

To debug, one can run a single test by specifying a file and a label, for instance:

```raw
cargo run --bin evm-tester  --release -- --environment=ZKOS --path ethereum-tests/GeneralStateTests/stCreateTest/CreateAddressWarmAfterFail.json --label create2-oog-post-constr
```
