#!/usr/bin/env python3
"""Compare zksync-os and zilkworm block-execution flamegraphs by logical part.

Both projects execute the same Ethereum blocks on the airbender transpiler;
their symbols differ, so each sampled stack is mapped to a project-neutral
category (transaction validation, EVM interpreter core, individual precompiles,
state access, state-root computation, ...) and the categories are compared as
absolute cycles (share of the flamegraph samples times the block's measured
cycle count) summed over the blocks both projects profiled.

Classification of a stack (root to leaf):
  level 1 (rules in priority order, first rule matching any frame wins):
          tx validation, state commitment, EVM tx execution, post-block work,
          tx bookkeeping; default "other / decode+setup".
  level 2, inside "EVM tx execution":
          precompile entry frames (priority order), otherwise cross-cutting
          costs (keccak, state access, jumpdest analysis, gas accounting, memory
          copies, 256-bit arithmetic, call/frame plumbing; innermost wins),
          otherwise "EVM: interpreter core".

Usage:
  compare_flamegraphs.py --a zksync-os:DIR_A:cycles_a.csv --b zilkworm:DIR_B:cycles_b.csv
The cycles CSVs are `block,cycles[,gas]`; SVGs are looked up as DIR/<block>.svg.
"""

import argparse
import collections
import csv
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import analyze_flamegraphs as fg  # noqa: E402

SAMPLING_RATE = 100  # cycles per sample used when the flamegraphs were recorded

OTHER = "other: input decode, setup, block bookkeeping"
UNATTRIBUTED = "unattributed (no frame-pointer chain / asm leaf)"
EVM = "EVM tx execution"
EVM_CORE = "EVM: interpreter core (dispatch, stack, plain opcodes)"

# Level-1 categories, checked outermost-first; first pattern that matches a
# frame wins.
# Witness verification comes first: zksync-os reads and hashes contract code and trie nodes
# lazily (inside the EVM call path or the commitment update, wherever they are first needed),
# zilkworm does all of it upfront, so only a dedicated category compares like with like.
WITNESS = "witness verification (read + keccak of code and trie node preimages)"

LEVEL1 = {
    "zksync-os": [
        (WITNESS, r"BytecodeKeccakPreimagesStorage.*expose_preimage|consult_cache_or_oracle"),
        ("tx validation + sender recovery", r"validate_and_compute_fee_for_transaction"),
        ("state commitment (MPT root update)", r"update_commitment|persist_changes|EthereumStoragePersister"),
        (EVM, r"run_till_completion"),
        ("post-block: receipts/withdrawals/roots/logs", r"EthereumPostOp|post_tx_loop_op|receipts_root|withdrawals|requests_hash|block_bloom|BlockBloom"),
        ("tx bookkeeping (pre/post tx, refunds, receipts)", r"process_l2_transaction|process_transaction"),
        ("block bookkeeping (header/body validation, receipts vector)", r"generic_loop_op|run_prepared"),
    ],
    "zilkworm": [
        (WITNESS, r"DirectState::sanitize"),
        ("tx validation + sender recovery", r"silkworm::protocol::validate_transaction|Transaction::sender"),
        ("state commitment (MPT root update)", r"StateTransition::check_root|calc_root_from_updates|GridMPT|apply_state_diff"),
        (EVM, r"evmone::state::transition"),
        ("post-block: receipts/withdrawals/roots/logs", r"root_hash<silkworm::Receipt|compute_withdrawals_root|Blockchain::insert_block::|finalize"),
        ("tx bookkeeping (pre/post tx, refunds, receipts)", r"ExecutionProcessor::execute_transaction"),
        ("block bookkeeping (header/body validation, receipts vector)", r"Blockchain::insert_block|ExecutionProcessor::execute_block"),
    ],
}

