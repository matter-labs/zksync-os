#!/bin/bash

set -e

cargo airbender build --app-name app_native_blake --release

cargo airbender build --app-name app_extended_delegation_blake --release -- --features single_round_with_control
