#!/bin/sh
rm app.bin
rm app.elf
rm app.text

cargo build --release # easier errors
# cargo build -Z build-std=core,panic_abort,alloc -Z build-std-features=panic_immediate_abort --release
cargo objcopy --release -- -O binary app.bin
cargo objcopy --release -- -R .text app.elf
cargo objcopy --release -- -O binary --only-section=.text app.text
# cargo objcopy -- -O binary app.bin
