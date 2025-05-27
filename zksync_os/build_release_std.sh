#!/bin/sh
# cargo build -Z build-std=core,panic_abort,alloc -Z build-std-features=panic_immediate_abort --release
cargo build -Z build-std=core,panic_abort,alloc --release