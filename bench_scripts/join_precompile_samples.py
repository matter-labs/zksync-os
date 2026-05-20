#!/usr/bin/env python3
"""Join per-execution precompile samples (gas/native from tracer, cycles from RISC-V).

Reads:
  - <tracer_dir>/<precompile>.samples: "gas,native" per line (execution order)
  - <cycles_dir>/<cycle_label>.effective.cycles (preferred) or
    <cycles_dir>/<cycle_label>.cycles (fallback): one cycle count per line in
    execution order.

The `.effective.cycles` variant includes delegation cost (Blake/BigInt/Keccak)
using the same coefficients as the block-wide `block_effective` formula in
`cycle_marker`. Without it, `cycles/gas` reflects raw RISC-V cycles only and
undercounts delegation-heavy precompiles (ecrecover, modexp, bn254). The
script prefers the effective variant and falls back to raw with a stderr note
when only raw is available.

Cycle-marker labels differ from the user-facing precompile names emitted by
`PrecompileStatsTracer::dump_samples`, so we apply a fixed mapping.
Unmapped `.cycles` files (e.g. `process_block.cycles`) are ignored silently.
Tracer `.samples` files with no matching cycle data are reported to stderr.

Outputs per-precompile CSV with (gas, native, cycles, cycles/gas, native/gas)
per execution, and a summary table with p50/p95/p99/max ratios.

Usage:
    python join_precompile_samples.py <tracer_dir> <cycles_dir> \
        [--out-dir <output_dir>] [--summary]
"""

import os
import sys
import argparse

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from benchlib import (  # noqa: E402
    load_gas_native_samples as load_tracer_samples,
    load_int_samples as load_cycle_samples,
    percentile,
    ratio,
    safe_listdir as _safe_listdir,
)


# Maps cycle_marker labels (used inside the RISC-V binary, written by
# `cycle_marker::wrap_with_resources!("<label>", ...)`) to the user-facing
# precompile names emitted by PrecompileStatsTracer::dump_samples /
# precompile_name(addr) in forward_system/src/system/tracers/precompile_stats.rs.
#
# The `_execution_environment` variants fire only on user-EVM-EE-triggered
# invocations (SHA3 opcode for keccak; EVM call-frame dispatch for ecrecover),
# producing samples that are 1:1 with the corresponding tracer/opcode gas
# samples — no positional intrinsic filter needed. When these are present
# they're preferred over the generic-label fallback.
CYCLE_LABEL_TO_PRECOMPILE = {
    "ecrecover_execution_environment": "ecrecover",
    "keccak_execution_environment": "keccak",
    "ecrecover": "ecrecover",
    "sha256": "sha256",
    "ripemd": "ripemd160",
    "id": "identity",
    "modexp": "modexp",
    "bn254_ecadd": "ecadd",
    "bn254_ecmul": "ecmul",
    "bn254_pairing": "ecpairing",
    "blake2f": "blake2f",
    "point_evaluation": "point_eval",
    "p256_verify": "p256_verify",
    "bls12_381_g1_add": "bls12_g1add",
    "bls12_381_g1_msm": "bls12_g1msm",
    "bls12_381_g2_add": "bls12_g2add",
    "bls12_381_g2_msm": "bls12_g2msm",
    "bls12_381_pairing": "bls12_pairing_check",
    "bls12_381_map_fp_to_g1": "bls12_map_fp_to_g1",
    "bls12_381_map_fp2_to_g2": "bls12_map_fp2_to_g2",
}

# Per-precompile gas-source override. By default the tracer's
# `<precompile>.samples` file is used; for the synthetic `keccak` entry,
# the gas/native pair comes from the SHA3 opcode's per-execution dump
# (`<opcode_dir>/SHA3.samples`) since there is no keccak precompile address.
SYNTHETIC_OPCODE_SOURCES = {
    "keccak": "SHA3",
}


