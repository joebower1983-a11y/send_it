# Kani Proof Strength Audit Report

**Program:** Send.it Slim v3 (`HTKq18cATdwCZb6XM66Mhn8JWKCFTrZqH6zU1zip88Zx`)
**Harness file:** `programs/send_it_slim/src/kani_proofs.rs`
**Date:** February 18, 2026
**Auditor:** Dog 🐕 (automated self-audit using kani-audit-prompt.md)

---

## Summary

| Proof | Classification | Inputs | Invariant Level | Vacuity |
|-------|---------------|--------|-----------------|---------|
| `proof_isqrt_correctness` | **STRONG** | 1 symbolic | canonical | ✅ non-vacuous |
| `proof_buy_fee_conservation` | **STRONG** | 4 symbolic | conservation | ✅ non-vacuous |
| `proof_buy_price_increases_with_supply` | **STRONG** | 3 symbolic | post-condition | ✅ non-vacuous |
| `proof_sell_fee_conservation` | **STRONG** | 5 symbolic | conservation | ✅ non-vacuous |
| `proof_swap_buy_k_non_decreasing` | **STRONG** | 3 symbolic | canonical (k invariant) | ✅ non-vacuous |
| `proof_swap_sell_k_non_decreasing` | **STRONG** | 3 symbolic | canonical (k invariant) | ✅ non-vacuous |
| `proof_swap_fee_split` | **WEAK** | 1 symbolic | conservation | ⚠️ partial |
| `proof_add_remove_liquidity_roundtrip` | **STRONG** | 4 symbolic | conservation | ✅ non-vacuous |
| `proof_initial_lp_sqrt` | **STRONG** | 2 symbolic | post-condition | ✅ non-vacuous |
| `proof_staking_reward_proportional` | **STRONG** | 2 symbolic | post-condition + monotonicity | ✅ non-vacuous |
| `proof_buy_sell_no_profit` | **STRONG** | 2 symbolic | conservation (no-arbitrage) | ✅ non-vacuous |

**Overall: 10 STRONG, 1 WEAK**

---

## Detailed Analysis

### 1. proof_isqrt_correctness

**Classification:** STRONG

**Inputs:**
- `n: u128` — symbolic, range [0, u64::MAX²]

**Branch Coverage:**
- `n == 0` early return: ✅ reachable (kani::cover confirms)
- Babylonian loop iterations: ✅ symbolic n exercises variable iteration counts
- `root == 0` vs `root > 0`: ✅ both covered

**Invariant Level:** Canonical — proves `root² ≤ n < (root+1)²`, which is the mathematical definition of integer square root.

**Vacuity Check:** ✅ Two `kani::cover!()` calls confirm both zero and nonzero paths reached.

**Symbolic Collapse:** ✅ None — single symbolic input, no derived narrowing.

---

### 2. proof_buy_fee_conservation

**Classification:** STRONG

**Inputs:**
- `sol_amount: u64` — symbolic (0, 1T]
- `tokens_sold: u64` — symbolic [0, DEFAULT_TOTAL_SUPPLY)
- `platform_fee_bps: u16` — symbolic [0, 1000]
- `creator_fee_bps: u16` — symbolic [0, 500]

**Branch Coverage:**
- `available == 0`: ✅ reachable (returns None)
- `tokens_out == 0`: ✅ reachable (returns None)
- `pf == 0` (zero platform fee): ✅ covered by `kani::cover!(pf == 0 && cf == 0)`
- `pf > 0 && cf > 0`: ✅ covered
- `raw_tokens > available` (clamping via min): ✅ symbolic ranges allow both `raw_tokens < available` and `raw_tokens >= available`

**Invariant Level:** Conservation — `pf + cf + net == sol_amount`. This is the strongest property: no SOL created or destroyed.

**Vacuity Check:** ✅ Two `kani::cover!()` calls confirm non-vacuity for both fee and no-fee paths. The `if let Some(...)` guard means assertions only fire on the Ok path, but covers confirm reachability.

