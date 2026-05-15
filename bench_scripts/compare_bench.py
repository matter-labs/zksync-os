import os
import sys
import re
import ast

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from benchlib import (  # noqa: E402
    BIGINT_DELEGATION_COEFF,
    BIGINT_DELEGATION_ID,
    BLAKE_DELEGATION_COEFF,
    BLAKE_DELEGATION_ID,
    KECCAK_DELEGATION_COEFF,
    KECCAK_DELEGATION_ID,
    pct as pct_change,  # historical name
)

# Unknown-delegation policy: when this parser computes effective from the
# raw `.bench` text it adds +1 per occurrence (coefficient = 1) for any
# delegation ID outside the weighted set (BLAKE/BIGINT/KECCAK). This is
# compare_bench's deliberate choice — it keeps unfamiliar delegation IDs
# visible in headline cycles without requiring script updates. The
# Rust-side `effective_of` helper does NOT apply this fallback because it
# operates on per-execution sample dumps restricted to the three weighted
# IDs; that's why headline `block_effective` can differ slightly from
# compare_bench.py's `Eff` column.

def parse_cycle_markers(text):
    results = {}
    blocks = text.split("==================")
    for block in blocks:
        match = re.search(r"=== Cycle markers:\n(.*?)(?:\nTotal delegations|\Z)", block, re.DOTALL)
        if not match:
            continue
        for line in match.group(1).strip().splitlines():
            m = re.match(r"(\w+): net cycles: (\d+), net delegations: (\{.*\})", line.strip())
            if m:
                name = m.group(1)
                # Compatibility aliases for marker-name transitions. These
                # let bench-base on older merge-bases match bench-head's
                # current names so the PR-comment rows pair up correctly.
                # TODO: drop each alias one PR cycle after the merge-base
                # SHA on `draft-0.4.0` reliably contains the new name.
                #
                # Alias 1: `<base>_execution_environment` → `<base>`.
                #   Introduced by feat(bench): mark user-EVM-EE keccak/ecrecover.
                #   The outer cycles INCLUDE the inner ones, so collapsing
                #   onto the base name + max-fold picks the larger value —
                #   avoiding double rows in the PR comment.
                # Alias 2: `verify_and_apply_batch` → `state_commitment_update`.
                #   Introduced when the marker was renamed (the old name was
                #   misleading — it always wrapped only the state-tree commit).
                if name.endswith("_execution_environment"):
                    name = name[: -len("_execution_environment")]
                elif name == "verify_and_apply_batch":
                    name = "state_commitment_update"
                raw = int(m.group(2))
                delegs = ast.literal_eval(m.group(3))

                blake = delegs.get(BLAKE_DELEGATION_ID, 0)
                bigint = delegs.get(BIGINT_DELEGATION_ID, 0)
                keccak = delegs.get(KECCAK_DELEGATION_ID, 0)
                weighted = (
                    blake * BLAKE_DELEGATION_COEFF
                    + bigint * BIGINT_DELEGATION_COEFF
                    + keccak * KECCAK_DELEGATION_COEFF
                )
                weighted += sum(
                    v
                    for k, v in delegs.items()
                    if k not in (BLAKE_DELEGATION_ID, BIGINT_DELEGATION_ID, KECCAK_DELEGATION_ID)
                )

                eff = raw + weighted
                prev = results.get(name)
                if not prev or eff > prev['effective']:
                    results[name] = {
                        'raw': raw,
                        'blake': blake,
                        'bigint': bigint,
                        'keccak': keccak,
                        'effective': eff
                    }
    return results

