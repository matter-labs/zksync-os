"""Visualize per-opcode benchmarking stats.

Reads the joined per-execution CSVs to produce:
  1. Total cycle consumption bar chart (top opcodes)
  2. Sorted cycles/gas ratio curves per opcode (top consumers)
  3. Outlier analysis report

Usage:
    python visualize_opcode_stats.py <joined_dir> [--out-dir <output_dir>] [--top N]
"""

import os
import sys
import argparse
import csv

try:
    import matplotlib
    matplotlib.use("Agg")
    import matplotlib.pyplot as plt
    import matplotlib.ticker as ticker
except ImportError:
    print("matplotlib is required: pip3 install matplotlib", file=sys.stderr)
    sys.exit(1)


def load_joined_csv(path):
    rows = []
    with open(path) as f:
        reader = csv.DictReader(f)
        for row in reader:
            rows.append({
                "gas": int(row["gas"]),
                "native": int(row["native"]),
                "cycles": int(row["cycles"]),
                "cpg": float(row["cycles_per_gas"]),
                "npg": float(row["native_per_gas"]),
            })
    return rows


def load_all(joined_dir):
    data = {}
    for fname in sorted(os.listdir(joined_dir)):
        if not fname.endswith(".csv"):
            continue
        name = fname.replace(".csv", "")
        rows = load_joined_csv(os.path.join(joined_dir, fname))
        if rows:
            data[name] = rows
    return data


def percentile(sorted_vals, p):
    if not sorted_vals:
        return 0
    idx = min(int(len(sorted_vals) * p / 100), len(sorted_vals) - 1)
    return sorted_vals[idx]


def plot_total_cycles(data, out_path, top_n=25):
    """Bar chart of total cycles consumed per opcode."""
    totals = []
    for name, rows in data.items():
        total = sum(r["cycles"] for r in rows)
        count = len(rows)
        totals.append((name, total, count))
    totals.sort(key=lambda x: x[1], reverse=True)
    totals = totals[:top_n]

    fig, ax = plt.subplots(figsize=(14, 6))
    names = [t[0] for t in totals]
    values = [t[1] for t in totals]
    x = range(len(names))

    bars = ax.bar(x, values, color="#4C72B0", alpha=0.8)
    ax.set_xticks(x)
    ax.set_xticklabels(names, rotation=45, ha="right", fontsize=8)
    ax.set_ylabel("Total Cycles")
    ax.set_title(f"Total Cycle Consumption by Opcode (top {top_n})")
    ax.yaxis.set_major_formatter(ticker.FuncFormatter(lambda v, _: f"{v/1e6:.1f}M"))
    ax.grid(axis="y", alpha=0.3)

    for bar, (_, total, count) in zip(bars, totals):
        ax.text(bar.get_x() + bar.get_width() / 2, bar.get_height(),
                f"n={count}", ha="center", va="bottom", fontsize=6)

    plt.tight_layout()
    plt.savefig(out_path, dpi=150)
    plt.close()
    return [t[0] for t in totals]


def plot_sorted_cpg(data, opcodes, out_path):
    """Plot sorted cycles/gas ratios for each opcode on a single chart.

    X axis: execution index (sorted by cycles/gas ascending)
    Y axis: cycles/gas ratio
    Each opcode is a separate line — shows the full distribution shape.
    """
    fig, ax = plt.subplots(figsize=(14, 8))
    colors = plt.cm.tab20.colors

    for idx, name in enumerate(opcodes):
        if name not in data:
            continue
        cpg = sorted([r["cpg"] for r in data[name] if r["gas"] > 0])
        if not cpg:
            continue
        # Normalize x to [0, 1] so opcodes with different counts are comparable
        n = len(cpg)
        xs = [i / (n - 1) if n > 1 else 0.5 for i in range(n)]
        color = colors[idx % len(colors)]
        p50 = percentile(cpg, 50)
        ax.plot(xs, cpg, color=color, linewidth=1.5, alpha=0.8,
                label=f"{name} (n={n}, p50={p50:.0f})")

    ax.set_xlabel("Percentile (fraction of executions)")
    ax.set_ylabel("Cycles / Gas")
    ax.set_title("Sorted Cycles/Gas Ratios by Opcode")
    ax.set_yscale("log")
    ax.legend(fontsize=7, loc="upper left", ncol=2)
    ax.grid(alpha=0.3, which="both")
    ax.set_xlim(0, 1)
    plt.tight_layout()
    plt.savefig(out_path, dpi=150)
    plt.close()


