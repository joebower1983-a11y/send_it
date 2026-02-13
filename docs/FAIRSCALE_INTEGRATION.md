# FairScale Integration — Send.it

## Overview

Send.it integrates [FairScale](https://api.fairscale.xyz) reputation scores to create a trust layer for token launches. Creators must meet minimum reputation thresholds to launch tokens, and higher reputation unlocks fee discounts and shorter vesting periods.

## Why Reputation Matters

Token launchpads are plagued by rug pulls and bad actors. By gating launches behind on-chain reputation:

- **Buyers** can assess creator trustworthiness at a glance
- **Good creators** get rewarded with lower fees and better terms
- **Bad actors** face higher barriers and longer lock periods
- **The ecosystem** builds trust over time

## Reputation Tiers

| Tier | FairScore | Fee Discount | Vesting | Launch Access |
|------|-----------|-------------|---------|---------------|
| **Unscored** | — | 0% | N/A | ❌ Cannot launch |
| **Bronze** | 30-49 | 0% | 2x (extended) | ✅ Standard only |
| **Silver** | 50-64 | 5% | 1x (normal) | ✅ Standard only |
| **Gold** | 65-79 | 10% | 1x (normal) | ✅ Standard + Premium |
| **Platinum** | 80-100 | 20% | 1x (normal) | ✅ Standard + Premium |

### Key Rules

- **Minimum score to launch:** 30 (configurable)
- **Premium launch minimum:** 60 (configurable)
- **Extended vesting:** Creators below score 40 get 2x vesting period
- Scores are cached on-chain and refreshed by the oracle

## Trust Indicators (UI)

Each token displays a trust indicator based on its creator's FairScore:

- 🟢 **High Trust** — FairScore ≥ 70
- 🟡 **Medium** — FairScore 40-69
- 🔴 **Low** — FairScore < 40
- ⚫ **Unscored** — No FairScore data

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                     Send.it Frontend                     │
│  ┌──────────┐  ┌──────────┐  ┌────────────────────────┐ │
│  │ Launch   │  │ Explore  │  │ Reputation Dashboard   │ │
│  │ Page     │  │ Page     │  │ (Score Gauge + Tier)   │ │
│  └────┬─────┘  └────┬─────┘  └───────────┬────────────┘ │
│       │              │                    │               │
│       └──────────────┼────────────────────┘               │
│                      │                                    │
│              ┌───────▼───────┐                            │
│              │  SDK Client   │                            │
│              │ (fairscale.ts)│                            │
│              └───────┬───────┘                            │
└──────────────────────┼────────────────────────────────────┘
                       │
          ┌────────────┼────────────────┐
          │            │                │
          ▼            ▼                ▼
┌─────────────┐  ┌──────────┐  ┌──────────────┐
│ FairScale   │  │ Solana   │  │ Oracle       │
│ API         │  │ Program  │  │ Service      │
│             │  │          │  │              │
│ GET /score  │  │ PDAs:    │  │ Fetches API  │
│ ?wallet=... │  │ Config   │  │ → submits    │
│             │  │ Attest.  │  │ on-chain     │
└─────────────┘  └──────────┘  └──────────────┘
```

## API Integration

### FairScale API

- **Base URL:** `https://api.fairscale.xyz`
- **Endpoint:** `GET /score?wallet=WALLET_ADDRESS`
- **Auth Header:** `fairkey: YOUR_API_KEY`
- **Response:**
  ```json
  {
    "fairscore": 72,
    "tier": "gold",
    "badges": ["early_adopter", "defi_native"],
    "social_score": 65,
    "features": {
      "tx_count": 1423,
      "active_days": 287,
      "wallet_age_days": 540
    }
  }
  ```

### Caching Strategy

- **Client-side:** SDK caches scores in memory for 1 hour
- **On-chain:** Oracle writes attestations to PDAs, checked at launch time
- **Staleness:** Attestations older than 24h should be refreshed before launch

## On-Chain Program

### PDAs

| Account | Seeds | Purpose |
|---------|-------|---------|
| ReputationConfig | `["reputation_config"]` | Global settings |
| ReputationAttestation | `["reputation", wallet]` | Per-wallet cached score |

### Instructions

| Instruction | Authority | Description |
|-------------|-----------|-------------|
| `initialize_reputation_config` | Platform | One-time setup |
| `update_reputation_config` | Platform | Adjust thresholds/discounts |
| `update_reputation` | Oracle | Submit FairScore for a wallet |
| `check_launch_eligibility` | Anyone | Verify wallet can launch |
| `get_fee_discount` | Anyone | Get discount bps for tier |
