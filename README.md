# 🚀 Send.it — Next-Gen Token Launchpad on Solana

A pump.fun competitor built with Anchor, featuring bonding curves, anti-snipe protection, rug protection, and auto-migration to Raydium.

## Features

### 🎯 Token Creation
- Anyone can create a new SPL token with metadata (name, symbol, URI)
- Choose bonding curve type: **Linear**, **Exponential**, or **Sigmoid**
- Configurable creator fee share, anti-snipe settings, and lock periods
- Creator token allocation with vesting schedules

### 📈 Bonding Curve Trading
- `buy()` and `sell()` instructions price tokens along the chosen curve
- Three curve types with distinct price dynamics:
  - **Linear** — steady price increase
  - **Exponential** — accelerating price growth
  - **Sigmoid** — S-curve: slow start, fast middle, plateaus
- All math uses fixed-point precision (1e12 scaling)

### 🛡️ Anti-Snipe Protection
- Configurable `launch_delay` — seconds after creation before trading starts
- `max_buy_per_wallet` enforced during the snipe window (first N seconds)
- Global blocklist for known bot addresses

### 💰 Creator Revenue Share
- Creators earn a configurable % (default 1%) of every trade fee
- Fees sent directly to creator wallet on each trade

### 🔒 Rug Protection
- Creator cannot withdraw liquidity during `lock_period`
- Creator token allocation follows a linear vesting schedule
- Emergency pause by platform authority only

### 🌊 Auto-Migration to Raydium
- When reserve SOL hits `migration_threshold` (default 85 SOL), migration is triggered
- Permissionless crank — anyone can call `migrate_to_raydium`
- LP tokens locked in PDA (Raydium CPI integration ready)

### 🏦 Platform Fees
- Configurable `platform_fee_bps` on every trade (default 1%)
- Fees collected to platform vault PDA

### 🏆 Leaderboard Tracking
- On-chain tracking of top 20 tokens by volume
- Top creators by launches and volume
- Permissionless update via `update_leaderboard` crank

### ⚙️ Platform Config
- Global config PDA with platform authority, fee rates, migration threshold
- Admin functions: update config, pause/unpause, manage blocklist

## Architecture

### PDAs
| Account | Seeds |
|---------|-------|
| PlatformConfig | `["platform_config"]` |
| TokenLaunch | `["token_launch", mint]` |
| UserPosition | `["user_position", owner, mint]` |
| CreatorVesting | `["creator_vesting", mint]` |
| PlatformVault | `["platform_vault"]` |
| Leaderboard | `["leaderboard"]` |
| Blocklist | `["blocklist"]` |
| SOL Vault | `["sol_vault", mint]` |

### Instructions
- `initialize_platform` — Setup global config (admin)
- `update_platform_config` — Update fees/threshold (admin)
- `set_paused` — Emergency pause (admin)
- `initialize_leaderboard` — Create leaderboard (admin)
- `initialize_blocklist` / `add_to_blocklist` / `remove_from_blocklist` — Bot protection (admin)
- `create_token` — Launch a new token with bonding curve
- `buy` — Buy tokens along the curve
- `sell` — Sell tokens back to the curve
- `claim_vested_tokens` — Creator claims vested allocation
- `migrate_to_raydium` — Migrate liquidity when threshold met
- `update_leaderboard` — Permissionless leaderboard update

### 🛡️ FairScale Reputation Integration
- On-chain reputation gating via [FairScale](https://api.fairscale.xyz) scores
- **Reputation tiers:** Bronze / Silver / Gold / Platinum with fee discounts (0-20%)
- **Launch gating:** Minimum FairScore of 30 required to create tokens
- **Trust indicators:** 🟢 High Trust / 🟡 Medium / 🔴 Low / ⚫ Unscored on every token
- **Vesting enforcement:** Low-reputation creators get 2x vesting periods
- **Premium launches:** Require FairScore ≥ 60 (Gold/Platinum tier)
- Oracle-attested scores cached on-chain as PDAs
- See [docs/FAIRSCALE_INTEGRATION.md](docs/FAIRSCALE_INTEGRATION.md) for full details

### PDAs
| Account | Seeds |
|---------|-------|
| ReputationConfig | `["reputation_config"]` |
| ReputationAttestation | `["reputation", wallet]` |

## Build

```bash
anchor build
```

## Test

```bash
anchor test
```

## Deploy

```bash
anchor deploy --provider.cluster devnet
```

## License

MIT
