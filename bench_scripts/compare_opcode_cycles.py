"""Compare per-opcode RISC-V cycle stats between base and head benchmark runs.

Reads per-opcode cycle stats from .bench files and (optionally) gas stats from
.out files to produce a compact markdown table showing:
  - Median and total cycle changes per opcode
  - Median and worst-case cycles/gas ratio changes (when .out files provided)

The worst-case cycles/gas ratio (max_cycles / min_gas) is the most
security-relevant metric: a spike means an opcode is underpriced relative to
its proving cost and could be a DoS vector.

Usage:
    python compare_opcode_cycles.py <base.bench> <head.bench> [label]
    python compare_opcode_cycles.py <base.bench> <head.bench> [label] \\
        --gas-stats <base.out> <head.out>

Exits 0 with no output if either side has no stats or nothing changed.
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


def parse_gas_stats(filename):
    """Parse '=== EVM Opcode Stats:' from .out file (gas columns only)."""
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
        if len(parts) < 10:
            continue
        name = parts[0]
        if parts[2] == "-":  # CALL-like opcodes
            continue
        try:
            stats[name] = {
                "med_gas": int(parts[3]),
                "min_gas": int(parts[4]),
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


def ratio(num, den):
    """Return num/den, or None if den is zero."""
    return num / den if den > 0 else None


def compare(base_cycles, head_cycles, base_gas, head_gas):
    """Return list of rows for opcodes with changed cycle counts or ratios."""
    all_opcodes = sorted(set(base_cycles) | set(head_cycles))
    has_gas = bool(base_gas) and bool(head_gas)
    rows = []

    for op in all_opcodes:
        bc = base_cycles.get(op, {})
        hc = head_cycles.get(op, {})

        b_med = bc.get("med_cycles", 0)
        h_med = hc.get("med_cycles", 0)
        b_total = bc.get("total_cycles", 0)
        h_total = hc.get("total_cycles", 0)
        b_count = bc.get("count", 0)
        h_count = hc.get("count", 0)

        # Cycles/gas ratios (only when both sides have gas data for this opcode)
        b_med_cg = None
        h_med_cg = None
        b_worst_cg = None
        h_worst_cg = None
        bg = base_gas.get(op) if has_gas else None
        hg = head_gas.get(op) if has_gas else None
        has_gas_op = bg is not None and hg is not None
        if has_gas_op:
            b_med_cg = ratio(b_med, bg.get("med_gas", 0))
            h_med_cg = ratio(h_med, hg.get("med_gas", 0))
            b_worst_cg = ratio(bc.get("max_cycles", 0), bg.get("min_gas", 0))
            h_worst_cg = ratio(hc.get("max_cycles", 0), hg.get("min_gas", 0))

        med_changed = b_med != h_med
        total_changed = b_total != h_total
        count_changed = b_count != h_count
        # Use small tolerance for float ratio comparison
        cg_changed = (has_gas_op and b_med_cg is not None and h_med_cg is not None
                      and abs(b_med_cg - h_med_cg) > 0.05)
        worst_changed = (has_gas_op and b_worst_cg is not None and h_worst_cg is not None
                         and abs(b_worst_cg - h_worst_cg) > 0.05)

        if not (med_changed or total_changed or count_changed
                or cg_changed or worst_changed):
            continue

        rows.append({
            "op": op,
            "h_count": h_count,
            "b_count": b_count,
            "b_med": b_med,
            "h_med": h_med,
            "b_total": b_total,
            "h_total": h_total,
            "b_med_cg": b_med_cg,
            "h_med_cg": h_med_cg,
            "b_worst_cg": b_worst_cg,
            "h_worst_cg": h_worst_cg,
        })
    return rows, has_gas


def fmt_val_pct(base, head):
    """Format a head value with % change from base."""
    p = pct(base, head)
    s = fmt_pct(p) if p != float("inf") else " (new)"
    return f"{head:,}{s}"


def fmt_ratio_pct(base, head):
    """Format a float ratio with % change. Returns 'n/a' if either is None."""
    if base is None or head is None:
        return "n/a"
    p = pct(base, head)
    s = fmt_pct(p) if p != float("inf") else " (new)"
    return f"{head:.1f}{s}"


def format_table(rows, has_gas, label=""):
    """Format comparison rows as a compact markdown table."""
    if not rows:
        return ""

    lines = []
    title = "#### Per-opcode cycle diff"
    if label:
        title += f" ({label})"
    lines.append(title)
    lines.append("")

    if has_gas:
        lines.append(
            "| Opcode | Count | Med Cycles (%) | Total Cycles (%) "
            "| Med Cyc/Gas (%) | Worst Cyc/Gas (%) |"
        )
        lines.append(
            "|--------|-------|----------------|------------------"
            "|-----------------|-------------------|"
        )
    else:
        lines.append(
            "| Opcode | Count | Med Cycles (%) | Total Cycles (%) |"
        )
        lines.append(
            "|--------|-------|----------------|------------------|"
        )

    # Sort by absolute total cycle change descending (biggest impact first)
    rows.sort(key=lambda r: abs(r["h_total"] - r["b_total"]), reverse=True)

    for r in rows:
        count_s = f"{r['h_count']}"
        if r['b_count'] != r['h_count']:
            count_pct = pct(r['b_count'], r['h_count'])
            count_s += fmt_pct(count_pct) if count_pct != float("inf") else " (new)"

        med_s = fmt_val_pct(r['b_med'], r['h_med'])
        total_s = fmt_val_pct(r['b_total'], r['h_total'])

        if has_gas:
            med_cg_s = fmt_ratio_pct(r['b_med_cg'], r['h_med_cg'])
            worst_cg_s = fmt_ratio_pct(r['b_worst_cg'], r['h_worst_cg'])
            lines.append(
                f"| `{r['op']}` | {count_s} | {med_s} | {total_s} "
                f"| {med_cg_s} | {worst_cg_s} |"
            )
        else:
            lines.append(
                f"| `{r['op']}` | {count_s} | {med_s} | {total_s} |"
            )

    lines.append("")  # trailing blank line to separate from next section
    return "\n".join(lines)


def main():
    # Parse args: <base.bench> <head.bench> [label] [--gas-stats <base.out> <head.out>]
    args = sys.argv[1:]
    if len(args) < 2:
        print(
            "Usage: python compare_opcode_cycles.py <base.bench> <head.bench> [label] "
            "[--gas-stats <base.out> <head.out>]",
            file=sys.stderr,
        )
        sys.exit(1)

    base_bench = args[0]
    head_bench = args[1]

    label = ""
    base_out = None
    head_out = None

    i = 2
    while i < len(args):
        if args[i] == "--gas-stats" and i + 2 < len(args):
            base_out = args[i + 1]
            head_out = args[i + 2]
            i += 3
        else:
            label = args[i]
            i += 1

    base_cycles = parse_cycle_stats(base_bench)
    head_cycles = parse_cycle_stats(head_bench)

    if not base_cycles or not head_cycles:
        sys.exit(0)

    base_gas = parse_gas_stats(base_out) if base_out else {}
    head_gas = parse_gas_stats(head_out) if head_out else {}

    rows, has_gas = compare(base_cycles, head_cycles, base_gas, head_gas)
    if not rows:
        sys.exit(0)

    print(format_table(rows, has_gas, label))


if __name__ == "__main__":
    main()