def plot_sorted_cpg_detail(name, rows, out_path):
    """Per-opcode sorted cycles/gas curve with p50/p95/p99 annotations."""
    cpg = sorted([r["cpg"] for r in rows if r["gas"] > 0])
    if len(cpg) < 3:
        return

    n = len(cpg)
    xs = [i / (n - 1) if n > 1 else 0.5 for i in range(n)]

    p50 = percentile(cpg, 50)
    p95 = percentile(cpg, 95)
    p99 = percentile(cpg, 99)
    max_val = cpg[-1]

    fig, ax = plt.subplots(figsize=(10, 5))
    ax.plot(xs, cpg, color="#4C72B0", linewidth=1.5)
    ax.fill_between(xs, cpg, alpha=0.15, color="#4C72B0")

    # Annotate percentiles
    for pct, val, color in [(50, p50, "green"), (95, p95, "orange"), (99, p99, "red")]:
        ax.axhline(val, color=color, linestyle="--", linewidth=0.8, alpha=0.7)
        ax.text(0.02, val, f"p{pct}={val:.1f}", fontsize=8, color=color,
                va="bottom")

    ax.set_xlabel("Percentile (fraction of executions)")
    ax.set_ylabel("Cycles / Gas")
    ax.set_title(f"{name} — Sorted Cycles/Gas (n={n}, max={max_val:.1f})")
    ax.grid(alpha=0.3)
    ax.set_xlim(0, 1)
    plt.tight_layout()
    plt.savefig(out_path, dpi=150)
    plt.close()


def analyze_outliers(data, top_n=15):
    """Find and report extreme outlier executions across all opcodes."""
    all_outliers = []

    for name, rows in data.items():
        cpg = sorted([r["cpg"] for r in rows if r["cpg"] > 0])
        if len(cpg) < 5:
            continue

        p50 = percentile(cpg, 50)
        p99 = percentile(cpg, 99)
        max_cpg = cpg[-1]

        if p50 == 0:
            continue

        severity = max_cpg / p50

        worst = max((r for r in rows if r["gas"] > 0), key=lambda r: r["cpg"], default=None)

        if worst and severity > 1.5:
            all_outliers.append({
                "opcode": name,
                "count": len(rows),
                "p50_cpg": p50,
                "p95_cpg": percentile(cpg, 95),
                "p99_cpg": p99,
                "max_cpg": max_cpg,
                "severity": severity,
                "worst_gas": worst["gas"],
                "worst_native": worst["native"],
                "worst_cycles": worst["cycles"],
            })

    all_outliers.sort(key=lambda x: x["severity"], reverse=True)
    return all_outliers[:top_n]


def write_outlier_report(outliers, out_path):
    """Write outlier analysis as markdown."""
    with open(out_path, "w") as f:
        f.write("# Opcode Outlier Analysis\n\n")
        f.write("Opcodes sorted by worst-case / median cycles/gas ratio (severity).\n")
        f.write("High severity = the worst execution is disproportionately expensive relative to typical.\n\n")

        f.write("| Opcode | Count | p50 c/g | p95 c/g | p99 c/g | max c/g | severity | worst execution |\n")
        f.write("|--------|-------|---------|---------|---------|---------|----------|------------------|\n")

        for o in outliers:
            worst = f"gas={o['worst_gas']}, cycles={o['worst_cycles']}, native={o['worst_native']}"
            f.write(f"| `{o['opcode']}` "
                    f"| {o['count']} "
                    f"| {o['p50_cpg']:.1f} "
                    f"| {o['p95_cpg']:.1f} "
                    f"| {o['p99_cpg']:.1f} "
                    f"| {o['max_cpg']:.1f} "
                    f"| **{o['severity']:.1f}x** "
                    f"| {worst} |\n")

        f.write("\n### Interpretation\n\n")
        f.write("- **severity** = max(cycles/gas) / median(cycles/gas). Higher = more variable proving cost per gas.\n")
        f.write("- Opcodes with severity > 10x are candidates for native model tuning.\n")
        f.write("- Worst execution shows the actual gas/native/cycles for the most expensive invocation.\n")


def main():
    parser = argparse.ArgumentParser(description="Visualize per-opcode benchmarking stats")
    parser.add_argument("joined_dir", help="Directory with per-execution .csv files")
    parser.add_argument("--out-dir", default=".", help="Output directory for charts")
    parser.add_argument("--top", type=int, default=12, help="Number of top opcodes for detail plots")
    args = parser.parse_args()

    data = load_all(args.joined_dir)
    if not data:
        print("No data found.", file=sys.stderr)
        sys.exit(1)

    os.makedirs(args.out_dir, exist_ok=True)

    # 1. Total cycles bar chart
    top_opcodes = plot_total_cycles(data, os.path.join(args.out_dir, "total_cycles.png"))
    print(f"  -> {args.out_dir}/total_cycles.png")

    # 2. Combined sorted cycles/gas curves
    plot_sorted_cpg(data, top_opcodes[:args.top], os.path.join(args.out_dir, "sorted_cpg.png"))
    print(f"  -> {args.out_dir}/sorted_cpg.png")

    # 3. Per-opcode detail curves for top consumers
    detail_dir = os.path.join(args.out_dir, "detail")
    os.makedirs(detail_dir, exist_ok=True)
    for name in top_opcodes[:args.top]:
        if name in data:
            out = os.path.join(detail_dir, f"{name}.png")
            plot_sorted_cpg_detail(name, data[name], out)
            print(f"  -> {out}")

    print("Done.")


if __name__ == "__main__":
    main()
