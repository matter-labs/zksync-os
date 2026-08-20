"""Join per-execution opcode samples (gas/native from tracer, cycles from RISC-V).

Reads:
  - <tracer_dir>/<OPCODE>.samples: "gas,native" per line (execution order)
  - <cycles_dir>/<OPCODE>.effective.cycles (preferred) or
    <cycles_dir>/<OPCODE>.cycles (fallback): one cycle count per line in
    execution order.

The `.effective.cycles` variant includes delegation cost (Blake/BigInt/Keccak)
using the same coefficients as the block-wide `block_effective` formula in
`cycle_marker`. Without it, `cycles/gas` reflects raw RISC-V cycles only and
undercounts opcodes whose handlers delegate (SHA3 → keccak; SLOAD/SSTORE,
BALANCE/EXTCODE*, SELFBALANCE → Blake via account/storage tree;
CALL/DELEGATECALL/STATICCALL/CALLCODE and CREATE/CREATE2 → keccak +
Blake + any inner precompile delegations). The script prefers the
effective variant and falls back to raw with a stderr note when only
raw is available.

Since both runs are deterministic, line K in both files corresponds to
the Kth execution of that opcode.

Outputs per-opcode CSV with (gas, native, cycles, cycles/gas, native/gas)
per execution, and a summary with worst-case ratios.

Usage:
    python join_samples.py <tracer_dir> <cycles_dir> [--out-dir <output_dir>] [--summary]
"""

import os
import sys
import argparse

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from benchlib import (  # noqa: E402
    load_gas_native_samples as load_tracer_samples,
    load_int_samples as load_cycle_samples,
    percentile as _benchlib_percentile,
    ratio,
)


def process_opcode(name, tracer_samples, cycle_samples, out_dir):
    """Join samples and write per-execution CSV. Return summary stats."""
    n = min(len(tracer_samples), len(cycle_samples))
    if n == 0:
        return None

    if len(tracer_samples) != len(cycle_samples):
        print(f"  WARNING: {name} count mismatch: tracer={len(tracer_samples)} cycles={len(cycle_samples)}, using first {n}",
              file=sys.stderr)

    rows = []
    for i in range(n):
        gas, native = tracer_samples[i]
        cycles = cycle_samples[i]
        rows.append((gas, native, cycles))

    # Write per-execution CSV
    if out_dir:
        os.makedirs(out_dir, exist_ok=True)
        path = os.path.join(out_dir, f"{name}.csv")
        with open(path, "w") as f:
            f.write("gas,native,cycles,cycles_per_gas,native_per_gas\n")
            for gas, native, cycles in rows:
                cpg = ratio(cycles, gas)
                npg = ratio(native, gas)
                f.write(f"{gas},{native},{cycles},{cpg:.2f},{npg:.2f}\n")

    # Compute summary
    cycles_per_gas_values = [ratio(c, g) for g, _, c in rows if g > 0]
    native_per_gas_values = [ratio(n, g) for g, n, _ in rows if g > 0]

    if not cycles_per_gas_values:
        return None

    cycles_per_gas_values.sort()
    native_per_gas_values.sort()

    percentile = _benchlib_percentile

    return {
        "name": name,
        "count": n,
        "med_cpg": percentile(cycles_per_gas_values, 50),
        "p95_cpg": percentile(cycles_per_gas_values, 95),
        "p99_cpg": percentile(cycles_per_gas_values, 99),
        "max_cpg": cycles_per_gas_values[-1],
        "med_npg": percentile(native_per_gas_values, 50),
        "p95_npg": percentile(native_per_gas_values, 95),
        "p99_npg": percentile(native_per_gas_values, 99),
        "max_npg": native_per_gas_values[-1],
    }


def main():
    parser = argparse.ArgumentParser(description="Join per-execution opcode samples")
    parser.add_argument("tracer_dir", help="Directory with .samples files (gas,native)")
    parser.add_argument("cycles_dir", help="Directory with .cycles files")
    parser.add_argument("--out-dir", help="Write per-execution CSVs to this directory")
    parser.add_argument("--summary", action="store_true", help="Print summary table")
    args = parser.parse_args()

    # Find opcodes present in both directories. Cycle files come in two
    # flavors: `<OPCODE>.effective.cycles` (preferred, includes delegations)
    # and `<OPCODE>.cycles` (raw). Strip both suffixes when computing the
    # opcode name set.
    tracer_opcodes = {f.replace(".samples", "") for f in os.listdir(args.tracer_dir) if f.endswith(".samples")}
    cycle_files = set(os.listdir(args.cycles_dir))
    cycle_opcodes = set()
    for f in cycle_files:
        if f.endswith(".effective.cycles"):
            cycle_opcodes.add(f[: -len(".effective.cycles")])
        elif f.endswith(".cycles"):
            cycle_opcodes.add(f[: -len(".cycles")])
    common = sorted(tracer_opcodes & cycle_opcodes)

    if not common:
        print("No matching opcodes found between tracer and cycle directories.", file=sys.stderr)
        sys.exit(1)

    summaries = []
    used_kinds = set()
    for name in common:
        tracer_path = os.path.join(args.tracer_dir, f"{name}.samples")
        effective_file = f"{name}.effective.cycles"
        if effective_file in cycle_files:
            cycles_path = os.path.join(args.cycles_dir, effective_file)
            used_kinds.add("effective")
        else:
            cycles_path = os.path.join(args.cycles_dir, f"{name}.cycles")
            used_kinds.add("raw")

        tracer_samples = load_tracer_samples(tracer_path)
        cycle_samples = load_cycle_samples(cycles_path)

        summary = process_opcode(name, tracer_samples, cycle_samples, args.out_dir)
        if summary:
            summaries.append(summary)

    if not args.summary and not args.out_dir:
        args.summary = True

    if args.summary and summaries:
        if used_kinds == {"effective"}:
            cycles_label = "cycles = effective (raw + Blake×16 + BigInt×4 + Keccak×4)"
        elif used_kinds == {"raw"}:
            cycles_label = "cycles = raw RISC-V (delegations NOT included)"
        elif used_kinds == {"raw", "effective"}:
            cycles_label = "cycles = mixed (some opcodes lack effective dump; see stderr)"
            print(
                "Warning: cycles dump kind mixed across opcodes — re-run with a"
                " consistent cycle_marker build to avoid skewed comparisons.",
                file=sys.stderr,
            )
        else:
            cycles_label = "cycles = (no samples consumed)"
        # Sort by worst-case cycles/gas
        summaries.sort(key=lambda s: s["max_cpg"], reverse=True)
        print(cycles_label)
        print(f"{'opcode':<16} {'count':>8}"
              f" {'med c/g':>8} {'p95 c/g':>8} {'p99 c/g':>8} {'max c/g':>8}"
              f" {'med n/g':>8} {'p95 n/g':>8} {'p99 n/g':>8} {'max n/g':>8}")
        print("-" * 104)
        for s in summaries:
            print(f"{s['name']:<16} {s['count']:>8}"
                  f" {s['med_cpg']:>8.1f} {s['p95_cpg']:>8.1f} {s['p99_cpg']:>8.1f} {s['max_cpg']:>8.1f}"
                  f" {s['med_npg']:>8.1f} {s['p95_npg']:>8.1f} {s['p99_npg']:>8.1f} {s['max_npg']:>8.1f}")

    if args.out_dir:
        print(f"\nPer-execution CSVs written to {args.out_dir}/")


if __name__ == "__main__":
    main()
