#!/bin/bash

set -e

cargo objcopy --release -- -O binary app.bin
