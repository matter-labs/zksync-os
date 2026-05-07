# Delegation Coefficients

## What they are

The effective cycle count approximates total proving cost as a single number:

```
effective_cycles = raw_risc_v_cycles
                 + C_blake  × blake_delegations
                 + C_bigint × bigint_delegations
                 + C_keccak × keccak_delegations
```

Each coefficient represents how many RISC-V cycles one delegation call is "worth" in proving cost. A coefficient of 16 means one delegation call is 16× more expensive to prove than one RISC-V cycle.

## Current values

| Delegation | CSR ID | Coefficient | Source |
|------------|--------|-------------|--------|
| Blake2 | 1991 | 16 | Historical (needs re-validation) |
| BigInt | 1994 | 4 | Historical (needs re-validation) |
| Keccak | 1995 | 16 | Temporary placeholder |

## How to calibrate empirically

Run the prover on a real block and measure per-circuit-type proving time:

1. Get proving time for N instances of the RISC-V circuit
2. Get proving time for M instances of each delegation circuit
3. Compute cost per row: `cost_per_row_X = proving_time_X / (instances_X × domain_size_X)`
4. Coefficient: `C_X = cost_per_row_X / cost_per_row_risc_v`

## Analytical approximation

Each delegation call occupies one row in its circuit, and each RISC-V cycle occupies one row in the RISC-V circuit. The coefficient is the ratio of per-row proving costs.

### Prover structure (from zksync-airbender)

The Airbender prover commits columns across 5 stages, each producing a separate Merkle tree:

| Stage | Content | Width |
|-------|---------|-------|
| 1a | Witness columns | W_witness |
| 1b | Memory columns | W_memory |
| (precomputed) | Setup columns | W_setup |
| 2 | Lookup/permutation argument | W_stage2 |
| 3 | Quotient polynomial | **4** (fixed leaf size) |

After commitment, stage 4 computes the deep polynomial (combining all openings), and stage 5 runs FRI with 53 queries.

Key implementation details:
- **FRI blowup factor F = 2** (uniform across all circuits)
- **Merkle hashing**: Blake2s, one leaf per row, leaf contains all columns for that tree
- **Quotient leaf size is fixed at 4** regardless of constraint count — Q quotient terms do NOT add Q columns to commitment cost
- **FRI folding** is independent of trace width — it operates on the deep polynomial
- **GPU prover**: memory bandwidth bound, NTT uses L2 persistence strategy with dual-stream pipelining. Cost structure does not fundamentally differ from CPU for the purpose of coefficient ratios (see below).

### Cost per circuit instance

The dominant costs for a circuit with effective width W_eff and domain size D:

1. **NTTs (stage 1 LDE)**: Extend W_eff columns from D to D×F evaluations.
   Cost: `W_eff × D × F × log₂(D × F)` field multiplications.

2. **Merkle leaf hashing**: Hash W_eff field elements per leaf, D×F leaves total.
   Cost: `D × F × ⌈W_eff / 16⌉` Blake2s calls (16 u32 words per Blake2s block, one Mersenne31 element = one u32 word).

3. **Merkle tree construction**: Build tree over D×F leaves.
   Cost: `D × F` hashes (independent of W).

4. **Quotient evaluation (stage 3)**: Evaluate Q constraint terms at D×F points, commit with leaf size 4.
   Cost: `Q × D × F` (independent of W).

5. **FRI folding (stage 5)**: Fold the deep polynomial through ~log₂(D) rounds.
   Cost: `O(D × F × log₂(D))` (independent of W).

6. **FRI queries**: 53 queries, each opening all 5 trees at a given row index.
   Cost: `53 × 5 × log₂(D×F)` hashes (negligible vs leaf hashing).

Items 1-2 dominate and both scale linearly with W_eff. Items 3-6 are either small or independent of W.

### Effective width

The effective width for proving cost is the total columns committed across stages 1-3, excluding amortized setup:

```
W_eff = W_witness + W_memory + W_stage2 + 4 (quotient)
```

Setup columns (W_setup) are preprocessed once and amortized across instances.

