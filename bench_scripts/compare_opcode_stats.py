"""Compare per-opcode EVM stats (gas, native, ratios) between base and head benchmark runs.

Usage:
    python compare_opcode_stats.py <base.out> <head.out> [label]
    python compare_opcode_stats.py <b1.out> <h1.out> <b2.out> <h2.out> ...

When multiple file pairs are given, stats are aggregated across all pairs.
Exits 0 with no output if nothing changed or base has no stats.
Prints a compact markdown table only when differences exist.
"""

import os
import re
import sys


def median_int(values):
    """Return the true median of integer samples."""
    if not values:
        return 0
    sorted_vals = sorted(values)
    mid = len(sorted_vals) // 2
    if len(sorted_vals) % 2 == 0:
        return (sorted_vals[mid - 1] + sorted_vals[mid]) // 2
    return sorted_vals[mid]


def parse_opcode_stats(filename):
    """Parse the '=== EVM Opcode Stats:' table from a benchmark .out file."""
    stats = {}
    try:
        with open(filename) as f:
            text = f.read()
    except FileNotFoundError:
        return stats

    match = re.search(
        r"=== EVM Opcode Stats:\n(.+?)\n={5,}",
        text,
        re.DOTALL,
    )
    if not match:
        return stats

    for line in match.group(1).strip().splitlines()[1:]:  # skip header
        parts = line.split()
        if len(parts) < 10:
            continue
        name = parts[0]
        # Skip CALL-like opcodes that show "-" for gas/native columns
        if parts[2] == "-":
            stats[name] = {"count": int(parts[1])}
            continue
        try:
            stats[name] = {
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


def load_tracer_samples(samples_dir):
    """Load per-opcode gas/native samples from a directory."""
    stats = {}
    try:
        entries = os.listdir(samples_dir)
    except OSError:
        return stats

    for name in entries:
        if not name.endswith(".samples"):
            continue
        opcode = name[:-len(".samples")]
        rows = []
        with open(os.path.join(samples_dir, name)) as f:
            for line in f:
                line = line.strip()
                if not line:
                    continue
                gas, native = line.split(",")
                rows.append((int(gas), int(native)))
        if rows:
            stats[opcode] = rows
    return stats


def aggregate_sampled_opcode_stats(sample_dirs):
    """Aggregate exact per-execution gas/native samples across runs."""
    combined = {}
    for samples_dir in sample_dirs:
        for op, rows in load_tracer_samples(samples_dir).items():
            entry = combined.setdefault(op, {"gas": [], "native": []})
            for gas, native in rows:
                entry["gas"].append(gas)
                entry["native"].append(native)

    result = {}
    for op, samples in combined.items():
        gas_samples = samples["gas"]
        native_samples = samples["native"]
        if not gas_samples:
            continue
        result[op] = {
            "count": len(gas_samples),
            "avg_gas": sum(gas_samples) / len(gas_samples),
            "med_gas": median_int(gas_samples),
            "min_gas": min(gas_samples),
            "max_gas": max(gas_samples),
            "avg_native": sum(native_samples) / len(native_samples),
            "med_native": median_int(native_samples),
            "min_native": min(native_samples),
            "max_native": max(native_samples),
            "total_native": sum(native_samples),
        }
    return result


def aggregate_opcode_stats(all_stats):
    """Aggregate opcode stats across multiple benchmark files.

    Sums counts, computes count-weighted averages for medians, and
    takes min/max across files.
    """
    combined = {}
    for stats in all_stats:
        for op, s in stats.items():
            if "avg_gas" not in s:
                # CALL-like opcode with count only
                if op not in combined:
                    combined[op] = {"count": 0}
                combined[op]["count"] += s.get("count", 0)
                continue
            cnt = s["count"]
            if op not in combined:
                combined[op] = {
                    "count": 0,
                    "_wt_med_gas": 0,
                    "_wt_med_native": 0,
                    "_total_native": 0,
                    "min_gas": s["min_gas"],
                    "max_gas": s["max_gas"],
                    "min_native": s["min_native"],
                    "max_native": s["max_native"],
                }
            c = combined[op]
            c["count"] += cnt
            c["_wt_med_gas"] += s["med_gas"] * cnt
            c["_wt_med_native"] += s["med_native"] * cnt
            c["_total_native"] += s["avg_native"] * cnt
            c["min_gas"] = min(c["min_gas"], s["min_gas"])
            c["max_gas"] = max(c["max_gas"], s["max_gas"])
            c["min_native"] = min(c["min_native"], s["min_native"])
            c["max_native"] = max(c["max_native"], s["max_native"])

    for c in combined.values():
        if "_wt_med_gas" in c:
            total = c["count"]
            if total > 0:
                c["avg_gas"] = c["_wt_med_gas"] / total
                c["med_gas"] = round(c["_wt_med_gas"] / total)
                c["avg_native"] = c["_wt_med_native"] / total
                c["med_native"] = round(c["_wt_med_native"] / total)
            else:
                c["avg_gas"] = 0
                c["med_gas"] = 0
                c["avg_native"] = 0
                c["med_native"] = 0
            c["total_native"] = c["_total_native"]
            del c["_wt_med_gas"]
            del c["_wt_med_native"]
            del c["_total_native"]
    return combined


def overlay_sampled_stats(base_stats, sampled_stats):
    """Replace aggregate summaries with exact sample-backed metrics when available."""
    merged = dict(base_stats)
    for op, sample_stats in sampled_stats.items():
        existing = dict(merged.get(op, {}))
        existing.update(sample_stats)
        merged[op] = existing
    return merged


def pct(old, new):
    if old == 0:
        return 0.0 if new == 0 else float("inf")
    return (new - old) / old * 100


def fmt_pct(val):
    if abs(val) < 0.005:
        return ""
    return f" ({val:+.1f}%)"


def compare(base_stats, head_stats):
    """Return list of rows for opcodes with changed avg_gas or avg_native."""
    all_opcodes = sorted(set(base_stats) | set(head_stats))
    rows = []
    for op in all_opcodes:
        b = base_stats.get(op, {})
        h = head_stats.get(op, {})

        # Skip if either side has no gas data (CALL-like or missing)
        if "avg_gas" not in b and "avg_gas" not in h:
            continue

        b_avg_gas = b.get("avg_gas", 0)
        h_avg_gas = h.get("avg_gas", 0)
        b_med_gas = b.get("med_gas", 0)
        h_med_gas = h.get("med_gas", 0)
        b_avg_native = b.get("avg_native", 0)
        h_avg_native = h.get("avg_native", 0)
        b_med_native = b.get("med_native", 0)
        h_med_native = h.get("med_native", 0)
        b_count = b.get("count", 0)
        h_count = h.get("count", 0)

        # Compute native/gas ratio (using medians for stability)
        b_ratio = b_med_native / b_med_gas if b_med_gas > 0 else 0
        h_ratio = h_med_native / h_med_gas if h_med_gas > 0 else 0

        # Check if anything meaningful changed
        gas_changed = b_med_gas != h_med_gas
        native_changed = b_med_native != h_med_native
        count_changed = b_count != h_count

        if not (gas_changed or native_changed or count_changed):
            continue

        rows.append({
            "op": op,
            "b_count": b_count,
            "h_count": h_count,
            "b_med_gas": b_med_gas,
            "h_med_gas": h_med_gas,
            "b_med_native": b_med_native,
            "h_med_native": h_med_native,
            "b_ratio": b_ratio,
            "h_ratio": h_ratio,
            "b_total_native": b.get("total_native", b_count * b_avg_native),
            "h_total_native": h.get("total_native", h_count * h_avg_native),
        })
    return rows


def format_table(rows, label=""):
    """Format comparison rows as a compact markdown table."""
    if not rows:
        return ""

    lines = []
    title = f"#### Opcode stats diff"
    if label:
        title += f" ({label})"
    lines.append(title)
    lines.append("")
    lines.append(
        "| Opcode | Count | Med Gas | Med Native | Native/Gas |"
    )
    lines.append(
        "|--------|-------|---------|------------|------------|"
    )
    # Sort by total native cost descending (count * avg_native, honest total)
    rows.sort(key=lambda r: r["h_total_native"], reverse=True)

    for r in rows:
        count_s = f"{r['h_count']}"
        if r['b_count'] != r['h_count']:
            count_s += fmt_pct(pct(r['b_count'], r['h_count']))

        gas_s = f"{r['h_med_gas']}"
        gas_s += fmt_pct(pct(r['b_med_gas'], r['h_med_gas']))

        native_s = f"{r['h_med_native']}"
        native_s += fmt_pct(pct(r['b_med_native'], r['h_med_native']))

        ratio_s = f"{r['h_ratio']:.1f}"
        ratio_s += fmt_pct(pct(r['b_ratio'], r['h_ratio']))

        lines.append(f"| `{r['op']}` | {count_s} | {gas_s} | {native_s} | {ratio_s} |")

    return "\n".join(lines)


def main():
    args = sys.argv[1:]
    if len(args) < 2:
        print(
            "Usage: python compare_opcode_stats.py <base.out> <head.out> [label] "
            "[--sample-dirs <base_dir> <head_dir>]\n"
            "       python compare_opcode_stats.py <b1.out> <h1.out> <b2.out> <h2.out> ... "
            "[--sample-dirs <b1_dir> <h1_dir> ...]",
            file=sys.stderr,
        )
        sys.exit(1)

    sample_args = []
    if "--sample-dirs" in args:
        idx = args.index("--sample-dirs")
        sample_args = args[idx + 1 :]
        args = args[:idx]

    label = ""
    # Backward compat: odd arg count means last is a label
    if len(args) % 2 == 1:
        label = args.pop()

    if len(args) < 2 or len(args) % 2 != 0:
        print("Error: need even number of files (base/head pairs)", file=sys.stderr)
        sys.exit(1)

    # Parse and aggregate all pairs
    all_base = [parse_opcode_stats(args[j]) for j in range(0, len(args), 2)]
    all_head = [parse_opcode_stats(args[j]) for j in range(1, len(args), 2)]
    base_stats = aggregate_opcode_stats(all_base)
    head_stats = aggregate_opcode_stats(all_head)

    if sample_args:
        if len(sample_args) % 2 != 0:
            print("Error: --sample-dirs needs even number of directories", file=sys.stderr)
            sys.exit(1)
        base_stats = overlay_sampled_stats(
            base_stats, aggregate_sampled_opcode_stats(sample_args[0::2])
        )
        head_stats = overlay_sampled_stats(
            head_stats, aggregate_sampled_opcode_stats(sample_args[1::2])
        )

    # If base has no stats (old branch), silently exit
    if not base_stats:
        sys.exit(0)

    rows = compare(base_stats, head_stats)
    if not rows:
        sys.exit(0)

    print(format_table(rows, label))


if __name__ == "__main__":
    main()
