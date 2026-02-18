# Kani Proof Strength Audit Prompt

Use this prompt to analyze Kani proof harnesses for weakness, vacuity, or collapse into unit tests.

---

## Instructions

You are auditing Kani verification harnesses for a Solana/Anchor program. For every `#[kani::proof]` function, evaluate the following dimensions and classify the proof.

### 1. Input Classification

For each input to the function-under-test, classify as:

| Type | Definition | Example |
|------|-----------|---------|
| **Concrete** | Hardcoded literal value | `let amount = 1_000_000u64;` |
| **Symbolic** | `kani::any()` with `kani::assume()` bounds | `let amount: u64 = kani::any(); kani::assume(amount > 0 && amount <= 1_000_000_000);` |
| **Derived** | Computed from other inputs | `let fee = amount * FEE_BPS / 10_000;` |

**Rule:** A proof where ALL function-under-test inputs are concrete is a unit test, not a proof. It exercises exactly one execution path and proves nothing about the general case.

### 2. Branch Coverage Analysis

Read the function-under-test source and enumerate every conditional branch:
- `if` / `else` / `else if`
- `match` arms (including implicit `_` wildcards)
- `.checked_add()` / `.checked_mul()` None vs Some paths
- `.saturating_sub()` / `.saturating_mul()` clamping behavior
- `min()` / `max()` — does the proof reach both sides?
- `require!()` / `assert!()` / error returns — can the solver trigger the error AND the success path?
- Integer overflow/underflow boundaries

For each branch, determine:
- Can the solver's symbolic inputs reach **both sides**?
- Is any branch **locked to one side** by concrete values or overly tight `kani::assume()` constraints?
- Are error paths tested, or does the proof only assert on the happy path?

**Flag** any branch that is unreachable given the proof's constraints.

### 3. Invariant Strength

Evaluate what the proof actually asserts:

| Level | Description | Strength |
|-------|-------------|----------|
| **No panic** | Proof only checks that the function doesn't panic | Weakest — misses logical bugs |
| **Type-level** | Return type is Ok/Err, proof checks `is_ok()` | Weak — doesn't verify output correctness |
| **Post-condition** | Asserts specific output properties (e.g., `fee <= amount`) | Medium |
| **Conservation** | Asserts input/output conservation (e.g., `sol_in == sol_out + fee`) | Strong |
| **Canonical invariant** | Full state invariant checked before and after (e.g., `reserves_x * reserves_y >= k`) | Strongest |

**Flag:**
- Proofs that use a weaker invariant when a stronger one exists
- Proofs that assert on Ok path only without proving the Ok path is reachable (vacuity risk)
- Proofs that check `valid_state()` (permissive) instead of `canonical_inv()` (strict) when both exist
- Missing conservation laws (tokens in == tokens out + fees, SOL debited == SOL credited)

### 4. Vacuity Risk

A proof is **vacuously true** if the solver can satisfy all `kani::assume()` constraints but never actually reaches the assertions. Check for:

- **Contradictory assumes:** `kani::assume(x > 100); kani::assume(x < 50);` — trivially true, no valid execution
- **Unreachable Ok path:** `assert!(result.is_ok()); assert!(result.unwrap().field > 0);` — if the function always returns Err given the constraints, both assertions are vacuously true because `assert!(false_result.is_ok())` will fail, meaning the proof "passes" only because Kani reports the assertion failure, OR if wrapped in `if let Ok(r) = result { assert!(r.field > 0); }` the assertion is never reached
- **Over-constrained state:** `kani::assume(canonical_inv(&state))` on a hand-constructed state that cannot satisfy the invariant — zero valid executions
- **Always-error inputs:** Symbolic inputs that always trigger an early return/error before reaching the assertions

**Mitigation:** Every proof should include a **non-vacuity assertion** — a `kani::cover!()` or equivalent that confirms the solver found at least one execution reaching the critical assertion.

### 5. Symbolic Collapse

Even with `kani::any()`, check if derived computations collapse the symbolic range:

**Example:**
```rust
let capital: u64 = 1_000_000_000; // concrete: 1000 SOL
let insurance: u64 = 500_000_000; // concrete: 500 SOL
let pnl: i64 = kani::any();      // symbolic
kani::assume(pnl >= -100_000_000 && pnl <= 100_000_000); // ±100 SOL

let total = (capital as i128 + insurance as i128 + pnl as i128) as u64;
let haircut_ratio = total * 100 / (capital + insurance); // always 93-106%
```

Here `haircut_ratio` never goes below 93% because the concrete values dominate. If the function has a branch at `haircut_ratio < 50`, it's unreachable. The proof appears symbolic but effectively tests a narrow slice.

**Check for:**
- Concrete values that dominate the arithmetic, narrowing symbolic range
- Symbolic values used only in conditions that are always true/false given concrete co-inputs
- Array indices derived from symbolic values but bounded to a single valid index
- Timestamps or slots where the symbolic range is too narrow to cross epoch/period boundaries

---

## Output Format

For each `#[kani::proof]` harness, output:

```
### proof_name

**Classification:** STRONG | WEAK | UNIT TEST | VACUOUS

**Inputs:**
- input_name: concrete | symbolic (range) | derived (from what)

**Branch Coverage:**
- branch_description: ✅ both sides reachable | ⚠️ locked to [side] because [reason]

**Invariant Level:** no-panic | type | post-condition | conservation | canonical

**Vacuity Check:** ✅ non-vacuous (cover! present) | ⚠️ risk: [description]

**Symbolic Collapse:** ✅ none detected | ⚠️ [variable] collapses because [reason]

**Recommendations:**
1. [specific fix to strengthen the proof]
```

---

## Classification Criteria

| Rating | Requirements |
|--------|-------------|
| **STRONG** | Symbolic inputs exercise all branches. Conservation or canonical invariant asserted. Non-vacuity confirmed via `kani::cover!()`. No symbolic collapse. |
| **WEAK** | Symbolic inputs present but: misses ≥1 branch, uses weaker invariant than available, or no non-vacuity check. List specific gaps. |
| **UNIT TEST** | All function-under-test inputs are concrete. Single execution path. Rename to `#[test]` — it's not a proof. |
| **VACUOUS** | Assertions may never be reached due to contradictory assumes, always-error paths, or over-constrained state. Proof passes trivially. |

---

## Solana/Anchor-Specific Checks

- **Lamport conservation:** For any instruction that moves SOL, assert `Σ pre_lamports == Σ post_lamports` across all accounts
- **Token conservation:** For any instruction that transfers tokens, assert `pre_supply == post_supply` (no mint/burn) or track mint/burn explicitly
- **PDA derivation:** If the function derives PDAs, verify the proof uses symbolic seeds that cover edge cases (empty seeds, max-length seeds, bump = 0 vs 255)
- **Overflow at boundaries:** `u64::MAX` for token amounts, `i64::MIN` for signed values, 0 for all amounts — these should be in the symbolic range
- **Fee rounding:** Assert `fee_paid + amount_received == amount_sent` (no dust creation/destruction)
- **Pool math:** For AMM functions, assert `k_post >= k_pre` (constant product invariant never decreases)

---

*Apply this prompt to the harnesses and report findings. Be specific about which lines of code create each weakness.*
