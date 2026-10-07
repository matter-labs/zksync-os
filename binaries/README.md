# Glamsterdam guest programs

One crate per program, each with a single fixed configuration (no feature
variants): the features of `proof_running_system` a program needs are set in
its `Cargo.toml`.

- `glamsterdam_pre_execution`: the state consistency check of a block through
  its block access list (`run_glamsterdam_state_consistency`).
- `glamsterdam_tx_range`: a range of block access indices executed and checked
  against the access list and the receipts (`run_glamsterdam_range`).

Both return the hash of their linkage output
(`basic_bootloader::...::ethereum_glamsterdam::linkage::ProgramOutput`), which
the aggregation of a block's programs links.

The programs build for the standard `riscv32i-unknown-none-elf` target with
the code generation flags of the `zksync_os` guest (`.cargo/config.toml`);
`glamsterdam_tx_range` enables the M extension, `glamsterdam_pre_execution`
does without it (its multiplications go through compiler-builtins).
`build.sh` builds them and packages `dist/` next to each (`app.bin`,
`app.elf`, `app.text`, `manifest.toml`) with `rust-objcopy`, the way the
Airbender SDK does, without `cargo-airbender`:

```bash
binaries/build.sh                        # both programs
binaries/build.sh glamsterdam_tx_range   # one of them
```

The `eth_runner` commands `glamsterdam-state-consistency-cycles` and
`glamsterdam-range-cycles` load these dists by default.
