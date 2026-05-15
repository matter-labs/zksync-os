#!/usr/bin/env python3
"""Compare base vs head precompile stats CSVs.

Accepts one or more base/head CSV pairs as positional args and aggregates
stats across all of them (matching `compare_opcode_stats.py`'s shape).

Usage:
    python compare_precompile_stats.py <base.csv> <head.csv> [label]
    python compare_precompile_stats.py <b1.csv> <h1.csv> <b2.csv> <h2.csv> ... [label]

CSV columns produced by PrecompileStatsTracer::write_csv:
    name,address,count,avg_gas,median_gas,min_gas,max_gas,
    avg_native,median_native,min_native,max_native,native_per_gas

Aggregation across sources (per precompile):
- `count`: sum
- `avg_gas` / `avg_native`: count-weighted mean (re-derived from totals)
- `med_gas` / `med_native`: count-weighted mean of per-source medians
- `min_gas` / `min_native`: min across sources
- `max_gas` / `max_native`: max across sources

Exits 0 with no output if nothing changed or base CSVs are empty/absent.
"""

import csv
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from benchlib import fmt_pct, pct  # noqa: E402


def parse_csv(path):
    """Return dict keyed by precompile name."""
    stats = {}
    try:
        with open(path) as f:
            reader = csv.DictReader(f)
            for row in reader:
                try:
                    stats[row["name"]] = {
                        "address": row["address"],
                        "count": int(row["count"]),
                        "avg_gas": float(row["avg_gas"]),
                        "med_gas": int(row["median_gas"]),
                        "min_gas": int(row["min_gas"]),
                        "max_gas": int(row["max_gas"]),
                        "avg_native": float(row["avg_native"]),
                        "med_native": int(row["median_native"]),
                        "min_native": int(row["min_native"]),
                        "max_native": int(row["max_native"]),
                    }
                except (ValueError, KeyError):
                    continue
    except FileNotFoundError:
        pass
    return stats


def aggregate(sources):
    """Combine N per-source stat dicts into one aggregate dict per precompile.

    `count`s sum; min/max are extremes; averages and medians are count-weighted.
    """
    combined = {}
    for stats in sources:
        for name, s in stats.items():
            cnt = s["count"]
            if cnt <= 0:
                continue
            if name not in combined:
                combined[name] = {
                    "address": s["address"],
                    "count": 0,
                    "_wt_avg_gas": 0.0,
                    "_wt_avg_native": 0.0,
                    "_wt_med_gas": 0.0,
                    "_wt_med_native": 0.0,
                    "min_gas": s["min_gas"],
                    "max_gas": s["max_gas"],
                    "min_native": s["min_native"],
                    "max_native": s["max_native"],
                }
            c = combined[name]
            c["count"] += cnt
            c["_wt_avg_gas"] += s["avg_gas"] * cnt
            c["_wt_avg_native"] += s["avg_native"] * cnt
            c["_wt_med_gas"] += s["med_gas"] * cnt
            c["_wt_med_native"] += s["med_native"] * cnt
            c["min_gas"] = min(c["min_gas"], s["min_gas"])
            c["max_gas"] = max(c["max_gas"], s["max_gas"])
            c["min_native"] = min(c["min_native"], s["min_native"])
            c["max_native"] = max(c["max_native"], s["max_native"])

    for c in combined.values():
        total = c["count"]
        if total > 0:
            c["avg_gas"] = c["_wt_avg_gas"] / total
            c["avg_native"] = c["_wt_avg_native"] / total
            c["med_gas"] = round(c["_wt_med_gas"] / total)
            c["med_native"] = round(c["_wt_med_native"] / total)
        else:
            c["avg_gas"] = 0.0
            c["avg_native"] = 0.0
            c["med_gas"] = 0
            c["med_native"] = 0
        for k in ("_wt_avg_gas", "_wt_avg_native", "_wt_med_gas", "_wt_med_native"):
            del c[k]
    return combined


