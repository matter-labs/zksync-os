#!/usr/bin/env python3
"""One-time fixture-prep step for eth_runner block dirs.

geth's `prestateTracer` omits the code of an EIP-7702 delegate *target* when an
account is delegated and called within the same block — the trace only records
the delegation indicator (`0xef0100 || target`) on the delegated account, never
the target's code. The replay then calls the delegated account, finds no code
at the target, and executes nothing — diverging on gas/state.

This script scans a block dir's `difftrace.json` for delegation indicators,
and for any target whose code is missing from `prestatetrace.json` it fetches
the code/balance/nonce as of the parent block (via `eth_getCode` etc.) and
merges the account into the prestate so the replay sees the true block-initial
state. It writes the augmented `prestatetrace.json` in place; the result is a
self-contained committed fixture (the bench reads only local files — no RPC at
run time).

Usage: bench_scripts/inject_delegate_code.py <block-dir> [<block-dir> ...]
"""
import json
import sys
import urllib.request

RPC = "https://eth-mainnet.g.alchemy.com/public"


def rpc(method, params):
    body = {"jsonrpc": "2.0", "id": 1, "method": method, "params": params}
    req = urllib.request.Request(
        RPC, data=json.dumps(body).encode(), headers={"content-type": "application/json"}
    )
    return json.load(urllib.request.urlopen(req, timeout=60))["result"]


def get_code(addr, parent):
    # Delegate-target code is stable; prefer the parent block, fall back to latest.
    for blk in (parent, "latest"):
        code = rpc("eth_getCode", [addr, blk])
        if code not in ("0x", "", None):
            return code, blk
    return None, None


def augment(block_dir):
    block = json.load(open(f"{block_dir}/block.json"))["result"]
    parent = hex(int(block["number"], 16) - 1)
    ps = json.load(open(f"{block_dir}/prestatetrace.json"))
    diff = json.load(open(f"{block_dir}/difftrace.json"))["result"]

    have_code = {
        a.lower()
        for item in ps["result"]
        for a, st in item["result"].items()
        if st.get("code")
    }
    # Delegation indicator: code == 0xef0100 || <20-byte target>.
    # Record the first tx index that delegates to each target.
    targets = {}
    for i, item in enumerate(diff):
        for _addr, st in item["result"].get("post", {}).items():
            c = (st.get("code") or "").lower()
            if c.startswith("0xef0100") and len(c) >= 48:
                targets.setdefault("0x" + c[8:48], i)

    needed = {t: i for t, i in targets.items() if t not in have_code}
    if not needed:
        print(f"{block_dir}: no missing delegate-target code")
        return

    changed = False
    for target, tx_index in needed.items():
        code, blk = get_code(target, parent)
        if code is None:
            print(f"  {target}: no code on-chain (skip)")
            continue
        entry = {"balance": rpc("eth_getBalance", [target, parent]), "code": code}
        nonce = int(rpc("eth_getTransactionCount", [target, parent]), 16)
        if nonce:
            entry["nonce"] = nonce
        ps["result"][tx_index]["result"][target] = entry
        changed = True
        print(f"  injected {target} codeLen={len(code) // 2 - 1} (@{blk}) into tx {tx_index}")

    if changed:
        json.dump(ps, open(f"{block_dir}/prestatetrace.json", "w"))
        print(f"{block_dir}: augmented")


if __name__ == "__main__":
    for d in sys.argv[1:]:
        augment(d)