def process_precompile(name, tracer_samples, cycle_samples, out_dir):
    """Join samples, write per-execution CSV if out_dir given, return summary stats."""
    n = min(len(tracer_samples), len(cycle_samples))
    if n == 0:
        return None

    if len(tracer_samples) != len(cycle_samples):
        print(
            f"  WARNING: {name} count mismatch: tracer={len(tracer_samples)} "
            f"cycles={len(cycle_samples)}, using first {n}",
            file=sys.stderr,
        )

    rows = [(tracer_samples[i][0], tracer_samples[i][1], cycle_samples[i]) for i in range(n)]

    # Write per-execution CSV
    if out_dir:
        os.makedirs(out_dir, exist_ok=True)
        path = os.path.join(out_dir, f"{name}.csv")
        with open(path, "w") as f:
            f.write("gas,native,cycles,cycles_per_gas,native_per_gas\n")
            for gas, native, cycles in rows:
                cpg = ratio(cycles, gas)
                npg = ratio(native, gas)
                f.write(f"{gas},{native},{cycles},{cpg:.2f},{npg:.2f}\n")

    # Compute summary statistics
    cycles_per_gas_values = sorted(ratio(c, g) for g, _, c in rows if g > 0)
    native_per_gas_values = sorted(ratio(nat, g) for g, nat, _ in rows if g > 0)

    if not cycles_per_gas_values:
        return None

    return {
        "name": name,
        "count": n,
        "med_cpg": percentile(cycles_per_gas_values, 50),
        "p95_cpg": percentile(cycles_per_gas_values, 95),
        "p99_cpg": percentile(cycles_per_gas_values, 99),
        "max_cpg": cycles_per_gas_values[-1],
        "med_npg": percentile(native_per_gas_values, 50),
        "p95_npg": percentile(native_per_gas_values, 95),
        "p99_npg": percentile(native_per_gas_values, 99),
        "max_npg": native_per_gas_values[-1],
    }


def filter_intrinsic_ecrecover_cycles(bench_path):
    """Identify indices into ecrecover.cycles that correspond to precompile-target
    calls (not tx-signature-verification intrinsics).

    Walks the cycle-marker bench file in start-cycle order (the order the file
    is written in) and tracks `process_transaction` boundaries. The FIRST
    `ecrecover` marker within each `process_transaction` is treated as the
    intrinsic sig verification; any subsequent `ecrecover` markers within that
    same transaction are precompile-target calls (via TxKind::Call(0x01)).

    Assumption: every `process_transaction` invokes ecrecover exactly once for
    signature verification before any user code runs. This holds for the
    standard L2 mainnet transaction path used by the bench fixtures. It does
    NOT hold for:
      - L1->L2 priority ops (no signature, no intrinsic ecrecover)
      - EIP-7702 set-code calls where authority recovery uses a different
        ecrecover invocation pattern
      - eth_call simulation paths that skip sig verification
    If those workloads ever land in the bench fixtures, replace this
    positional heuristic with a distinct cycle-marker label
    (e.g. `ecrecover_intrinsic`) emitted at the sig-verification call site.

    Returns the indices to KEEP (precompile-target only).
    """
    keep = []
    in_tx = False
    sig_seen_this_tx = False
    ecrecover_idx = -1
    try:
        with open(bench_path) as f:
            for line in f:
                stripped = line.strip()
                if stripped.startswith("process_transaction:"):
                    in_tx = True
                    sig_seen_this_tx = False
                elif stripped.startswith("ecrecover:"):
                    ecrecover_idx += 1
                    if in_tx and not sig_seen_this_tx:
                        sig_seen_this_tx = True
                        # Intrinsic — skip.
                    else:
                        keep.append(ecrecover_idx)
    except FileNotFoundError:
        return None
    return keep


