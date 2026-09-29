# Bytecode Lab

A deterministic differential and stress runner for the programs `program_gen`
produces. Each case deploys a generated program to ZKsync OS and, for
gas-agnostic programs, replays the same block in REVM and reports any divergence.
It is the in-repo (VM-side) half of the fuzzer: it hunts internal errors, EVM
divergences, panics and slow blocks without a running node.

The REVM comparison runs in **two gas modes** per case. First it uses the node's
`gas_used` and force-failure overrides, as the consistency checker does. State
mismatches become `RevmMismatch` (S1). It then runs with independent EVM gas;
discrepancies seen only there become `GasScheduleDiff` (S3) and are saved for
triage. The forced-gas runner does not compare return data or logs, so an
independent-only mismatch is not automatically a harmless gas difference.
Neither comparison equalizes nested call budgets; all mismatches require triage.

## Prerequisites

- The lab compares against the **production** EVM surface, including the P-256
  precompile. This crate enables `forward_system/production` in `Cargo.toml`, so
  the commands below already include it. **Do not disable this feature** when
  preparing a bug-bounty PoC or reproducing a finding — a non-production build
  has a different precompile set and would give different answers.
- The optional RISC-V (proving) sampling needs the app binary. Build it once:
  ```bash
  cd zksync_os && ./dump_bin.sh --type for-tests
  ```
  Without it, `--riscv-every` is skipped (not a finding) and `replay --riscv`
  exits 3.

This crate has its own `[workspace]` table so it is excluded from the root
workspace (the `production`/`no_print` features would otherwise leak into other
tests through feature unification). Run it from its own directory.

On a host without a CUDA toolkit (for example macOS), set
`ZKSYNC_USE_CUDA_STUBS=true` when building or running. `rig` pulls the GPU prover
crate through `zksync_os_runner`, and this is the same switch the repository's CI
uses to build it without CUDA. On Linux CI it is exported globally, so the
commands below assume it is already set; prepend it otherwise:

```bash
ZKSYNC_USE_CUDA_STUBS=true cargo run --release -- run --seed 1 --cases 2000 --mode gas-agnostic --out /tmp/lab1
```

## Usage

```bash
# from the crate directory
cd tests/bytecode_lab

# Run a batch of gas-agnostic cases; writes findings + summary.json under the out dir.
cargo run --release -- run --seed 1 --cases 2000 --mode gas-agnostic --out /tmp/lab1

# Soak: run for a wall-clock budget across all modes on 8 threads, sampling the
# proving path, deduplicating findings by signature. This is the "run it long"
# form; `--duration` accepts Ns/Nm/Nh.
cargo run --release -- run --seed 1 --duration 2h --modes all --threads 8 \
  --riscv-every 2000 --max-per-signature 8 --out /tmp/lab-soak

# Sample the proving path every 200 cases (needs the RISC-V app binary).
cargo run --release -- run --seed 1 --cases 2000 --mode gas-agnostic --riscv-every 200 --out /tmp/lab1

# Crash / slowness hunting with the unrestricted mode (no REVM comparison).
cargo run --release -- run --seed 7 --cases 5000 --mode unrestricted --slow-ms 3000 --out /tmp/lab-crash

# Re-run a single case (forward), or in the proving path.
cargo run --release -- replay --case /tmp/lab1/findings/case-000123-revm-mismatch.json
cargo run --release -- replay --case /tmp/lab1/findings/case-000123-revm-mismatch.json --riscv

# Export a finding as an evm_divergence_validator scenario the VM team can confirm.
cargo run --release -- export --case /tmp/lab1/findings/case-000123-revm-mismatch.json --out /tmp/case.yaml
```

You can also build once and run the binary directly:

```bash
cargo build --release
./target/release/bytecode-lab run --seed 1 --cases 2000 --mode gas-agnostic --out /tmp/lab1
```

## Modes

