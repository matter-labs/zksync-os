#!/usr/bin/env python3
"""Analyze inferno flamegraph SVGs produced by `eth_runner ethproofs-flamegraph`.

Reads one or more SVGs, rebuilds the frame tree from the `<g><title>name (N
samples, P%)</title><rect .../>` elements and reports:

  * top-down: functions with the largest *inclusive* share (samples on any
    stack that contains the function, counted once per stack), i.e. the
    largest logical pieces of the execution;
  * bottom-up: functions with the largest *exclusive* (self) share, i.e. the
    individual hot functions where cycles are actually spent.

With several SVGs the shares are averaged per block (each block weighted
equally) and the per-block min/max share is shown.

Usage: analyze_flamegraphs.py [--top N] [--csv out.csv] block.svg [block.svg ...]
"""

import argparse
import collections
import csv
import html
import re
import sys

FRAME_RE = re.compile(
    r"<g>\s*<title>(?P<title>.*?)</title>\s*<rect\s+(?P<attrs>[^>]*?)/?>",
    re.DOTALL,
)
TITLE_RE = re.compile(r"^(?P<name>.*) \((?P<samples>[\d,]+) samples?, (?P<pct>[\d.]+)%\)$")
ATTR_RE = re.compile(r'([\w:]+)="([^"]*)"')


def parse_svg(path):
    """Return (total_samples, frames) where frames = list of dicts with
    name, samples, x (sample offset), w (sample width), level."""
    text = open(path, encoding="utf-8").read()
    frames = []
    for match in FRAME_RE.finditer(text):
        title = html.unescape(match.group("title"))
        attrs = dict(ATTR_RE.findall(match.group("attrs")))
        tm = TITLE_RE.match(title)
        if not tm or "fg:x" not in attrs or "fg:w" not in attrs or "y" not in attrs:
            continue
        frames.append(
            {
                "name": tm.group("name"),
                "samples": int(tm.group("samples").replace(",", "")),
                "x": int(attrs["fg:x"]),
                "w": int(attrs["fg:w"]),
                "y": float(attrs["y"]),
            }
        )
    if not frames:
        raise SystemExit(f"{path}: no frames found (is this an inferno flamegraph?)")
    # Levels: distinct y values; the root is drawn at the bottom (largest y).
    ys = sorted({f["y"] for f in frames}, reverse=True)
    level_of = {y: i for i, y in enumerate(ys)}
    for f in frames:
        f["level"] = level_of[f["y"]]
    root = min(frames, key=lambda f: f["level"])
    return root["samples"], frames


def build_tree(frames):
    """Attach children to each frame: next level, sample range inside the parent."""
    by_level = collections.defaultdict(list)
    for f in frames:
        f["children"] = []
        by_level[f["level"]].append(f)
    for level in sorted(by_level):
        parents = sorted(by_level[level], key=lambda f: f["x"])
        children = sorted(by_level.get(level + 1, []), key=lambda f: f["x"])
        pi = 0
        for c in children:
            while pi < len(parents) and parents[pi]["x"] + parents[pi]["w"] <= c["x"]:
                pi += 1
            if pi < len(parents) and parents[pi]["x"] <= c["x"] < parents[pi]["x"] + parents[pi]["w"]:
                parents[pi]["children"].append(c)
    return [f for f in frames if f["level"] == 0]


def strip_generics(name):
    """Remove balanced `<...>` generic parameter lists, but keep the
    `<Type as Trait>::method` shape readable by turning it into
    `Type as Trait::method`."""
    out = []
    depth = 0
    i = 0
    while i < len(name):
        ch = name[i]
        if ch == "<":
            # A leading `<` of a qualified path (`<X as Y>::m`) is kept when it
            # starts the name or follows `::`/a space; generic lists follow an
            # identifier character directly.
            prev = name[i - 1] if i > 0 else ""
            if depth == 0 and (i == 0 or prev in ": ,(["):
                out.append(ch)
                i += 1
                continue
            depth += 1
        elif ch == ">" and depth > 0:
            depth -= 1
        elif depth == 0:
            out.append(ch)
        i += 1
    return "".join(out)


def strip_name(name, short=True):
    # Drop rustc hash suffixes and generic noise to merge monomorphizations.
    name = re.sub(r"::h[0-9a-f]{16}$", "", name)
    if short:
        name = strip_generics(name)
    return name


def accumulate(roots, short=True):
    """Return (inclusive, exclusive) dicts of name -> samples."""
    inclusive = collections.Counter()
    exclusive = collections.Counter()

    def walk2(frame, ancestors):
        name = strip_name(frame["name"], short)
        child_samples = sum(c["samples"] for c in frame["children"])
        exclusive[name] += frame["samples"] - child_samples
        first = name not in ancestors
        if first:
            inclusive[name] += frame["samples"]
            ancestors.add(name)
        for c in frame["children"]:
            walk2(c, ancestors)
        if first:
            ancestors.remove(name)

    for r in roots:
        walk2(r, set())
    return inclusive, exclusive


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("svgs", nargs="+")
    ap.add_argument("--top", type=int, default=30)
    ap.add_argument("--csv", help="write the per-function averaged shares to this CSV")
    ap.add_argument("--skip", default="all", help="comma separated frame names to skip (default: the synthetic root)")
    ap.add_argument("--full-names", action="store_true", help="keep generic parameters in function names")
    args = ap.parse_args()
    skip = set(args.skip.split(",")) if args.skip else set()

    per_block_incl = []
    per_block_excl = []
    totals = []
    for path in args.svgs:
        total, frames = parse_svg(path)
        roots = build_tree(frames)
        incl, excl = accumulate(roots, short=not args.full_names)
        totals.append((path, total))
        per_block_incl.append({k: v / total for k, v in incl.items() if k not in skip})
        per_block_excl.append({k: v / total for k, v in excl.items() if k not in skip})

    n = len(args.svgs)
    print(f"{n} flamegraph(s):")
    for path, total in totals:
        print(f"  {path}: {total:,} samples")

    def summarize(per_block, label):
        names = set()
        for d in per_block:
            names.update(d)
        rows = []
        for name in names:
            shares = [d.get(name, 0.0) for d in per_block]
            rows.append((sum(shares) / n, min(shares), max(shares), name))
        rows.sort(reverse=True)
        print()
        print(f"=== {label} (top {args.top}) ===")
        print(f"{'avg %':>7} {'min %':>7} {'max %':>7}  function")
        for avg, lo, hi, name in rows[: args.top]:
            print(f"{avg*100:7.2f} {lo*100:7.2f} {hi*100:7.2f}  {name}")
        return rows

    incl_rows = summarize(per_block_incl, "TOP-DOWN: inclusive share (largest logical pieces)")
    excl_rows = summarize(per_block_excl, "BOTTOM-UP: exclusive/self share (hottest individual functions)")

    if args.csv:
        with open(args.csv, "w", newline="") as fh:
            w = csv.writer(fh)
            w.writerow(["kind", "avg_share", "min_share", "max_share", "function"])
            for avg, lo, hi, name in incl_rows:
                w.writerow(["inclusive", f"{avg:.6f}", f"{lo:.6f}", f"{hi:.6f}", name])
            for avg, lo, hi, name in excl_rows:
                w.writerow(["exclusive", f"{avg:.6f}", f"{lo:.6f}", f"{hi:.6f}", name])
        print(f"\nwrote {args.csv}")


if __name__ == "__main__":
    main()
