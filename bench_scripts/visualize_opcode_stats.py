"""Visualize per-opcode benchmarking stats.

Reads the joined per-execution CSVs and summary data to produce charts:
  1. Cycles/gas ratio distribution per opcode (box plot)
  2. Top opcodes by total cycle consumption (bar chart)
  3. Per-opcode scatter: gas vs cycles for selected opcodes

Usage:
    python visualize_opcode_stats.py <joined_dir> [--out-dir <output_dir>] [--opcodes OP1,OP2,...]
    python visualize_opcode_stats.py bench_results/joined --out-dir bench_results/charts
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


def load_all(joined_dir, filter_opcodes=None):
    data = {}
    for fname in sorted(os.listdir(joined_dir)):
        if not fname.endswith(".csv"):
            continue
        name = fname.replace(".csv", "")
        if filter_opcodes and name not in filter_opcodes:
            continue
        rows = load_joined_csv(os.path.join(joined_dir, fname))
        if rows:
            data[name] = rows
    return data


def plot_cpg_boxplot(data, out_path):
    """Box plot of cycles/gas ratio distribution per opcode."""
    # Sort by median cycles/gas descending
    items = []
    for name, rows in data.items():
        cpg_values = [r["cpg"] for r in rows if r["cpg"] > 0]
        if cpg_values:
            med = sorted(cpg_values)[len(cpg_values) // 2]
            items.append((name, cpg_values, med))
    items.sort(key=lambda x: x[2], reverse=True)

    # Top 30 for readability
    items = items[:30]

    if not items:
        return

    fig, ax = plt.subplots(figsize=(14, 8))
    labels = [it[0] for it in items]
    box_data = [it[1] for it in items]

    bp = ax.boxplot(box_data, vert=True, patch_artist=True, showfliers=True,
                    flierprops=dict(marker=".", markersize=2, alpha=0.3))

    for patch in bp["boxes"]:
        patch.set_facecolor("#4C72B0")
        patch.set_alpha(0.7)

    ax.set_xticklabels(labels, rotation=45, ha="right", fontsize=8)
    ax.set_ylabel("Cycles / Gas")
    ax.set_title("Cycles/Gas Ratio Distribution by Opcode (top 30 by median)")
    ax.grid(axis="y", alpha=0.3)
    plt.tight_layout()
    plt.savefig(out_path, dpi=150)
    plt.close()


def plot_total_cycles(data, out_path):
    """Bar chart of total cycles consumed per opcode."""
    totals = []
    for name, rows in data.items():
        total = sum(r["cycles"] for r in rows)
        count = len(rows)
        totals.append((name, total, count))
    totals.sort(key=lambda x: x[1], reverse=True)
    totals = totals[:30]

    if not totals:
        return

    fig, ax = plt.subplots(figsize=(14, 6))
    names = [t[0] for t in totals]
    values = [t[1] for t in totals]

    bars = ax.bar(names, values, color="#4C72B0", alpha=0.8)
    ax.set_xticklabels(names, rotation=45, ha="right", fontsize=8)
    ax.set_ylabel("Total Cycles")
    ax.set_title("Total Cycle Consumption by Opcode (top 30)")
    ax.yaxis.set_major_formatter(ticker.FuncFormatter(lambda x, _: f"{x/1e6:.1f}M"))
    ax.grid(axis="y", alpha=0.3)

    for bar, (_, total, count) in zip(bars, totals):
        ax.text(bar.get_x() + bar.get_width() / 2, bar.get_height(),
                f"n={count}", ha="center", va="bottom", fontsize=6)

    plt.tight_layout()
    plt.savefig(out_path, dpi=150)
    plt.close()


def plot_scatter(data, opcodes, out_path):
    """Scatter plot: gas vs cycles for selected opcodes."""
    fig, ax = plt.subplots(figsize=(10, 8))
    colors = plt.cm.tab10.colors

    for idx, name in enumerate(opcodes):
        if name not in data:
            continue
        rows = data[name]
        gas = [r["gas"] for r in rows]
        cycles = [r["cycles"] for r in rows]
        color = colors[idx % len(colors)]
        ax.scatter(gas, cycles, label=name, alpha=0.5, s=15, color=color)

    ax.set_xlabel("Gas")
    ax.set_ylabel("Cycles")
    ax.set_title("Gas vs Cycles per Execution")
    ax.legend(fontsize=8)
    ax.grid(alpha=0.3)
    plt.tight_layout()
    plt.savefig(out_path, dpi=150)
    plt.close()


def plot_cpg_histogram(data, opcodes, out_path):
    """Histogram of cycles/gas ratio for selected opcodes."""
    fig, axes = plt.subplots(len(opcodes), 1, figsize=(10, 3 * len(opcodes)))
    if len(opcodes) == 1:
        axes = [axes]

    for ax, name in zip(axes, opcodes):
        if name not in data:
            ax.set_title(f"{name} — no data")
            continue
        rows = data[name]
        cpg = [r["cpg"] for r in rows if r["cpg"] > 0]
        if not cpg:
            continue
        ax.hist(cpg, bins=50, color="#4C72B0", alpha=0.8, edgecolor="white")
        ax.set_title(f"{name} — cycles/gas distribution (n={len(cpg)})")
        ax.set_xlabel("Cycles / Gas")
        ax.set_ylabel("Count")
        ax.grid(alpha=0.3)

    plt.tight_layout()
    plt.savefig(out_path, dpi=150)
    plt.close()


def main():
    parser = argparse.ArgumentParser(description="Visualize per-opcode benchmarking stats")
    parser.add_argument("joined_dir", help="Directory with per-execution .csv files")
    parser.add_argument("--out-dir", default=".", help="Output directory for charts")
    parser.add_argument("--opcodes", help="Comma-separated opcodes for scatter/histogram (default: auto-select interesting ones)")
    args = parser.parse_args()

    data = load_all(args.joined_dir)
    if not data:
        print("No data found.", file=sys.stderr)
        sys.exit(1)

    os.makedirs(args.out_dir, exist_ok=True)

    # Auto-select interesting opcodes (high variance in cycles/gas)
    if args.opcodes:
        selected = args.opcodes.split(",")
    else:
        # Pick opcodes with highest max/median cpg ratio (most variance)
        variance = []
        for name, rows in data.items():
            cpg = sorted([r["cpg"] for r in rows if r["cpg"] > 0])
            if len(cpg) > 10:
                med = cpg[len(cpg) // 2]
                mx = cpg[-1]
                if med > 0:
                    variance.append((name, mx / med))
        variance.sort(key=lambda x: x[1], reverse=True)
        selected = [v[0] for v in variance[:6]]

    print(f"Generating charts for {len(data)} opcodes...")
    print(f"Selected for detail: {', '.join(selected)}")

    plot_cpg_boxplot(data, os.path.join(args.out_dir, "cpg_boxplot.png"))
    print(f"  -> {args.out_dir}/cpg_boxplot.png")

    plot_total_cycles(data, os.path.join(args.out_dir, "total_cycles.png"))
    print(f"  -> {args.out_dir}/total_cycles.png")

    if selected:
        plot_scatter(data, selected, os.path.join(args.out_dir, "gas_vs_cycles.png"))
        print(f"  -> {args.out_dir}/gas_vs_cycles.png")

        plot_cpg_histogram(data, selected, os.path.join(args.out_dir, "cpg_histograms.png"))
        print(f"  -> {args.out_dir}/cpg_histograms.png")

    print("Done.")


if __name__ == "__main__":
    main()
