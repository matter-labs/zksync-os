#!/usr/bin/env python3
"""Layout of the transactions and receipts tries of a block.

Both tries are Merkle-Patricia tries keyed by `rlp(index)`, so their shape is a
pure function of the item count. This script

1. rebuilds both tries of each fixture under `glamsterdam_blocks/` from the raw
   transactions (`raw_block.json`) and raw receipts (`raw_receipts.json`),
   checks the roots against the header and prints the node layout per depth;
2. sweeps item counts `n = 1..N` and reports where extension nodes appear and
   how deep the trie gets, which is what a specialised encoder has to handle.

No dependencies: ships its own keccak-256, RLP and trie.

    python3 tests/instances/eth_runner/scripts/index_trie_layout.py [--sweep 2000] [DIR...]
"""
import argparse
import json
import os
import sys
from collections import Counter, defaultdict

# --- keccak-256 ----------------------------------------------------------------

_RC = [
    0x0000000000000001, 0x0000000000008082, 0x800000000000808A, 0x8000000080008000,
    0x000000000000808B, 0x0000000080000001, 0x8000000080008081, 0x8000000000008009,
    0x000000000000008A, 0x0000000000000088, 0x0000000080008009, 0x000000008000000A,
    0x000000008000808B, 0x800000000000008B, 0x8000000000008089, 0x8000000000008003,
    0x8000000000008002, 0x8000000000000080, 0x000000000000800A, 0x800000008000000A,
    0x8000000080008081, 0x8000000000008080, 0x0000000080000001, 0x8000000080008008,
]
_ROT = [[0, 36, 3, 41, 18], [1, 44, 10, 45, 2], [62, 6, 43, 15, 61], [28, 55, 25, 21, 56], [27, 20, 39, 8, 14]]
_MASK = (1 << 64) - 1


def _keccak_f(a):
    for rc in _RC:
        c = [a[x][0] ^ a[x][1] ^ a[x][2] ^ a[x][3] ^ a[x][4] for x in range(5)]
        d = [c[(x - 1) % 5] ^ (((c[(x + 1) % 5] << 1) | (c[(x + 1) % 5] >> 63)) & _MASK) for x in range(5)]
        a = [[a[x][y] ^ d[x] for y in range(5)] for x in range(5)]
        b = [[0] * 5 for _ in range(5)]
        for x in range(5):
            for y in range(5):
                r = _ROT[x][y]
                b[y][(2 * x + 3 * y) % 5] = ((a[x][y] << r) | (a[x][y] >> (64 - r))) & _MASK if r else a[x][y]
        a = [[b[x][y] ^ ((~b[(x + 1) % 5][y]) & b[(x + 2) % 5][y]) for y in range(5)] for x in range(5)]
        a[0][0] ^= rc
    return a


