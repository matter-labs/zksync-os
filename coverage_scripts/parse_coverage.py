#!/usr/bin/env python3
"""Parse cargo-llvm-cov JSON output into a per-crate coverage summary table.

Usage:
    python3 parse_coverage.py [--workspace-root DIR] coverage.json

The JSON file is the output of:
    cargo llvm-cov report --json > coverage.json

The script maps each source file to its containing workspace crate by matching
the file path against crate directory prefixes. Files that don't belong to any
known crate are grouped under "(other)".

Output is a Markdown table sorted by line coverage percentage (ascending),
making it easy to spot under-tested crates.
"""

import argparse
import json
import os
import sys
from pathlib import Path


# Workspace member directory prefixes that contain test infrastructure,
# not production code. These are excluded from the coverage summary.
TEST_INFRA_PREFIXES = ("tests/", "scripts/")


def load_workspace_members(
    workspace_root: str,
) -> tuple[list[tuple[str, str]], list[tuple[str, str]]]:
    """Read Cargo.toml to discover workspace member directories.

    Returns two lists of (directory_prefix, crate_label) tuples, sorted
    longest-prefix-first so that nested crates match before their parents:
      1. Production crates
      2. Test infrastructure crates (under tests/, scripts/)
    """
    import re

    cargo_toml = os.path.join(workspace_root, "Cargo.toml")
    with open(cargo_toml) as f:
        content = f.read()

    # Extract the members array from [workspace] section.
    # This is a simple parser — it looks for `members = [...]` and extracts
    # quoted strings. It does not handle full TOML semantics.
    match = re.search(r"members\s*=\s*\[([^\]]*)\]", content, re.DOTALL)
    if not match:
        print("Warning: could not parse workspace members from Cargo.toml", file=sys.stderr)
        return [], []

    members_block = match.group(1)
    members = re.findall(r'"([^"]+)"', members_block)

    # Build (prefix, label) pairs. The prefix is the directory path with a
    # trailing slash so that "basic_system/" doesn't match "basic_system_hooks/".
    production = []
    test_infra = []
    for member in members:
        # Normalise path separators
        member = member.strip().replace("\\", "/")
        # Use the last path component as the crate label
        label = member.rstrip("/").split("/")[-1]
        prefix = member.rstrip("/") + "/"

        if any(member.startswith(tip) for tip in TEST_INFRA_PREFIXES):
            test_infra.append((prefix, label))
        else:
            production.append((prefix, label))

    # Sort longest prefix first for correct matching of nested crates
    production.sort(key=lambda x: -len(x[0]))
    test_infra.sort(key=lambda x: -len(x[0]))
    return production, test_infra


def map_file_to_crate(
    filename: str, members: list[tuple[str, str]]
) -> str:
    """Map a source file path to its containing crate label."""
    # Normalise the path (llvm-cov may emit absolute or relative paths)
    norm = filename.replace("\\", "/")

    for prefix, label in members:
        if prefix in norm:
            # Find the prefix position and check it starts at a path boundary
            idx = norm.find(prefix)
            if idx >= 0:
                return label

    return "(other)"


def parse_coverage(json_path: str, workspace_root: str) -> None:
    with open(json_path) as f:
        data = json.load(f)

    production_members, _test_members = load_workspace_members(workspace_root)
    # Only map files against production crates; test infrastructure is excluded
    members = production_members

    # Aggregate per-crate: {label: {lines_count, lines_covered, fn_count, fn_covered}}
    crates: dict[str, dict[str, int]] = {}

    for export in data.get("data", []):
        for file_entry in export.get("files", []):
            filename = file_entry.get("filename", "")
            summary = file_entry.get("summary", {})

            lines = summary.get("lines", {})
            functions = summary.get("functions", {})

            label = map_file_to_crate(filename, members)

            if label not in crates:
                crates[label] = {
                    "lines_count": 0,
                    "lines_covered": 0,
                    "fn_count": 0,
                    "fn_covered": 0,
                }

            crates[label]["lines_count"] += lines.get("count", 0)
            crates[label]["lines_covered"] += lines.get("covered", 0)
            crates[label]["fn_count"] += functions.get("count", 0)
            crates[label]["fn_covered"] += functions.get("covered", 0)

    if not crates:
        print("No coverage data found.")
        return

    # Remove "(other)" if empty
    if "(other)" in crates and crates["(other)"]["lines_count"] == 0:
        del crates["(other)"]

    # Sort by line coverage percentage ascending (lowest coverage first)
    def sort_key(item: tuple[str, dict[str, int]]) -> tuple[float, str]:
        label, stats = item
        pct = (
            (stats["lines_covered"] / stats["lines_count"] * 100)
            if stats["lines_count"] > 0
            else 0.0
        )
        return (pct, label)

    sorted_crates = sorted(crates.items(), key=sort_key)

    # Compute totals
    total_lines = sum(s["lines_count"] for _, s in sorted_crates)
    total_lines_covered = sum(s["lines_covered"] for _, s in sorted_crates)
    total_fns = sum(s["fn_count"] for _, s in sorted_crates)
    total_fns_covered = sum(s["fn_covered"] for _, s in sorted_crates)

    # Print table
    print("")
    print("## Per-Crate Coverage Summary")
    print("")
    print(
        f"| {'Crate':<30} | {'Lines':>7} | {'Covered':>7} | {'Line %':>7} "
        f"| {'Functions':>9} | {'Covered':>7} | {'Fn %':>7} |"
    )
    print(
        f"|{'-' * 32}|{'-' * 9}|{'-' * 9}|{'-' * 9}"
        f"|{'-' * 11}|{'-' * 9}|{'-' * 9}|"
    )

    for label, stats in sorted_crates:
        line_pct = (
            stats["lines_covered"] / stats["lines_count"] * 100
            if stats["lines_count"] > 0
            else 0.0
        )
        fn_pct = (
            stats["fn_covered"] / stats["fn_count"] * 100
            if stats["fn_count"] > 0
            else 0.0
        )
        print(
            f"| {label:<30} | {stats['lines_count']:>7} | {stats['lines_covered']:>7} "
            f"| {line_pct:>6.1f}% | {stats['fn_count']:>9} | {stats['fn_covered']:>7} "
            f"| {fn_pct:>6.1f}% |"
        )

    # Totals row
    total_line_pct = (
        total_lines_covered / total_lines * 100 if total_lines > 0 else 0.0
    )
    total_fn_pct = (
        total_fns_covered / total_fns * 100 if total_fns > 0 else 0.0
    )
    print(
        f"|{'-' * 32}|{'-' * 9}|{'-' * 9}|{'-' * 9}"
        f"|{'-' * 11}|{'-' * 9}|{'-' * 9}|"
    )
    print(
        f"| {'TOTAL':<30} | {total_lines:>7} | {total_lines_covered:>7} "
        f"| {total_line_pct:>6.1f}% | {total_fns:>9} | {total_fns_covered:>7} "
        f"| {total_fn_pct:>6.1f}% |"
    )
    print("")


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Parse cargo-llvm-cov JSON into per-crate coverage table"
    )
    parser.add_argument("json_file", help="Path to coverage.json")
    parser.add_argument(
        "--workspace-root",
        default=".",
        help="Path to workspace root (default: current directory)",
    )
    args = parser.parse_args()

    if not os.path.isfile(args.json_file):
        print(f"Error: {args.json_file} not found", file=sys.stderr)
        sys.exit(1)

    parse_coverage(args.json_file, args.workspace_root)


if __name__ == "__main__":
    main()
