#!/bin/sh
set -e

# script that measures how many RV32 cycles our blocks are consuming

# prep work (toolchain from zksync_os folder)
rustup target add riscv32i-unknown-none-elf --toolchain nightly-2025-05-24
rustup component add llvm-tools --toolchain nightly-2025-05-24

# build zkos binary using our precompiles
(cd zksync_os && ./dump_bin.sh --type evm-replay-benchmarking)

# benchmark
for dir in tests/instances/eth_runner/blocks/*; do
    blk=$(basename "$dir")
    cargo run --profile test-release -p eth_runner -j 3 --features rig/no_print,rig/unlimited_native -- single-run --block-dir $dir > block_${blk}.out
done

echo "FINISHED! look in the .out files to see cycle counts."