def collect_source(
    tracer_dir,
    cycles_dir,
    bench_file=None,
    used_kinds=None,
    opcode_samples_dir=None,
):
    """Read one source (tracer_dir, cycles_dir, optional bench_file).

    Returns a dict mapping precompile_name -> (tracer_samples, cycle_samples).
    `cycle_samples` is filtered for intrinsic ecrecovers when bench_file is set
    AND the `_execution_environment` cycle marker is absent (legacy fallback;
    new binaries emit a distinct marker that obviates the positional filter).
    Sources with missing dirs are skipped silently (returns {}).

    Prefers `<label>.effective.cycles` (raw cycles + Blake/BigInt/Keccak
    weights) over `<label>.cycles` (raw only); falls back per-label if the
    effective variant is absent. `used_kinds`, when supplied, is populated
    with the set of kinds ("effective", "raw") that were actually consumed.

    For synthetic-precompile entries (see `SYNTHETIC_OPCODE_SOURCES`, e.g.
    `keccak` ← `SHA3.samples`), the tracer gas/native source is the named
    opcode sample file under `opcode_samples_dir` rather than a precompile
    tracer sample. Multiple cycle labels pointing to the same precompile name
    (e.g. `ecrecover_execution_environment` + `ecrecover`) are deduplicated:
    the first one with both tracer + cycle data wins, in iteration order of
    `CYCLE_LABEL_TO_PRECOMPILE` (so `_execution_environment` variants take
    precedence).
    """
    # Group cycle files by label, preferring `.effective.cycles` (raw +
    # delegation weights) over `.cycles` (raw only) per label.
    raw_files = set()
    effective_files = set()
    for f in _safe_listdir(cycles_dir):
        if f.endswith(".effective.cycles"):
            effective_files.add(f[: -len(".effective.cycles")])
        elif f.endswith(".cycles"):
            raw_files.add(f[: -len(".cycles")])
    cycles_labels = raw_files | effective_files
    tracer_files = {
        f[: -len(".samples")]
        for f in _safe_listdir(tracer_dir)
        if f.endswith(".samples")
    }
    opcode_files = {
        f[: -len(".samples")]
        for f in _safe_listdir(opcode_samples_dir or "")
        if f.endswith(".samples")
    }
    # Only invoke the positional filter if no execution-environment marker is
    # present (legacy binaries built before the dedicated marker landed).
    ecrecover_keep = (
        filter_intrinsic_ecrecover_cycles(bench_file)
        if bench_file and "ecrecover_execution_environment" not in cycles_labels
        else None
    )

    out = {}
    for cycle_label, precompile_name in CYCLE_LABEL_TO_PRECOMPILE.items():
        if cycle_label not in cycles_labels:
            continue
        if precompile_name in out:
            # An earlier (higher-priority) label already supplied this precompile.
            continue
        synthetic_opcode = SYNTHETIC_OPCODE_SOURCES.get(precompile_name)
        if synthetic_opcode is not None:
            if synthetic_opcode not in opcode_files:
                continue
            tracer_path = os.path.join(opcode_samples_dir, f"{synthetic_opcode}.samples")
        else:
            if precompile_name not in tracer_files:
                continue
            tracer_path = os.path.join(tracer_dir, f"{precompile_name}.samples")
        if cycle_label in effective_files:
            cycles_path = os.path.join(cycles_dir, f"{cycle_label}.effective.cycles")
            kind = "effective"
        else:
            cycles_path = os.path.join(cycles_dir, f"{cycle_label}.cycles")
            kind = "raw"
        if used_kinds is not None:
            used_kinds.add(kind)
        tracer_samples = load_tracer_samples(tracer_path)
        cycle_samples = load_cycle_samples(cycles_path)
        if cycle_label == "ecrecover" and ecrecover_keep is not None:
            cycle_samples = [
                cycle_samples[i] for i in ecrecover_keep if i < len(cycle_samples)
            ]
        out[precompile_name] = (tracer_samples, cycle_samples)
    return out


