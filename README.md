<p align="center">
  <h1 align="center">🚀 Send.it</h1>
  <p align="center"><strong>The Most Feature-Rich Token Launchpad on Solana</strong></p>
  <p align="center">
    <img src="https://img.shields.io/badge/modules-31-00c896-00c896" alt="31 modules">
    <img src="https://img.shields.io/badge/Rust_LOC-16k%2B-orange" alt="Rust LOC">
    <img src="https://img.shields.io/badge/Solana-Anchor-9945FF?logo=solana" alt="Solana Anchor">
    <img src="https://img.shields.io/badge/devnet-live-brightgreen" alt="Devnet Live">
    <img src="https://img.shields.io/badge/5IVE_VM-ported-blueviolet" alt="5IVE VM Port">
    <img src="https://img.shields.io/badge/license-MIT-blue" alt="MIT License">
  </p>
  <p align="center">
    <em>31 on-chain Anchor modules · ~16,000 lines of Rust · 13 frontend pages · Ported to 5IVE VM · Built for the next era of DeFi</em>
  </p>
  <p align="center">
    <a href="https://senditsolana.io/">📄 GitHub Pages</a> ·
    <a href="https://senditsolana.io">🌐 Live App</a> ·
    <a href="https://senditsolana.io">🔗 sendsenditsolana.io</a>
  </p>
</p>

---

## 🏆 For Hackathon Judges

