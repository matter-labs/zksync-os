"""Compare per-opcode RISC-V cycle stats between base and head benchmark runs.

Reads per-opcode cycle stats from .bench files and (optionally) gas stats from
.out files or paired per-execution samples to produce a compact markdown table showing:
  - Median and total cycle changes per opcode
  - Median and worst-case cycles/gas ratio changes (when .out files provided)

When paired per-execution samples are available, medians and worst-case
cycles/gas are computed from aligned executions. Otherwise the script falls back
to aggregate summaries from the .bench/.out files.

Usage:
    python compare_opcode_cycles.py <base.bench> <head.bench> [label] \\
        [--gas-stats <base.out> <head.out>]
    python compare_opcode_cycles.py <b1.bench> <h1.bench> <b2.bench> <h2.bench> ... \\
        [--gas-stats <b1.out> <h1.out> <b2.out> <h2.out> ...]

When multiple file pairs are given, stats are aggregated across all pairs.
Exits 0 with no output if either side has no stats or nothing changed.
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


def median_float(values):
    """Return the true median of float samples."""
    if not values:
        return None
    sorted_vals = sorted(values)
    mid = len(sorted_vals) // 2
    if len(sorted_vals) % 2 == 0:
        return (sorted_vals[mid - 1] + sorted_vals[mid]) / 2
    return sorted_vals[mid]


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
                "count": int(parts[1]),
                "med_gas": int(parts[3]),
                "min_gas": int(parts[4]),
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


def load_cycle_samples(samples_dir):
    """Load per-opcode cycle samples from a directory."""
    stats = {}
    try:
        entries = os.listdir(samples_dir)
    except OSError:
        return stats

    for name in entries:
        if not name.endswith(".cycles"):
            continue
        opcode = name[:-len(".cycles")]
        rows = []
        with open(os.path.join(samples_dir, name)) as f:
            for line in f:
                line = line.strip()
                if line:
                    rows.append(int(line))
        if rows:
            stats[opcode] = rows
    return stats


def aggregate_sampled_cycle_stats(sample_dir_pairs):
    """Aggregate per-execution opcode samples across multiple block runs."""
    combined = {}
    for tracer_dir, cycle_dir in sample_dir_pairs:
        tracer = load_tracer_samples(tracer_dir)
        cycles = load_cycle_samples(cycle_dir)
        for op in sorted(set(tracer) & set(cycles)):
            n = min(len(tracer[op]), len(cycles[op]))
            if n == 0:
                continue
            entry = combined.setdefault(op, {"cycles": [], "gas": []})
            for idx in range(n):
                gas, _native = tracer[op][idx]
                entry["cycles"].append(cycles[op][idx])
                entry["gas"].append(gas)

    result = {}
    for op, samples in combined.items():
        cycle_samples = samples["cycles"]
        gas_samples = samples["gas"]
        ratios = [cycles / gas for cycles, gas in zip(cycle_samples, gas_samples) if gas > 0]
        if not cycle_samples:
            continue
        result[op] = {
            "count": len(cycle_samples),
            "total_cycles": sum(cycle_samples),
            "med_cycles": median_int(cycle_samples),
            "min_cycles": min(cycle_samples),
            "max_cycles": max(cycle_samples),
            "med_cg": median_float(ratios),
            "worst_cg": max(ratios) if ratios else None,
        }
    return result


def aggregate_cycle_stats(all_stats):
    """Aggregate cycle stats dicts, summing counts/totals across benchmark files."""
    combined = {}
    for stats in all_stats:
        for op, s in stats.items():
            if op not in combined:
                combined[op] = {
                    "count": 0,
                    "total_cycles": 0,
                    "_weighted_med": 0,
                    "min_cycles": s["min_cycles"],
                    "max_cycles": s["max_cycles"],
                }
            c = combined[op]
            c["count"] += s["count"]
            c["total_cycles"] += s["total_cycles"]
            c["_weighted_med"] += s["med_cycles"] * s["count"]
            c["min_cycles"] = min(c["min_cycles"], s["min_cycles"])
            c["max_cycles"] = max(c["max_cycles"], s["max_cycles"])
    for c in combined.values():
        c["med_cycles"] = round(c["_weighted_med"] / c["count"]) if c["count"] > 0 else 0
        del c["_weighted_med"]
    return combined


def aggregate_gas_stats(all_stats):
    """Aggregate gas stats dicts, taking min of min_gas and weighted avg of med_gas."""
    combined = {}
    for stats in all_stats:
        for op, s in stats.items():
            if op not in combined:
                combined[op] = {
                    "_total_count": 0,
                    "_weighted_med": 0,
                    "min_gas": s["min_gas"],
                }
            c = combined[op]
            c["_total_count"] += s["count"]
            c["_weighted_med"] += s["med_gas"] * s["count"]
            c["min_gas"] = min(c["min_gas"], s["min_gas"])
    for c in combined.values():
        c["med_gas"] = round(c["_weighted_med"] / c["_total_count"]) if c["_total_count"] > 0 else 0
        del c["_weighted_med"]
        del c["_total_count"]
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


def ratio(num, den):
    """Return num/den, or None if den is zero."""
    return num / den if den > 0 else None


def compare(base_cycles, head_cycles, base_gas, head_gas):
    """Return list of rows for opcodes with changed cycle counts or ratios."""
    all_opcodes = sorted(set(base_cycles) | set(head_cycles))
    has_gas = False
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

        # Prefer exact paired sample ratios when available; otherwise fall back
        # to ratios synthesized from aggregate gas/cycle summaries.
        b_med_cg = bc.get("med_cg")
        h_med_cg = hc.get("med_cg")
        b_worst_cg = bc.get("worst_cg")
        h_worst_cg = hc.get("worst_cg")

        has_gas_op = False
        if (
            b_med_cg is None
            or h_med_cg is None
            or b_worst_cg is None
            or h_worst_cg is None
        ):
            bg = base_gas.get(op)
            hg = head_gas.get(op)
            has_gas_op = bg is not None and hg is not None
            if has_gas_op:
                if b_med_cg is None:
                    b_med_cg = ratio(b_med, bg.get("med_gas", 0))
                if h_med_cg is None:
                    h_med_cg = ratio(h_med, hg.get("med_gas", 0))
                if b_worst_cg is None:
                    b_worst_cg = ratio(bc.get("max_cycles", 0), bg.get("min_gas", 0))
                if h_worst_cg is None:
                    h_worst_cg = ratio(hc.get("max_cycles", 0), hg.get("min_gas", 0))

        med_changed = b_med != h_med
        total_changed = b_total != h_total
        count_changed = b_count != h_count
        has_ratio_data = (
            b_med_cg is not None
            and h_med_cg is not None
            and b_worst_cg is not None
            and h_worst_cg is not None
        )
        # Use small tolerance for float ratio comparison
        cg_changed = (
            has_ratio_data
            and abs(b_med_cg - h_med_cg) > 0.05
        )
        worst_changed = (
            has_ratio_data
            and abs(b_worst_cg - h_worst_cg) > 0.05
        )

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
        has_gas = has_gas or has_ratio_data
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

    # Sort by head total cycles descending (biggest cost first)
    rows.sort(key=lambda r: r["h_total"], reverse=True)

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
    args = sys.argv[1:]
    if len(args) < 2:
        print(
            "Usage: python compare_opcode_cycles.py <base.bench> <head.bench> [label] "
            "[--gas-stats <base.out> <head.out>] "
            "[--sample-dirs <base_tracer_dir> <base_cycle_dir> <head_tracer_dir> <head_cycle_dir>]\n"
            "       python compare_opcode_cycles.py <b1.bench> <h1.bench> ... "
            "[--gas-stats <b1.out> <h1.out> ...] "
            "[--sample-dirs <b1_tracer> <b1_cycles> <h1_tracer> <h1_cycles> ...]",
            file=sys.stderr,
        )
        sys.exit(1)

    # Split args into bench files, gas-stats files, and optional label
    bench_args = []
    gas_args = []
    sample_args = []
    label = ""
    current = bench_args
    i = 0
    while i < len(args):
        if args[i] == "--gas-stats":
            current = gas_args
            i += 1
        elif args[i] == "--sample-dirs":
            current = sample_args
            i += 1
        elif args[i] == "--label" and i + 1 < len(args):
            label = args[i + 1]
            i += 2
        else:
            current.append(args[i])
            i += 1

    # Backward compat: odd positional arg count means last is a label
    if len(bench_args) % 2 == 1:
        label = bench_args.pop()

    if len(bench_args) < 2 or len(bench_args) % 2 != 0:
        print("Error: need even number of bench files (base/head pairs)", file=sys.stderr)
        sys.exit(1)

    # Parse and aggregate all pairs
    all_base = [parse_cycle_stats(bench_args[j]) for j in range(0, len(bench_args), 2)]
    all_head = [parse_cycle_stats(bench_args[j]) for j in range(1, len(bench_args), 2)]
    base_cycles = aggregate_cycle_stats(all_base)
    head_cycles = aggregate_cycle_stats(all_head)

    if not base_cycles or not head_cycles:
        sys.exit(0)

    base_gas = {}
    head_gas = {}
    if gas_args:
        if len(gas_args) % 2 != 0:
            print("Error: --gas-stats needs even number of files", file=sys.stderr)
            sys.exit(1)
        all_base_gas = [parse_gas_stats(gas_args[j]) for j in range(0, len(gas_args), 2)]
        all_head_gas = [parse_gas_stats(gas_args[j]) for j in range(1, len(gas_args), 2)]
        base_gas = aggregate_gas_stats(all_base_gas)
        head_gas = aggregate_gas_stats(all_head_gas)

    if sample_args:
        if len(sample_args) % 4 != 0:
            print(
                "Error: --sample-dirs needs groups of 4 dirs "
                "(base_tracer base_cycles head_tracer head_cycles)",
                file=sys.stderr,
            )
            sys.exit(1)
        base_sample_pairs = [
            (sample_args[j], sample_args[j + 1]) for j in range(0, len(sample_args), 4)
        ]
        head_sample_pairs = [
            (sample_args[j + 2], sample_args[j + 3]) for j in range(0, len(sample_args), 4)
        ]
        base_cycles = overlay_sampled_stats(
            base_cycles, aggregate_sampled_cycle_stats(base_sample_pairs)
        )
        head_cycles = overlay_sampled_stats(
            head_cycles, aggregate_sampled_cycle_stats(head_sample_pairs)
        )

    rows, has_gas = compare(base_cycles, head_cycles, base_gas, head_gas)
    if not rows:
        sys.exit(0)

    print(format_table(rows, has_gas, label))


if __name__ == "__main__":
    main()
