# ecrecover_bench

A cycle benchmark of `ecrecover` as the `airbender-crypto` secp256k1 code
computes it, independent of ZKsync OS: the guest recovers one hardcoded
signature (geth's `ecrecover` test vector) a number of times, each recovery
(the recovery and the keccak of the public key, as the precompile does)
between two cycle markers, so the host reads the cycles and the delegation
calls of every recovery. The hints (square roots, inverses, quotients of the
base field, scalar inverses, and the witness of the recovery) come from the
host through the crate's own CSR hooks (`crypto::secp256k1::oracle::CsrHooks`,
the memory-based oracle protocol of ZKsync OS: a query is the query id and the
address of a request in the guest's memory, which the host reads with the
operand) and are checked on the guest; without them the crate's own field
algorithms run. The host answers with `crypto::secp256k1::oracle::host::serve`,
in the representation of the dist (Montgomery words for a `bigint` variant).

Variants (crate features, `build_variants.sh` builds all four into
`dist/<variant>/`):

| variant | field arithmetic | keccak-f[1600] |
|---|---|---|
| `bigint_keccak` | bigint delegation | delegation (production) |
| `bigint` | bigint delegation | software |
| `keccak` | software (10×26 limbs) | delegation |
| `none` | software | software |

The host is `../ecrecover_bench_host` (a standalone crate that drives the
`riscv_transpiler` VM; `cargo build --release` there):

```
../ecrecover_bench_host/target/release/ecrecover_bench_host dist/bigint_keccak dist/none --repeats 8 --hints on
../ecrecover_bench_host/target/release/ecrecover_bench_host dist/none --hints off --machine full
../ecrecover_bench_host/target/release/ecrecover_bench_host dist/none --hints on --flamegraph none.svg --sampling 20
../ecrecover_bench_host/target/release/ecrecover_bench_host dist/bigint_keccak dist/none --micro --machine full
```

`--hints witness` (or `all`) runs the witnessed recovery: the host answers the
witness request of every recovery (`--hints on` declines it, so the key is
computed) with the recovered key and a 4-dimensional decomposition of the
verification equation (`crypto::secp256k1::witness_host`), and the guest checks
the relation `a R + b φ(R) - c Q - d φ(Q) - u G = O` with four ~64-bit variable
scalars instead of computing `(s R - z G) / r` with two 128-bit ones (about 143
hinted divisions instead of 190).

`--micro` runs the field operations one by one (`MICRO_OPS` of each between two
markers) and prints the cycles and delegation calls of one operation; the
operations whose repetition the compiler folds (additions, zero tests) read as
zero.

`--machine full` executes the signed multiplications the compiler emits in
the software modular inversion of the no-hint path (`mulh`), which the
production (full unsigned) profile rejects; the cycle count of an instruction
is the same under both.

Results (geth's test vector, 2026-10-29, per recovery, the hints through the
crate's CSR hooks):

| build | computed, hints on | witnessed key |
|---|---|---|
| bigint + keccak delegations | 61,748 cycles, 7,451 bigint calls | 59,208 cycles, 5,294 bigint calls |
| bigint delegation | 76,423 cycles, 7,451 bigint calls | 73,883 cycles, 5,294 bigint calls |
| software field, keccak delegation | 852,376 cycles | 640,565 cycles |
| all software | 867,141 cycles | 655,234 cycles |

Without hints: 117,366 cycles and 21,623 bigint calls (production build); the
software builds need `--machine full` for that mode. The delegated builds send
their operands where they are (the Montgomery words) and store the answers
straight into the elements; the software builds convert every operand and
answer to canonical little-endian words (the representation of a native
querier of ZKsync OS), which costs them about 5% over an in-band byte
transport, and they are not a proving target.
