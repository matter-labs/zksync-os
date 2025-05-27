import sys
import re
import numpy as np
import matplotlib.pyplot as plt

# Constants
U256BIGINTOPS_RATIO = 8
BLAKE2ROUNDEXTENDED_RATIO = 16
ERGS_PER_GAS = 256

def parse_blocks(text):
    return text.strip().split("==================")

def extract_param(block):
    match = re.search(r"Params: (\w+):(\d+)", block)
    if match:
        return match.group(1), int(match.group(2))
    return None, None

def extract_ergs(block):
    return {
        m.group(1): int(m.group(2))
        for m in re.finditer(r"Spent ergs for \[(\w+)\]: (\d+)", block)
    }

def extract_markers(block):
    markers = {}
    match = re.search(r"=== Cycle markers:\n(.*?)\nTotal delegations", block, re.DOTALL)
    if not match:
        return markers

    for line in match.group(1).strip().splitlines():
        m = re.match(r"(\w+): net cycles: (\d+), net delegations: ({.*})", line.strip())
        if m:
            name = m.group(1)
            cycles = int(m.group(2))
            delegs = eval(m.group(3))
            markers[name] = {"cycles": cycles, "delegations": delegs}
    return markers

def compute_ratio(marker_data, ergs):
    cycles = marker_data['cycles']
    delegations = marker_data['delegations']
    weighted_deleg = 0
    for k, v in delegations.items():
        if k == 1994:
            weighted_deleg += v * U256BIGINTOPS_RATIO
        elif k == 1991:
            weighted_deleg += v * BLAKE2ROUNDEXTENDED_RATIO
        else:
            weighted_deleg += v

    total_cycles = cycles + weighted_deleg
    gas = ergs / ERGS_PER_GAS
    return total_cycles / gas if gas > 0 else None

def main():
    if len(sys.argv) < 3:
        print("Usage: python plot_ratios.py <marker> file1.txt [file2.txt ...]")
        sys.exit(1)

    target_marker = sys.argv[1]
    files = sys.argv[2:]
    data = []
    param_name = None

    for path in files:
        with open(path) as f:
            blocks = parse_blocks(f.read())
            for block in blocks:
                key, val = extract_param(block)
                if param_name is None:
                    param_name = key  # use the first one we see
                elif key != param_name:
                    continue  # skip inconsistent keys

                ergs_map = extract_ergs(block)
                markers = extract_markers(block)
                if key and val is not None and target_marker in markers and target_marker in ergs_map:
                    ratio = compute_ratio(markers[target_marker], ergs_map[target_marker])
                    if ratio is not None:
                        data.append((val, ratio))

    if not data:
        print(f"No data found for marker '{target_marker}'")
        sys.exit(0)

    # Sort and unpack
    data.sort()
    x, y = zip(*data)

    max_param = max(x)
    xticks = [2**i for i in range(int(np.log2(max_param)) + 1)]
    if max_param not in xticks:
        xticks.append(max_param)

    plt.figure(figsize=(8, 5))
    plt.plot(x, y, marker='o')
    plt.xscale("log", base=2)
    plt.xticks(xticks)

    plt.title(f"Ratio vs {param_name} for '{target_marker}'")
    plt.xlabel(f"{param_name}")
    plt.ylabel("Cycle/Gas Ratio")
    plt.grid(True, which='both', linestyle='--', linewidth=0.5)
    plt.tight_layout()
    plt.show()

if __name__ == "__main__":
    main()