def main():
    parser = argparse.ArgumentParser(
        description="Join per-execution precompile samples across one or more sources"
    )
    parser.add_argument(
        "pairs",
        nargs="+",
        help="Alternating tracer_dir cycles_dir paths; one pair per data source.",
    )
    parser.add_argument("--out-dir", help="Write per-execution CSVs to this directory")
    parser.add_argument("--summary", action="store_true", help="Print summary table")
    parser.add_argument(
        "--bench-file",
        action="append",
        default=[],
        help="Cycle-marker .bench file. When the `ecrecover_execution_environment`"
        " marker is absent (legacy binaries), used to apply a positional filter"
        " that drops the per-tx intrinsic sig-verification ecrecover. Ignored"
        " when the marker is present. May be repeated; matched positionally to"
        " the (tracer_dir, cycles_dir) pairs.",
    )
    parser.add_argument(
        "--opcode-samples-dir",
        action="append",
        default=[],
        help="Directory containing per-opcode `<OPCODE>.samples` files (gas,native)."
        " Used to source gas/native for synthetic precompile entries that have no"
        " precompile address (currently `keccak` ← `SHA3.samples`). May be repeated;"
        " matched positionally to the (tracer_dir, cycles_dir) pairs.",
    )
    args = parser.parse_args()

    if len(args.pairs) % 2 != 0:
        print(
            "Error: need an even number of positional args (tracer_dir cycles_dir pairs)",
            file=sys.stderr,
        )
        sys.exit(1)

    sources = []
    used_kinds = set()
    for i in range(0, len(args.pairs), 2):
        tracer_dir = args.pairs[i]
        cycles_dir = args.pairs[i + 1]
        bench_file = (
            args.bench_file[i // 2] if (i // 2) < len(args.bench_file) else None
        )
        opcode_samples_dir = (
            args.opcode_samples_dir[i // 2]
            if (i // 2) < len(args.opcode_samples_dir)
            else None
        )
        sources.append(
            collect_source(
                tracer_dir,
                cycles_dir,
                bench_file,
                used_kinds=used_kinds,
                opcode_samples_dir=opcode_samples_dir,
            )
        )

    # Concatenate samples + cycles per precompile across all sources.
    aggregated = {}
    for src in sources:
        for name, (tracer_samples, cycle_samples) in src.items():
            agg = aggregated.setdefault(name, ([], []))
            agg[0].extend(tracer_samples)
            agg[1].extend(cycle_samples)

    summaries = []
    seen_precompiles = set(aggregated.keys())
    for name, (tracer_samples, cycle_samples) in aggregated.items():
        s = process_precompile(name, tracer_samples, cycle_samples, args.out_dir)
        if s:
            summaries.append(s)

    # Report tracer files that had no cycle counterpart across all sources.
    all_tracer_files = set()
    for i in range(0, len(args.pairs), 2):
        for f in _safe_listdir(args.pairs[i]):
            if f.endswith(".samples"):
                all_tracer_files.add(f[: -len(".samples")])
    missing_cycles = sorted(all_tracer_files - seen_precompiles)
    if missing_cycles:
        print(
            f"Note: no cycle data for {len(missing_cycles)} precompile(s): "
            f"{', '.join(missing_cycles)}",
            file=sys.stderr,
        )

    if not args.summary and not args.out_dir:
        args.summary = True

    if args.summary and summaries:
        if used_kinds == {"effective"}:
            cycles_label = "cycles = effective (raw + Blake×16 + BigInt×4 + Keccak×4)"
        elif used_kinds == {"raw"}:
            cycles_label = "cycles = raw RISC-V (delegations NOT included)"
        elif used_kinds == {"raw", "effective"}:
            cycles_label = "cycles = mixed (some labels lack effective dump; see stderr)"
            print(
                "Warning: cycles dump kind mixed across labels — re-run with a"
                " consistent cycle_marker build to avoid skewed comparisons.",
                file=sys.stderr,
            )
        else:
            cycles_label = "cycles = (no samples consumed)"
        summaries.sort(key=lambda s: s["max_cpg"], reverse=True)
        print(cycles_label)
        print(
            f"{'precompile':<22} {'count':>8}"
            f" {'med c/g':>10} {'p95 c/g':>10} {'p99 c/g':>10} {'max c/g':>10}"
            f" {'med n/g':>10} {'p95 n/g':>10} {'p99 n/g':>10} {'max n/g':>10}"
        )
        print("-" * 120)
        for s in summaries:
            print(
                f"{s['name']:<22} {s['count']:>8}"
                f" {s['med_cpg']:>10.1f} {s['p95_cpg']:>10.1f} {s['p99_cpg']:>10.1f} {s['max_cpg']:>10.1f}"
                f" {s['med_npg']:>10.1f} {s['p95_npg']:>10.1f} {s['p99_npg']:>10.1f} {s['max_npg']:>10.1f}"
            )

    if args.out_dir:
        print(f"\nPer-execution CSVs written to {args.out_dir}/")


if __name__ == "__main__":
    main()
