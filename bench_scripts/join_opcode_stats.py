"""Join per-opcode tracer stats (gas/native) with cycle stats to produce combined ratios.

Reads:
  - .out file: EVM Opcode Stats table (gas, native with min/max/median)
  - .bench file: Per-opcode cycle stats (cycles with min/max/median)
  - tracer_dir: per-execution gas/native samples (<OPCODE>.samples)
  - cycles_dir: per-execution cycle samples (<OPCODE>.effective.cycles or .cycles)

Per-execution ratios (cycles/gas, native/gas) are computed from paired samples
(one per invocation), producing accurate p50/p95/p99/max statistics. Aggregate
medians from the .out/.bench files are shown for context.

Usage:
    python join_opcode_stats.py <block.out> <block.bench> \
        <tracer_dir> <cycles_dir> [--csv output.csv]
"""

import os
import sys
import re
import argparse

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from benchlib import (  # noqa: E402
    load_gas_native_samples,
    load_int_samples,
    list_label_files,
    percentile,
    ratio,
    safe_listdir,
)


def parse_tracer_stats(filename):
    """Parse '=== EVM Opcode Stats:' from .out file."""
    stats = {}
    try:
        with open(filename) as f:
            text = f.read()
    except FileNotFoundError:
        return stats

    match = re.search(r"=== EVM Opcode Stats:\n(.+?)\n={5,}", text, re.DOTALL)
    if not match:
        return stats

    for line in match.group(1).strip().splitlines()[1:]:
        parts = line.split()
        if len(parts) < 10 or parts[1] == "-":
            continue
        try:
            stats[parts[0]] = {
                "count": int(parts[1]),
                "avg_gas": float(parts[2]),
                "med_gas": int(parts[3]),
                "min_gas": int(parts[4]),
                "max_gas": int(parts[5]),
                "avg_native": float(parts[6]),
                "med_native": int(parts[7]),
                "min_native": int(parts[8]),
                "max_native": int(parts[9]),
            }
        except (ValueError, IndexError):
            continue
    return stats


def parse_cycle_stats(filename):
    """Parse '=== Per-opcode cycle stats:' from .bench file."""
    stats = {}
    try:
        with open(filename) as f:
            text = f.read()
    except FileNotFoundError:
        return stats

    match = re.search(r"=== Per-opcode cycle stats:\n(.+?)\n={5,}", text, re.DOTALL)
    if not match:
        return stats

    for line in match.group(1).strip().splitlines()[1:]:
        parts = line.split()
        if len(parts) < 7:
            continue
        try:
            stats[parts[0]] = {
                "count": int(parts[1]),
                "total_cycles": int(parts[2]),
                "avg_cycles": float(parts[3]),
                "med_cycles": int(parts[4]),
                "min_cycles": int(parts[5]),
                "max_cycles": int(parts[6]),
            }
        except (ValueError, IndexError):
            continue
    return stats


def load_per_execution_samples(tracer_dir, cycles_dir):
    """Load paired (gas, native, cycles) per-execution from sample dirs.

    Returns dict: opcode -> list of (gas, native, cycles) tuples.
    """
    tracer_files = {
        f[: -len(".samples")]
        for f in safe_listdir(tracer_dir)
        if f.endswith(".samples")
    }
    _, _, opcode_to_file = list_label_files(cycles_dir)

    paired = {}
    for op in sorted(tracer_files & set(opcode_to_file)):
        tracer_path = os.path.join(tracer_dir, f"{op}.samples")
        cycles_path = os.path.join(cycles_dir, opcode_to_file[op])
        tracer_samples = load_gas_native_samples(tracer_path)
        cycle_samples = load_int_samples(cycles_path)
        n = min(len(tracer_samples), len(cycle_samples))
        if n == 0:
            continue
        if len(tracer_samples) != len(cycle_samples):
            print(
                f"  WARNING: {op} count mismatch: tracer={len(tracer_samples)} "
                f"cycles={len(cycle_samples)}, using first {n}",
                file=sys.stderr,
            )
        paired[op] = [(tracer_samples[i][0], tracer_samples[i][1], cycle_samples[i]) for i in range(n)]
    return paired


def fmt(val):
    """Format a ratio value, or '—' for None."""
    if val is None:
        return "—".rjust(10)
    return f"{val:>10.1f}"


