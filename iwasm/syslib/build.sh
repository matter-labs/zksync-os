#!/bin/sh
# RUSTFLAGS="-C link-arg=-zstack-size=1024 -C target-feature=+multivalue" cargo build --release --target wasm32-unknown-unknown
RUSTFLAGS="-C link-arg=-zstack-size=1024 -C target-feature=+multivalue" cargo build -Z build-std=core,panic_abort,alloc -Z build-std-features=panic_immediate_abort --release --target wasm32-unknown-unknown
# RUSTFLAGS="--cfg no_global_oom_handling -C link-arg=-zstack-size=1024 -C target-feature=+multivalue" cargo build -Z build-std --release --target wasm32-unknown-unknown
# cargo build --release --target wasm32-unknown-unknown