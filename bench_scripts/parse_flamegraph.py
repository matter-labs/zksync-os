"""Parse a flamegraph SVG and produce a text summary for AI agent consumption.

Usage:
    python3 bench_scripts/parse_flamegraph.py <input.svg> [output.txt]

If output.txt is omitted, prints to stdout.

Output format:
    Two sections:
    1. Top functions by self-cost (where cycles actually burn)
    2. Collapsed call stacks leading to those functions
"""

import re
import sys


def parse_flamegraph_svg(svg_text):
    """Extract frames from flamegraph SVG: (name, samples, pct, x, y, width)."""
    pattern = (
        r'<g><title>([^<]+)</title>'
        r'<rect x="([^"]+)%" y="(\d+)" width="([^"]+)%"'
    )
    frames = []
    for m in re.finditer(pattern, svg_text):
        title = m.group(1)
        x = float(m.group(2))
        y = int(m.group(3))
        w = float(m.group(4))

        tm = re.match(r"(.+) \((\S+) samples, (\S+)%\)", title)
        if tm:
            name = tm.group(1)
            samples = int(tm.group(2).replace(",", ""))
            pct = float(tm.group(3))
            frames.append((name, samples, pct, x, y, w))

    return frames


def shorten_name(name):
    """Shorten Rust fully-qualified names for readability.

    Strategy:
    1. Decode HTML entities
    2. Parse the top-level structure (impl blocks, free functions)
    3. Extract meaningful type + method names
    """
    # Decode HTML entities
    s = name.replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&")

    # Try to parse as <Type as Trait>::method... using bracket-depth matching
    impl_result = _parse_impl_block(s)
    if impl_result:
        impl_type_raw, trait_raw, method_raw = impl_result
        impl_type = _last_type_name(impl_type_raw)
        method_part = _simplify_method(method_raw)
        return f"{impl_type}::{method_part}"

    # Free function: path::to::module::function::<GenericParams>::{closure#N}
    # Strip all generic param blocks ::<...> respecting nesting
    stripped = _strip_generic_params(s)
    # Handle closure suffixes
    closure = ""
    cm = re.search(r"(::(\{closure#\d+\}))+$", stripped)
    if cm:
        closure = cm.group(0)
        stripped = stripped[: cm.start()]

    parts = stripped.split("::")
    if len(parts) >= 2:
        return f"{parts[-2]}::{parts[-1]}{closure}"
    return f"{stripped}{closure}"


def _parse_impl_block(s):
    """Parse '<Type as Trait>::method' handling nested generics.

    Returns (impl_type, trait, method) or None if not an impl block.
    """
    if not s.startswith("<"):
        return None

    # Find the matching '>' for the outer '<', tracking ' as ' at depth 1
    depth = 0
    as_pos = None
    close_pos = None
    i = 0
    while i < len(s):
        ch = s[i]
        if ch == "<":
            depth += 1
        elif ch == ">":
            depth -= 1
            if depth == 0:
                close_pos = i
                break
        elif depth == 1 and s[i : i + 4] == " as ":
            as_pos = i
        i += 1

    if close_pos is None:
        return None

    # Must be followed by ::method
    rest = s[close_pos + 1 :]
    if not rest.startswith("::"):
        return None
    method = rest[2:]

    if as_pos is not None:
        impl_type = s[1:as_pos]
        trait = s[as_pos + 4 : close_pos]
    else:
        # <Type>::method (inherent impl)
        impl_type = s[1:close_pos]
        trait = None

    return impl_type, trait, method


def _strip_generic_params(s):
    """Remove all ::<...> generic parameter blocks, respecting nesting."""
    result = []
    i = 0
    while i < len(s):
        # Check for ::< pattern
        if s[i : i + 3] == "::<":
            # Skip to matching >
            depth = 0
            j = i + 2
            while j < len(s):
                if s[j] == "<":
                    depth += 1
                elif s[j] == ">":
                    depth -= 1
                    if depth == 0:
                        break
                j += 1
            i = j + 1
        else:
            result.append(s[i])
            i += 1
    return "".join(result)


def _last_type_name(qualified):
    """Extract the short type name from a fully-qualified Rust type.

    E.g. 'alloc::collections::btree::map::BTreeMap<K, V>' -> 'BTreeMap'
    """
    # Strip generic params
    depth = 0
    result = []
    for ch in qualified:
        if ch == "<":
            depth += 1
        elif ch == ">":
            depth -= 1
        elif depth == 0:
            result.append(ch)
    clean = "".join(result).rstrip(":")
    parts = clean.split("::")
    # Return last non-empty segment
    for p in reversed(parts):
        if p:
            return p
    return clean


