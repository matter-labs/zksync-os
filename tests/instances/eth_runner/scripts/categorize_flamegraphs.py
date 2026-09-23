#!/usr/bin/env python3
"""Attribute the cycles of guest flamegraphs to logical pieces of the STF.

Every sample of a flamegraph (inferno SVG written by `eth_runner
ethproofs-flamegraph`) is attributed along two axes derived from its stack:

* phase: the outermost block-flow stage on the stack (transaction validation,
  fee collection, execution, post-transaction work, state commitment, ...);
* component: the innermost logical owner on the stack (EVM dispatch, storage
  slot cache, account cache, MPT, a precompile, receipts, ...).

Cross-cutting leaf work (memcpy, keccak, big-integer field arithmetic, ergs
charging, allocation) is reported as a third "kind" axis inside the component
that called it, instead of becoming a component of its own.

Usage: categorize_flamegraphs.py <dir with *.svg and cycles.csv> [--scale 1]
       [--baseline <other dir>]
`--scale` is the cycles per sample (1 for `--sampling-rate 1`).
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

# A delegation call advances the VM cycle counter by the number of circuit rows it
# costs (649 for one keccak-f1600) but is sampled once, so a profile holds fewer
# samples than the reported cycles. The difference is spread over the samples whose
# leaf frame is the delegation call site, which attributes the rows to the callers.
DELEGATION_LEAF = re.compile(r"delegation_types::keccak_special5::keccak_f1600")

# (name, regex) — the innermost matching frame decides the component
COMPONENTS = [
    ("MPT / state commitment", r"persist_changes|::mpt::|StackMPT|SortedMPTWithInterner|EthereumMPT|trie"),
    ("storage slot cache", r"generic_pubdata_aware_plain_storage|addressed_plain_storage|AddressedPlainStorage|full_storage_cache|EthereumStorageCache|storage_read|storage_write|storage_touch|StorageCacheModel"),
    ("account cache", r"account_cache|EthereumAccountCache|read_account_properties|touch_account|account_properties|increment_nonce|nominal_token|mark_for_deconstruction|deploy_code"),
    ("bytecode / preimage cache", r"preimage|get_executable_bytecode|analyze_into|jumpdest|Bytecode"),
    ("secp256k1 (ecrecover)", r"ecrecover|secp256k1"),
    ("bn254 precompiles", r"bn254"),
    ("bls12-381 / kzg", r"bls12_381|point_evaluation|kzg|Bls12"),
    ("modexp", r"modexp"),
    ("hash precompiles", r"sha256|ripemd|blake2"),
    ("system hooks / other precompiles", r"system_hooks|call_hooks::precompiles|SystemFunction"),
    ("receipts / logs / bloom", r"receipt|bloom|logs_storage|events_storage|LogsStorage|EventsStorage"),
    ("tx parsing / rlp", r"rlp_encoded|parse_transaction|TransactionParser|::rlp::|RLP"),
    ("access / authorization lists", r"access_list|authorization"),
    ("oracle IO", r"CsrBasedIOOracle|io_oracle|NonDeterminism|nondeterminism|raw_query"),
    ("EVM interpreter", r"evm_interpreter|Dispatch|Interpreter"),
    ("bootloader runner", r"bootloader::runner|handle_requested_external_call|execute_call|run_till_completion|supported_ees"),
    ("transaction flow", r"transaction_flow|process_transaction|process_l2_transaction"),
    ("block flow", r"block_flow|loop_op|run_prepared|run_proving"),
]

# innermost matching frame decides the phase (stages nest: block > tx > validation)
PHASES = [
    ("state commitment", r"update_commitment|persist_changes"),
    ("post-tx (receipts, logs)", r"EthereumPostOp|post_tx_op|post_tx_loop"),
    ("tx validation", r"validate_and_compute_fee_for_transaction|validation_impl|validate_"),
    ("tx fees / refund", r"charge_fee|collect_fee|refund|precharge|pay_for"),
    ("tx execution", r"execute_or_deploy_inner|execute_call|run_single_interaction"),
    ("tx flow other", r"process_transaction|process_l2_transaction|transaction_flow"),
    ("block flow other", r"generic_loop_op|run_prepared|run_proving"),
]

KINDS = [
    ("memcpy/memset/memcmp", r"^memcpy$|^memset$|^memcmp$|memcpy_impl|memset_impl|memcmp_impl|copy_from_slice|copy_nonoverlapping"),
    ("keccak", r"keccak|sha3|Keccak"),
    ("bigint / field arithmetic", r"bigint_delegation|ark_ff|ark_ec|montgomery|FieldElement|Jacobian|Fp as|QuadExt|CubicExt|glv"),
    ("ergs / native charging", r"Ergs|charge_|Resources>::charge|spend_"),
    ("btree / history map", r"BTreeMap|btree|history_map|HistoryMap"),
    ("allocation", r"TalcWrapper|ProxyAllocator as core::alloc::Allocator|talc::|__rust_alloc|__rust_dealloc"),
]


def classify(path_names):
    """path_names: stripped frame names from root to the leaf frame."""
    component = "other"
    for name in path_names:  # innermost wins: keep overwriting
        for cname, rx in COMPONENTS:
            if re.search(rx, name):
                component = cname
                break
    phase = "no fp chain"
    for name in path_names:  # innermost wins: nested stages are more specific
        for pname, rx in PHASES:
            if re.search(rx, name):
                phase = pname
                break
    if component == "other" and phase == "no fp chain":
        # leaf samples without a frame-pointer chain: name them by the leaf
        leaf = path_names[-1] if path_names else ""
        for kname, rx in KINDS:
            if re.search(rx, leaf):
                component = f"{kname} (no stack)"
                break
    kind = "own code"
    for name in reversed(path_names):  # innermost wins
        matched = False
        for kname, rx in KINDS:
            if re.search(rx, name):
                kind = kname
                matched = True
                break
        if matched:
            break
    return phase, component, kind


def attribute(svg, reported_cycles=None, scale=1.0):
    """`scale` is the cycles per sample; the counters returned are in samples."""
    total, frames = fg.parse_svg(svg)
    roots = fg.build_tree(frames)
    # weight of a delegation-leaf sample: 1 + (missing cycles / such samples), in samples
    delegation_samples = sum(
        f["samples"] - sum(c["samples"] for c in f["children"])
        for f in frames if DELEGATION_LEAF.search(f["name"]))
    weight = 1.0
    if reported_cycles and reported_cycles > total * scale and delegation_samples:
        weight = 1.0 + (reported_cycles - total * scale) / (delegation_samples * scale)
    by_component = collections.Counter()
    by_phase = collections.Counter()
    by_pc = collections.Counter()
    by_ck = collections.Counter()
    by_pck = collections.Counter()
    attributed = 0

    def walk(frame, path):
        nonlocal attributed
        name = fg.strip_name(frame["name"])
        path.append(name)
        own = frame["samples"] - sum(c["samples"] for c in frame["children"])
        if own > 0 and DELEGATION_LEAF.search(frame["name"]):
            own = own * weight
        if own > 0:
            phase, component, kind = classify(path[1:])  # skip synthetic root
            by_component[component] += own
            by_phase[phase] += own
            by_pc[(phase, component)] += own
            by_ck[(component, kind)] += own
            by_pck[(phase, component, kind)] += own
            attributed += own
        for c in frame["children"]:
            walk(c, path)
        path.pop()

    for r in roots:
        walk(r, [])
    if weight > 1.0:
        total = reported_cycles / scale
    return total, attributed, by_component, by_phase, by_pc, by_ck, by_pck


def load_dir(d, scale=1.0):
    tot = collections.Counter()
    comp = collections.Counter()
    phase = collections.Counter()
    pc = collections.Counter()
    ck = collections.Counter()
    pck = collections.Counter()
    gas = 0
    blocks = 0
    cycles_of = {}
    with open(os.path.join(d, "cycles.csv")) as fh:
        for row in csv.DictReader(fh):
            if row.get("gas") and row["gas"].isdigit():
                gas += int(row["gas"])
                cycles_of[row["block"]] = int(row["cycles"])
    for svg in sorted(glob.glob(os.path.join(d, "*.svg"))):
        if "inverse" in svg:
            continue
        block = os.path.basename(svg).split(".")[0]
        total, attributed, c, p, x, k, y = attribute(svg, cycles_of.get(block), scale)
        tot["samples"] += total
        tot["attributed"] += attributed
        comp.update(c)
        phase.update(p)
        pc.update(x)
        ck.update(k)
        pck.update(y)
        blocks += 1
    return blocks, gas, tot, comp, phase, pc, ck, pck


def fmt_table(title, counter, total, scale, gas, baseline=None, base_total=None, base_gas=None):
    print(f"\n=== {title} ===")
    head = f"{'Mcycles':>10} {'share':>7} {'c/gas':>7}"
    if baseline is not None:
        head += f" {'base Mcyc':>10} {'delta':>9}"
    print(f"{head}  piece")
    for name, s in counter.most_common():
        line = f"{s*scale/1e6:10.1f} {100*s/total:6.2f}% {s*scale/gas:7.2f}"
        if baseline is not None:
            b = baseline.get(name, 0)
            line += f" {b*scale/1e6:10.1f} {(s-b)*scale/1e6:+9.1f}"
        print(f"{line}  {name}")


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("dir")
    ap.add_argument("--scale", type=float, default=1.0, help="cycles per sample")
    ap.add_argument("--baseline", help="another profile dir to show deltas against")
    ap.add_argument("--matrix-top", type=int, default=12)
    args = ap.parse_args()

    scale = args.scale
    blocks, gas, tot, comp, phase, pc, ck, pck = load_dir(args.dir, scale)
    total = tot["samples"]
    print(f"{args.dir}: {blocks} blocks, {gas/1e6:.1f} Mgas, {total*scale/1e6:.1f} Mcycles "
          f"({total*scale/gas:.2f} cycles/gas), {100*tot['attributed']/total:.1f}% of cycles attributed "
          f"(delegation rows spread over their call sites)")
    base = None
    if args.baseline:
        bblocks, bgas, btot, bcomp, bphase, bpc, bck, _ = load_dir(args.baseline, scale)
        print(f"baseline {args.baseline}: {bblocks} blocks, {bgas/1e6:.1f} Mgas, {btot['samples']*scale/1e6:.1f} Mcycles "
              f"({btot['samples']*scale/bgas:.2f} cycles/gas)")
        base = (bcomp, bphase)

    fmt_table("by component (innermost logical owner)", comp, total, scale, gas,
              base[0] if base else None)
    fmt_table("by phase (outermost block-flow stage)", phase, total, scale, gas,
              base[1] if base else None)

    print("\n=== phase x component (Mcycles) ===")
    top_components = [n for n, _ in comp.most_common(args.matrix_top)]
    phases = [n for n, _ in phase.most_common()]
    print(f"{'':32}" + "".join(f"{c[:14]:>15}" for c in top_components))
    for p in phases:
        print(f"{p[:32]:32}" + "".join(f"{pc.get((p, c), 0)*scale/1e6:15.1f}" for c in top_components))

    print("\n=== component x kind of leaf work (Mcycles) ===")
    kinds = ["own code"] + [k for k, _ in KINDS]
    print(f"{'':32}" + "".join(f"{k[:14]:>15}" for k in kinds))
    for c in top_components:
        print(f"{c[:32]:32}" + "".join(f"{ck.get((c, k), 0)*scale/1e6:15.1f}" for k in kinds))

    for detail in ["tx validation", "state commitment", "tx fees / refund", "post-tx (receipts, logs)", "tx flow other", "block flow other"]:
        rows = collections.Counter()
        for (p, c, k), v in pck.items():
            if p == detail:
                rows[(c, k)] += v
        if not rows:
            continue
        ptotal = sum(rows.values())
        print(f"\n=== inside '{detail}': {ptotal*scale/1e6:.1f} Mcycles ({ptotal*scale/gas:.2f} c/gas) ===")
        print(f"{'Mcycles':>10} {'share':>7}  component / kind")
        for (c, k), v in rows.most_common(14):
            print(f"{v*scale/1e6:10.1f} {100*v/ptotal:6.2f}%  {c} / {k}")


if __name__ == "__main__":
    main()