def main():
    # `--no-title` lets the caller (e.g. bench.yml) provide its own section
    # heading; useful when the same script is invoked multiple times to
    # render different sub-tables under separate headings/spoilers.
    # `--sort-by-symbol` groups rows by Symbol (then benchmark name) so
    # all rows for the same marker line up — easier to scan when the
    # table has many (benchmark × symbol) combinations like the
    # block-level sub-phases view.
    cli_flags = {"--no-title", "--sort-by-symbol"}
    args = [a for a in sys.argv[1:] if a not in cli_flags]
    emit_title = "--no-title" not in sys.argv[1:]
    sort_by_symbol = "--sort-by-symbol" in sys.argv[1:]
    if len(args) != 1:
        print("Usage: python compare_bench.py [--no-title] [--sort-by-symbol] '[...]'")
        sys.exit(1)

    try:
        benchmarks = ast.literal_eval(args[0])
    except Exception as e:
        print(f"Invalid input format: {e}")
        sys.exit(1)

    rows = []

    for entry in benchmarks:
        if len(entry) < 3:
            print(f"Invalid benchmark entry: {entry}")
            continue

        name, base_file, head_file = entry[:3]
        explicit_symbol = entry[3] if len(entry) >= 4 else None

        try:
            with open(base_file) as f:
                base_text = f.read()
        except FileNotFoundError:
            base_text = ""
        try:
            with open(head_file) as f:
                head_text = f.read()
        except FileNotFoundError:
            head_text = ""

        base = parse_cycle_markers(base_text)
        head = parse_cycle_markers(head_text)

        symbols = [explicit_symbol] if explicit_symbol else sorted(set(base) | set(head))

        for sym in symbols:
            b = base.get(sym, {})
            h = head.get(sym, {})

            # Skip symbols absent on both sides (e.g. an explicitly-requested
            # block-level sub-phase that doesn't exist in this run's bench
            # file would otherwise produce a noisy all-zero row).
            if not b and not h:
                continue

            b_raw = b.get('raw', 0)
            h_raw = h.get('raw', 0)
            b_blake = b.get('blake', 0)
            h_blake = h.get('blake', 0)
            b_bigint = b.get('bigint', 0)
            h_bigint = h.get('bigint', 0)
            b_keccak = b.get('keccak', 0)
            h_keccak = h.get('keccak', 0)
            b_eff = b.get('effective', 0)
            h_eff = h.get('effective', 0)

            rows.append((
                name, sym,
                b_raw, h_raw, pct_change(b_raw, h_raw),
                b_blake, h_blake, pct_change(b_blake, h_blake),
                b_bigint, h_bigint, pct_change(b_bigint, h_bigint),
                b_keccak, h_keccak, pct_change(b_keccak, h_keccak),
                b_eff, h_eff, pct_change(b_eff, h_eff)
            ))

    # Skip emitting anything when there are no rows so callers wrapping the
    # output in `<details>` don't produce an empty section.
    if not rows:
        return

    if sort_by_symbol:
        # row[0] = benchmark name, row[1] = symbol. Stable sort on
        # (symbol, name) groups all rows of the same marker together.
        rows.sort(key=lambda r: (r[1], r[0]))

    # Markdown table
    if emit_title:
        print("### Benchmark report\n")
    print("| Benchmark | Symbol | Base Eff | Head Eff (%) | Base Raw | Head Raw (%) | Base Blake | Head Blake (%) | Base Bigint | Head Bigint (%) | Base Keccak | Head Keccak (%) |")
    print("|-----------|--------|-----------|----------------|-----------|----------------|-------------|------------------|---------------|--------------------|--------------|--------------------|")

    for r in rows:
        print(f"| `{r[0]}` | `{r[1]}` "
              f"| {r[14]:,} | {r[15]:,} ({r[16]:+.2f}%) "
              f"| {r[2]:,} | {r[3]:,} ({r[4]:+.2f}%) "
              f"| {r[5]:,} | {r[6]:,} ({r[7]:+.2f}%) "
              f"| {r[8]:,} | {r[9]:,} ({r[10]:+.2f}%) "
              f"| {r[11]:,} | {r[12]:,} ({r[13]:+.2f}%)")

if __name__ == "__main__":
    main()
