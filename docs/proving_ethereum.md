# Proving Ethereum blocks

Proving ethereum blocks consists of 3 steps:
* getting necessary information about the block (transactions, traces etc)
* putting this information into a format that prover understands (witness generation)
* running the prover.


You can run the provers either on GPU (faster) or on CPU.

## Running end-to-end

TL;DR:

(from this repo's main dir)
```shell

mkdir /tmp/witness

cargo run -p eth_runner --release --features rig/no_print,rig/unlimited_native -- single-run --block-dir tests/instances/eth_runner --randomized --witness-output-dir /tmp/witness
```

from zksync-airbender (suggested version v0.3.0):
```shell
mkdir /tmp/output
CUDA_LAUNCH_BLOCKING=1 CUDA_VISIBLE_DEVICES=0 cargo run -p cli --release --features gpu prove --bin ../zksync-os/zksync_os/evm_replay .bin --input-file /tmp/witness/22244135_witness --until final-recursion --output-dir /tmp/output --gpu --cycles 400000000
```

Then you will have the final proof in /tmp/output/recursion_program_proof.json

You can verify it on http://fri-verifier.vercel.app


## Detailed info

### Getting blocks

The command above takes the block information from `tests/instances/eth_runner`

We've put some additional blocks in https://github.com/antoniolocascio/ethereum-block-examples/tree/main/blocks.

Alternatively, you can download them using the `live-run` command from eth_runner.


### Running on GPU vs CPU

The command above is running on gpu - but you can run the same code on CPU - just don't pass the `--gpu` flag at the end.

GPU requires at least 22GB of VRAM.