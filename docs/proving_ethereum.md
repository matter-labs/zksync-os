# Proving Ethereum blocks

Proving ethereum blocks consists of 3 steps:
* getting necessary information about the block (transactions, traces etc)
* putting this information into a format that prover understands (witness generation)
* running the prover.


You can run the provers either on GPU (faster) or on CPU.

## Running end-to-end

TL;DR:

(from this repo's main directory)
```shell

mkdir /tmp/witness

cargo run -p eth_runner --release --features rig/no_print,evm_replay -- single-run --block-dir tests/instances/eth_runner/blocks/19299001 --randomized --witness-output-dir /tmp/witness
```

Now, clone [zksync-airbender](https://github.com/matter-labs/zksync-airbender/tree/main) (suggested version v0.3.0).
From Airbender's [tools/cli directory](https://github.com/matter-labs/zksync-airbender/tree/main/tools/cli) run the prover with GPU or CPU as follows:

### With GPU (requires at least 22GB of device RAM):
```shell
mkdir /tmp/output
CUDA_VISIBLE_DEVICES=0 cargo run -p cli --release --features gpu prove --bin ../../../zksync-os/zksync_os/evm_replay.bin --input-file /tmp/witness/19299001_witness --until final-recursion --output-dir /tmp/output --gpu --cycles 500000000
```

To hide latency, Airbender uses an asynchronous allocator and internal pipelining (e.g. overlapping cpu<->gpu transfers with computations). The pipelining requires dedicated allocations.
If you encounter an error, it may be because you ran out of memory. You can reduce the necessary high-water mark by running the GPU in synchronous mode with CUDA_LAUNCH_BLOCKING=1:
```shell
mkdir /tmp/output
CUDA_LAUNCH_BLOCKING=1 CUDA_VISIBLE_DEVICES=0 cargo run -p cli --release --features gpu prove --bin ../../../zksync-os/zksync_os/evm_replay.bin --input-file /tmp/witness/19299001_witness --until final-recursion --output-dir /tmp/output --gpu --cycles 500000000
```

### With CPU:
```shell
mkdir /tmp/output
cargo run -p cli --release prove --bin ../../../zksync-os/zksync_os/evm_replay.bin --input-file /tmp/witness/19299001_witness --until final-recursion --output-dir /tmp/output --cycles 500000000
```

The final proof will appear in /tmp/output/recursion_program_proof.json.

You can verify it on [http://fri-verifier.vercel.app](http://fri-verifier.vercel.app).


## Detailed info

### Getting blocks

The command above takes the block information from `tests/instances/eth_runner`

We've put some additional blocks in https://github.com/antoniolocascio/ethereum-block-examples/tree/main/blocks.

Alternatively, you can download them using the `live-run` command from eth_runner.