def _simplify_method(method_str):
    """Simplify the method part after '>::'.

    Handles: method_name::<long::Generic::Params>::{closure#0}
    Returns: method_name::{closure#0}
    """
    # Extract closure suffix if present
    closure = ""
    cm = re.search(r"(::(\{closure#\d+\}))+$", method_str)
    if cm:
        closure = cm.group(0)
        method_str = method_str[: cm.start()]

    # Strip generic params ::<...>
    method_str = re.sub(r"::<.*$", "", method_str)

    # If still has path segments, take last 2
    parts = method_str.split("::")
    if len(parts) > 2:
        method_str = "::".join(parts[-2:])

    return f"{method_str}{closure}"


def _is_truncated_name(raw_name):
    """Detect names truncated by the flamegraph generator.

    These are fragments like ' 32]>' that come from array types like [u8; 32]
    whose prefix was cut off.
    """
    decoded = raw_name.replace("&lt;", "<").replace("&gt;", ">")
    stripped = decoded.strip()
    # Truncated if it starts with a number, bracket, or space (no valid function does)
    if not stripped or stripped[0] in "0123456789] )>,":
        return True
    return False


def build_tree(frames):
    """Build a tree from flamegraph frames using spatial containment.

    Parent-child relationship: a child's [x, x+w] range is contained within
    its parent's range. In inferno-produced flamegraphs, the root ("all") has
    the highest y value, and children have lower y values (stack grows upward
    visually).
    """
    if not frames:
        return {}, {}, {}, {}

    # Sort by y descending (root/shallowest first), then by x
    frames.sort(key=lambda f: (-f[4], f[3]))

    # Find the y-step between levels
    ys = sorted(set(f[4] for f in frames), reverse=True)
    if len(ys) < 2:
        frame_lookup = {(f[4], f[3]): f for f in frames}
        self_costs = {k: f[1] for k, f in frame_lookup.items()}
        return frame_lookup, {}, {}, self_costs
    y_step = ys[0] - ys[1]  # positive step downward from parent to child

    # Build lookup: y -> list of frames sorted by x
    by_y = {}
    for f in frames:
        by_y.setdefault(f[4], []).append(f)
    for y in by_y:
        by_y[y].sort(key=lambda f: f[3])

    # Frame key: (y, x) is unique
    children_map = {}  # (y, x) -> [(y2, x2), ...]
    parent_map = {}  # (y, x) -> (y_parent, x_parent)

    for y_level in ys:
        child_y = y_level - y_step  # children have lower y
        if child_y not in by_y:
            continue
        parents = by_y[y_level]
        children = by_y[child_y]

        ci = 0
        for p in parents:
            p_name, p_samples, p_pct, p_x, p_y, p_w = p
            p_right = p_x + p_w
            p_key = (p_y, p_x)
            if p_key not in children_map:
                children_map[p_key] = []

            while ci < len(children):
                c = children[ci]
                c_x = c[3]
                c_w = c[5]
                c_right = c_x + c_w
                # Child starts after parent ends
                if c_x >= p_right - 0.001:
                    break
                # Child is within parent
                if c_x >= p_x - 0.001 and c_right <= p_right + 0.001:
                    c_key = (c[4], c[3])
                    children_map[p_key].append(c_key)
                    parent_map[c_key] = p_key
                    ci += 1
                else:
                    ci += 1

    # Compute self-cost for each frame
    frame_lookup = {(f[4], f[3]): f for f in frames}
    self_costs = {}
    for key, f in frame_lookup.items():
        child_samples = sum(
            frame_lookup[ck][1] for ck in children_map.get(key, [])
        )
        self_costs[key] = f[1] - child_samples

    return frame_lookup, children_map, parent_map, self_costs


def get_path(key, parent_map, frame_lookup):
    """Reconstruct the call stack path for a frame."""
    path = []
    k = key
    while k is not None:
        f = frame_lookup[k]
        path.append(f[0])
        k = parent_map.get(k)
    path.reverse()
    return path


def collapse_path(short_path):
    """Collapse consecutive repeated frames and repeated sequences.

    First collapses single repeated frames ('A A A' -> 'A (x3)'),
    then collapses repeated sequences ('A > B > A > B' -> '[A > B] (x2)').
    """
    if not short_path:
        return []

    # Step 1: collapse single repeated frames
    collapsed = []
    prev = short_path[0]
    count = 1
    for name in short_path[1:]:
        if name == prev:
            count += 1
        else:
            if count > 1:
                collapsed.append(f"{prev} (x{count})")
            else:
                collapsed.append(prev)
            prev = name
            count = 1
    if count > 1:
        collapsed.append(f"{prev} (x{count})")
    else:
        collapsed.append(prev)

    # Step 2: collapse repeated sequences (length 2-4)
    for seq_len in range(2, 5):
        result = []
        i = 0
        while i < len(collapsed):
            seq = collapsed[i : i + seq_len]
            if len(seq) < seq_len:
                result.extend(collapsed[i:])
                break
            # Count how many times this sequence repeats
            reps = 1
            while collapsed[i + reps * seq_len : i + (reps + 1) * seq_len] == seq:
                reps += 1
            if reps > 1:
                result.append(f"[{' > '.join(seq)}] (x{reps})")
                i += reps * seq_len
            else:
                result.append(collapsed[i])
                i += 1
        collapsed = result

    return collapsed


