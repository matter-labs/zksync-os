#!/usr/bin/env python3
"""Per-block cycle comparison of zksync-os and zilkworm over a block set.

Reads the zksync-os per-block logs written by the profiling runs (lines
`Block <n> (<gas> gas): <cycles> cycles ...`) and zilkworm's execution logs
(lines `... block=<n> gas_used=<gas> cycles=<cycles> ... ok=<bool> ...`),
writes a `cycles.csv` (`block,cycles,gas`) next to each side's flamegraphs for
`compare_flamegraphs.py`, and prints the totals, the distribution of the
per-block cycle ratio and the blocks where either side is unusually slow.

Usage: ref1024_compare.py --zksync-logs DIR --zilk-logs GLOB
       --zksync-out DIR --zilk-out DIR [--metadata block-metadata.json] [--top 15]
"""
import argparse
import csv
import glob
import json
import os
import re
import statistics

ZK = re.compile(r"^Block (\d+) \((\d+) gas\): (\d+) cycles")
ZW = re.compile(r"block=(\d+) gas_used=(\d+) cycles=(\d+) .*reached_end=(\w+) ok=(\w+)")


def read_zksync(logs_dir):
    out = {}
    for path in glob.glob(os.path.join(logs_dir, "*.txt")):
        with open(path) as fh:
            for line in fh:
                m = ZK.match(line)
                if m:
                    out[int(m.group(1))] = (int(m.group(3)), int(m.group(2)))
    return out


def read_zilk(logs_glob):
    out = {}
    failed = {}
    for path in glob.glob(logs_glob):
        with open(path) as fh:
            for line in fh:
                m = ZW.search(line)
                if not m:
                    continue
                block = int(m.group(1))
                if m.group(4) == "true" and m.group(5) == "true":
                    out[block] = (int(m.group(3)), int(m.group(2)))
                else:
                    failed[block] = line.strip()
    return out, failed


def write_cycles_csv(path, table):
    with open(path, "w", newline="") as fh:
        w = csv.writer(fh)
        w.writerow(["block", "cycles", "gas"])
        for block in sorted(table):
            cycles, gas = table[block]
            w.writerow([block, cycles, gas])


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--zksync-logs", required=True)
    ap.add_argument("--zilk-logs", required=True, help="glob of zilkworm execution logs")
    ap.add_argument("--zksync-out", required=True, help="zksync-os flamegraph dir (cycles.csv is written there)")
    ap.add_argument("--zilk-out", required=True, help="zilkworm flamegraph dir (cycles.csv is written there)")
    ap.add_argument("--metadata", help="block-metadata.json of the set, to report blocks missing on a side")
    ap.add_argument("--top", type=int, default=15)
    args = ap.parse_args()

    zk = read_zksync(args.zksync_logs)
    zw, zw_failed = read_zilk(args.zilk_logs)
    write_cycles_csv(os.path.join(args.zksync_out, "cycles.csv"), zk)
    write_cycles_csv(os.path.join(args.zilk_out, "cycles.csv"), zw)

    if args.metadata:
        with open(args.metadata) as fh:
            expected = {m["block"] for m in json.load(fh)}
        for name, have in [("zksync-os", zk), ("zilkworm", zw)]:
            missing = sorted(expected - set(have))
            if missing:
                print(f"{name}: {len(missing)} blocks missing: {missing[:10]}{' ...' if len(missing) > 10 else ''}")
    if zw_failed:
        print(f"zilkworm: {len(zw_failed)} blocks did not succeed:")
        for block in sorted(zw_failed)[:10]:
            print("  ", zw_failed[block][:160])

    common = sorted(set(zk) & set(zw))
    gas = sum(zk[b][1] for b in common)
    zk_total = sum(zk[b][0] for b in common)
    zw_total = sum(zw[b][0] for b in common)
    print(f"\n{len(common)} blocks in common, {gas/1e9:.3f} Ggas")
    print(f"  zksync-os {zk_total/1e9:.3f} Gcycles = {zk_total/gas:.3f} cycles/gas")
    print(f"  zilkworm  {zw_total/1e9:.3f} Gcycles = {zw_total/gas:.3f} cycles/gas")
    print(f"  ratio zksync-os/zilkworm {zk_total/zw_total:.3f}")

    ratios = {b: zk[b][0] / zw[b][0] for b in common}
    values = sorted(ratios.values())
    q = lambda p: values[min(len(values) - 1, int(p * len(values)))]
    print(f"\nper-block ratio: min {values[0]:.3f}, p10 {q(0.1):.3f}, median {statistics.median(values):.3f}, "
          f"p90 {q(0.9):.3f}, max {values[-1]:.3f}; blocks where zksync-os is slower: "
          f"{sum(1 for r in values if r > 1)}")
    cpg_zk = {b: zk[b][0] / zk[b][1] for b in common}
    cpg_zw = {b: zw[b][0] / zw[b][1] for b in common}

    def table(title, blocks):
        print(f"\n{title}")
        print(f"{'block':>9} {'Mgas':>7} {'zksync-os Mcyc':>15} {'zilkworm Mcyc':>14} {'zk c/gas':>9} {'zw c/gas':>9} {'ratio':>6}")
        for b in blocks:
            print(f"{b:>9} {zk[b][1]/1e6:7.2f} {zk[b][0]/1e6:15.1f} {zw[b][0]/1e6:14.1f} "
                  f"{cpg_zk[b]:9.2f} {cpg_zw[b]:9.2f} {ratios[b]:6.3f}")

    table("blocks where zksync-os is slowest relative to zilkworm",
          sorted(common, key=lambda b: -ratios[b])[: args.top])
    table("blocks with the highest zksync-os cycles per gas",
          sorted(common, key=lambda b: -cpg_zk[b])[: args.top])
    table("blocks with the highest zilkworm cycles per gas",
          sorted(common, key=lambda b: -cpg_zw[b])[: args.top])


if __name__ == "__main__":
    main()
