# Send.it — PL_Genesis: Frontiers of Collaboration

## Project Summary

**Send.it** is an open-source, community-governed token launchpad on Solana with deep Filecoin/Storacha integration for permanent, decentralized token metadata storage.

Unlike existing launchpads where token metadata lives on centralized servers (and can disappear), Send.it stores all token metadata — images, descriptions, and properties — on Filecoin via Storacha. Every token launched through Send.it gets a content-addressed, immutable record that outlives any single server.

## Category

- **Track:** Crypto — "Upgrade Economies & Governance Systems"
- **Code Type:** Existing Code
- **Sponsor Challenges:** Storacha, Filecoin

## Problem

Token launchpads have three core problems:

1. **Centralized metadata** — Token images and descriptions hosted on AWS/Cloudflare. Server goes down? Your token has no face.
2. **Unfair launches** — Insiders snipe supply at launch. Regular users get the worst prices.
3. **No governance** — Token communities have no on-chain way to govern their protocol.

## Solution

Send.it solves all three:

### 1. Permanent Metadata (Storacha × Filecoin)
Every token launched on Send.it stores its metadata on Filecoin via Storacha:
- **Image upload** → Storacha → Filecoin (content-addressed, permanent)
- **Metadata JSON** (Metaplex-compatible) → Storacha → Filecoin
- **CID verification** — users can verify their token's metadata exists on Filecoin
- **Graduation archives** — when tokens migrate to AMM, the event is archived on Filecoin

The flow:
```
User creates token → Image uploaded to Storacha → CID returned
→ Metadata JSON (with image CID) uploaded to Storacha → Metadata CID returned  
→ Metadata URI (storacha.link/ipfs/{CID}) passed to on-chain createToken instruction
→ Metaplex metadata points to Filecoin-backed URI forever
```

### 2. Fair Launch Mechanics
- **Bonding curves** — price increases mathematically as supply is bought
- **No presales** — everyone starts at the same price
- **Creator fee caps** — max 10%, transparent and on-chain
- **Anti-snipe protection** — built into the curve mechanics

### 3. Full Governance Stack
- **Realms DAO** — on-chain proposals and voting
- **Governance token** — depositable for voting power
- **Proposal lifecycle** — create → vote → execute

## Architecture

```
┌─────────────────────────────────────────────┐
│                  Frontend                    │
│  (Vanilla JS, Wallet Standard, Vercel)      │
├─────────────────────────────────────────────┤
│              Storacha Upload API             │
│  (Vercel Serverless → @storacha/client)     │
│  - Image upload → CID                       │
│  - Metadata JSON → CID                      │
│  - Graduation archive → CID                 │
├──────────────┬──────────────────────────────┤
│   Filecoin   │      Solana Program          │
│  (Storage)   │  HTKq18cATdwCZb6XM66Mhn...  │
│  via IPFS    │  34 Anchor modules           │
│  gateway     │  11 core instructions        │
│              │  createToken, buy, sell,      │
│              │  stake, unstake, createPool,  │
│              │  swap, addLiquidity, ...      │
└──────────────┴──────────────────────────────┘
```

## Storacha/Filecoin Integration Details

### What we store on Filecoin:
| Data | Format | When |
|------|--------|------|
| Token images | PNG/JPG/GIF/WEBP | At token creation |
| Token metadata | JSON (Metaplex-compatible) | At token creation |
| Graduation records | JSON | When token migrates to AMM |

### How it works:
1. **Browser** → User fills in token form, uploads image
2. **Vercel API** (`/api/storacha-upload`) → Authenticates with Storacha using Ed25519 key + UCAN delegation proof
3. **@storacha/client** → Uploads to Storacha network
4. **Storacha** → Stores on IPFS + Filecoin (content-addressed, persistent)
5. **CID returned** → Used as Metaplex metadata URI on Solana
6. **Frontend** → Shows "Verified on Filecoin" badge with CID link

