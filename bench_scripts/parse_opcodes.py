import sys
import re
import csv
import matplotlib.pyplot as plt

def parse_opcodes(filename):
    opcodes = []
    with open(filename) as f:
        for line in f:
            match = re.match(r"Opcode (\w+): used ([\d_]+) times", line.strip())
            if match:
                opcode = match.group(1)
                count = int(match.group(2).replace('_', ''))
                opcodes.append((opcode, count))
    return opcodes

def write_csv(opcodes, out_file):
    with open(out_file, 'w', newline='') as f:
        writer = csv.writer(f)
        writer.writerow(["opcode", "count", "percentage"])
        total = sum(c for _, c in opcodes)
        for opcode, count in opcodes:
            pct = (count / total) * 100
            writer.writerow([opcode, count, f"{pct:.2f}"])

def plot_opcodes(opcodes, out_file):
    total = sum(count for _, count in opcodes)
    data = [(op, (count / total) * 100) for op, count in opcodes]
    data.sort(key=lambda x: x[1], reverse=True)

    labels = [op for op, _ in data]
    percentages = [pct for _, pct in data]

    fig, ax = plt.subplots(figsize=(12, 6))
    bars = ax.bar(labels, percentages)
    ax.set_ylabel("Opcode Usage (%)")
    ax.set_title("Opcode Frequency (Relative %)")

    plt.xticks(rotation=45, ha='right')

    for bar, pct in zip(bars, percentages):
        ax.text(bar.get_x() + bar.get_width()/2, bar.get_height() + 0.5,
                f"{pct:.1f}%", ha='center', va='bottom', fontsize=8)

    plt.tight_layout()
    plt.savefig(out_file)

if __name__ == "__main__":
    if len(sys.argv) != 4:
        print("Usage: python parse_opcodes.py <input.txt> <output.csv> <output.png>")
        sys.exit(1)

    input_file = sys.argv[1]
    csv_out = sys.argv[2]
    png_out = sys.argv[3]

    opcodes = parse_opcodes(input_file)
    write_csv(opcodes, csv_out)
    plot_opcodes(opcodes, png_out)
