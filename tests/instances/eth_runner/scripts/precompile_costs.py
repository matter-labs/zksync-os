#!/usr/bin/env python3
"""Cycles spent per precompile over a directory of flamegraphs.

For zksync-os the precompiles enter through `pure_system_function_hook_impl`; the
frame right below it names the system function. For zilkworm (evmone) the entry is
`call_precompile` and the frame below it is the `*_execute` / `*_analyze` function.
Prints the cycles per precompile summed over the blocks (inclusive, delegation rows
included by weighting with the block's reported cycles when a cycles.csv is present),
and the blocks where a precompile takes the largest share.

Usage: precompile_costs.py DIR [--rate 100] [--side zksync-os|zilkworm] [--top 12]
"""
import argparse
import collections
import csv
import glob
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import analyze_flamegraphs as fg  # noqa: E402

sys.setrecursionlimit(20000)

ENTRY = {
    "zksync-os": re.compile(r"pure_system_function_hook_impl"),
    "zilkworm": re.compile(r"call_precompile|evmone::state::.*precompile"),
}
# the first frame below the entry that names the precompile
NAME = {
    "zksync-os": re.compile(r"system_functions::([a-z0-9_]+)::"),
    "zilkworm": re.compile(r"(?:^|::|\s)([a-z0-9_]+)_(?:execute|analyze)\b"),
}
DELEGATION_LEAF = re.compile(r"keccak_special5::keccak_f1600|keccak_f1600")
NO_STACK_LEAVES = [
    (re.compile(r"sha2::|compress256"), "sha256 (no stack)"),
    (re.compile(r"ripemd"), "ripemd160 (no stack)"),
    (re.compile(r"blake2"), "blake2f (no stack)"),
]


def short(name):
    name = re.sub(r"::<.*", "", name)
    name = re.sub(r"^<(.*) as .*>::(\w+)$", r"\1::\2", name)
    return name[-90:]


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("dir")
    ap.add_argument("--rate", type=float, default=100.0, help="cycles per sample")
    ap.add_argument("--side", default="zksync-os", choices=list(ENTRY))
    ap.add_argument("--top", type=int, default=12)
    args = ap.parse_args()
    entry = ENTRY[args.side]
    name_rx = NAME[args.side]

    reported = {}
    gas_of = {}
    csv_path = os.path.join(args.dir, "cycles.csv")
    if os.path.exists(csv_path):
        with open(csv_path) as fh:
            for row in csv.DictReader(fh):
                if (row.get("cycles") or "").isdigit():
                    reported[row["block"]] = int(row["cycles"])
                    gas_of[row["block"]] = int(row.get("gas") or 0)

    total_by = collections.Counter()
    blocks_by = collections.Counter()
    per_block = {}  # block -> Counter
    total_cycles = 0
    for svg in sorted(glob.glob(os.path.join(args.dir, "*.svg"))):
        block = os.path.basename(svg).split(".")[0]
        samples, frames = fg.parse_svg(svg)
        roots = fg.build_tree(frames)
        cycles = reported.get(block, samples * args.rate)
        # delegation rows: samples on the keccak delegation leaf stand for more cycles
        leaf = sum(f["samples"] - sum(c["samples"] for c in f["children"])
                   for f in frames if DELEGATION_LEAF.search(f["name"]))
        weight = 1.0
        if cycles > samples * args.rate and leaf:
            weight = 1.0 + (cycles - samples * args.rate) / (leaf * args.rate)
        total_cycles += cycles
        here = collections.Counter()

        def weighted(f):
            own = f["samples"] - sum(c["samples"] for c in f["children"])
            w = own * (weight if DELEGATION_LEAF.search(f["name"]) else 1.0)
            return w + sum(weighted(c) for c in f["children"])

        def walk(f, under_entry):
            name = fg.strip_name(f["name"])
            if under_entry:
                m = name_rx.search(name)
                if m:
                    here[m.group(1)] += weighted(f) * args.rate
                    return
                own = f["samples"] - sum(c["samples"] for c in f["children"])
                if own:
                    here["(entry / dispatch)"] += own * args.rate
                for c in f["children"]:
                    walk(c, True)
                return
            for c in f["children"]:
                walk(c, bool(entry.search(name)))

        for r in roots:
            walk(r, False)
        # leaf hash functions compiled without frame pointers are sampled with no chain
        # (zksync-os); count them for their precompile
        for r in roots:
            # the synthetic root holds the chainless samples as direct children
            for top in [r] + r["children"]:
                name = fg.strip_name(top["name"])
                for rx, key in NO_STACK_LEAVES:
                    if rx.search(name):
                        here[key] += weighted(top) * args.rate
        per_block[block] = here
        for k, v in here.items():
            total_by[k] += v
            blocks_by[k] += 1

    print(f"{args.dir}: {len(per_block)} blocks, {total_cycles/1e9:.3f} Gcycles; "
          f"precompiles {sum(total_by.values())/1e9:.3f} Gcycles ({100*sum(total_by.values())/total_cycles:.1f}%)")
    print(f"{'Mcycles':>10} {'share':>6} {'blocks':>6} {'max share':>9} {'in block':>9}  precompile")
    for k, v in total_by.most_common(args.top):
        worst = max(per_block, key=lambda b: per_block[b].get(k, 0) / reported.get(b, 1))
        wshare = per_block[worst].get(k, 0) / reported.get(worst, 1)
        print(f"{v/1e6:10.1f} {100*v/total_cycles:5.1f}% {blocks_by[k]:6d} {100*wshare:8.1f}% {worst:>9}  {k}")

    print("\nblocks with the largest precompile share:")
    ranked = sorted(per_block, key=lambda b: -sum(per_block[b].values()) / reported.get(b, 1))
    for b in ranked[: args.top]:
        tot = sum(per_block[b].values())
        parts = ", ".join(f"{k.split('::')[-1]} {v/1e6:.0f}M" for k, v in per_block[b].most_common(3))
        cpg = reported.get(b, 0) / gas_of.get(b, 1) if gas_of.get(b) else 0
        print(f"  {b}: {100*tot/reported.get(b, 1):5.1f}% of {reported.get(b, 0)/1e6:7.1f} M ({cpg:.2f} c/gas): {parts}")


if __name__ == "__main__":
    main()