# Precompile entry frames (outermost-first inside the EVM subtree).
PRECOMPILES = {
    "zksync-os": [
        ("precompile: ecrecover", r"EcRecoverEEInvocation|ecrecover_inner|ecrecover_as_system_function|EcRecoverImpl"),
        ("precompile: KZG point evaluation", r"point_evaluation|PointEvaluation"),
        ("precompile: bn254 pairing", r"bn254_pairing|Bn254Pairing"),
        ("precompile: bn254 ecmul", r"bn254_ecmul|Bn254Mul"),
        ("precompile: bn254 ecadd", r"bn254_ecadd|Bn254Add"),
        ("precompile: modexp", r"modexp|ModExp"),
        ("precompile: sha256", r"system_functions::sha256|Sha256Impl"),
        ("precompile: ripemd160", r"ripemd|Ripemd"),
        ("precompile: identity", r"identity|Identity"),
        ("precompile: blake2f", r"blake2f|Blake2F"),
        ("precompile: bls12-381 ops", r"bls12_381_|Bls12381"),
        ("precompile: other system hook", r"run_call_hook|pure_system_function_hook_impl"),
    ],
    "zilkworm": [
        ("precompile: ecrecover", r"silkworm_ecrecover_execute|ecrec_run|ecrecover_execute|silkworm_recover_address"),
        ("precompile: KZG point evaluation", r"point_evaluation_execute"),
        ("precompile: bn254 pairing", r"ecpairing_execute"),
        ("precompile: bn254 ecmul", r"ecmul_execute"),
        ("precompile: bn254 ecadd", r"ecadd_execute"),
        ("precompile: modexp", r"expmod_execute"),
        ("precompile: sha256", r"sha256_execute"),
        ("precompile: ripemd160", r"ripemd160_execute"),
        ("precompile: identity", r"identity_execute"),
        ("precompile: blake2f", r"blake2bf_execute|blake2b_execute"),
        ("precompile: bls12-381 ops", r"bls12_"),
        ("precompile: other system hook", r"call_precompile"),
    ],
}

# Cross-cutting costs inside the EVM subtree (innermost-first).
CROSSCUT = {
    "zksync-os": [
        ("EVM: keccak (SHA3 opcode, hashing)", r"Keccak256Core|keccak"),
        ("EVM: bytecode analysis (jumpdests)", r"evm_interpreter::analyze|create_artifacts"),
        ("EVM: state access (storage/account/code)", r"io_subsystem|storage_model|EthereumStorageCache|history_map|warm_storage_key|IOSubsystem|preimage|read_storage|storage_read|storage_write|touch_account|account_properties"),
        ("EVM: gas / resource accounting", r"::charge\b|spend_gas|with_infinite_ergs|Resource>::charge"),
        ("EVM: memory copies / compares", r"memcpy|memset|memmove|copy_nonoverlapping|copy_backward|compare_bytes|memcmp"),
        ("EVM: 256-bit arithmetic", r"ruint|u256::|delegated_u256|bigint_op_delegation|bigint_delegation"),
        ("EVM: call/frame plumbing", r"handle_requested_external_call|call_execute_callee_frame|start_executing_frame|continue_after_preemption|finish_execution|copy_return"),
        (EVM_CORE, r"Interpreter>::run|execute_till_yield_point|evm_interpreter::"),
    ],
    "zilkworm": [
        ("EVM: keccak (SHA3 opcode, hashing)", r"keccak"),
        ("EVM: bytecode analysis (jumpdests)", r"analyze_jumpdests|baseline::analyze|analyze_legacy|CodeAnalysis"),
        ("EVM: state access (storage/account/code)", r"Host::get_storage|Host::set_storage|Host::access_|Host::get_balance|Host::get_code|Host::copy_code|Host::selfdestruct|Host::get_nonce|Host::account_exists|Host::get_block_hash|State::get_storage|journal_|DirectState|IntraBlockState|HostContext::(get|set|access)|internal::(get|set|access)"),
        ("EVM: gas / resource accounting", r"check_requirements"),
        ("EVM: memory copies / compares", r"memcpy|memset|memmove|memcmp"),
        ("EVM: 256-bit arithmetic", r"intx::|evmmax::"),
        ("EVM: call/frame plumbing", r"Host::call|execute_message|prepare_message|call_impl|HostContext::call|internal::call|execute_cached_code"),
        (EVM_CORE, r"evmone::baseline::execute|dispatch_cgoto|evmone::instr::"),
    ],
}


