#!/usr/bin/env python3
"""Builds eth_runner inputs (block.json, witness.json) for one block of a Besu chain.

Besu only serves debug_executionWitness from Osaka on, so the witness is assembled from
eth_getProof at the parent block over the set of touched accounts/slots:
  - accounts and slots touched by transactions (prestateTracer),
  - slots touched by the pre/post-block system calls (EIP-4788, 2935, 7002, 7251),
  - coinbase, the system address and precompiles (the STF warms them),
  - code of every touched contract,
  - raw ancestor headers (up to 256) for BLOCKHASH and parent-chain verification.

QBFT block hashes are keccak(header) with the commit seals removed from extraData, so both the
target and ancestor headers are given to the STF in that form; seals are consensus data and
are not needed for execution.

Known gap: a storage/account deletion that collapses a branch needs the sibling node,
which eth_getProof does not return.

Usage: make_witness.py <block_number> <out_dir> [rpc_url]
"""
import json
import os
import sys
import urllib.request

SYSTEM_ADDRESS = "0xfffffffffffffffffffffffffffffffffffffffe"
BEACON_ROOTS = "0x000f3df6d732807ef1319fb7b8bb8522d0beac02"
HISTORY_STORAGE = "0x0000f90827f1c53a10cb7a02335b175320002935"
WITHDRAWAL_REQUESTS = "0x00000961ef480eb55e80d19ad83579a64c007002"
CONSOLIDATION_REQUESTS = "0x0000bbddc7ce488642fb579f8b00f3a590007251"
PRECOMPILES = [f"0x{i:040x}" for i in list(range(0x01, 0x12)) + [0x100]]
BEACON_HISTORY_BUFFER = 8191
HISTORY_SERVE_WINDOW = 8191


def rpc(url, method, params):
    body = json.dumps({"jsonrpc": "2.0", "id": 1, "method": method, "params": params}).encode()
    req = urllib.request.Request(url, body, {"content-type": "application/json"})
    with urllib.request.urlopen(req) as resp:
        out = json.load(resp)
    if "error" in out:
        raise RuntimeError(f"{method}: {out['error']}")
    return out["result"]


def rlp_decode(data, pos=0):
    """Returns (item, next_pos); item is bytes or a list."""
    b = data[pos]
    if b < 0x80:
        return data[pos:pos + 1], pos + 1
    if b < 0xb8:
        n = b - 0x80
        return data[pos + 1:pos + 1 + n], pos + 1 + n
    if b < 0xc0:
        ll = b - 0xb7
        n = int.from_bytes(data[pos + 1:pos + 1 + ll], "big")
        start = pos + 1 + ll
        return data[start:start + n], start + n
    if b < 0xf8:
        n, start = b - 0xc0, pos + 1
    else:
        ll = b - 0xf7
        n = int.from_bytes(data[pos + 1:pos + 1 + ll], "big")
        start = pos + 1 + ll
    items, cur = [], start
    while cur < start + n:
        item, cur = rlp_decode(data, cur)
        items.append(item)
    return items, start + n


def rlp_encode(item):
    def length_prefix(n, short_base, long_base):
        if n < 56:
            return bytes([short_base + n])
        nb = n.to_bytes((n.bit_length() + 7) // 8, "big")
        return bytes([long_base + len(nb)]) + nb

    if isinstance(item, (bytes, bytearray)):
        if len(item) == 1 and item[0] < 0x80:
            return bytes(item)
        return length_prefix(len(item), 0x80, 0xb7) + bytes(item)
    payload = b"".join(rlp_encode(x) for x in item)
    return length_prefix(len(payload), 0xc0, 0xf7) + payload


def strip_qbft_seals(extra_data):
    """QBFT extraData = RLP([vanity, validators, vote, round, seals]); drop the seals."""
    fields, _ = rlp_decode(extra_data)
    fields[4] = []
    return rlp_encode(fields)


def hash_header(raw_header_hex):
    fields, _ = rlp_decode(bytes.fromhex(raw_header_hex[2:]))
    fields[12] = strip_qbft_seals(fields[12])
    return "0x" + rlp_encode(fields).hex()


def slot_hex(n):
    return f"0x{n:064x}"


def main():
    number = int(sys.argv[1])
    out_dir = sys.argv[2]
    url = sys.argv[3] if len(sys.argv) > 3 else "http://localhost:8545"
    parent = hex(number - 1)

    block = rpc(url, "eth_getBlockByNumber", [hex(number), True])
    timestamp = int(block["timestamp"], 16)

    touched = {}  # address -> set of slot hex strings

    def touch(addr, slots=()):
        touched.setdefault(addr.lower(), set()).update(s.lower() for s in slots)

    traces = rpc(url, "debug_traceBlockByNumber", [hex(number), {"tracer": "prestateTracer"}])
    for tx in traces:
        for addr, acc in tx["result"].items():
            touch(addr, (slot_hex(int(k, 16)) for k in acc.get("storage", {})))

    touch(block["miner"])
    touch(SYSTEM_ADDRESS)
    for p in PRECOMPILES:
        touch(p)
    ts_idx = timestamp % BEACON_HISTORY_BUFFER
    touch(BEACON_ROOTS, [slot_hex(ts_idx), slot_hex(ts_idx + BEACON_HISTORY_BUFFER)])
    touch(HISTORY_STORAGE, [slot_hex((number - 1) % HISTORY_SERVE_WINDOW)])
    for queue in (WITHDRAWAL_REQUESTS, CONSOLIDATION_REQUESTS):
        touch(queue, [slot_hex(i) for i in range(4)])

    state, codes, keys = set(), set(), set()
    for addr, slots in sorted(touched.items()):
        slots = sorted(slots)
        proof = rpc(url, "eth_getProof", [addr, slots, parent])
        state.update(n.lower() for n in proof["accountProof"])
        for sp in proof["storageProof"]:
            state.update(n.lower() for n in sp["proof"])
            keys.add(slot_hex(int(sp["key"], 16)))
        keys.add(addr)
        code = rpc(url, "eth_getCode", [addr, parent])
        if code not in ("0x", "0x0"):
            codes.add(code.lower())

    headers = []
    for n in range(max(0, number - 256), number):
        headers.append(hash_header(rpc(url, "debug_getRawHeader", [hex(n)])))
    block["extraData"] = "0x" + strip_qbft_seals(bytes.fromhex(block["extraData"][2:])).hex()

    os.makedirs(out_dir, exist_ok=True)
    with open(os.path.join(out_dir, "block.json"), "w") as f:
        json.dump({"jsonrpc": "2.0", "id": 1, "result": block}, f)
    witness = {
        "state": sorted(state),
        "codes": sorted(codes),
        "keys": sorted(keys),
        "headers": headers,
    }
    with open(os.path.join(out_dir, "witness.json"), "w") as f:
        json.dump({"jsonrpc": "2.0", "id": 1, "result": witness}, f)
    print(
        f"block {number}: {len(block['transactions'])} txs, {len(touched)} accounts, "
        f"{len(state)} trie nodes, {len(codes)} codes, {len(headers)} headers"
    )


if __name__ == "__main__":
    main()