def compare(base, head):
    names = sorted(set(base) | set(head))
    rows = []
    for name in names:
        b = base.get(name, {})
        h = head.get(name, {})
        b_max_gas = b.get("max_gas", 0)
        h_max_gas = h.get("max_gas", 0)
        b_max_native = b.get("max_native", 0)
        h_max_native = h.get("max_native", 0)
        if b_max_gas == h_max_gas and b_max_native == h_max_native:
            continue
        rows.append(
            {
                "name": name,
                "address": h.get("address", b.get("address", "")),
                "b_count": b.get("count", 0),
                "h_count": h.get("count", 0),
                "b_max_gas": b_max_gas,
                "h_max_gas": h_max_gas,
                "b_med_gas": b.get("med_gas", 0),
                "h_med_gas": h.get("med_gas", 0),
                "b_max_native": b_max_native,
                "h_max_native": h_max_native,
                "b_med_native": b.get("med_native", 0),
                "h_med_native": h.get("med_native", 0),
            }
        )
    return rows


def format_table(rows, label=""):
    if not rows:
        return ""
    lines = []
    title = "#### Precompile gas/native worst-case"
    if label:
        title += f" ({label})"
    lines.append(title)
    lines.append("")
    lines.append(
        "| Precompile | Address | Count | Max Gas | Med Gas | Max Native | Med Native |"
    )
    lines.append(
        "|------------|---------|-------|---------|---------|------------|------------|"
    )
    rows.sort(key=lambda r: r["h_max_native"], reverse=True)
    for r in rows:
        count_s = f"{r['h_count']}"
        if r["b_count"] != r["h_count"]:
            count_s += fmt_pct(pct(r["b_count"], r["h_count"]))
        max_gas_s = f"{r['h_max_gas']}" + fmt_pct(pct(r["b_max_gas"], r["h_max_gas"]))
        med_gas_s = f"{r['h_med_gas']}" + fmt_pct(pct(r["b_med_gas"], r["h_med_gas"]))
        max_native_s = f"{r['h_max_native']}" + fmt_pct(
            pct(r["b_max_native"], r["h_max_native"])
        )
        med_native_s = f"{r['h_med_native']}" + fmt_pct(
            pct(r["b_med_native"], r["h_med_native"])
        )
        lines.append(
            f"| `{r['name']}` | `{r['address']}` | {count_s} | "
            f"{max_gas_s} | {med_gas_s} | {max_native_s} | {med_native_s} |"
        )
    return "\n".join(lines)


def main():
    args = sys.argv[1:]
    if len(args) < 2:
        print(
            "Usage: compare_precompile_stats.py <base.csv> <head.csv> [label]\n"
            "       compare_precompile_stats.py <b1.csv> <h1.csv> "
            "<b2.csv> <h2.csv> ... [label]",
            file=sys.stderr,
        )
        sys.exit(1)

    label = ""
    # Backward compat: odd arg count means the last arg is a label.
    if len(args) % 2 == 1:
        label = args.pop()

    if len(args) < 2 or len(args) % 2 != 0:
        print("Error: need even number of files (base/head pairs)", file=sys.stderr)
        sys.exit(1)

    base_paths = [args[j] for j in range(0, len(args), 2)]
    head_paths = [args[j] for j in range(1, len(args), 2)]
    base = aggregate([parse_csv(p) for p in base_paths])
    head = aggregate([parse_csv(p) for p in head_paths])

    if not head:
        # Head CSVs missing or unparsable (e.g. partial artifact). Without
        # head numbers there's nothing to report.
        sys.exit(0)
    if not base:
        # Base side has no instrumentation (typical on the PR that introduces
        # the precompile bench, where merge-base lacks the tracer). Print a
        # head-only table so the data is still visible in the PR comment.
        print(format_head_only_table(head, label))
        sys.exit(0)
    rows = compare(base, head)
    if not rows:
        sys.exit(0)
    print(format_table(rows, label))


def format_head_only_table(head, label=""):
    if not head:
        return ""
    summary = "Precompile gas/native worst-case (head only — base lacks instrumentation)"
    if label:
        summary += f" ({label})"
    lines = [
        f"<details><summary>{summary}</summary>",
        "",
        "| Precompile | Address | Count | Max Gas | Med Gas | Max Native | Med Native |",
        "|------------|---------|-------|---------|---------|------------|------------|",
    ]
    rows = sorted(head.items(), key=lambda kv: kv[1].get("max_native", 0), reverse=True)
    for name, h in rows:
        lines.append(
            f"| `{name}` | `{h.get('address', '')}` | {h['count']} | "
            f"{h['max_gas']} | {h['med_gas']} | {h['max_native']} | {h['med_native']} |"
        )
    lines.append("")
    lines.append("</details>")
    return "\n".join(lines)


if __name__ == "__main__":
    main()
