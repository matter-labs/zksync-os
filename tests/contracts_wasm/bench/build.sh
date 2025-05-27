#!/bin/sh

# https://github.com/johnthagen/min-sized-rust

RUSTFLAGS="-C link-arg=-zstack-size=4096 -C target-feature=+multivalue" \
  cargo build \
  -Z build-std=core,panic_abort,alloc \
  --release --target wasm32-unknown-unknown
  # -Z build-std-features=panic_immediate_abort \

if command -v wasm2wat >/dev/null 2>&1; then
  wasm2wat ./target/wasm32-unknown-unknown/release/*.wasm -o ./out.wat
fi
