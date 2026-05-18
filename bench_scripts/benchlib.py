"""Shared helpers for bench_scripts/*.

Concentrates the formatting, percentile, and sample-loading utilities that
multiple scripts had been re-implementing slightly differently. Keep this
module dependency-free (stdlib only) so any script in `bench_scripts/` can
import it without adding a wheel.

Effective-cycle constants live alongside the helpers and MUST stay in
lockstep with `cycle_marker/src/lib.rs::print_cycle_markers` (see
`compare_bench.py` for the unknown-delegation handling rationale).
"""

import os


# Delegation IDs — must match cycle_marker/src/lib.rs::print_cycle_markers
BLAKE_DELEGATION_ID = 1991
BIGINT_DELEGATION_ID = 1994
KECCAK_DELEGATION_ID = 1995

# Effective-cycle weights — must match cycle_marker's BLAKE_DELEGATION_COEFF,
# BIGINT_DELEGATION_COEFF, KECCAK_DELEGATION_COEFF. If these drift, the
# Python-side reports will diverge from Rust-side `block_effective` and
# from the per-execution `.effective.cycles` dumps.
BLAKE_DELEGATION_COEFF = 16
BIGINT_DELEGATION_COEFF = 4
KECCAK_DELEGATION_COEFF = 4


def median_int(values):
    """Median of an iterable of integers (returns int). Empty → 0."""
    vals = sorted(values)
    if not vals:
        return 0
    mid = len(vals) // 2
    if len(vals) % 2 == 0:
        return (vals[mid - 1] + vals[mid]) // 2
    return vals[mid]


def median_float(values):
    """Median of an iterable of floats. Empty → None."""
    vals = sorted(values)
    if not vals:
        return None
    mid = len(vals) // 2
    if len(vals) % 2 == 0:
        return (vals[mid - 1] + vals[mid]) / 2
    return vals[mid]


def percentile(sorted_vals, p):
    """Nearest-rank percentile (1-indexed). `sorted_vals` must already be sorted.

    Returns 0 for an empty input.
    """
    if not sorted_vals:
        return 0
    rank = max(1, -(-len(sorted_vals) * p // 100))  # ceiling division
    return sorted_vals[min(rank, len(sorted_vals)) - 1]


def pct(old, new):
    """Percent change `(new - old) / old * 100`.

    Returns 0 when both sides are 0, `inf` when old is 0 and new > 0.
    """
    if old == 0:
        return 0.0 if new == 0 else float("inf")
    return (new - old) / old * 100


def fmt_pct(val):
    """Format a percent value as ` (+1.2%)` / ` (-3.4%)`. Empty for ~0."""
    if val is None:
        return ""
    if val == float("inf"):
        return " (new)"
    if abs(val) < 0.005:
        return ""
    return f" ({val:+.1f}%)"


def fmt_val_pct(base, head):
    """Format `head (+1.2%)` for a base/head integer pair."""
    return f"{head}{fmt_pct(pct(base, head))}"


def fmt_ratio_pct(base, head):
    """Like `fmt_val_pct` but for float ratios with one decimal."""
    if base is None or head is None:
        return "—"
    return f"{head:.2f}{fmt_pct(pct(base, head))}"


def ratio(num, den):
    """`num / den` for positive `den`, else 0.0."""
    return num / den if den > 0 else 0.0


def safe_listdir(path):
    """`os.listdir(path)` that returns `[]` on any OSError (missing path, not a directory, /dev/null, …)."""
    try:
        return os.listdir(path)
    except OSError:
        return []


def load_int_samples(path):
    """Load one integer per non-empty line. Used for `.cycles` / `.effective.cycles` files."""
    samples = []
    with open(path) as f:
        for line in f:
            line = line.strip()
            if line:
                samples.append(int(line))
    return samples


def load_gas_native_samples(path):
    """Load `gas,native` per line. Used for `.samples` files emitted by tracers."""
    samples = []
    with open(path) as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            parts = line.split(",")
            samples.append((int(parts[0]), int(parts[1])))
    return samples


def list_label_files(samples_dir, raw_suffix=".cycles", effective_suffix=".effective.cycles"):
    """Return `(raw_names, effective_names, opcode_to_file)` for a samples dir.

    - `raw_names`: set of labels with a `.cycles` file (NOT `.effective.cycles`).
    - `effective_names`: set of labels with a `.effective.cycles` file.
    - `opcode_to_file`: dict label → filename to prefer (effective when present).
    """
    entries = set(safe_listdir(samples_dir))
    raw_names = set()
    effective_names = set()
    opcode_to_file = {}
    for name in entries:
        if name.endswith(effective_suffix):
            label = name[: -len(effective_suffix)]
            effective_names.add(label)
            opcode_to_file[label] = name
    for name in entries:
        if name.endswith(raw_suffix) and not name.endswith(effective_suffix):
            label = name[: -len(raw_suffix)]
            raw_names.add(label)
            opcode_to_file.setdefault(label, name)
    return raw_names, effective_names, opcode_to_file