| What | Link |
|------|------|
| **Live Demo** | [senditsolana.io](https://senditsolana.io) |
| **Launchpad (Devnet)** | [senditsolana.io/launchpad.html](https://senditsolana.io/launchpad.html) |
| **Social Hub (Tapestry)** | [senditsolana.io/social.html](https://senditsolana.io/social.html) |
| **Devnet Program** | [`98Vxqk2dHjLsUb4svNaZWwVZxt9DZZwkRQZZNQmYRm1L`](https://solscan.io/account/98Vxqk2dHjLsUb4svNaZWwVZxt9DZZwkRQZZNQmYRm1L?cluster=devnet) |
| **SENDIT Token** | [`F8qWTN8JfyDCvj4RoCHuvNMVbTV9XQksLuziA8PYpump`](https://pump.fun/coin/F8qWTN8JfyDCvj4RoCHuvNMVbTV9XQksLuziA8PYpump) |
| **5IVE VM Port** | [github.com/joebower1983-a11y/sendit-5ive](https://github.com/joebower1983-a11y/sendit-5ive) |
| **Docs & Whitepaper** | [GitHub Pages](https://senditsolana.io/) |
| **CLI Tool** | `scripts/sendit-cli.mjs` — interact with devnet from terminal |

**Try it:** Connect Phantom/Solflare (set to devnet) → Create token → Buy → Sell — all on-chain.

**Verified transactions:** `create_token`, `buy`, `sell` — all confirmed on Solana devnet with real PDA-signed transfers.

---

## 🔗 Devnet Deployment — LIVE & TESTED ✅

| | Address |
|---|---|
| **Program ID** | `98Vxqk2dHjLsUb4svNaZWwVZxt9DZZwkRQZZNQmYRm1L` |
| **Platform Config** | `AsxZyH56EQ6LdvzZYr9LsUzvaMgVMDoLv3on2AsGMve8` |
| **Network** | Solana Devnet |
| **SENDIT Token** | `F8qWTN8JfyDCvj4RoCHuvNMVbTV9XQksLuziA8PYpump` (Token-2022, pump.fun) |

### Verified On-Chain Transactions

| Instruction | Tx Signature | Status |
|---|---|---|
| `initialize_platform` | [View on Solscan](https://solscan.io/tx/QfXTss...?cluster=devnet) | ✅ |
| `create_token` | `38jwRpoyja6gQN6wHNZrHcZC9YrMNaM4spBfvV8ZFhMejupWG7J2LoxkPgzrKt9q8UULnbMrTFr54FrjeUofm3so` | ✅ |
| `buy` (bonding curve) | `2CQsDE2ZD2A1N5Kwipa2djQM6ah41cNrQPU1xZLqapHGREhfyLT2TPNt2dt1wJVqroLJ3SRvWGUNAV9vFSS9yvBw` | ✅ |
| `sell` (bonding curve) | `4RwW6uDZWduztUzTMtiwuDUjV5gFFFWtSNNLri8GaWH7B66HJJ9uem92vdMA2DnYLxn9E4dh1boyRycewFcJhYzE` | ✅ |

Full DeFi loop verified: platform init → token launch → bonding curve buy → bonding curve sell with fee distribution.

### 5IVE VM Port
The entire protocol has also been ported to [5IVE DSL](https://github.com/joebower1983-a11y/sendit-5ive) — 63% code reduction (16k → 6k lines), 25KB total bytecode, 159 integration tests.

---

## What is Send.it?

Send.it is a **next-generation token launchpad** on Solana that goes far beyond simple token creation. While platforms like pump.fun offer basic bonding curve launches, Send.it delivers a **full-stack DeFi ecosystem** — from token creation with anti-snipe protection and auto-migration to Raydium, to perpetuals trading, prediction markets, lending, staking, social features, and governance — all on-chain via Anchor.

---

## ✨ Features

### 🎯 Core — Token Launch Engine
| Feature | Description |
|---------|-------------|
| **Bonding Curve Trading** | Linear, Exponential, and Sigmoid curves with fixed-point math (1e12 precision) |
| **Anti-Snipe Protection** | Configurable launch delays, per-wallet buy caps during snipe windows, global bot blocklist |
| **Rug Protection** | Lock periods on liquidity, linear creator vesting schedules, emergency pause |
| **Auto-Migration to Raydium** | Permissionless crank triggers migration when reserve hits threshold (default 85 SOL) |
| **Creator Revenue Share** | Configurable creator fee on every trade (up to 5%), paid directly on each tx |
| **Platform Config & Fees** | Global PDA config, adjustable platform fees, admin controls |
| **Leaderboard** | On-chain top-20 tokens/creators by volume, permissionless crank updates |
| **Creator Dashboard** | Analytics and management tools for token creators |
| **Custom Pages** | Creator-customizable token landing pages |

### 💎 DeFi — Advanced Financial Instruments
| Module | Description |
|--------|-------------|
| **Staking** | Stake tokens to earn yield, with configurable reward rates and lock periods |
| **Lending** | Peer-to-pool lending and borrowing against token positions |
| **Limit Orders** | On-chain limit order book for bonding curve tokens |
| **Perps** | Perpetual futures contracts on launched tokens with leverage |
| **Bridge** | Cross-chain bridging infrastructure for token portability |
| **Holder Rewards** | Automatic reward distribution to long-term holders |

### 🗳️ Governance & Reputation
| Module | Description |
|--------|-------------|
| **Voting** | On-chain governance proposals and token-weighted voting |
| **Reputation** | FairScale-integrated reputation scores with tier-based fee discounts and launch gating |
| **Prediction Market** | Binary outcome markets on token milestones and events |

### 💬 Social — Community Layer
| Module | Description |
|--------|-------------|
| **Token Chat** | Real-time on-chain messaging tied to token communities |
| **Live Chat** | Live discussion rooms for active token launches |
| **Token Videos** | Video content linked to token pages for creator storytelling |
| **Share Cards** | Shareable on-chain cards for token stats and achievements |

### 🚀 Growth — Engagement & Retention
| Module | Description |
|--------|-------------|
| **Achievements** | On-chain achievement system rewarding user milestones |
| **Daily Rewards** | Daily check-in rewards to drive retention |
| **Seasons** | Time-boxed competitive seasons with leaderboards and prizes |
| **Referral** | On-chain referral tracking with fee-sharing incentives |
| **Airdrops** | Configurable airdrop campaigns with Merkle-tree verification |
| **Raffle** | Randomized prize draws for token communities |
| **Premium** | Premium tier access with enhanced features and reduced fees |
| **Price Alerts** | On-chain subscription for price movement notifications |

### 💸 Creator Monetization (Inspired by Bags.fm)
| Module | Description |
|--------|-------------|
| **Fee Splitting** | Split creator fees with co-creators, charities, or any wallet (up to 5 recipients) |
| **Content Claims** | "Get Bagged" — content owners can claim tokens based on their content and redirect fees |
| **Embeddable Widgets** | On-chain config for embeddable price badges, trading cards, and mini charts |

### 📊 Analytics & Intelligence
| Module | Description |
|--------|-------------|
| **Analytics** | On-chain volume, trade count, and trend data per token |
| **Copy Trading** | Follow and mirror successful traders' positions |

---

## 🏗️ Architecture

```
programs/send_it/src/
├── lib.rs                  # Core program: bonding curves, buy/sell, migration, admin
├── achievements.rs         # Achievement system
├── airdrops.rs             # Airdrop campaigns
├── analytics.rs            # On-chain analytics
├── bridge.rs               # Cross-chain bridge
├── copy_trading.rs         # Copy trading engine
├── creator_dashboard.rs    # Creator management tools
├── custom_pages.rs         # Custom token pages
├── daily_rewards.rs        # Daily reward system
├── holder_rewards.rs       # Holder reward distribution
├── lending.rs              # Lending & borrowing
├── limit_orders.rs         # Limit order book
├── live_chat.rs            # Live chat rooms
├── perps.rs                # Perpetual futures
├── prediction_market.rs    # Prediction markets
├── premium.rs              # Premium tier system
├── price_alerts.rs         # Price alert subscriptions
├── raffle.rs               # Raffle system
├── fee_splitting.rs        # Creator fee splitting
├── content_claims.rs       # Content ownership claims
├── embeddable_widgets.rs   # Embeddable widget config
├── referral.rs             # Referral program
├── reputation.rs           # Reputation & trust scoring
├── seasons.rs              # Seasonal competitions
├── share_cards.rs          # Shareable stat cards
├── staking.rs              # Token staking
├── token_chat.rs           # Token community chat
├── token_videos.rs         # Video content
└── voting.rs               # Governance voting
```

### On-Chain PDAs

| Account | Seeds | Purpose |
|---------|-------|---------|
| PlatformConfig | `["platform_config"]` | Global settings & admin |
| TokenLaunch | `["token_launch", mint]` | Per-token bonding curve state |
| UserPosition | `["user_position", owner, mint]` | User trade tracking |
| CreatorVesting | `["creator_vesting", mint]` | Creator token vesting |
| PlatformVault | `["platform_vault"]` | Platform fee collection |
| Leaderboard | `["leaderboard"]` | Global rankings |
| Blocklist | `["blocklist"]` | Anti-bot wallet list |
| SOL Vault | `["sol_vault", mint]` | Per-token SOL reserve |

---

## 🛠️ Tech Stack

- **Blockchain:** Solana (Mainnet/Devnet)
- **Smart Contracts:** Anchor Framework (Rust)
- **Frontend:** React + TypeScript, deployed on Vercel
- **Math:** Fixed-point arithmetic with 1e12 precision scaling
- **Testing:** Anchor test suite + Solana Playground
- **CI/CD:** GitHub Actions → GitHub Pages + Vercel

---

## 🌐 Tapestry Social Integration — NEW

Send.it integrates with [Tapestry](https://www.usetapestry.dev) — Solana's leading social protocol — to add an on-chain social layer:

| Feature | Description |
|---------|-------------|
| **Creator Profiles** | Wallet-linked profiles via Tapestry's unified social graph |
| **Follow Creators** | Follow token creators, get feed updates on new launches |
| **Launch Posts** | Each token launch generates a social post on the graph |
| **Likes & Comments** | Engage with token launches — like, comment, discuss |
| **Personal Feed** | See launches from creators you follow |

All social data is on-chain via Tapestry and composable with other Solana apps in the ecosystem.

```js
import { SendItSocial } from './lib/tapestry.mjs';
const social = new SendItSocial(TAPESTRY_API_KEY);

// Create a profile
await social.findOrCreateProfile(walletAddress, 'username', 'DeFi builder');

// Post a token launch to the social graph
await social.postTokenLaunch('username', { mint, name: 'MyToken', symbol: 'MTK' });

// Follow a creator
await social.follow('myProfile', 'creatorProfile');
```

---

## 🚀 Quick Start

### CLI Tool (Devnet)
Interact with Send.it on devnet directly from the command line:

```bash
cd scripts/

# Check wallet balance
node sendit-cli.mjs balance

# Initialize the platform
node sendit-cli.mjs init

# Create a new token launch
node sendit-cli.mjs create --name "MyToken" --symbol MTK

# Buy tokens (0.01 SOL)
node sendit-cli.mjs buy --mint <MINT_ADDRESS> --sol 0.01

# Sell tokens
node sendit-cli.mjs sell --mint <MINT_ADDRESS> --tokens 5000000

# View token launch info
node sendit-cli.mjs info --mint <MINT_ADDRESS>
```

Set `KEYPAIR_PATH` to your Solana keypair JSON, or place `deployer.json` in the working directory.

### Build with Solana Playground
The easiest way to build and deploy is via [Solana Playground](https://beta.solpg.io/):
1. Import the repo
2. Build & deploy to devnet in one click

### Build Locally
```bash
# Install dependencies
anchor build

# Run tests
anchor test

# Deploy to devnet
anchor deploy --provider.cluster devnet
```

---

## 📊 Stats

| Metric | Value |
|--------|-------|
| On-chain modules | **31** |
| Lines of Rust | **~16,000** |
| Lines of 5IVE DSL | **~6,000** |
| Bonding curve types | **3** (Linear, Exponential, Sigmoid) |
| Core instructions | **11** |
| PDA account types | **8+** |
| Frontend pages | **13** |
| Integration tests | **159** (5IVE) + **4,300+** (Anchor) |
| 5IVE bytecode | **25KB** |

---

## 🗺️ Roadmap

- [x] Core bonding curve engine (Linear, Exponential, Sigmoid)
- [x] Anti-snipe protection & bot blocklist
- [x] Creator vesting & rug protection
- [x] Auto-migration to Raydium
- [x] Leaderboard system
- [x] 31 feature modules (staking, perps, lending, social, governance, PYUSD…)
- [x] FairScale reputation integration
- [x] GitHub Pages deployment
- [x] Vercel frontend deployment
- [x] 5IVE VM port (63% code reduction)
- [x] Cross-module composition layer (6 bridge patterns)
- [x] Devnet deployment (`98Vxqk2dHjLsUb4svNaZWwVZxt9DZZwkRQZZNQmYRm1L`)
- [x] Token-2022 audit (SENDIT token)
- [x] PYUSD vault integration
- [x] Tapestry social integration (profiles, follows, feeds, likes)
- [x] Devnet CLI tool (`scripts/sendit-cli.mjs`)
- [ ] Security audit
- [ ] Mainnet launch (target: April 2026)
- [ ] senditsolana.io custom domain
- [ ] Full Raydium CPI integration
- [ ] Mobile app
- [ ] Multi-chain bridge activation
- [ ] DAO governance launch

---

## 🔗 Links

| | |
|---|---|
| **GitHub Pages** | [joebower1983-a11y.github.io/send_it](https://senditsolana.io/) |
| **Live App** | [senditsolana.io](https://senditsolana.io) |
| **5IVE VM Port** | [github.com/joebower1983-a11y/sendit-5ive](https://github.com/joebower1983-a11y/sendit-5ive) |
| **PYUSD Monitor** | [github.com/joebower1983-a11y/pyusd-monitor](https://github.com/joebower1983-a11y/pyusd-monitor) |
| **SENDIT Token** | [pump.fun](https://pump.fun/coin/F8qWTN8JfyDCvj4RoCHuvNMVbTV9XQksLuziA8PYpump) |
| **Discord** | [discord.gg/vKRTyG85](https://discord.gg/vKRTyG85) |
| **Telegram** | [t.me/+Xw4E2sJ0Z3Q5ZDYx](https://t.me/+Xw4E2sJ0Z3Q5ZDYx) |
| **Twitter** | [@SendItSolana420](https://x.com/SendItSolana420) |
| **Custom Domain** | [senditsolana.io](https://senditsolana.io) ✅ |

---

## 📄 License

MIT — see [LICENSE](LICENSE) for details.

---

<p align="center">
  <strong>Built on Solana ⚡ Powered by Anchor 🦀</strong>
</p>
