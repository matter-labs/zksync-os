#!/bin/bash

set -e

cargo objcopy --release -- -O binary app_native_blake.bin
cargo objcopy --release --features single_round_with_control -- -O binary app_extended_delegation_blake.bin