**Symbolic Collapse:** ✅ None — all 4 inputs are independent symbolic values. Fee percentages are independent of sol_amount.

---

### 3. proof_buy_price_increases_with_supply

**Classification:** STRONG

**Inputs:**
- `sol_amount: u64` — symbolic (0, 100 SOL]
- `tokens_sold_low: u64` — symbolic
- `tokens_sold_high: u64` — symbolic, constrained `low < high < SUPPLY`

**Branch Coverage:**
- Both `buy_math` calls can return Some or None: ✅ symbolic ranges allow both
- The `tokens_low >= tokens_high` assertion exercises the comparison: ✅

**Invariant Level:** Post-condition — proves bonding curve is monotonically increasing (economic correctness).

**Vacuity Check:** ✅ `kani::cover!(tokens_low > tokens_high)` confirms strict inequality is reachable.

**Symbolic Collapse:** ⚠️ Minor — `sol_amount` upper bound of 100 SOL means very large buys that could exhaust supply aren't tested. However, the property being proved (price monotonicity) doesn't depend on absolute amounts, so this is acceptable.

---

### 4. proof_sell_fee_conservation

**Classification:** STRONG

**Inputs:**
- `token_amount: u64` — symbolic (0, TOTAL_SUPPLY]
- `tokens_sold: u64` — symbolic [token_amount, TOTAL_SUPPLY]
- `reserve_sol: u64` — symbolic (0, 1T]
- `platform_fee_bps: u16` — symbolic [0, 1000]
- `creator_fee_bps: u16` — symbolic [0, 500]

**Branch Coverage:**
- `token_amount > tokens_sold`: ✅ returns None (pre-check)
- `sol_out == 0`: ✅ reachable for very small sells
- `net > reserve_sol`: ✅ returns None (insufficient reserve path)
- Fee branches: ✅ both zero-fee and nonzero-fee reachable

**Invariant Level:** Conservation — `pf + cf + net == sol_out`

**Vacuity Check:** ✅ `kani::cover!()` confirms the assertion is reached with meaningful values.

**Symbolic Collapse:** ✅ None — 5 independent symbolic inputs.

---

### 5. proof_swap_buy_k_non_decreasing

**Classification:** STRONG

**Inputs:**
- `sol_amount: u64` — symbolic (0, 100 SOL]
- `sol_reserve: u64` — symbolic [0.001, 1000 SOL]
- `token_reserve: u64` — symbolic [1M, TOTAL_SUPPLY]

**Branch Coverage:**
- `fee == 0` (sol_amount < 100): ✅ reachable
- `token_out == 0`: ✅ reachable (returns None)
- `token_out >= token_reserve`: would be caught by checked_sub returning None: ✅

**Invariant Level:** Canonical — `k_after >= k_before` is THE defining invariant of constant-product AMMs. This is the strongest possible assertion.

**Vacuity Check:** ✅ `kani::cover!(fee > 0 && token_out > 0)` confirms meaningful execution.

**Symbolic Collapse:** ⚠️ Minor — `sol_amount` capped at 100 SOL while reserves can be up to 1000 SOL. Very large swaps (>100% of pool) aren't tested. However, in practice, such swaps would have catastrophic price impact and wouldn't occur.

**Recommendation:** Consider extending `sol_amount` upper bound to `sol_reserve` to test pool-draining swaps.

---

### 6. proof_swap_sell_k_non_decreasing

**Classification:** STRONG

**Inputs:**
- `token_amount: u64` — symbolic (0, TOTAL_SUPPLY]
- `sol_reserve: u64` — symbolic [0.001, 1000 SOL]
- `token_reserve: u64` — symbolic [1M, TOTAL_SUPPLY]

**Branch Coverage:** Same analysis as buy swap — both sides of all branches reachable.

