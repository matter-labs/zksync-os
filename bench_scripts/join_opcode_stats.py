"""Join per-opcode tracer stats (gas/native) with cycle stats to produce combined ratios.

Reads:
  - .out file: EVM Opcode Stats table (gas, native with min/max/median)
  - .bench file: Per-opcode cycle stats (cycles with min/max/median)

Outputs a combined table with cycles/gas and native/gas ratios,
including worst-case bounds (max_cycles/min_gas).

Usage:
    python join_opcode_stats.py <block.out> <block.bench> [--csv output.csv]
"""

import sys
import re
import argparse


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


def ratio(num, den):
    return num / den if den > 0 else 0.0


def main():
    parser = argparse.ArgumentParser(description="Join opcode tracer and cycle stats")
    parser.add_argument("out_file", help=".out file with tracer stats")
    parser.add_argument("bench_file", help=".bench file with cycle stats")
    parser.add_argument("--csv", help="Write CSV output to file")
    args = parser.parse_args()

    tracer = parse_tracer_stats(args.out_file)
    cycles = parse_cycle_stats(args.bench_file)

    if not tracer or not cycles:
        print("No data to join.", file=sys.stderr)
        sys.exit(1)

    opcodes = sorted(set(tracer) & set(cycles))

    rows = []
    for op in opcodes:
        t = tracer[op]
        c = cycles[op]
        rows.append({
            "op": op,
            "count": t["count"],
            # Median ratios (typical case)
            "med_cycles_per_gas": ratio(c["med_cycles"], t["med_gas"]),
            "med_native_per_gas": ratio(t["med_native"], t["med_gas"]),
            "med_cycles_per_native": ratio(c["med_cycles"], t["med_native"]),
            # Worst-case ratios (upper bounds)
            "worst_cycles_per_gas": ratio(c["max_cycles"], t["min_gas"]) if t["min_gas"] > 0 else 0,
            "worst_native_per_gas": ratio(t["max_native"], t["min_gas"]) if t["min_gas"] > 0 else 0,
            # Raw values for reference
            "med_gas": t["med_gas"],
            "med_native": t["med_native"],
            "med_cycles": c["med_cycles"],
            "max_cycles": c["max_cycles"],
            "min_gas": t["min_gas"],
            "max_native": t["max_native"],
        })

    # Sort by worst cycles/gas descending (most expensive first)
    rows.sort(key=lambda r: r["worst_cycles_per_gas"], reverse=True)

    # Print table
    print(f"{'opcode':<16} {'count':>8} {'med_gas':>8} {'med_nat':>8} {'med_cyc':>8}"
          f" {'cyc/gas':>8} {'nat/gas':>8} {'cyc/nat':>8}"
          f" {'W cyc/gas':>10} {'W nat/gas':>10}")
    print("-" * 114)
    for r in rows:
        print(f"{r['op']:<16} {r['count']:>8} {r['med_gas']:>8} {r['med_native']:>8} {r['med_cycles']:>8}"
              f" {r['med_cycles_per_gas']:>8.1f} {r['med_native_per_gas']:>8.1f} {r['med_cycles_per_native']:>8.2f}"
              f" {r['worst_cycles_per_gas']:>10.1f} {r['worst_native_per_gas']:>10.1f}")

    if args.csv:
        with open(args.csv, "w") as f:
            f.write("opcode,count,"
                    "med_gas,med_native,med_cycles,"
                    "min_gas,max_gas,min_native,max_native,min_cycles,max_cycles,"
                    "med_cycles_per_gas,med_native_per_gas,med_cycles_per_native,"
                    "worst_cycles_per_gas,worst_native_per_gas\n")
            for r in rows:
                t = tracer[r["op"]]
                c = cycles[r["op"]]
                f.write(f"{r['op']},{r['count']},"
                        f"{t['med_gas']},{t['med_native']},{c['med_cycles']},"
                        f"{t['min_gas']},{t['max_gas']},{t['min_native']},{t['max_native']},"
                        f"{c['min_cycles']},{c['max_cycles']},"
                        f"{r['med_cycles_per_gas']:.2f},{r['med_native_per_gas']:.2f},"
                        f"{r['med_cycles_per_native']:.4f},"
                        f"{r['worst_cycles_per_gas']:.2f},{r['worst_native_per_gas']:.2f}\n")
        print(f"\nCSV written to {args.csv}")


if __name__ == "__main__":
    main()
