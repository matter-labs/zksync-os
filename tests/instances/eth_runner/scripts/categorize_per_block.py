#!/usr/bin/env python3
"""Per-block component cycles/gas table from a flamegraph dir (see categorize_flamegraphs.py).

Usage: categorize_per_block.py DIR --scale 1 [--blocks A,B,..] [--components 'EVM interpreter,MPT / state commitment,...']
"""
import argparse, collections, csv, os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import categorize_flamegraphs as cf  # noqa: E402

ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
ap.add_argument("dir"); ap.add_argument("--scale", type=float, default=1.0)
ap.add_argument("--blocks"); ap.add_argument("--components", default="EVM interpreter,keccak (no stack),MPT / state commitment,secp256k1 (ecrecover),bytecode / preimage cache,storage slot cache,account cache,bn254 precompiles,bls12-381 / kzg,bootloader runner,oracle IO,other")
args = ap.parse_args()
reported, gas = {}, {}
for row in csv.DictReader(open(os.path.join(args.dir, "cycles.csv"))):
    if (row.get("cycles") or "").isdigit():
        reported[row["block"]] = int(row["cycles"]); gas[row["block"]] = int(row["gas"])
blocks = args.blocks.split(",") if args.blocks else sorted(reported)
comps = args.components.split(",")
print(f"{'block':>9} {'Mgas':>6} {'c/gas':>6} " + " ".join(f"{c[:12]:>12}" for c in comps))
for b in blocks:
    by_comp = collections.Counter()
    total, attributed, by_comp, *_ = cf.attribute(os.path.join(args.dir, f"{b}.svg"), reported[b], args.scale)
    G = gas[b]
    print(f"{b:>9} {G/1e6:6.1f} {reported[b]/G:6.2f} " + " ".join(f"{by_comp.get(c, 0)*args.scale/G:12.2f}" for c in comps))
