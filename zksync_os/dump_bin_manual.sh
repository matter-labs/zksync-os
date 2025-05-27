#!/bin/sh
rm app.bin
rm app.elf
rm app.text

cargo build -Z build-std=core,panic_abort,alloc -Z build-std-features=panic_immediate_abort --release

~/.rustup/toolchains/stable-aarch64-apple-darwin/lib/rustlib/aarch64-apple-darwin/bin/llvm-objcopy ./target/riscv32i-unknown-none-elf/release/zksync_os -O binary app.bin
~/.rustup/toolchains/stable-aarch64-apple-darwin/lib/rustlib/aarch64-apple-darwin/bin/llvm-objcopy ./target/riscv32i-unknown-none-elf/release/zksync_os -R .text app.elf
~/.rustup/toolchains/stable-aarch64-apple-darwin/lib/rustlib/aarch64-apple-darwin/bin/llvm-objcopy ./target/riscv32i-unknown-none-elf/release/zksync_os -O binary --only-section=.text app.text