def keccak256(data: bytes) -> bytes:
    rate = 136
    padded = bytearray(data) + b"\x01" + b"\x00" * (rate - 1 - len(data) % rate)
    padded[-1] |= 0x80
    a = [[0] * 5 for _ in range(5)]
    for off in range(0, len(padded), rate):
        block = padded[off:off + rate]
        for i in range(rate // 8):
            a[i % 5][i // 5] ^= int.from_bytes(block[8 * i:8 * i + 8], "little")
        a = _keccak_f(a)
    out = b"".join(a[i % 5][i // 5].to_bytes(8, "little") for i in range(4))
    return out


# --- RLP -----------------------------------------------------------------------

def rlp_encode(item) -> bytes:
    if isinstance(item, (bytes, bytearray)):
        if len(item) == 1 and item[0] < 0x80:
            return bytes(item)
        return _rlp_len(len(item), 0x80) + bytes(item)
    if isinstance(item, int):
        return rlp_encode(b"" if item == 0 else item.to_bytes((item.bit_length() + 7) // 8, "big"))
    payload = b"".join(rlp_encode(x) for x in item)
    return _rlp_len(len(payload), 0xC0) + payload


def _rlp_len(length: int, offset: int) -> bytes:
    if length < 56:
        return bytes([offset + length])
    size = length.to_bytes((length.bit_length() + 7) // 8, "big")
    return bytes([offset + 55 + len(size)]) + size


def rlp_decode(data: bytes):
    item, rest = _rlp_decode(memoryview(data))
    assert not rest, "trailing bytes after RLP item"
    return item


def _rlp_decode(data):
    first = data[0]
    if first < 0x80:
        return bytes(data[:1]), data[1:]
    if first < 0xB8:
        n = first - 0x80
        return bytes(data[1:1 + n]), data[1 + n:]
    if first < 0xC0:
        ln = first - 0xB7
        n = int.from_bytes(data[1:1 + ln], "big")
        return bytes(data[1 + ln:1 + ln + n]), data[1 + ln + n:]
    if first < 0xF8:
        n = first - 0xC0
        header = 1
    else:
        ln = first - 0xF7
        n = int.from_bytes(data[1:1 + ln], "big")
        header = 1 + ln
    payload = data[header:header + n]
    items = []
    while payload:
        item, payload = _rlp_decode(payload)
        items.append(item)
    return items, data[header + n:]


# --- Merkle-Patricia trie ------------------------------------------------------

def nibbles(key: bytes):
    out = []
    for b in key:
        out.append(b >> 4)
        out.append(b & 0xF)
    return out


def hex_prefix(path, leaf: bool) -> bytes:
    flag = 2 if leaf else 0
    if len(path) % 2:
        head = [(flag + 1) << 4 | path[0]]
        body = path[1:]
    else:
        head = [flag << 4]
        body = path
    return bytes(head + [body[i] << 4 | body[i + 1] for i in range(0, len(body), 2)])


class Trie:
    """Builds the trie for `items[i]` under key `rlp(i)`, recording its layout."""

    def __init__(self, items, hash_nodes=True):
        self.nodes = []  # (depth, kind, path_len, encoded_len, embedded)
        # The shape does not depend on the hashes, only on the encoded sizes
        # (a child reference is 32 bytes either way), so the sweep skips keccak.
        self.hash = keccak256 if hash_nodes else (lambda data: b"\x00" * 32)
        keyed = sorted(((nibbles(rlp_encode(i)), v) for i, v in enumerate(items)), key=lambda kv: kv[0])
        self.root = self._build(keyed, 0) if keyed else keccak256(rlp_encode(b""))

    def _node(self, depth, kind, path_len, encoded, must_hash=False):
        embedded = len(encoded) < 32 and not must_hash
        self.nodes.append((depth, kind, path_len, len(encoded), embedded))
        return encoded if embedded else self.hash(encoded)

    def _build(self, keyed, depth, root=False):
        # keyed: sorted list of (remaining nibble path, value), all distinct
        if len(keyed) == 1:
            path, value = keyed[0]
            return self._node(depth, "leaf", len(path), rlp_encode([hex_prefix(path, True), value]), root)
        prefix = 0
        first = keyed[0][0]
        while all(len(p) > prefix and p[prefix] == first[prefix] for p, _ in keyed):
            prefix += 1
        if prefix:
            stripped = [(p[prefix:], v) for p, v in keyed]
            child = self._build(stripped, depth + 1)
            return self._node(depth, "extension", prefix, rlp_encode([hex_prefix(first[:prefix], False), child]), root)
        children = [b""] * 17
        groups = defaultdict(list)
        for p, v in keyed:
            if not p:
                children[16] = v  # key ends at this branch: value slot
            else:
                groups[p[0]].append((p[1:], v))
        for nib, group in groups.items():
            children[nib] = self._build(group, depth + 1)
        return self._node(depth, "branch", 0, rlp_encode(children), root)

    def layout(self):
        by_depth = defaultdict(Counter)
        for depth, kind, path_len, _, embedded in self.nodes:
            by_depth[depth][f"{kind}{'/' + str(path_len) if kind != 'branch' else ''}{' (embedded)' if embedded else ''}"] += 1
        return {d: dict(c) for d, c in sorted(by_depth.items())}

    def kinds(self):
        return Counter(kind for _, kind, _, _, _ in self.nodes)


# --- fixtures ------------------------------------------------------------------

def load_result(path):
    with open(path) as f:
        return json.load(f)["result"]


def unhex(s: str) -> bytes:
    return bytes.fromhex(s[2:] if s.startswith("0x") else s)


def check_fixture(fixture_dir):
    header = load_result(os.path.join(fixture_dir, "block.json"))
    raw_block = rlp_decode(unhex(load_result(os.path.join(fixture_dir, "raw_block.json"))))
    raw_receipts = [unhex(r) for r in load_result(os.path.join(fixture_dir, "raw_receipts.json"))]
    # Body: [header, transactions, ommers, withdrawals, (block access list, ...)]
    transactions = [tx if isinstance(tx, bytes) else rlp_encode(tx) for tx in raw_block[1]]
    print(f"\n== block {int(header['number'], 16)}: {len(transactions)} transactions, "
          f"{len(raw_receipts)} receipts, raw block has {len(raw_block)} body fields")
    for name, items, expected in (
        ("transactions", transactions, header["transactionsRoot"]),
        ("receipts", raw_receipts, header["receiptsRoot"]),
    ):
        trie = Trie(items)
        ok = trie.root == unhex(expected)
        print(f"  {name} root {'OK' if ok else 'MISMATCH'}: {trie.kinds()}")
        for depth, counts in trie.layout().items():
            print(f"    depth {depth}: {counts}")
        if not ok:
            return False
    return True


def sweep(limit):
    print(f"\n== structure sweep for n = 1..{limit} items (keys rlp(0..n-1))")
    with_extension = []
    max_depth = Counter()
    for n in range(1, limit + 1):
        trie = Trie([b"\x00" * 32] * n, hash_nodes=False)  # 32-byte values never embed
        kinds = trie.kinds()
        if kinds["extension"]:
            with_extension.append(n)
        max_depth[max(d for d, *_ in trie.nodes) + 1] += 1
    ranges = []
    for n in with_extension:
        if ranges and ranges[-1][1] == n - 1:
            ranges[-1][1] = n
        else:
            ranges.append([n, n])
    print("  item counts whose trie has an extension node:", ", ".join(f"{a}..{b}" if a != b else str(a) for a, b in ranges) or "none")
    print("  trie depth (levels, root = 1) -> how many item counts:", dict(sorted(max_depth.items())))


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("dirs", nargs="*", help="fixture directories (default: every one under glamsterdam_blocks/)")
    parser.add_argument("--sweep", type=int, default=2000, help="sweep item counts up to this (0 to skip)")
    args = parser.parse_args()
    dirs = args.dirs
    if not dirs:
        base = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "glamsterdam_blocks")
        dirs = sorted(os.path.join(base, d) for d in os.listdir(base) if os.path.isdir(os.path.join(base, d)))
    ok = all([check_fixture(d) for d in dirs])
    if args.sweep:
        sweep(args.sweep)
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
