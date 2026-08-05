# Integration Test Philosophy

This document is about the `tests/instances/` layer, which is the main home for ZKsync OS integration tests built on top of the TestingFramework.

## Purpose

`tests/instances/` should verify observable state-transition behavior of the system, not just isolated helper behavior.

These tests are expected to catch:
- divergence between forward execution and the RISC-V path
- protocol regressions in transaction validation and execution semantics
- rollback and side-effect mistakes that only show up at the full-system level
- general system regressions and mistakes

## Broader testing stack

`tests/instances/` is only one part of the overall testing strategy.

Other important layers are:
- `tests/evm_tester`, which executes Ethereum Foundation execution-spec test vectors and checks EVM compatibility against external fixture coverage
- `tests/fuzzer`, which explores larger input spaces and helps find edge cases that are hard to enumerate manually
- `tests/instances/eth_runner`, which reexecutes Ethereum blocks and compares results against Ethereum execution behavior

These layers are complementary:
- `instances` tests are best for targeted, intention-revealing full-system scenarios
- `evm_tester` is best for broad spec-vector compatibility
- fuzzing is best for adversarial and combinatorial exploration
- `eth_runner` is best for replaying real-world execution traces

## Default execution model

Default strategy should be paranoid and security-first.

- Instance tests should use the normal rig defaults (all checks) unless there is a concrete reason not to.
- Do not force `forward_only()` just to make tests faster locally.
- The normal rig behavior already skips RISC-V simulation locally unless `ZKSYNC_RISC_V_RUN` or `CI` is set, and still runs the RISC-V path when the environment asks for it.

## `forward_only()` policy

- `forward_only()` is an escape hatch for instance tests.
- It should only be used when the test genuinely cannot run through the RISC-V path.
- If an instance test uses `forward_only()`, document the reason in code next to that use.

## Structure

- Prefer `TestingFramework` over direct `Chain` usage unless the low-level path itself is what the test is exercising.
- Organize instance tests by domain, not by vague buckets.
- Validation and bootloader rejection tests belong with transaction-focused suites.
- Runtime execution, deployment semantics, and rollback behavior belong with EVM-focused suites.
- Reuse existing rig and instance helpers when they already fit the test. Prefer existing assertion macros, transaction constructors, bytecode helpers, and shared setup functions over introducing near-duplicates.
- Add a new helper only when it meaningfully reduces repetition and still keeps the test intent obvious.

## Assertions

- Assert observable outcomes, not just that “something happened”.
- For revert and rollback tests, make sure the test observes the same execution context whose effects are supposed to be rolled back.
- Keep test helpers narrow and intention-revealing. They should remove boilerplate, not hide semantics.