def fmt_csv(val):
    """Format a ratio value for CSV, or empty for None."""
    if val is None:
        return ""
    return f"{val:.2f}"


def main():
    parser = argparse.ArgumentParser(description="Join opcode tracer and cycle stats")
    parser.add_argument("out_file", help=".out file with tracer stats")
    parser.add_argument("bench_file", help=".bench file with cycle stats")
    parser.add_argument("tracer_dir", help="Directory with per-execution .samples files")
    parser.add_argument("cycles_dir", help="Directory with per-execution .cycles files")
    parser.add_argument("--csv", help="Write CSV output to file")
    args = parser.parse_args()

    tracer = parse_tracer_stats(args.out_file)
    cycles = parse_cycle_stats(args.bench_file)

    if not tracer or not cycles:
        print("No data to join.", file=sys.stderr)
        sys.exit(1)

    per_exec = load_per_execution_samples(args.tracer_dir, args.cycles_dir)

    opcodes = sorted(set(tracer) & set(cycles))

    rows = []
    for op in opcodes:
        t = tracer[op]
        c = cycles[op]

        row = {
            "op": op,
            "count": t["count"],
            "med_gas": t["med_gas"],
            "med_native": t["med_native"],
            "med_cycles": c["med_cycles"],
            "p50_cpg": None,
            "p95_cpg": None,
            "p99_cpg": None,
            "max_cpg": None,
            "p50_npg": None,
            "p95_npg": None,
            "p99_npg": None,
            "max_npg": None,
        }

        if op in per_exec:
            samples = per_exec[op]
            cpg_values = sorted(ratio(cyc, g) for g, _, cyc in samples if g > 0)
            npg_values = sorted(ratio(nat, g) for g, nat, _ in samples if g > 0)
            if cpg_values:
                row["p50_cpg"] = percentile(cpg_values, 50)
                row["p95_cpg"] = percentile(cpg_values, 95)
                row["p99_cpg"] = percentile(cpg_values, 99)
                row["max_cpg"] = cpg_values[-1]
            if npg_values:
                row["p50_npg"] = percentile(npg_values, 50)
                row["p95_npg"] = percentile(npg_values, 95)
                row["p99_npg"] = percentile(npg_values, 99)
                row["max_npg"] = npg_values[-1]

        rows.append(row)

    rows.sort(key=lambda r: r["max_cpg"] or 0, reverse=True)

    print(f"{'opcode':<16} {'count':>8} {'med_gas':>8} {'med_nat':>8} {'med_cyc':>8}"
          f" {'p50 c/g':>10} {'p95 c/g':>10} {'p99 c/g':>10} {'max c/g':>10}"
          f" {'p50 n/g':>10} {'p95 n/g':>10} {'max n/g':>10}")
    print("-" * 142)
    for r in rows:
        print(f"{r['op']:<16} {r['count']:>8} {r['med_gas']:>8} {r['med_native']:>8} {r['med_cycles']:>8}"
              f" {fmt(r['p50_cpg'])} {fmt(r['p95_cpg'])} {fmt(r['p99_cpg'])} {fmt(r['max_cpg'])}"
              f" {fmt(r['p50_npg'])} {fmt(r['p95_npg'])} {fmt(r['max_npg'])}")

    if args.csv:
        with open(args.csv, "w") as f:
            f.write("opcode,count,"
                    "med_gas,med_native,med_cycles,"
                    "p50_cpg,p95_cpg,p99_cpg,max_cpg,"
                    "p50_npg,p95_npg,p99_npg,max_npg\n")
            for r in rows:
                f.write(f"{r['op']},{r['count']},"
                        f"{r['med_gas']},{r['med_native']},{r['med_cycles']},"
                        f"{fmt_csv(r['p50_cpg'])},{fmt_csv(r['p95_cpg'])},"
                        f"{fmt_csv(r['p99_cpg'])},{fmt_csv(r['max_cpg'])},"
                        f"{fmt_csv(r['p50_npg'])},{fmt_csv(r['p95_npg'])},"
                        f"{fmt_csv(r['p99_npg'])},{fmt_csv(r['max_npg'])}\n")
        print(f"\nCSV written to {args.csv}")


if __name__ == "__main__":
    main()
