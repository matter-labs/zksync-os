# program_gen

A deterministic generator of EVM bytecode programs, with no dependency on any
part of the virtual machine. Give it a seed, a fork and a mode and it hands back
a program: the runtime code, the initcode that deploys it, a set of calls to run
against the deployed contract, and plain-language notes describing what each part
does.

It exists so other tools (an in-process VM runner, a black-box node driver) can
share one corpus of programs. Because output is fully determined by the seed, a
finding travels as a small seed plus its fork and mode and reproduces anywhere.

## What it guarantees

Every generated program is stack valid: replaying the bytecode, the stack never
underflows and never passes 1024 items. Every JUMP and JUMPI targets a real
JUMPDEST, written as a push of that offset immediately before the jump. Runtime
code stays within the interpreter's code-size limit and initcode within the
initcode limit. The builder keeps these true by construction, and the crate
ships independent replayers (`trace_stack`, `verify_jump_targets`) that re-derive
them from the finished bytes, which the tests use.

## Forks

- `Fork::Osaka` is what this repository runs: every precompile is registered and
  the CLZ opcode exists.
- `Fork::CancunWith7702` is an older surface a downstream sequencer still runs
  (Cancun opcodes plus the EIP-7702 transaction type): no CLZ, and only the
  precompiles `0x01`–`0x08` plus P256 at `0x100` are backed by real precompiles.

This repository only executes Osaka, but the generator can emit for either so
the same tool serves both lanes.

## Modes

- `Mode::GasAgnostic`: never reads remaining gas and never branches on it, so a
  program's result does not depend on the exact gas schedule. Use it for
  differential comparison against a plain EVM.
- `Mode::HookFree`: never targets the chain's system or hook addresses, so a
  generic EVM would reproduce the behavior.
- `Mode::Unrestricted`: no restrictions. Use it for crash and slowness hunting.

## Using it

```rust
use program_gen::{generate, Fork, Mode};

let program = generate(42, Fork::Osaka, Mode::GasAgnostic);

// Deploy program.initcode, then send each call against the deployed address.
let deploy_bytes: &[u8] = &program.initcode;
for call in &program.calls {
    let _calldata: &[u8] = &call.calldata;
    let _value: u128 = call.value;
    let _gas: u64 = call.gas;
}

// Persist a case, or read one back.
let json = program.to_json();
let restored = program_gen::Program::from_json(&json).unwrap();
assert_eq!(program, restored);
```

`program.notes` lists the fork and mode, then one line per block with its byte
range in the runtime code and the properties it guarantees. A shrinker can read
those ranges to truncate a program one block at a time.

## Tests

From the repository root:

```bash
cargo test -p program_gen
```

The suite generates a thousand programs per mode and checks stack validity, jump
targets, size limits and determinism, and confirms gas-agnostic programs never
read gas.

## Checks

```bash
cargo fmt -p program_gen -- --check
cargo clippy -p program_gen -- -D warnings
```