# Keccak by purpose: frames of the hash function itself, and what the enclosing code is doing
# (priority order, any ancestor matches). zilkworm verifies code and trie nodes in one upfront
# pass over the witness, so there they can not be told apart.
KECCAK_FRAME = {
    "zksync-os": r"^<?airbender_crypto::sha3::",
    "zilkworm": r"^ethash_keccak|^ethash::keccak|^silkworm::keccak256|^zilkworm::keccak_|^keccak$",
}
KECCAK_WITNESS_NODES = "MPT: witness trie node verification"
KECCAK_CODE = "bytecode hashing (code hash verification, deployment)"
KECCAK_CONTEXT = {
    "zksync-os": [
        (KECCAK_WITNESS_NODES, r"consult_cache_or_oracle"),
        (KECCAK_CODE, r"BytecodeKeccakPreimagesStorage|expose_preimage|set_bytecode|deploy_code|deployed_code"),
        ("EVM: SHA3 opcode", r"Interpreter>::sha3"),
        ("EVM: other (CREATE2 address, ...)", r"Interpreter>::"),
        ("MPT: node hashing for the new root", r"ethereum_storage_model::mpt|EthereumMPT|update_commitment|persist_changes|mpt_leaf"),
        ("block: logs bloom, tx/receipt roots, header", r"logs_bloom|EthereumPostOp|receipts|block_data|block_header"),
        ("tx: signed hash, tx hash, sender address", r"validate_and_compute_fee|ecrecover|rlp_encoded|transaction::"),
    ],
    "zilkworm": [
        (KECCAK_WITNESS_NODES + " + " + KECCAK_CODE + " (one upfront pass)", r"DirectState::sanitize"),
        ("EVM: SHA3 opcode", r"evmone::instr::core::keccak256"),
        ("EVM: other (CREATE2 address, ...)", r"evmone::"),
        ("MPT: node hashing for the new root", r"GridMPT|calc_root_from_updates|check_root"),
        ("tx: signed hash, tx hash, sender address", r"Transaction::sender|validate_transaction|Transaction::hash"),
        ("block: logs bloom, tx/receipt roots, header", r"logs_bloom|m3_2048|HashBuilder|root_hash|BlockHeader"),
    ],
}


def keccak_by_purpose(path, project):
    """Samples under the outermost keccak frame, by what the enclosing code does."""
    _total, frames = fg.parse_svg(path)
    roots = fg.build_tree(frames)
    is_keccak = re.compile(KECCAK_FRAME[project])
    contexts = compile_rules(KECCAK_CONTEXT[project])
    counts = collections.Counter()

    def walk(frame, names):
        name = fg.strip_name(frame["name"])
        if is_keccak.search(name):
            purpose, _ = first_match(contexts, names)
            counts[purpose or "other"] += frame["samples"]
            return
        names.append(name)
        for c in frame["children"]:
            walk(c, names)
        names.pop()

    for r in roots:
        walk(r, [])
    return counts


def compile_rules(rules):
    return [(name, re.compile(pattern)) for name, pattern in rules]


def first_match(rules, names):
    """Rules are in priority order: the first rule matching *any* frame wins,
    so a specific part (e.g. tx validation) beats the generic transaction
    frame that encloses it. Returns (category, index of the matched frame)."""
    for cat, rx in rules:
        for idx, name in enumerate(names):
            if rx.search(name):
                return cat, idx
    return None, None


def classify(path, level1, precompiles, crosscut):
    """path: list of stripped frame names from root to the frame owning the samples."""
    lvl1, evm_start = first_match(level1, path)
    if lvl1 is None:
        return OTHER
    if lvl1 != EVM:
        return lvl1
    sub = path[evm_start:]
    pre, _ = first_match(precompiles, sub)
    if pre is not None:
        return pre
    # cross-cutting costs: innermost frame wins
    for name in reversed(sub):
        for cat, rx in crosscut:
            if rx.search(name):
                return cat
    return EVM_CORE