### Storacha Space:
- DID: `did:key:z6Mkv8HdSSik1Y8dXFrv21ysDf1UjLTQuTjmGNV4e549C3Hs`
- Account: Registered and delegated via `@storacha/client`

### Verification:
Users can verify any token's metadata by:
1. Clicking the "📦 Filecoin" badge on any token card
2. Following the CID link to `storacha.link/ipfs/{CID}`
3. The metadata JSON includes `storage.provider: "Storacha"` and `storage.network: "Filecoin"`

## Tech Stack

| Component | Technology |
|-----------|------------|
| Smart Contracts | Solana (Anchor), 34 modules, 16k+ lines of Rust |
| Decentralized Storage | Storacha → Filecoin/IPFS |
| Frontend | Vanilla JS, Wallet Standard API |
| Hosting | Vercel (serverless) |
| Governance | Realms DAO (SPL Governance) |
| Social | Tapestry Protocol |
| Testing | 4,300+ test cases |

## On-Chain Program

- **Program ID:** `HTKq18cATdwCZb6XM66Mhn8JWKCFTrZqH6zU1zip88Zx`
- **Network:** Solana Devnet (mainnet deployment in progress)
- **Instructions:** createToken, buy, sell, stake, unstake, createPool, swap, addLiquidity, removeLiquidity, initializePlatform, updatePlatform
- **Deployed:** Non-upgradeable (devnet), upgradeable (mainnet)

## Links

| Resource | URL |
|----------|-----|
| Live App | https://send-it-seven-sigma.vercel.app/app/ |
| GitHub | https://github.com/joebower1983-a11y/send_it |
| Website | https://senditsolana.io |
| Linktree | https://senditsolana.io/links.html |
| Whitepaper | https://github.com/joebower1983-a11y/send_it/blob/main/docs/WHITEPAPER.md |
| Solscan | https://solscan.io/account/HTKq18cATdwCZb6XM66Mhn8JWKCFTrZqH6zU1zip88Zx?cluster=devnet |
| DexScreener | https://dexscreener.com/solana/F8qWTN8JfyDCvj4RoCHuvNMVbTV9XQksLuziA8PYpump |
| Twitter | https://twitter.com/SendItSolana420 |
| Discord | https://discord.gg/vKRTyG85 |
| Telegram | https://t.me/+Xw4E2sJ0Z3Q5ZDYx |

## Team

- **Joseph Bower** — Founder & Developer
- **Dog** 🐕 — AI Engineering Assistant (OpenClaw)

## What's New for PL_Genesis

This submission builds on an existing codebase with significant new work:

1. **Full frontend rewrite** — Replaced mock data with real on-chain integration (wallet connect, token creation, explore, buy/sell, portfolio)
2. **Filecoin verification badges** — Visual proof of Storacha storage on every token
3. **Wallet Standard integration** — Modern wallet detection protocol
4. **Mainnet/devnet toggle** — Network switcher with persistent preference
5. **Storacha upload pipeline** — End-to-end: image → metadata → Filecoin → on-chain
6. **Graduation archival** — AMM migration events archived to Filecoin

## Demo Video Script (2-5 minutes)

1. **Intro** (30s) — "Send.it is a fair token launchpad on Solana where every token's metadata is permanently stored on Filecoin via Storacha"
2. **Connect wallet** (15s) — Show Wallet Standard picker, devnet balance
3. **Create token** (60s) — Fill form, upload image, show Storacha upload progress, token created on-chain
4. **Show Filecoin proof** (30s) — Click CID link, show metadata on storacha.link, point out storage fields
5. **Explore & Buy** (30s) — Browse tokens, click one, buy on bonding curve
6. **Architecture** (30s) — Quick diagram of Solana ↔ Storacha ↔ Filecoin flow
7. **Wrap up** (15s) — "No insiders. No presales. Permanent metadata. Just send it."
