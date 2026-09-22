#!/usr/bin/env python3
"""Per-opcode handler costs of the zksync-os EVM interpreter from full-resolution
flamegraphs of a guest built with `--cfg opcode_profile` (hot handlers outlined),
optionally next to zilkworm's evmone handlers for the same blocks.

Usage: opcode_costs.py <zksync-os svg dir> [--zilkworm <dir>] [--rate-b 100]
Prints inclusive and self cycles per handler summed over the blocks, and the
host-side opcode counts when `opcode_sequences.csv` is present in the dir.
"""
import argparse, collections, glob, os, re, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import analyze_flamegraphs as fg  # noqa: E402
sys.setrecursionlimit(20000)

HANDLER = re.compile(r"hot::Hot.*>::(?P<name>[a-z_0-9]+)(?:::<(?P<generic>[^<>]*)>)?$")

def handlers(d, rate):
    inc = collections.Counter(); exc = collections.Counter(); total = 0
    for p in sorted(glob.glob(os.path.join(d, "2599*.svg"))):
        t, frames = fg.parse_svg(p); roots = fg.build_tree(frames); total += t
        i, e = fg.accumulate(roots, short=False)
        for name, s in i.items():
            m = HANDLER.search(name)
            if m:
                key = m.group("name") + (f"<{m.group('generic')}>" if m.group("generic") else "")
                inc[key] += s * rate
        for name, s in e.items():
            m = HANDLER.search(name)
            if m:
                key = m.group("name") + (f"<{m.group('generic')}>" if m.group("generic") else "")
                exc[key] += s * rate
    return total * rate, inc, exc

def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("dir"); ap.add_argument("--rate", type=float, default=1.0)
    ap.add_argument("--zilkworm"); ap.add_argument("--rate-b", type=float, default=100.0)
    ap.add_argument("--top", type=int, default=60)
    args = ap.parse_args()
    total, inc, exc = handlers(args.dir, args.rate)
    counts = collections.Counter()
    seq = os.path.join(args.dir, "opcode_sequences.csv")
    print(f"{args.dir}: {total/1e6:.0f} Mcycles in the flamegraphs; handler total incl {sum(inc.values())/1e6:.0f} M, self {sum(exc.values())/1e6:.0f} M")
    print(f"{'Mcyc incl':>10} {'Mcyc self':>10}  handler")
    for name, s in inc.most_common(args.top):
        print(f"{s/1e6:10.1f} {exc.get(name,0)/1e6:10.1f}  {name}")
    if args.zilkworm:
        zinc = collections.Counter(); zexc = collections.Counter()
        for p in sorted(glob.glob(os.path.join(args.zilkworm, "2599*.svg"))):
            t, frames = fg.parse_svg(p); roots = fg.build_tree(frames)
            i, e = fg.accumulate(roots)
            for name, s in i.items():
                if "evmone::instr::core::" in name or name in ("dispatch_cgoto",):
                    zinc[name] += s * args.rate_b; zexc[name] += e.get(name, 0) * args.rate_b
        print(f"\nzilkworm evmone handlers: {'Mcyc incl':>10} {'Mcyc self':>10}")
        for name, s in zinc.most_common(args.top):
            print(f"{s/1e6:10.1f} {zexc.get(name,0)/1e6:10.1f}  {name[:110]}")

if __name__ == "__main__":
    main()
