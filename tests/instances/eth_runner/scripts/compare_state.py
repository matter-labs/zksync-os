#!/usr/bin/env python3
"""Compare the STF's final state of a block with the chain, using only
`eth_getProof` (works on archive nodes without the debug namespace).

Inputs: the json written by `eth_runner eth-run` with `ETH_RUN_STATE_DUMP` set,
the block's receipts (`eth_getBlockReceipts`) and the block's witness.json. Per
transaction the gas and status are compared with the receipts. For every address
the witness lists (plus the ones the STF changed) the account is fetched at N-1
and N, and the slots the STF wrote are fetched at N through the same proof call.
Reports: our account/slot values that differ from the chain at N, chain changes
we did not report. Accounts that did not exist at N-1 show zero hashes from the
node; a report of only their storage root changing is noise.
"""
import json, os, sys, time, urllib.request

if len(sys.argv) != 5 or "ETH_RPC_URL" not in os.environ:
    sys.exit("usage: ETH_RPC_URL=<archive node> compare_state.py <dump.json> <receipts.json> <witness.json> <block number>\n"
             "dump.json: ETH_RUN_STATE_DUMP output of `eth_runner eth-run`; receipts.json: eth_getBlockReceipts response")
URL = os.environ["ETH_RPC_URL"]
dump_path, receipts_path, witness_path, block_number = sys.argv[1:5]
block_number = int(block_number)
S = dump_path.rsplit("/", 1)[0]

dump = json.load(open(dump_path))
receipts = json.load(open(receipts_path))["result"]
witness = json.load(open(witness_path))["result"]

EMPTY_CODE = "0xc5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"
EMPTY_ROOT = "0x56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421"

# ---- 1. per-tx gas / status -------------------------------------------------
prev = 0
first_bad = None
for i, (r, ours) in enumerate(zip(receipts, dump["txs"])):
    cum = int(r["cumulativeGasUsed"], 16)
    gas = cum - prev
    prev = cum
    status = r["status"] == "0x1"
    if "error" in ours or ours["gas_used"] != gas or ours["status"] != status:
        print(f"tx {i} {r['transactionHash']}: chain gas={gas} status={status}, ours={ours}")
        if first_bad is None:
            first_bad = i
print(f"tx count: chain {len(receipts)}, ours {len(dump['txs'])}; first gas/status mismatch: {first_bad}")

# ---- 2. rpc helpers ---------------------------------------------------------
def rpc_batch(calls):
    body = json.dumps([{"jsonrpc": "2.0", "id": i, "method": m, "params": p} for i, (m, p) in enumerate(calls)]).encode()
    for attempt in range(6):
        try:
            req = urllib.request.Request(URL, data=body, headers={"content-type": "application/json"})
            with urllib.request.urlopen(req, timeout=120) as resp:
                out = json.load(resp)
            if isinstance(out, dict):
                raise RuntimeError(out)
            res = {}
            for o in out:
                if "error" in o:
                    raise RuntimeError(o["error"])
                res[o["id"]] = o["result"]
            return [res[i] for i in range(len(calls))]
        except Exception as e:  # rate limit etc
            print("rpc retry", attempt, str(e)[:120], file=sys.stderr)
            time.sleep(2 * (attempt + 1))
    raise RuntimeError("rpc failed")

def get_proofs(addr_keys, block):
    """addr_keys: dict address -> list of slot keys. Returns dict address -> proof result."""
    items = list(addr_keys.items())
    out = {}
    B = 10
    for i in range(0, len(items), B):
        chunk = items[i:i + B]
        calls = [("eth_getProof", [a, keys, hex(block)]) for a, keys in chunk]
        for (a, _), r in zip(chunk, rpc_batch(calls)):
            out[a] = r
        time.sleep(0.2)
    return out

# ---- 3. address set ---------------------------------------------------------
witness_addrs = set()
for k in witness["keys"]:
    h = k[2:] if k.startswith("0x") else k
    if len(h) == 40:
        witness_addrs.add("0x" + h.lower())
our_accounts = {a["address"].lower(): a for a in dump["accounts"]}
our_slots = {}
for s in dump["slots"]:
    our_slots.setdefault(s["address"].lower(), {})[s["key"].lower()] = s["value"].lower()
addrs = witness_addrs | set(our_accounts) | set(our_slots)
print(f"witness addresses: {len(witness_addrs)}, our account diffs: {len(our_accounts)}, "
      f"our slot writes: {len(dump['slots'])} over {len(our_slots)} accounts, total addresses to fetch: {len(addrs)}")

cache_file = f"{S}/proofs_{block_number}.json"
try:
    cached = json.load(open(cache_file))
    pre, post = cached["pre"], cached["post"]
    print("using cached proofs")
except Exception:
    pre = get_proofs({a: [] for a in addrs}, block_number - 1)
    post = get_proofs({a: list(our_slots.get(a, {}).keys()) for a in addrs}, block_number)
    json.dump({"pre": pre, "post": post}, open(cache_file, "w"))

def acct(p):
    return (int(p["nonce"], 16), int(p["balance"], 16), p["codeHash"].lower(), p["storageHash"].lower())

def norm32(v):
    return "0x" + int(v, 16).to_bytes(32, "big").hex()

# ---- 4. compare accounts ----------------------------------------------------
problems = 0
for a in sorted(addrs):
    p0, p1 = acct(pre[a]), acct(post[a])
    chain_changed = p0 != p1
    ours = our_accounts.get(a)
    if ours is not None:
        o = (ours["nonce"], int(ours["balance"], 16), ours["code_hash"].lower())
        # our zero code hash means "account does not exist"
        chain = (p1[0], p1[1], p1[2])
        if o[2] == norm32("0x0"):
            o = (o[0], o[1], EMPTY_CODE)
        if o != chain:
            problems += 1
            print(f"ACCOUNT MISMATCH {a}: ours nonce={o[0]} bal={o[1]} code={o[2]} | chain N: nonce={p1[0]} bal={p1[1]} code={p1[2]} | chain N-1: nonce={p0[0]} bal={p0[1]} code={p0[2]}")
    elif chain_changed and p0[:3] != p1[:3]:
        problems += 1
        print(f"MISSING ACCOUNT CHANGE {a}: chain N-1 {p0} -> N {p1}, not in our account diffs")
    # storage
    ours_slots = our_slots.get(a, {})
    if ours_slots:
        for sp in post[a]["storageProof"]:
            k = norm32(sp["key"])
            v = norm32(sp["value"])
            ov = ours_slots.get(k)
            if ov != v:
                problems += 1
                print(f"SLOT MISMATCH {a} key {k}: ours {ov} chain {v}")
    elif p0[3] != p1[3] and {p0[3], p1[3]} - {norm32("0x0"), EMPTY_ROOT}:
        problems += 1
        print(f"MISSING STORAGE CHANGE {a}: storage root {p0[3]} -> {p1[3]} on chain, no writes from us")
print(f"problems: {problems}")
