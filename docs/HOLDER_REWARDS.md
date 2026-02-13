# Holder Rewards System — Design Document

## Overview

Every token launched on Send.it has an associated **reward pool**. A portion of every trade fee (buy or sell) is deposited into this pool and distributed pro-rata to all holders. Holders claim accumulated SOL rewards on-demand, with an optional auto-compound feature that reinvests rewards into more tokens.

---

## 1. Fee Flow

```
Trade (buy/sell)
  └─ Platform fee (e.g. 1%)
       ├─ 50% → Platform treasury
       └─ 50% → Token's RewardPool (holder rewards)
```

The reward split percentage is configurable per-launch via `reward_fee_bps` on the `RewardPool` account (basis points of the total platform fee directed to holders).

---

## 2. Reward Math — Cumulative Reward-Per-Token (Synthetix Model)

This avoids iterating over all holders. Two levels of state:

### Global (per token): `RewardPool`

| Field | Description |
|---|---|
| `reward_per_token_stored` | Cumulative rewards per token (scaled by 1e12) |
| `total_supply_eligible` | Total token supply currently eligible for rewards |
| `last_update_timestamp` | Last time rewards were accrued |
| `pending_rewards` | SOL deposited but not yet factored into reward_per_token |

### Per-user (per token): `UserRewardState`

| Field | Description |
|---|---|
| `reward_per_token_paid` | Snapshot of global `reward_per_token_stored` at last interaction |
| `rewards_earned` | Accumulated unclaimed rewards (lamports) |
| `balance` | User's token balance known to the reward system |
| `first_hold_timestamp` | When user first acquired tokens (for min hold check) |
| `auto_compound` | Whether to auto-reinvest on claim |

### Core Formula

When new fees (`reward_amount`) are deposited:

```
reward_per_token_stored += (reward_amount * 1e12) / total_supply_eligible
```

When a user interacts (trade, claim):

```
earned = user.balance * (global.reward_per_token_stored - user.reward_per_token_paid) / 1e12
user.rewards_earned += earned
user.reward_per_token_paid = global.reward_per_token_stored
```

**Precision:** We scale by 1e12 to maintain precision with integer math. All reward amounts are in lamports (u64).

### Why This Works

- Adding rewards: O(1) — just update the global accumulator
- Claiming: O(1) — just compute the delta for one user
- No iteration over holders, ever
- Latecomers only earn from rewards deposited after they acquired tokens

---

## 3. Instructions

### `initialize_reward_pool`
Called when a token is launched. Creates the `RewardPool` PDA for that mint.

### `accrue_rewards(reward_amount: u64)`
Called internally during every trade. Deposits SOL into the reward pool vault and updates `reward_per_token_stored`. Not user-facing — invoked by the trade instruction via CPI or inline.

### `update_user_reward_state(new_balance: u64)`
Called on every trade that changes a user's balance. Updates the user's earned rewards and snapshots the new balance. Creates `UserRewardState` if it doesn't exist.

### `claim_holder_rewards()`
User-facing. Calculates pending rewards, checks minimum hold time, transfers SOL from the pool vault to the user (or auto-compounds by buying more tokens).

### `toggle_auto_compound(enabled: bool)`
User-facing. Sets `auto_compound` flag on the user's reward state.

---

## 4. Anti-Gaming: Minimum Hold Time

The `RewardPool` has a configurable `min_hold_seconds` (default: 0 = disabled). When set:

- `UserRewardState.first_hold_timestamp` is set when balance goes from 0 → >0
- On claim, we check: `now - first_hold_timestamp >= min_hold_seconds`
- If not met, claim fails with `HoldTimeNotMet`
- Selling resets the timestamp if balance drops to 0

This prevents flash-loan style buy→claim→sell attacks.

---

## 5. Auto-Compound

When `auto_compound = true` and the user calls `claim_holder_rewards()`:

1. Calculate claimable SOL
2. Instead of transferring SOL to user, execute a buy on the bonding curve
3. Update user's balance in the reward state
4. Emit `RewardAutoCompounded` event

The buy uses the existing bonding curve logic via CPI.

---

## 6. Events

```rust
RewardAccrued { mint, reward_amount, new_reward_per_token }
RewardClaimed { user, mint, amount }
RewardAutoCompounded { user, mint, sol_amount, tokens_received }
AutoCompoundToggled { user, mint, enabled }
```

---

## 7. Account Sizes & PDAs

| Account | Seeds | Size (bytes) |
|---|---|---|
| `RewardPool` | `["reward_pool", mint]` | 8 + 32 + 32 + 16 + 8 + 8 + 8 + 2 + 2 + 1 = ~117 (+padding = 128) |
| `UserRewardState` | `["user_reward", mint, user]` | 8 + 32 + 32 + 16 + 16 + 8 + 8 + 8 + 1 + 1 = ~130 (+padding = 144) |
| `RewardVault` | `["reward_vault", mint]` | System account (SOL holder) |

---

## 8. Security Considerations

- **Overflow protection**: All math uses checked arithmetic; `reward_per_token_stored` is u128 scaled by 1e12
- **Rounding**: Dust stays in the pool (rounds down on claim). This is standard and benefits remaining holders slightly.
- **Rent**: `UserRewardState` rent is paid by the user on first interaction. Can be reclaimed if balance is 0 and rewards are fully claimed (close account).
- **Authority**: Only the trade instruction (program authority) can call `accrue_rewards` and `update_user_reward_state`
- **Vault safety**: Reward vault is a PDA-owned system account; only the program can sign transfers out