def format_report(frames, top_n=30):
    """Produce the text report."""
    if not frames:
        return "No frames found in flamegraph SVG.\n"

    frame_lookup, children_map, parent_map, self_costs = build_tree(frames)

    # Find total samples (root frame)
    total_samples = max(f[1] for f in frame_lookup.values())
    if total_samples == 0:
        return "No samples found.\n"

    lines = []
    lines.append(f"Total samples: {total_samples:,}\n")

    # Top functions by self-cost (where cycles are actually spent)
    lines.append("=" * 90)
    lines.append("TOP FUNCTIONS BY SELF COST (where cycles are actually spent)")
    lines.append("=" * 90)
    lines.append(
        f"{'SELF%':>7}  {'SELF':>10}  {'TOTAL%':>7}  {'TOTAL':>10}  FUNCTION"
    )
    lines.append("-" * 90)

    by_self = sorted(self_costs.items(), key=lambda x: x[1], reverse=True)
    # Filter out truncated/broken names from flamegraph generator
    by_self = [
        (k, s) for k, s in by_self
        if not _is_truncated_name(frame_lookup[k][0])
    ]

    # Aggregate by shortened function name for the summary table
    aggregated = {}  # short_name -> (total_self, total_inclusive)
    for key, self_samples in by_self:
        if self_samples <= 0:
            continue
        f = frame_lookup[key]
        name = shorten_name(f[0])
        prev_self, prev_total = aggregated.get(name, (0, 0))
        aggregated[name] = (prev_self + self_samples, prev_total + f[1])

    agg_sorted = sorted(aggregated.items(), key=lambda x: x[1][0], reverse=True)
    for name, (agg_self, agg_total) in agg_sorted[:top_n]:
        self_pct = agg_self / total_samples * 100
        total_pct = agg_total / total_samples * 100
        lines.append(
            f"{self_pct:6.2f}%  {agg_self:10,}  {total_pct:6.2f}%  {agg_total:10,}  {name}"
        )

    # Build call stacks, shortened and collapsed
    stack_entries = []
    for key, self_samples in by_self[:top_n * 2]:
        if self_samples <= 0:
            continue
        path = get_path(key, parent_map, frame_lookup)
        short_path = [shorten_name(p) for p in path]
        # Skip the generic root frames
        if short_path and short_path[0] == "all":
            short_path = short_path[1:]
        short_path = collapse_path(short_path)
        stack_entries.append((self_samples, short_path))

    # Find common prefix across all stacks and trim it
    if stack_entries:
        common = stack_entries[0][1][:]
        for _, path in stack_entries[1:]:
            new_common = []
            for a, b in zip(common, path):
                if a == b:
                    new_common.append(a)
                else:
                    break
            common = new_common
        # Keep at least the last element of common prefix for context
        trim_len = max(0, len(common) - 1)
    else:
        trim_len = 0

    lines.append("")
    lines.append("=" * 90)
    lines.append("TOP CALL STACKS BY SELF COST (call path leading to hot functions)")
    lines.append("=" * 90)
    if trim_len > 0:
        trimmed = stack_entries[0][1][:trim_len]
        lines.append(f"(common prefix: {' > '.join(trimmed)} > ...)")

    for self_samples, short_path in stack_entries[:top_n]:
        self_pct = self_samples / total_samples * 100
        trimmed_path = short_path[trim_len:]
        lines.append(f"\n  {self_pct:.2f}% ({self_samples:,} samples):")
        lines.append(f"    {' > '.join(trimmed_path)}")

    lines.append("")
    return "\n".join(lines)


def main():
    if len(sys.argv) < 2:
        print(
            "Usage: python3 parse_flamegraph.py <input.svg> [output.txt]",
            file=sys.stderr,
        )
        sys.exit(1)

    svg_path = sys.argv[1]
    output_path = sys.argv[2] if len(sys.argv) >= 3 else None

    with open(svg_path) as f:
        svg_text = f.read()

    frames = parse_flamegraph_svg(svg_text)
    report = format_report(frames)

    if output_path:
        with open(output_path, "w") as f:
            f.write(report)
        print(f"Report written to {output_path}", file=sys.stderr)
    else:
        print(report)


if __name__ == "__main__":
    main()
