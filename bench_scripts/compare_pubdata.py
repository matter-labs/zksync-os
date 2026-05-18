"""
Compare pubdata sizes across a list of (benchmark_name, base.bench, head.bench)
triples and emit a markdown table — but only when at least one pair's value
actually changed between base and head.

Reads the `pubdata_bytes: N` line that `tests/instances/eth_runner/src/single_run.rs`
appends to each cycle-marker bench file.

When the base file predates the pubdata-bytes marker plumbing (e.g. an older
merge-base on the PR's base branch), the missing value is treated as 0; the
row will still surface so the absolute head value is visible, with the change
clearly attributable to the marker not existing yet on base.
"""

import ast
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from benchlib import pct as pct_change  # noqa: E402


# Sentinel for "no marker line found" — distinguishes a file that was
# generated before this measurement existed from a file that legitimately
# recorded 0 bytes (e.g. an `empty_no_da` DA-scheme run).
MISSING = object()


def parse_pubdata_bytes(text):
    """Return the last `pubdata_bytes: N` value in `text`, or `MISSING`.

    `single_run.rs` appends once per block run, so a single bench file
    normally contains exactly one line. Reading the last match keeps the
    behavior obvious if a future change writes the line more than once.
    """
    matches = re.findall(r"^pubdata_bytes:\s*(\d+)\s*$", text, re.MULTILINE)
    if not matches:
        return MISSING
    return int(matches[-1])


def _read_file(path):
    try:
        with open(path) as f:
            return f.read()
    except FileNotFoundError:
        return ""


def main():
    if len(sys.argv) != 2:
        print("Usage: python compare_pubdata.py '[(name, base.bench, head.bench), ...]'", file=sys.stderr)
        sys.exit(1)

    try:
        pairs = ast.literal_eval(sys.argv[1])
    except Exception as e:
        print(f"Invalid input format: {e}", file=sys.stderr)
        sys.exit(1)

    rows = []
    any_changed = False

    for entry in pairs:
        if len(entry) < 3:
            print(f"Invalid pair: {entry}", file=sys.stderr)
            continue
        name, base_file, head_file = entry[:3]

        base = parse_pubdata_bytes(_read_file(base_file))
        head = parse_pubdata_bytes(_read_file(head_file))

        if base is MISSING and head is MISSING:
            # Neither side recorded a value — nothing meaningful to compare.
            continue

        base_val = 0 if base is MISSING else base
        head_val = 0 if head is MISSING else head

        if base_val != head_val:
            any_changed = True
        rows.append((name, base_val, head_val, pct_change(base_val, head_val)))

    if not any_changed:
        return

    print("## Pubdata bytes\n")
    print("| Benchmark | Base | Head (Δ%) |")
    print("|-----------|------|-----------|")
    for name, base_val, head_val, delta in rows:
        print(f"| `{name}` | {base_val:,} | {head_val:,} ({delta:+.2f}%) |")


if __name__ == "__main__":
    main()
