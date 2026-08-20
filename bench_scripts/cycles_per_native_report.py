#!/usr/bin/env python3
"""Per-execution cycles/native ratios for opcodes and precompiles.

Reads:
  - per-opcode tracer samples (`<dir>/<OPCODE>.samples`, `gas,native` per line)
  - per-opcode cycle samples (`<dir>/<OPCODE>.effective.cycles` preferred,
    else `<OPCODE>.cycles`)
  - per-precompile tracer samples (`<dir>/<precompile>.samples`)
  - per-label cycle samples (`<dir>/<label>.effective.cycles` preferred,
    else `<label>.cycles`) — `label → precompile` mapping via
    `bench_scripts.join_precompile_samples.CYCLE_LABEL_TO_PRECOMPILE`.

Computes `cycles / native` per execution. Reports median, p95, and max
ratio per opcode / precompile. Output as Markdown to stdout (or `--out`
file). Skips entries with `native == 0` on a given execution to avoid
divide-by-zero.

Usage:
    python cycles_per_native_report.py \\
        --opcode-samples-dir <DIR> --opcode-cycles-dir <DIR> \\
        --precompile-samples-dir <DIR> --precompile-cycles-dir <DIR> \\
        [--out report.md]
"""

import argparse
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from benchlib import (  # noqa: E402
    list_label_files,
    load_gas_native_samples,
    load_int_samples,
    percentile,
    ratio,
)
from join_precompile_samples import (  # noqa: E402
    CYCLE_LABEL_TO_PRECOMPILE,
    SYNTHETIC_OPCODE_SOURCES,
)


def collect_per_label_ratios(sources, label_to_sample_name):
    """For each label in `label_to_sample_name`:

    - across every `(samples_dir, cycles_dir)` pair in `sources`:
      - load `(gas, native)` pairs from `samples_dir/<sample_name>.samples`
      - load cycles from `cycles_dir/<label>.effective.cycles` preferred,
        else `cycles_dir/<label>.cycles`
      - pair `native[i]` with `cycles[i]` for `i in min(len(...), len(...))`
    - concatenate per-execution `cycles / native` ratios across all sources
    - return the merged sorted ratio list (with `native == 0` excluded)

    `sources`: iterable of (samples_dir, cycles_dir) tuples. Missing dirs
    or files are silently skipped per-source.
    `label_to_sample_name`: dict mapping cycle-marker label → tracer sample
    filename root (without `.samples`). For opcodes it's identity
    (OPCODE → OPCODE); for precompiles it's `CYCLE_LABEL_TO_PRECOMPILE`.

    Returns dict label → (ratios_sorted, kind, count_skipped_zero_native, total_paired_executions).
    `kind` reflects whichever variant was used in the LAST source that
    contributed data (effective preferred); if any source had only raw,
    `kind` may be "raw" — the field is informational, all ratios are
    pooled together.
    """
    out = {}
    for samples_dir, cycles_dir in sources:
        if not samples_dir or not cycles_dir:
            continue
        _, effective_files, _ = list_label_files(cycles_dir)
        for label, sample_name in label_to_sample_name.items():
            samples_path = os.path.join(samples_dir, f"{sample_name}.samples")
            if not os.path.isfile(samples_path):
                continue
            if label in effective_files:
                cycles_path = os.path.join(cycles_dir, f"{label}.effective.cycles")
                kind = "effective"
            else:
                cycles_path = os.path.join(cycles_dir, f"{label}.cycles")
                if not os.path.isfile(cycles_path):
                    continue
                kind = "raw"
            tracer_samples = load_gas_native_samples(samples_path)
            cycle_samples = load_int_samples(cycles_path)
            n = min(len(tracer_samples), len(cycle_samples))
            if n == 0:
                continue
            entry_ratios, entry_kind, entry_skipped, entry_n = out.get(
                label, ([], kind, 0, 0)
            )
            for i in range(n):
                _gas, native = tracer_samples[i]
                cycles = cycle_samples[i]
                if native > 0:
                    entry_ratios.append(ratio(cycles, native))
                else:
                    entry_skipped += 1
            entry_n += n
            # If any source provided "raw" for this label, downgrade kind
            # to "raw" so the report flags mixed sourcing.
            if kind == "raw":
                entry_kind = "raw"
            out[label] = (entry_ratios, entry_kind, entry_skipped, entry_n)
    # Sort accumulated ratios per label.
    for label, (rs, k, s, n) in list(out.items()):
        rs.sort()
        out[label] = (rs, k, s, n)
    # Strip labels that ended up with no usable ratios.
    return {label: data for label, data in out.items() if data[0]}


def opcode_label_map(samples_dirs):
    """For opcodes the label name IS the sample-file name (e.g. `SHA3` → `SHA3.samples` + `SHA3.cycles`).

    Accepts a list of opcode samples dirs and takes the union of opcode
    names found in any of them.
    """
    names = set()
    for d in samples_dirs:
        if not d:
            continue
        try:
            for f in os.listdir(d):
                if f.endswith(".samples"):
                    names.add(f[: -len(".samples")])
        except OSError:
            continue
    return {n: n for n in sorted(names)}


def precompile_label_map():
    """Map cycle-marker label → tracer-sample filename root.

    `CYCLE_LABEL_TO_PRECOMPILE` provides the mapping for real precompiles.
    Synthetic entries (currently `keccak` ← `SHA3` opcode samples) are
    surfaced in a separate report section.
    """
    return dict(CYCLE_LABEL_TO_PRECOMPILE)