### Per-row cost and coefficient

Dividing total cost by D (rows per instance):

```
cost_per_row_X ≈ W_eff_X × F × log₂(D_X × F)
```

The coefficient:

```
C_X = W_eff_X × log₂(D_X × F) / (W_eff_rv × log₂(D_rv × F))
```

F = 2 is the same for all circuits, so it cancels from the multiplicative part.

### Why this formula holds for both CPU and GPU

On CPU, the cost per NTT column is `D × F × log₂(D×F)` field multiplications. On GPU (memory bandwidth bound), the cost per NTT column is `num_kernel_launches × D × F × bytes_per_element / bandwidth`, where `num_kernel_launches ≈ ⌈log₂(D×F) / 7⌉` (7-8 NTT stages per kernel launch).

In both cases, total cost per instance scales as `W_eff × D × f(D)`, where f(D) is some function of D (log D for CPU, ⌈log D / 7⌉ for GPU). Dividing by D (delegations per instance) gives per-delegation cost `W_eff × f(D)`.

The domain size D **cancels** when computing per-delegation cost — it appears in both the total cost and the delegation count, so it drops out. What remains is `W_eff × f(D)`, where f(D) varies by less than 10% across our domain sizes (f(2^21)=21 vs f(2^23)=23 for CPU; 3 vs 4 kernel launches for GPU).

### Circuit data

Extracted from `generated/circuit_layout.rs` in `zksync-airbender/circuit_defs/`:

| Circuit | Domain | Witness | Memory | Setup | Stage 2 | Quotient | W_eff |
|---------|--------|---------|--------|-------|---------|----------|-------|
| RISC-V | 2^20 | 197 | 30 | 32 | 180 | 4 | 411 |
| Blake2 | 2^20 | 648 | 226 | 6 | 1244 | 4 | 2122 |
| BigInt | 2^21 | 229 | 98 | 6 | 520 | 4 | 851 |
| Keccak | 2^22 | 177 | 92 | 6 | 324 | 4 | 597 |

### Analytical coefficients

| Circuit | W_eff / W_rv | With log correction | Current |
|---------|-------------|---------------------|---------|
| Blake2 | 5.16 | 5.16 | 16 |
| BigInt | 2.07 | 2.17 | 4 |
| Keccak | 1.45 | 1.59 | 16 |

Blake and RISC-V share the same domain size so the log factor cancels exactly.

### Why the gap?

The analytical model predicts ~5 (blake), ~2 (bigint), ~1.5 (keccak), but the current blake coefficient is 16 — a 3× gap. Possible explanations:

- **The current coefficients may be inaccurate** — they may have been set as rough estimates or under conditions that no longer apply. The bigint coefficient (4 vs analytical 2.2) also shows a ~2× gap.
- **Merkle leaf hashing has non-linear overhead**: Blake2s has fixed per-call overhead from padding and finalization. Blake2 rows require ~133 Blake2s blocks per leaf vs ~26 for RISC-V. If per-block overhead is significant relative to throughput, the effective cost ratio for hashing could exceed the width ratio.
- **Grand product computation for Stage 2**: Blake has 1244 Stage 2 columns requiring sequential grand product scans. On GPU, these are parallelized across columns but each is O(D) sequential — this could add overhead not captured by the NTT model.
- **Implementation factors**: Cache behavior, memory allocation, kernel scheduling, and host-device transfer patterns may disproportionately affect wider circuits.

### Keccak estimate

Keccak has the narrowest effective trace (597 columns, 1.45× RISC-V). The log correction for its larger domain (2^22 vs 2^20) adds ~10%, giving 1.59×. Even applying the same empirical correction as blake (3.1×), keccak would be ~5. The current placeholder of 16 is likely conservative.

## Recommendation

Calibrate all three coefficients empirically using actual prover benchmarks. The analytical column-ratio model predicts approximately 5 (blake), 2 (bigint), 1.5 (keccak). The model applies equally to CPU and GPU proving since the domain size cancels in the per-delegation ratio.