**Invariant Level:** Canonical — constant product `k_after >= k_before`.

**Vacuity Check:** ✅

**Symbolic Collapse:** ✅ None — token_amount range is very wide relative to token_reserve.

---

### 7. proof_swap_fee_split

**Classification:** WEAK

**Inputs:**
- `sol_amount: u64` — symbolic [10K, 1T]

**Branch Coverage:**
- ⚠️ `total_fee == 0` path: The `kani::cover!(total_fee == 0)` is **unreachable** given `sol_amount >= 10_000` — fee is `sol_amount * 100 / 10_000 = sol_amount / 100`, which is ≥ 100 for all valid inputs. This cover is dead code.
- Only 1 input — no interaction between pool state and fee calculation tested.

**Invariant Level:** Conservation — `protocol_fee + lp_fee == total_fee`. Correct but narrow.

**Vacuity Check:** ⚠️ The `total_fee == 0` cover will never be reached. Not truly vacuous (the first cover works), but misleading.

**Symbolic Collapse:** ⚠️ The fee calculation uses only constants (SWAP_FEE_BPS, PROTOCOL_FEE_BPS, LP_FEE_BPS). The proof only varies `sol_amount`, which just scales the result. The interesting question — "do the BPS constants sum correctly?" — is verified by a single execution. This is effectively a **parameterized unit test** over amounts.

**Recommendations:**
1. Remove the unreachable `kani::cover!(total_fee == 0)` or lower `sol_amount` minimum to 0
2. Make fee BPS values symbolic to verify the split math works for any fee configuration
3. Add assertion: `protocol_fee * SWAP_FEE_BPS == total_fee * PROTOCOL_FEE_BPS` (proportionality)

---

### 8. proof_add_remove_liquidity_roundtrip

**Classification:** STRONG

**Inputs:**
- `sol_add: u64` — symbolic (0, 100 SOL]
- `sol_reserve: u64` — symbolic [0.001, 1000 SOL]
- `token_reserve: u64` — symbolic [1M, TOTAL_SUPPLY]
- `lp_supply: u64` — symbolic [MIN_LIQUIDITY, 1Q]

**Branch Coverage:**
- `token_add == 0` early return: ✅ reachable
- `lp_minted == 0` early return: ✅ reachable
- Rounding direction (sol_back < sol_add vs ==): ✅ both covered

**Invariant Level:** Conservation — roundtrip: add then remove gets back ≤ original (rounding favors pool, no value extraction).

**Vacuity Check:** ✅ `kani::cover!()` confirms meaningful execution.

**Symbolic Collapse:** ✅ None — 4 independent symbolic inputs with wide ranges.

---

### 9. proof_initial_lp_sqrt

**Classification:** STRONG

**Inputs:**
- `initial_sol: u64` — symbolic [1M, 1T]
- `initial_tokens: u64` — symbolic [1M, TOTAL_SUPPLY]

**Branch Coverage:**
- `product` overflow (returns None): ✅ reachable for large values
- `lp > MIN_LIQUIDITY` vs not: ✅ both reachable
- `to_creator > 0` vs `lp <= MIN_LIQUIDITY`: ✅

**Invariant Level:** Post-condition — `lp² ≤ product` (correct sqrt) and creator gets meaningful LP.

**Vacuity Check:** ✅ Cover on `to_creator > 0`.

**Symbolic Collapse:** ✅ None.

---

### 10. proof_staking_reward_proportional

**Classification:** STRONG

**Inputs:**
- `amount: u64` — symbolic (0, TOTAL_SUPPLY]
- `duration: u64` — symbolic [60, 31536000] (1 min to 1 year)

**Branch Coverage:**
- `reward == 0` (dust amounts): ✅ covered
- `reward > 0`: ✅ covered
- Monotonicity checks (duration-1, amount-1): ✅ exercises comparisons

