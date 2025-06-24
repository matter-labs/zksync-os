#!/bin/sh
rm evm_replay.bin
rm evm_replay.elf
rm evm_replay.text

cargo build --features proving,unlimited_native --release # easier errors
# cargo build -Z build-std=core,panic_abort,alloc -Z build-std-features=panic_immediate_abort --release
cargo objcopy --features proving,unlimited_native --release -- -O binary evm_replay.bin
cargo objcopy --features proving,unlimited_native --release -- -R .text evm_replay.elf
cargo objcopy --features proving,unlimited_native --release -- -O binary --only-section=.text evm_replay.text
# cargo objcopy -- -O binary evm_replay.bin
