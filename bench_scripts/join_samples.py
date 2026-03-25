"""Join per-execution opcode samples (gas/native from tracer, cycles from RISC-V).

Reads:
  - <tracer_dir>/<OPCODE>.samples: "gas,native" per line (execution order)
  - <cycles_dir>/<OPCODE>.cycles: "cycles" per line (execution order)

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


def load_tracer_samples(path):
    """Load gas,native pairs from .samples file."""
    samples = []
    with open(path) as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            parts = line.split(",")
            samples.append((int(parts[0]), int(parts[1])))
    return samples


def load_cycle_samples(path):
    """Load cycle values from .cycles file."""
    samples = []
    with open(path) as f:
        for line in f:
            line = line.strip()
            if line:
                samples.append(int(line))
    return samples


def ratio(num, den):
    return num / den if den > 0 else 0.0


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

    def percentile(sorted_vals, p):
        # Nearest-rank method: rank = ceil(p/100 * N), 1-indexed
        rank = max(1, -(-len(sorted_vals) * p // 100))  # ceiling division
        return sorted_vals[min(rank, len(sorted_vals)) - 1]

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

    # Find opcodes present in both directories
    tracer_opcodes = {f.replace(".samples", "") for f in os.listdir(args.tracer_dir) if f.endswith(".samples")}
    cycle_opcodes = {f.replace(".cycles", "") for f in os.listdir(args.cycles_dir) if f.endswith(".cycles")}
    common = sorted(tracer_opcodes & cycle_opcodes)

    if not common:
        print("No matching opcodes found between tracer and cycle directories.", file=sys.stderr)
        sys.exit(1)

    summaries = []
    for name in common:
        tracer_path = os.path.join(args.tracer_dir, f"{name}.samples")
        cycles_path = os.path.join(args.cycles_dir, f"{name}.cycles")

        tracer_samples = load_tracer_samples(tracer_path)
        cycle_samples = load_cycle_samples(cycles_path)

        summary = process_opcode(name, tracer_samples, cycle_samples, args.out_dir)
        if summary:
            summaries.append(summary)

    if not args.summary and not args.out_dir:
        args.summary = True

    if args.summary and summaries:
        # Sort by worst-case cycles/gas
        summaries.sort(key=lambda s: s["max_cpg"], reverse=True)
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