def format_section(title, label_to_data, sort_key="max"):
    """Format `label_to_data` (returned by `collect_per_label_ratios`) as a
    Markdown table sorted by the worst-case ratio descending (or by p95 /
    median when `sort_key` differs)."""
    lines = []
    lines.append(f"### {title}")
    lines.append("")
    if not label_to_data:
        lines.append("_No samples available._")
        lines.append("")
        return lines
    lines.append("| Name | Count | Med cyc/native | p95 cyc/native | Max cyc/native | Cycle source |")
    lines.append("|---|---:|---:|---:|---:|---|")

    def sort_value(item):
        ratios, _kind, _skipped, _n = item[1]
        if sort_key == "median":
            return percentile(ratios, 50)
        if sort_key == "p95":
            return percentile(ratios, 95)
        return ratios[-1]

    for label, (ratios, kind, skipped, n) in sorted(
        label_to_data.items(), key=sort_value, reverse=True
    ):
        med = percentile(ratios, 50)
        p95 = percentile(ratios, 95)
        worst = ratios[-1]
        skipped_note = f" (+{skipped} skipped native=0)" if skipped else ""
        lines.append(
            f"| `{label}` | {len(ratios)}{skipped_note} | {med:.2f} | {p95:.2f} | {worst:.2f} | {kind} |"
        )
    lines.append("")
    return lines


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--opcode-samples-dir",
        action="append",
        default=[],
        help="Per-opcode tracer samples dir. Repeat to aggregate across multiple sources (e.g. one per block).",
    )
    parser.add_argument(
        "--opcode-cycles-dir",
        action="append",
        default=[],
        help="Per-opcode cycle samples dir. Repeat in the same order as --opcode-samples-dir to pair up sources.",
    )
    parser.add_argument(
        "--precompile-samples-dir",
        action="append",
        default=[],
        help="Per-precompile tracer samples dir. Repeat to aggregate.",
    )
    parser.add_argument(
        "--precompile-cycles-dir",
        action="append",
        default=[],
        help="Per-label cycle samples dir. Repeat in the same order as --precompile-samples-dir.",
    )
    parser.add_argument("--out", help="Write Markdown report to this file (default stdout)")
    parser.add_argument(
        "--sort-by",
        choices=("max", "p95", "median"),
        default="max",
        help="Sort each table by this percentile, descending (default: max)",
    )
    parser.add_argument(
        "--source-label",
        default="",
        help="Free-form text describing the source data (block, scheme, etc.); printed at the top of the report",
    )
    args = parser.parse_args()

    if not any(
        [
            args.opcode_samples_dir,
            args.opcode_cycles_dir,
            args.precompile_samples_dir,
            args.precompile_cycles_dir,
        ]
    ):
        parser.error("At least one --*-dir pair must be specified.")

    if len(args.opcode_samples_dir) != len(args.opcode_cycles_dir):
        parser.error(
            "--opcode-samples-dir and --opcode-cycles-dir must be specified the same number of times"
        )
    if len(args.precompile_samples_dir) != len(args.precompile_cycles_dir):
        parser.error(
            "--precompile-samples-dir and --precompile-cycles-dir must be specified the same number of times"
        )

    opcode_sources = list(zip(args.opcode_samples_dir, args.opcode_cycles_dir))
    precompile_sources = list(
        zip(args.precompile_samples_dir, args.precompile_cycles_dir)
    )

    sections = ["# Cycles / native ratios", ""]
    if args.source_label:
        sections.append(f"_Source: {args.source_label}._")
        sections.append("")
    sections.append(
        "Ratios computed per execution from paired `<sample>.samples` "
        "(gas,native) and `<label>.effective.cycles` (preferred) / "
        "`<label>.cycles`. Median, p95, max across all executions, "
        "pooled across every input source. Executions with `native == 0` "
        "are excluded."
    )
    sections.append("")

    # Opcodes.
    opcode_data = collect_per_label_ratios(
        opcode_sources,
        opcode_label_map(args.opcode_samples_dir),
    )
    sections += format_section("Per-opcode", opcode_data, sort_key=args.sort_by)

    # Precompiles (real precompile addresses).
    pre_data = collect_per_label_ratios(
        precompile_sources,
        precompile_label_map(),
    )
    sections += format_section("Per-precompile", pre_data, sort_key=args.sort_by)

    # Synthetic precompile entries (e.g. `keccak` sourced from SHA3 opcode
    # samples paired against the corresponding label-level cycle dump
    # from the precompile side). Surfaced separately so the source is
    # obvious. Requires that opcode-samples and precompile-cycles dirs
    # were provided in matching counts; pair them positionally.
    if (
        args.opcode_samples_dir
        and args.precompile_cycles_dir
        and len(args.opcode_samples_dir) == len(args.precompile_cycles_dir)
    ):
        synthetic_map = {}
        for label, prec_name in CYCLE_LABEL_TO_PRECOMPILE.items():
            opcode_name = SYNTHETIC_OPCODE_SOURCES.get(prec_name)
            if opcode_name:
                synthetic_map[label] = opcode_name
        synth_sources = list(zip(args.opcode_samples_dir, args.precompile_cycles_dir))
        synth_data = collect_per_label_ratios(synth_sources, synthetic_map)
        if synth_data:
            sections += format_section(
                "Per-precompile (synthetic — gas/native from opcode tracer)",
                synth_data,
                sort_key=args.sort_by,
            )

    out_text = "\n".join(sections) + "\n"
    if args.out:
        with open(args.out, "w") as f:
            f.write(out_text)
        print(f"Wrote {args.out}")
    else:
        sys.stdout.write(out_text)


if __name__ == "__main__":
    main()
