import sys
import re
import ast

U256BIGINTOPS_RATIO = 4
BLAKE2ROUNDEXTENDED_RATIO = 16

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
                raw = int(m.group(2))
                delegs = eval(m.group(3))

                blake = delegs.get(1991, 0)
                bigint = delegs.get(1994, 0)
                weighted = blake * BLAKE2ROUNDEXTENDED_RATIO + bigint * U256BIGINTOPS_RATIO
                weighted += sum(v for k, v in delegs.items() if k not in (1991, 1994))

                eff = raw + weighted
                prev = results.get(name)
                if not prev or eff > prev['effective']:
                    results[name] = {
                        'raw': raw,
                        'blake': blake,
                        'bigint': bigint,
                        'effective': eff
                    }
    return results

def pct_change(old, new):
    if old == 0:
        return float('inf') if new > 0 else 0.0
    return (new - old) / old * 100

def main():
    if len(sys.argv) != 2:
        print("Usage: python compare_bench.py '[...]'")
        sys.exit(1)

    try:
        benchmarks = ast.literal_eval(sys.argv[1])
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

            b_raw = b.get('raw', 0)
            h_raw = h.get('raw', 0)
            b_blake = b.get('blake', 0)
            h_blake = h.get('blake', 0)
            b_bigint = b.get('bigint', 0)
            h_bigint = h.get('bigint', 0)
            b_eff = b.get('effective', 0)
            h_eff = h.get('effective', 0)

            rows.append((
                name, sym,
                b_raw, h_raw, pct_change(b_raw, h_raw),
                b_blake, h_blake, pct_change(b_blake, h_blake),
                b_bigint, h_bigint, pct_change(b_bigint, h_bigint),
                b_eff, h_eff, pct_change(b_eff, h_eff)
            ))

    # Markdown table
    print("### Benchmark report\n")
    print("| Benchmark | Symbol | Base Eff | Head Eff (%) | Base Raw | Head Raw (%) | Base Blake | Head Blake (%) | Base Bigint | Head Bigint (%) |")
    print("|-----------|--------|-----------|----------------|-----------|----------------|-------------|------------------|---------------|--------------------|")

    for r in rows:
        print(f"| `{r[0]}` | `{r[1]}` "
              f"| {r[11]:,} | {r[12]:,} ({r[13]:+.2f}%) "
              f"| {r[2]:,} | {r[3]:,} ({r[4]:+.2f}%) "
              f"| {r[5]:,} | {r[6]:,} ({r[7]:+.2f}%) "
              f"| {r[8]:,} | {r[9]:,} ({r[10]:+.2f}%)")

if __name__ == "__main__":
    main()