| Mode | native price | REVM comparison | Purpose |
|------|--------------|-----------------|---------|
| `gas-agnostic` | 0 | yes (dual: forced-gas + independent) | Differential correctness vs REVM. The program avoids GAS and system addresses; native correction is disabled. Nested gas limits can still cause differences. |
| `hook-free` | rig default | no | Address-restricted stress; safe against a plain EVM fork, but it may read gas, so no differential comparison. |
| `unrestricted` | rig default | no | Crash and slowness hunting; hits the system/hook addresses and gas-dependent paths on purpose. REVM cannot model the system contracts, so there is no differential comparison — the value here is panics, internal errors, and (with `--riscv-every`) forward-vs-RISC-V self-consistency. |
| `delegation-7702` | 0 | yes (dual, like gas-agnostic) | EIP-7702 delegation. The lab deploys the generated runtime, then builds a type-4 setcode transaction whose structure is drawn deterministically from the case seed: the authorized EOA delegates to the deployed program, a precompile, the zero address (a clear), itself, an empty account, or a second delegated EOA (a single-hop chain); plus optional extra authorizations (a stale duplicate, an unreachable nonce, a second independent authority, a chain-id-0 authorization), an optional re-delegation or clearing step, and an optional observation probe contract that records the delegated EOA's code size, code hash, copied code and balance (all gas-independent account reads). Calls are driven at the EOA's own address so the delegated code runs in its context. Uses the same comparison and limitations as gas-agnostic. |

Use `--mode X` for one mode or `--modes X,Y` / `--modes all` to cycle several
(a soak wants `all`, so it covers the differential surface and the system/crash
surface together).

## Run knobs

| Flag | Meaning |
|------|---------|
| `--cases N` | Stop after N cases. |
| `--duration Ns\|Nm\|Nh` | Stop after a wall-clock budget. Give `--cases` and/or `--duration`. |
| `--modes all` / `--modes a,b` / `--mode a` | Which generation mode(s) to cycle through. |
| `--threads N` | Worker threads. Each owns a disjoint set of case indices, and each case's seed is derived from the master seed and its index, so a run is deterministic in `--seed` regardless of the thread count. |
| `--riscv-every K` | Sample the proving path (forward vs RISC-V) on every Kth case, in a child process. |
| `--max-per-signature N` | Write at most N findings of each signature to disk (default 8). Every occurrence is still counted; `summary.json` lists each distinct signature and its total, so a soak reports distinct bugs, not thousands of copies. |
| `--slow-ms MS` | Wall-time threshold for the `Slow` outcome (default 5000). |

## Outcomes and exit codes

Per case: `Ok`, `InternalError` (block-level bootloader error, S1),
`RevmMismatch` (a state or per-transaction divergence that survives forced-gas,
S1), `Panic` (caught, S1), `Slow` (a block over `--slow-ms`, S3), or
`GasScheduleDiff` (a discrepancy only under independent gas, S3, saved for triage). A deployment failure the program
itself predicts (initcode too large, an `0xEF` runtime prefix) is a normal revert
and counts as `Ok`.

The shrinker and the run-level dedup key on the outcome's *signature* (its class
plus, for a REVM mismatch, whether it is a state / per-tx / runner-error
divergence), not just the class, so a reduction cannot morph a state divergence
into a gas-schedule difference. These are coarse groups; unrelated bugs can
share a signature.

`run` exits 0 with no findings, 1 with any S1 finding, 2 with only S3 findings,
and 3 on a tool error, including worker or finding-output failures. `replay --riscv` runs in a child process on purpose: a
forward/proving divergence asserts inside the rig and aborts the child with a
non-zero exit, so one bad case does not end a batch.

## Output layout

```
<out>/summary.json                       # seed, mode, counts, programs/min, findings
<out>/findings/case-000123-<class>.json  # program, block context, outcome, revm detail
<out>/findings/case-000123-<class>.minimized.json   # shrunk reproducer (S1 findings)
<out>/findings/case-000123-riscv-divergence.{json,log}   # sampled proving finding
```

Each finding is shrunk (drop calls, then truncate the runtime block by block,
keeping the smallest program with the same outcome class) into a `minimized.json`
next to it.

`export` preserves bytecode and calls, but changes the sender and block layout;
use `replay` for exact reproduction. EIP-7702 export is rejected because the
scenario exporter does not preserve its authorization sequence.
