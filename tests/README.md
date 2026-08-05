This folder contains ZKsync OS integration tests.

- [Integration test philosophy for `instances`](./instances/TESTING.md)

## Directory structure

This directory contains the following subdirectories:
- `rig` - ZKsync OS integration testing framework
- `contracts_sol` - solidity contracts to be used in the tests
- `contracts_wasm` - wasm contracts to be used in the tests (currently not used as wasm tests disabled)
- `forge` - forge project with test solidity contracts
- `instances` - test cases implemented using rig
- `fuzzer`