def categorize_svg(path, project):
    total, frames = fg.parse_svg(path)
    roots = fg.build_tree(frames)
    level1 = compile_rules(LEVEL1[project])
    pre = compile_rules(PRECOMPILES[project])
    cross = compile_rules(CROSSCUT[project])
    counts = collections.Counter()

    def walk(frame, names):
        name = fg.strip_name(frame["name"])
        names.append(name)
        self_samples = frame["samples"] - sum(c["samples"] for c in frame["children"])
        if self_samples > 0:
            counts[classify(names, level1, pre, cross)] += self_samples
        for c in frame["children"]:
            walk(c, names)
        names.pop()

    for r in roots:
        walk(r, [])
    return total, counts


def debug_paths(path, project, category, acc):
    """Accumulate, per stack path (root..frame, shortened), the samples that
    land in `category`, to check what a category really contains."""
    total, frames = fg.parse_svg(path)
    roots = fg.build_tree(frames)
    level1 = compile_rules(LEVEL1[project])
    pre = compile_rules(PRECOMPILES[project])
    cross = compile_rules(CROSSCUT[project])

    def short(n):
        n = re.sub(r"\(.*$", "", n)
        return n[-60:]

    def walk(frame, names):
        names.append(fg.strip_name(frame["name"]))
        self_samples = frame["samples"] - sum(c["samples"] for c in frame["children"])
        if self_samples > 0 and classify(names, level1, pre, cross) == category:
            key = " > ".join(short(n) for n in names[-6:])
            acc[key] += self_samples
        for c in frame["children"]:
            walk(c, names)
        names.pop()

    for r in roots:
        walk(r, [])


def read_cycles(path):
    out = {}
    with open(path) as fh:
        for row in csv.DictReader(fh):
            out[int(row["block"])] = (int(row["cycles"]), int(row.get("gas") or 0))
    return out


