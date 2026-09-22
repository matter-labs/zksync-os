#!/usr/bin/env python3
"""Break the EVM interpreter's cycles down per opcode handler and compare block groups.

For every block the subtree of `Interpreter::run_loop` is attributed to the first
handler frame below it (`hot::Hot::<handler>` for the inlined hot handlers, the
`run_loop::{closure#N}` outlined cold paths, or the run loop's own self time).
With `--opseq DIR` (per-block `<block>.csv` from `eth_runner
ethproofs-opcode-sequences`, bigrams -> opcode counts) the handler cycles are
also split into an opcode-mix effect and a per-opcode-cost effect between the two
groups. Keccak delegation rows are not attributed (see block_profile).

Usage: interp_breakdown.py DIR --scale 1 --blocks A,B,.. --baseline X,Y,.. [--opseq DIR] [--top 30]
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

RUN_LOOP = re.compile(r"Interpreter>::run_loop(::<[^{]*>)?$")
HANDLER = re.compile(r"hot::Hot>::(?P<name>[a-z_0-9]+)(?:::<(?P<generic>[^<>]*)>)?")
CLOSURE = re.compile(r"run_loop::.*\{closure#(\d+)\}")
DELEGATION_LEAF = re.compile(r"delegation_types::keccak_special5::keccak_f1600")

# handler -> opcode name used by the opcode-sequence tracer
def opcode_of(handler):
    handler = re.sub(r" \(cold#\d+\)$", "", handler)
    m = re.match(r"(\w+)(?:<(.*)>)?$", handler)
    if not m:
        return None
    base, gen = m.group(1), (m.group(2) or "").split(",")[0].strip()
    table = {
        "push_small": f"PUSH{gen}", "push_wide": f"PUSH{gen}", "push0": "PUSH0",
        "dup_op": f"DUP{gen}", "swap_op": f"SWAP{gen}", "log": f"LOG{gen}",
        "storage_read": "TLOAD" if gen == "true" else "SLOAD",
        "storage_write": "TSTORE" if gen == "true" else "SSTORE",
        "bitand": "AND", "bitor": "OR", "bitxor": "XOR", "ret": "RETURN",
        "land_on_jumpdest": None, "fetch_and_advance": None, "ensure_heap": None,
    }
    if base in table:
        return table[base]
    return base.upper()


def handler_key(name):
    m = HANDLER.search(name)
    if m and m.group("name") != "outlined":
        gen = m.group("generic")
        gen = gen.split(",")[0].strip() if gen else ""
        gen = "" if gen.startswith("zk_ee") else gen
        return m.group("name") + (f"<{gen}>" if gen else "")
    m = CLOSURE.search(name)
    if m:
        return f"cold#{m.group(1)}"
    return None


def block_profile(svg, scale, reported):
    samples, frames = fg.parse_svg(svg)
    roots = fg.build_tree(frames)
    total = reported / scale
    leaf = sum(f["samples"] - sum(c["samples"] for c in f["children"])
               for f in frames if DELEGATION_LEAF.search(f["name"]))
    # Keccak delegation rows are deliberately not spread here: whether a delegation
    # sample carries a stack chain is an accident of the leaf's code path, so the
    # rows would land on some blocks' SHA3 handlers and not others. The handler
    # figures below are the RISC-V instructions only; the permutation rows (649 per
    # keccak-f1600) belong to the keccak category of categorize_flamegraphs.py.
    weight = 1.0

    def weighted(f):
        own = f["samples"] - sum(c["samples"] for c in f["children"])
        w = own * (weight if DELEGATION_LEAF.search(f["name"]) else 1.0)
        return w + sum(weighted(c) for c in f["children"])

    per = collections.Counter()   # handler -> cycles
    cold_children = collections.defaultdict(collections.Counter)
    def attribute(f, label):
        """Self time of `f` goes to `label`; each child is a handler or is descended."""
        own = f["samples"] - sum(c["samples"] for c in f["children"])
        per[label] += own * (weight if DELEGATION_LEAF.search(f["name"]) else 1.0) * scale
        for c in f["children"]:
            key = handler_key(fg.strip_name(c["name"]))
            if key is None:
                attribute(c, "other:" + fg.strip_name(c["name"])[-60:])
            else:
                if key.startswith("cold"):
                    # name the outlined cold path after the handler it wraps
                    for g in c["children"]:
                        cold_children[key][fg.strip_name(g["name"])[-70:]] += weighted(g) * scale
                    node, inner = c, None
                    while node is not None and inner is None:
                        keys = [handler_key(fg.strip_name(g["name"])) for g in node["children"]]
                        named = [k for k in keys if k and not k.startswith("cold")]
                        if named:
                            inner = named[0]
                        else:
                            nxt = [g for g, k in zip(node["children"], keys) if k]
                            node = max(nxt, key=lambda g: g["samples"]) if nxt else None
                    if inner:
                        key = f"{inner} ({key})"
                per[key] += weighted(c) * scale

    def under_loop(f):
        attribute(f, "(run loop self)")

    def walk(f):
        if RUN_LOOP.search(fg.strip_name(f["name"])):
            under_loop(f)
            return weighted(f) * scale
        return sum(walk(c) for c in f["children"])

    loop_total = sum(walk(r) for r in roots)
    return loop_total, per, cold_children


def opcode_counts(path):
    """Opcode counts from the tracer's bigrams. Only fall-through bigrams are
    recorded, so an opcode is counted in whichever position sees it fully
    (JUMP/JUMPI/CALL-like as the second element, JUMPDEST as the first)."""
    first, second = collections.Counter(), collections.Counter()
    unigrams = collections.Counter()
    for r in csv.DictReader(open(path)):
        if r["length"] == "1":
            unigrams[r["sequence"]] += int(r["count"])
        if r["length"] == "2":
            a, b = r["sequence"].split()
            first[a] += int(r["count"])
            second[b] += int(r["count"])
    if unigrams:
        return unigrams
    return collections.Counter({op: max(first[op], second[op]) for op in set(first) | set(second)})


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("dir")
    ap.add_argument("--scale", type=float, default=1.0)
    ap.add_argument("--blocks", required=True)
    ap.add_argument("--baseline", required=True)
    ap.add_argument("--opseq")
    ap.add_argument("--top", type=int, default=30)
    args = ap.parse_args()
    groups = {"outliers": args.blocks.split(","), "baseline": args.baseline.split(",")}
    reported, gas = {}, {}
    for row in csv.DictReader(open(os.path.join(args.dir, "cycles.csv"))):
        if (row.get("cycles") or "").isdigit():
            reported[row["block"]] = int(row["cycles"])
            gas[row["block"]] = int(row["gas"])

    prof = {}
    for g, blocks in groups.items():
        for b in blocks:
            prof[b] = block_profile(os.path.join(args.dir, f"{b}.svg"), args.scale, reported[b])
    counts = {b: opcode_counts(os.path.join(args.opseq, f"{b}.csv")) for b in prof} if args.opseq else {}

    print(f"{'block':>9} {'group':>8} {'Mgas':>6} {'c/gas':>6} {'loop c/gas':>10} {'self c/gas':>10} {'steps/kgas':>10} {'cyc/step':>8}  top handlers (c/gas)")
    for g, blocks in groups.items():
        for b in blocks:
            loop, per, _ = prof[b]
            G = gas[b]
            steps = sum(counts[b].values()) if counts else 0
            tops = ", ".join(f"{k} {v/G:.2f}" for k, v in per.most_common(6))
            print(f"{b:>9} {g:>8} {G/1e6:6.1f} {reported[b]/G:6.2f} {loop/G:10.2f} {per['(run loop self)']/G:10.2f} "
                  f"{steps/G*1e3:10.1f} {loop/steps if steps else 0:8.0f}  {tops}")

    agg = {}
    for g, blocks in groups.items():
        G = sum(gas[b] for b in blocks)
        per = collections.Counter()
        cnt = collections.Counter()
        for b in blocks:
            per.update(prof[b][1])
            if counts:
                cnt.update(counts[b])
        agg[g] = (G, per, cnt)
    Go, po, co = agg["outliers"]
    Gb, pb, cb = agg["baseline"]
    print(f"\ninterpreter run loop: outliers {sum(po.values())/Go:.3f} c/gas, baseline {sum(pb.values())/Gb:.3f} c/gas, "
          f"delta {sum(po.values())/Go - sum(pb.values())/Gb:+.3f}")
    keys = sorted(set(po) | set(pb), key=lambda k: -(po[k] / Go - pb[k] / Gb))
    hdr = f"{'handler':>28} {'out c/gas':>9} {'base c/gas':>10} {'delta':>7}"
    if counts:
        hdr += f" {'out /kgas':>9} {'base /kgas':>10} {'out cyc/op':>10} {'base cyc/op':>11} {'mix':>7} {'cost':>7}"
    print(hdr)
    shown = keys[: args.top] + [k for k in keys[-8:] if k not in keys[: args.top]]
    for k in shown:
        o, bse = po[k] / Go, pb[k] / Gb
        line = f"{k[-28:]:>28} {o:9.3f} {bse:10.3f} {o - bse:+7.3f}"
        if counts:
            op = opcode_of(k) if not k.startswith(("cold", "(", "other")) else None
            no, nb = (co.get(op, 0), cb.get(op, 0)) if op else (0, 0)
            if no and nb:
                cyo, cyb = po[k] / no, pb[k] / nb
                mix = (no / Go - nb / Gb) * cyb
                cost = no / Go * (cyo - cyb)
                line += f" {no/Go*1e3:9.2f} {nb/Gb*1e3:10.2f} {cyo:10.0f} {cyb:11.0f} {mix:+7.3f} {cost:+7.3f}"
        print(line)
    print("\nper outlier block: run-loop c/gas delta vs the baseline group, top handler contributions")
    for b in groups["outliers"]:
        G = gas[b]
        per = prof[b][1]
        d = {k: per[k] / G - pb[k] / Gb for k in set(per) | set(pb)}
        tot = sum(per.values()) / G - sum(pb.values()) / Gb
        tops = ", ".join(f"{k.split(' (')[0]} {v:+.2f}" for k, v in sorted(d.items(), key=lambda kv: -abs(kv[1]))[:7])
        steps = sum(counts[b].values()) / G * 1e3 if counts else 0
        print(f"  {b}: {tot:+.2f} c/gas ({steps:.0f} steps/kgas): {tops}")

    # intrinsic cost of the handlers: cycles per gas charged, from the baseline group
    static_gas = {"push_small": 3, "push_wide": 3, "push0": 2, "dup_op": 3, "swap_op": 3, "pop": 2, "add": 3, "sub": 3,
                  "mul": 5, "div": 5, "sdiv": 5, "mod": 5, "smod": 5, "addmod": 8, "mulmod": 8, "exp": 10, "signextend": 5,
                  "lt": 3, "gt": 3, "slt": 3, "sgt": 3, "eq": 3, "iszero": 3, "bitand": 3, "bitor": 3, "bitxor": 3, "not": 3,
                  "byte": 3, "shl": 3, "shr": 3, "sar": 3, "sha3": 36, "mload": 3, "mstore": 3, "mstore8": 3, "jump": 8,
                  "jumpi": 10, "jumpdest": 1, "calldataload": 3, "calldatasize": 2, "calldatacopy": 3, "codecopy": 3,
                  "storage_read<false>": 100, "storage_write<false>": 100, "storage_read<true>": 100,
                  "storage_write<true>": 100, "extcodesize": 100, "log<2>": 375 + 375 * 2, "log<3>": 375 + 375 * 3,
                  "log<4>": 375 + 375 * 4, "gas": 2, "mcopy": 3, "returndatacopy": 3}
    print("\nintrinsic cycles per gas charged (baseline group; static part of the gas cost only, "
          "SLOAD/SSTORE/EXTCODESIZE assumed warm, SHA3 assumed one word):")
    rows = []
    for k in pb:
        base = re.sub(r" \(cold#\d+\)$", "", k)
        stem = re.sub(r"<.*", "", base) if base not in static_gas else base
        g = static_gas.get(base, static_gas.get(stem))
        op = opcode_of(k) if not k.startswith(("cold", "(", "other")) else None
        n = cb.get(op, 0) if op else 0
        if g and n:
            rows.append((pb[k] / n / g, pb[k] / n, g, base, n / Gb * 1e3, co.get(op, 0) / Go * 1e3))
    print(f"{'handler':>22} {'cyc/op':>7} {'gas':>5} {'cyc/gas':>8} {'base /kgas':>10} {'out /kgas':>9}")
    for r in sorted(rows, reverse=True)[:30]:
        print(f"{r[3]:>22} {r[1]:7.0f} {r[2]:5d} {r[0]:8.1f} {r[4]:10.2f} {r[5]:9.2f}")

    print("\ncold paths (top children, outliers):")
    cold = collections.defaultdict(collections.Counter)
    for b in groups["outliers"]:
        for k, c in prof[b][2].items():
            cold[k].update(c)
    for k in sorted(cold, key=lambda k: -sum(cold[k].values()))[:12]:
        print(f"  {k} {sum(cold[k].values())/Go:.3f} c/gas: " + ", ".join(f"{n} {v/Go:.3f}" for n, v in cold[k].most_common(3)))


if __name__ == "__main__":
    main()