**Invariant Level:** Post-condition + monotonicity — reward ≤ 10% APY cap, and reward is monotonically increasing in both amount and duration. This is strong economic correctness.

**Vacuity Check:** ✅ Two covers confirm both zero and nonzero rewards.

**Symbolic Collapse:** ✅ None — both inputs independently vary across wide ranges.

---

### 11. proof_buy_sell_no_profit

**Classification:** STRONG

**Inputs:**
- `sol_amount: u64` — symbolic [1M, 10B] (0.001 - 10 SOL)
- `tokens_sold_initial: u64` — symbolic [0, TOTAL_SUPPLY/2)

**Branch Coverage:**
- `buy_math` returns None: ✅ reachable
- `sell_math` returns None (e.g., reserve too low): ✅ reachable
- `sol_back == sol_amount` (exact roundtrip): ✅ covered
- `sol_back < sol_amount` (rounding loss): ✅ covered

**Invariant Level:** Conservation (no-arbitrage) — buying and immediately selling cannot extract value from the bonding curve. This is a critical economic security property.

**Vacuity Check:** ✅ Two covers confirm both exact and lossy roundtrips.

**Symbolic Collapse:** ⚠️ Minor — fees are set to 0 to isolate curve math. This is intentional (testing the curve, not fees), but a separate proof with nonzero fees would strengthen the overall suite.

**Recommendation:** Add a companion proof `proof_buy_sell_no_profit_with_fees` using symbolic fee values to verify the no-arbitrage property holds even with fee rounding.

---

## Missing Proofs (Recommendations)

### HIGH PRIORITY

1. **proof_swap_no_drain** — Verify a single swap cannot extract more than a bounded percentage of pool reserves. Prevents pool-draining attacks.

2. **proof_remove_liquidity_proportional** — Verify that removing X% of LP supply returns exactly X% of each reserve (within rounding). Currently only tested as part of the roundtrip.

3. **proof_bonding_curve_reserve_solvency** — For any sequence of buys and sells, `reserve_sol` should always be sufficient to pay out the remaining sellers. This is the critical "bank run" invariant.

### MEDIUM PRIORITY

4. **proof_fee_never_exceeds_amount** — Verify `platform_fee + creator_fee < sol_amount` for all valid fee BPS values. Prevents fee configurations that would underflow `net`.

5. **proof_staking_reward_cap** — Verify reward never exceeds available vault tokens. The program caps at `available_for_rewards`, but proving this is correct under all states would be valuable.

6. **proof_migration_threshold_sound** — Verify that when `reserve_sol >= migration_threshold`, the pool creation math doesn't fail (sufficient liquidity for a viable pool).

### LOW PRIORITY

7. **proof_isqrt_no_panic** — The current proof is strong, but Kani's `#[kani::unwind(130)]` should be verified as sufficient for the maximum iteration count.

8. **proof_overflow_boundaries** — Explicitly test u64::MAX inputs for every arithmetic function to verify checked_mul/checked_div properly catch overflows.

---

## Conclusion

The proof suite is **strong overall** — 10 of 11 proofs are classified STRONG with symbolic inputs, canonical or conservation invariants, and confirmed non-vacuity. The one WEAK proof (`proof_swap_fee_split`) is easily strengthened by making fee BPS values symbolic.

Key strengths:
- ✅ Constant-product invariant (k non-decreasing) verified for both buy and sell swaps
- ✅ Fee conservation verified for bonding curve buy and sell
- ✅ No-arbitrage property verified for buy-sell roundtrips
- ✅ LP add-remove roundtrip conservation verified
- ✅ Staking reward monotonicity and APY cap verified
- ✅ Every proof includes `kani::cover!()` for non-vacuity

Key gaps to address:
- ⚠️ No pool-drain proof
- ⚠️ No "bank run" solvency proof for bonding curve
- ⚠️ Fee split proof is effectively a parameterized unit test
- ⚠️ No proof combining fees + curve math (buy-sell with fees)
