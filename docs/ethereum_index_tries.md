# Layout of the transactions and receipts tries

The `transactionsRoot` and `receiptsRoot` of an Ethereum block header are roots of
Merkle-Patricia tries keyed by `rlp(index)`, with the item's EIP-2718 encoding as the
value (typed transactions/receipts: `type || rlp(payload)`; legacy ones: the RLP list).
Because the keys are consecutive small integers, the shape of both tries is a pure
function of the item count `n`, and the two tries of a block always have the same
shape. This allows a specialised encoder/hasher instead of generic trie code.

Verified on the Sepolia Glamsterdam fixtures under
`tests/instances/eth_runner/glamsterdam_blocks/` with
`tests/instances/eth_runner/scripts/index_trie_layout.py`, which rebuilds both tries
node by node and checks the roots against the headers.

## Keys

| index `i`      | `rlp(i)`           | nibble path (before the terminator) |
|----------------|--------------------|-------------------------------------|
| 0              | `80`               | `8 0`                               |
| 1 .. 127       | `i` (one byte)     | `hi(i) lo(i)`                       |
| 128 .. 255     | `81 i`             | `8 1 hi(i) lo(i)`                   |
| 256 .. 65535   | `82 i_hi i_lo`     | `8 2 ...` (six nibbles)             |

So the root branch has children `0..7` for indices 1..127 (grouped by their high
nibble) and child `8` for index 0 and everything from 128 on. Note that index 0 sits
under the same root child as the large indices, not next to index 1.

## Node layout by item count

Every leaf is a 2-item RLP list `[hex_prefix(path, leaf), rlp(value)]`. Almost all leaves
have an empty path (`0x20` prefix): their key is fully consumed by the branches above.
A leaf with a non-empty path only appears where a subtree holds a single item: index 0
while `n <= 128` (path `0`), the last partially filled group, or a lone `81..`/`82..`
key. Nothing is ever embedded: transactions and receipts are longer than 32 bytes, and
branch nodes hold 32-byte child hashes, so every node is referenced by its keccak hash.

| `n`          | levels | nodes besides leaves                                                             |
|--------------|--------|----------------------------------------------------------------------------------|
| 1            | 1      | a single leaf with the full path                                                  |
| 2            | 2      | root branch, two leaves with 1-nibble paths                                       |
| 3 .. 129     | 3      | root branch + up to 8 branches for the `0x`..`7x` groups; index 0 is a leaf under child 8 (`n <= 128`) or the group has become a branch with the 0x80 leaf (`n = 129`) |
| 130 .. 144   | 5      | one **extension** node (path `8`) under `8 1`: indices 128..143 share the nibble `8` |
| 145 .. 257   | 5      | no extension, `8 1` is a branch over the third nibble                             |
| 258 .. 272   | 5      | extension node (path `0 1 0`) under `8 2`                                         |
| 273 .. 512   | 6      | extension node (path `0 1`) under `8 2`                                           |
| 513 .. 4096  | 7      | extension node (path `0`) under `8 2`                                             |
| 4097 ..      | 7      | no extension until the next power-of-16 boundary                                  |

So the claim "only branch and leaf nodes" holds for `n <= 129` and `145 <= n <= 257`
only. Extension nodes do appear for other counts, but always at most one, always under
the `8 1` or `8 2` child, and with a path determined by `n` alone. Mainnet and Sepolia
blocks today have 100..300 transactions, so the 130..144 and 258..272 cases are hit
regularly (two of the ten Sepolia fixtures have 130 transactions).

## Consequences for an encoder

- The hasher needs the item count and the keccak hash of every leaf node only; the
  branch/extension skeleton is fixed by `n` and can be generated instead of built by
  inserting keys. No key comparison, no nibble arithmetic beyond `rlp(i)`.
- Leaf hashing dominates: one keccak over `rlp([0x20, rlp(value)])` per item, i.e. the
  value with a 3..6 byte prefix. Branch nodes are at most 17 * 33 + 3 bytes.
- For `n <= 127` the structure is: root branch, one branch per non-empty high nibble
  group, and the index-0 leaf with path `0`; this covers the small-block case with no
  extension handling at all.

## Proving the item count

The Glamsterdam programs (`basic_bootloader::...::ethereum_glamsterdam`) need the number
of transactions `n` of a block and must reject a header whose tries are not the tries
of exactly `n` items: a leaf at an index past `n`, a gap, or a leaf at a key that is not
`rlp(i)` of any `i` (`0x00`, a non-canonical `0x8105`, a path of the wrong length) all
make a header no body can match. No program reads the whole trie: a leaf is a whole
transaction or receipt, and the ranges of a block read only their own.

The proof is split (see the module documentation of `tx_range::index_trie` for the
diagram and the argument):

- Every lookup of `rlp(i)` applies the *density rule* at every branch it passes, for the
  count it runs with: at a branch whose path is `p`, only the slots `c` such that some
  `rlp(j)`, `j < n`, starts with `p ++ c` may hold a child (`index_trie::expected_children`
  computes them from `p` and `n` alone), and the value slot must be empty. A branch that
  breaks the rule fails the lookup instead of being walked past.
- The pre-execution check witnesses `n`, looks up the leaf `n - 1` in both tries under
  the rule and publishes `n` (`tx_count` of the linkage output). That pins the leaf at
  `n - 1` and the absence of keys at or past `n` on its path only: keys that share no
  branch with that path (for `n = 128`, everything under `0x81..`, which hangs off the
  root's child `8` while `rlp(127) = 0x7f` sits under child `7`) are not seen.
- The ranges tile `0..n` and look up every transaction and receipt leaf, each under the
  rule and with the count the aggregation binds to the pre-execution check's. A leaf
  outside `{rlp(i) : i < n}` would hang off a slot, or sit in a value slot, of a branch
  that one of these lookups passes (take the deepest node on its path that a lookup also
  passed: it cannot be a leaf or an extension, so it is a branch, and the leaf is below
  a slot no item's key goes through), where the rule rejects it. So the tries hold exactly
  the items `0..n`.

The earlier check (present at `n - 1`, absent at `n`) did not exclude keys past `n` that
branch off elsewhere, nor non-canonical keys, and a range running with its own unverified
count could have admitted them: that is why the count is part of every program's public
output and the aggregation requires them to agree.
