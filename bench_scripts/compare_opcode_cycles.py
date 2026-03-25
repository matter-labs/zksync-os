"""Compare per-opcode RISC-V cycle stats between base and head benchmark runs.

Reads the '=== Per-opcode cycle stats:' section from .bench files produced
by cycle_marker::print_cycle_markers() and outputs a compact markdown table
showing median cycle changes per opcode.

Usage:
    python compare_opcode_cycles.py <base.bench> <head.bench> [label]

Exits 0 with no output if nothing changed or base has no stats.
"""

import sys
import re


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

    for line in match.group(1).strip().splitlines()[1:]:  # skip header
        parts = line.split()
        if len(parts) < 7:
            continue
        try:
            stats[parts[0]] = {
                "count": int(parts[1]),
                "total_cycles": int(parts[2]),
                "med_cycles": int(parts[4]),
                "min_cycles": int(parts[5]),
                "max_cycles": int(parts[6]),
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
    """Return list of rows for opcodes with changed cycle counts."""
    all_opcodes = sorted(set(base_stats) | set(head_stats))
    rows = []
    for op in all_opcodes:
        b = base_stats.get(op, {})
        h = head_stats.get(op, {})

        b_med = b.get("med_cycles", 0)
        h_med = h.get("med_cycles", 0)
        b_total = b.get("total_cycles", 0)
        h_total = h.get("total_cycles", 0)
        b_count = b.get("count", 0)
        h_count = h.get("count", 0)

        med_changed = b_med != h_med
        count_changed = b_count != h_count

        if not (med_changed or count_changed):
            continue

        rows.append({
            "op": op,
            "b_count": b_count,
            "h_count": h_count,
            "b_med": b_med,
            "h_med": h_med,
            "b_total": b_total,
            "h_total": h_total,
        })
    return rows


def format_table(rows, label=""):
    """Format comparison rows as a compact markdown table."""
    if not rows:
        return ""

    lines = []
    title = "#### Per-opcode cycle diff"
    if label:
        title += f" ({label})"
    lines.append(title)
    lines.append("")
    lines.append(
        "| Opcode | Count | Base Med Cycles | Head Med Cycles (%) | Base Total | Head Total (%) |"
    )
    lines.append(
        "|--------|-------|-----------------|---------------------|------------|----------------|"
    )

    # Sort by absolute total cycle change descending (biggest impact first)
    rows.sort(key=lambda r: abs(r["h_total"] - r["b_total"]), reverse=True)

    for r in rows:
        count_s = f"{r['h_count']}"
        if r['b_count'] != r['h_count']:
            count_s += fmt_pct(pct(r['b_count'], r['h_count']))

        med_pct = pct(r['b_med'], r['h_med'])
        med_pct_s = fmt_pct(med_pct) if med_pct != float("inf") else " (new)"

        total_pct = pct(r['b_total'], r['h_total'])
        total_pct_s = fmt_pct(total_pct) if total_pct != float("inf") else " (new)"

        lines.append(
            f"| `{r['op']}` | {count_s} | {r['b_med']:,} | {r['h_med']:,}{med_pct_s} "
            f"| {r['b_total']:,} | {r['h_total']:,}{total_pct_s} |"
        )

    lines.append("")  # trailing blank line to separate from next section
    return "\n".join(lines)


def main():
    if len(sys.argv) < 3:
        print(
            "Usage: python compare_opcode_cycles.py <base.bench> <head.bench> [label]",
            file=sys.stderr,
        )
        sys.exit(1)

    base_file = sys.argv[1]
    head_file = sys.argv[2]
    label = sys.argv[3] if len(sys.argv) > 3 else ""

    base_stats = parse_cycle_stats(base_file)
    head_stats = parse_cycle_stats(head_file)

    # If either side has no stats (old branch or broken build), silently exit
    if not base_stats or not head_stats:
        sys.exit(0)

    rows = compare(base_stats, head_stats)
    if not rows:
        sys.exit(0)

    print(format_table(rows, label))


if __name__ == "__main__":
    main()
