"""Compare per-opcode EVM stats (gas, native, ratios) between base and head benchmark runs.

Usage:
    python compare_opcode_stats.py base_block.out head_block.out [label]

Exits 0 with no output if nothing changed or base has no stats.
Prints a compact markdown table only when differences exist.
"""

import sys
import re


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
    if len(sys.argv) < 3:
        print("Usage: python compare_opcode_stats.py <base.out> <head.out> [label]", file=sys.stderr)
        sys.exit(1)

    base_file = sys.argv[1]
    head_file = sys.argv[2]
    label = sys.argv[3] if len(sys.argv) > 3 else ""

    base_stats = parse_opcode_stats(base_file)
    head_stats = parse_opcode_stats(head_file)

    # If base has no stats (old branch), silently exit
    if not base_stats:
        sys.exit(0)

    rows = compare(base_stats, head_stats)
    if not rows:
        sys.exit(0)

    print(format_table(rows, label))


if __name__ == "__main__":
    main()