def parse_side(spec):
    name, svg_dir, cycles_csv = spec.split(":", 2)
    if name not in LEVEL1:
        raise SystemExit(f"unknown project {name!r}; known: {', '.join(LEVEL1)}")
    return name, svg_dir, read_cycles(cycles_csv)


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--a", required=True, help="PROJECT:SVG_DIR:CYCLES_CSV")
    ap.add_argument("--b", required=True, help="PROJECT:SVG_DIR:CYCLES_CSV")
    ap.add_argument("--min-share", type=float, default=1.0, help="flag threshold: min %% of total on either side")
    ap.add_argument("--ratio", type=float, default=2.0, help="flag threshold: cycle ratio between sides")
    ap.add_argument("--debug", help="print the top stack paths attributed to this category")
    ap.add_argument("--debug-side", help="project name the --debug category is inspected on")
    args = ap.parse_args()
    debug_acc = collections.Counter()

    (name_a, dir_a, cyc_a) = parse_side(args.a)
    (name_b, dir_b, cyc_b) = parse_side(args.b)
    blocks = sorted(b for b in cyc_a if b in cyc_b
                    and os.path.exists(os.path.join(dir_a, f"{b}.svg"))
                    and os.path.exists(os.path.join(dir_b, f"{b}.svg")))
    if not blocks:
        raise SystemExit("no common blocks with flamegraphs on both sides")

    totals = {name_a: collections.Counter(), name_b: collections.Counter()}
    unattributed = {name_a: 0, name_b: 0}
    keccak = {name_a: collections.Counter(), name_b: collections.Counter()}
    sum_cycles = {name_a: 0, name_b: 0}
    sum_gas = 0
    print(f"{'block':>9} {'gas':>11} | {name_a+' cycles':>18} {'c/gas':>6} | {name_b+' cycles':>18} {'c/gas':>6} | ratio")
    for b in blocks:
        rows = {}
        for name, d, cyc in ((name_a, dir_a, cyc_a), (name_b, dir_b, cyc_b)):
            total_samples, counts = categorize_svg(os.path.join(d, f"{b}.svg"), name)
            cycles, gas = cyc[b]
            for cat, samples in counts.items():
                totals[name][cat] += samples * SAMPLING_RATE
            # samples whose frame-pointer chain could not be walked are dropped by
            # the profiler; account for them explicitly instead of inflating the rest
            missing = max(0, cycles - total_samples * SAMPLING_RATE)
            totals[name][UNATTRIBUTED] += missing
            unattributed[name] += missing
            for purpose, samples in keccak_by_purpose(os.path.join(d, f"{b}.svg"), name).items():
                keccak[name][purpose] += samples * SAMPLING_RATE
            if args.debug and name == args.debug_side:
                debug_paths(os.path.join(d, f"{b}.svg"), name, args.debug, debug_acc)
            sum_cycles[name] += cycles
            rows[name] = (cycles, gas)
        gas = rows[name_a][1] or rows[name_b][1]
        sum_gas += gas
        ca, cb = rows[name_a][0], rows[name_b][0]
        print(f"{b:>9} {gas:>11,} | {ca:>18,} {ca/gas:>6.1f} | {cb:>18,} {cb/gas:>6.1f} | {cb/ca:5.2f}")
    ca, cb = sum_cycles[name_a], sum_cycles[name_b]
    print(f"{'total':>9} {sum_gas:>11,} | {ca:>18,} {ca/sum_gas:>6.1f} | {cb:>18,} {cb/sum_gas:>6.1f} | {cb/ca:5.2f}")
    for name in (name_a, name_b):
        print(f"  {name}: samples with no frame-pointer chain (unattributed) ~ {unattributed[name]/sum_cycles[name]*100:.1f}% of cycles")

    cats = sorted(set(totals[name_a]) | set(totals[name_b]),
                  key=lambda c: -(totals[name_a][c] + totals[name_b][c]))
    print()
    print(f"{'category':<58} {name_a+' Mcyc':>15} {'%':>6} {name_b+' Mcyc':>15} {'%':>6} {'b/a':>6}  flag")
    flagged = []
    for cat in cats:
        va, vb = totals[name_a][cat], totals[name_b][cat]
        pa, pb = va / ca * 100, vb / cb * 100
        ratio = vb / va if va > 0 else float("inf")
        flag = ""
        if max(pa, pb) >= args.min_share and (ratio >= args.ratio or ratio <= 1 / args.ratio):
            flag = "<<" if ratio < 1 else ">>"
            flagged.append((cat, va, vb, pa, pb, ratio))
        print(f"{cat:<58} {va/1e6:>15,.0f} {pa:>6.1f} {vb/1e6:>15,.0f} {pb:>6.1f} {ratio:>6.2f}  {flag}")
    print()
    print("Keccak by purpose (inclusive cycles of the hash function, already counted in the categories above):")
    for name, total in ((name_a, ca), (name_b, cb)):
        all_keccak = sum(keccak[name].values())
        print(f"  {name}: {all_keccak/1e6:,.0f} Mcyc ({all_keccak/total*100:.1f}% of cycles)")
        for purpose, v in keccak[name].most_common():
            print(f"    {purpose:<96} {v/1e6:>8,.1f} {v/total*100:>6.2f}%")
    if args.debug:
        print()
        print(f"Top stack paths in category {args.debug!r} on {args.debug_side} (samples):")
        for key, n in debug_acc.most_common(25):
            print(f"  {n:>9}  {key}")
    print()
    print(f"Flagged (same logical part, >= {args.ratio}x cycle discrepancy, >= {args.min_share}% share on a side):")
    for cat, va, vb, pa, pb, ratio in flagged:
        cheaper = name_b if ratio < 1 else name_a
        print(f"  {cat}: {name_a} {va/1e6:,.0f} Mcyc ({pa:.1f}%) vs {name_b} {vb/1e6:,.0f} Mcyc ({pb:.1f}%); {cheaper} is {max(ratio, 1/ratio):.1f}x cheaper")


if __name__ == "__main__":
    main()
