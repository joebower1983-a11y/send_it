# Solana Foundation Grant Application — Send.it

> **Submit at:** https://share.hsforms.com/1GE1hYdApQGaDiCgaiWMXHA5lohw
>
> Copy each section below into the corresponding form field.

---

## Project Name

Send.it

## Applicant Name

Joseph Bower

## Contact Email

*(Fill in your email)*

## Project Links

| Field | Value |
|-------|-------|
| GitHub | https://github.com/joebower1983-a11y/send_it |
| Website | https://itsolana.io / https://itsolana.io |
| Twitter | [@SendItSolana420](https://twitter.com/SendItSolana420) |
| Discord | https://discord.gg/vKRTyG85 |
| Telegram Bot | @senditsol42069bot |

---

## 1. Project Overview

Send.it is an open-source token launchpad built on Solana that prioritises safety, fairness, and composability. While existing launchpads have driven explosive growth in Solana's token ecosystem, they have also normalised rug pulls, bot-dominated launches, and opaque insider allocations. Send.it addresses these problems at the protocol level — not through terms of service, but through enforceable on-chain mechanics: locked liquidity, vesting schedules, anti-snipe protection, and fair launch curves with zero insider allocation.

The entire project — over 20 Anchor modules covering launch mechanics, holder rewards, perpetual trading, and cross-chain bridging — is fully open source under a permissive licence. Send.it is designed not just as a product, but as **public infrastructure**: a composable SDK that any developer can fork, extend, or integrate into their own application. The goal is to raise the floor for what "safe" looks like on Solana, the same way OpenBook raised the floor for on-chain order books.

Send.it also introduces SolForge burn integration, creating deflationary pressure on SOL with every launch. Post-graduation, tokens gain access to a perpetual trading module and a planned cross-chain bridge — giving projects launched through Send.it a genuine path beyond day-one speculation. This is infrastructure for builders, not just traders.

---

## 2. Public Good Justification

Send.it is a public good for the Solana ecosystem in three concrete ways:

**Safer launches for everyone.** Anti-snipe protection ensures that bots cannot front-run human participants during token launches. Locked liquidity and vesting schedules are enforced on-chain — not by a dashboard toggle that a deployer can undo. These protections are embedded in the program logic and are therefore trustless. Every project launched through Send.it inherits these guarantees automatically.

**Open-source composable infrastructure.** The full codebase — 20+ Anchor modules — is public at [github.com/joebower1983-a11y/send_it](https://github.com/joebower1983-a11y/send_it). This includes the launch curve logic, the holder rewards system, the perps module, and the cross-chain bridge design. Any team building on Solana can fork these modules, integrate them via the SDK, or study the architecture. This is not a walled garden; it is a library.

**Ecosystem-aligned tokenomics.** SolForge burn integration means that every launch on Send.it creates deflationary pressure on SOL itself. The holder rewards system incentivises long-term participation rather than dump-and-exit behaviour. Fair launch mechanics — no team allocations, no pre-sales, no insider advantages — directly combat the reputation damage that rug pulls inflict on the broader Solana brand.

The Solana ecosystem's rapid memecoin growth has created real demand for safer infrastructure. Send.it fills that gap as a public, forkable, open-source protocol rather than a proprietary product.

---

## 3. Technical Architecture

Send.it is built entirely in **Anchor/Rust** for the on-chain programs and **TypeScript/React** for the frontend and bot interfaces. The architecture is modular by design:

- **Core Launch Program** — Bonding curve logic, anti-snipe timing guards, locked liquidity vault, and vesting schedule enforcement. All parameters are configurable per launch but safety constraints (lock duration minimums, snipe windows) are enforced at the program level.
- **Holder Rewards Module** — On-chain reward distribution based on holding duration and amount, funded by configurable fee splits from trades.
- **Perps Module** — Perpetual trading for graduated tokens, using an oracle-fed mark price and on-chain funding rate calculation.
- **Cross-Chain Bridge Module** — Lock-and-mint bridge design targeting EVM chains, with a relayer architecture documented in the repo.
- **SDK** — TypeScript SDK wrapping all program interactions, designed for third-party integration.
- **Bot Layer** — Telegram and Discord bots providing launch notifications, portfolio tracking, and direct swap execution.

**DEX Integration:** Post-bonding-curve graduation routes liquidity to Raydium, leveraging Solana's existing DEX infrastructure rather than building a competing AMM.

All code is available at: https://github.com/joebower1983-a11y/send_it

---

## 4. Team

Send.it is built by **Joseph Bower** — a solo, self-taught developer. The entire codebase, from the Anchor programs to the frontend to the Telegram bot, is the work of one person. This is not presented as a weakness but as evidence of commitment and execution speed. The GitHub history speaks for itself.

Joe's background is in learning by building. The Send.it repo demonstrates working knowledge of Rust/Anchor program development, TypeScript frontend and bot development, Solana program architecture, and DEX integration patterns. The project has progressed from concept to a functional multi-module codebase without external funding.

Additional contributors and auditors will be brought on as the project scales, funded in part by this grant. The open-source nature of the project also means the community can contribute directly.

- **GitHub:** https://github.com/joebower1983-a11y

---

## 5. Milestones

| Milestone | Timeline | Deliverables | Verification |
|-----------|----------|--------------|--------------|
| **M1 — Mainnet Deployment** | Month 1 | Core launch program deployed to mainnet; full devnet test suite passing; deployment scripts documented | Program ID on mainnet; passing CI; deployment guide in repo |
| **M2 — Frontend & Bots Live** | Month 2 | Production frontend wired to live program; Telegram bot (@senditsol42069bot) and Discord bot operational; SDK v1.0 published to npm | Live site at itsolana.io; working bot demos; npm package |
| **M3 — Perps & Rewards** | Month 3–4 | Perpetual trading module live for graduated tokens; holder rewards distribution operational; documentation complete | On-chain perps trades; reward claim transactions; SDK docs |
| **M4 — Bridge & Audit** | Month 5–6 | Cross-chain bridge module deployed (testnet minimum); full third-party security audit completed and published; audit remediation merged | Audit report (public); bridge testnet demo; remediation commits |

---

## 6. Budget

**Total Request: $12,500 USD equivalent**

| Category | Amount | Justification |
|----------|--------|---------------|
| Program Deployment & On-chain Costs | $500 | Mainnet deployments, account rent, testing transactions |
| RPC Infrastructure | $1,500 | Dedicated RPC node (6 months) for reliable frontend and bot operation |
| Security Audit | $5,000 | Third-party audit of core launch program and rewards module (e.g., via Sec3 or OtterSec community tier) |
| Marketing & Community Building | $1,500 | Community incentives, content creation, launch event costs |
| Developer Time | $4,000 | 6 months of focused development time (supplementing self-funded work) |
| **Total** | **$12,500** | |

> **Note:** Developer time is deliberately modest — the majority of the budget is directed at the security audit and infrastructure, which are the highest-leverage uses of grant funding. Joe is committed to building Send.it regardless; this grant accelerates the timeline and funds the audit that makes the protocol trustworthy.

---

## 7. Why Solana

Send.it is built on Solana because the technical requirements of a fair token launchpad align precisely with Solana's strengths:

- **Sub-second finality** makes anti-snipe protection meaningful. On a chain with 12-second blocks, timing-based fairness mechanisms are imprecise. On Solana, a 400ms slot time allows fine-grained launch windows that genuinely protect human participants.
- **Low transaction fees** make micro-trades viable. Token launches attract thousands of small participants — $5, $10, $50 trades. On Ethereum L1, gas costs make this uneconomical. On Solana, a $0.001 transaction fee means the launchpad is accessible to everyone.
- **Existing DEX infrastructure** — specifically Raydium — provides a natural graduation path. Send.it doesn't need to build an AMM; it routes graduated liquidity into Solana's existing, battle-tested DEX ecosystem.
- **Cultural fit.** Solana has the most active memecoin and token launch community in crypto. That community currently relies on platforms with minimal safety guarantees. Send.it meets users where they already are, with infrastructure that protects them.

Building this on any other chain would mean slower finality, higher costs, and a smaller addressable community. Solana is the only chain where this design works as intended.

---

## 8. Competitive Advantage

**vs. pump.fun:**
pump.fun popularised the bonding curve launchpad model but offers no rug protection, no liquidity locking, and no anti-snipe mechanisms. Launches are routinely dominated by bots, and deployers can (and do) abandon projects immediately after graduation. Send.it enforces safety at the protocol level — locked liquidity, vesting, and anti-snipe are not optional features; they are program constraints. Additionally, Send.it is fully open source; pump.fun is proprietary.

**vs. bonk.fun and similar:**
Most competing launchpads are closed-source products operated by a single team. They cannot be forked, extended, or audited by the community. Send.it's 20+ open-source Anchor modules and published SDK mean that other developers can build on this infrastructure. The value accrues to the ecosystem, not to a single operator.

**What Send.it adds that no competitor offers:**
- SolForge burn integration (deflationary SOL pressure per launch)
- Post-graduation perpetual trading module
- Cross-chain bridge for graduated tokens
- Holder rewards system incentivising long-term participation
- Full SDK for third-party integration
- Fully open-source, forkable architecture

Send.it is not competing to be the "next pump.fun." It is building the open-source safety layer that the Solana token launch ecosystem currently lacks.

---

## Form Field Quick Reference

*When filling out the HubSpot form, map sections as follows:*

| Form Field | Content Source |
|------------|---------------|
| Project Name | "Send.it" |
| Project Description | Section 1 (Project Overview) |
| How does this benefit the Solana ecosystem? | Section 2 (Public Good Justification) |
| Technical Details | Section 3 (Technical Architecture) |
| Team Information | Section 4 (Team) |
| Milestones | Section 5 (Milestones) |
| Budget Breakdown | Section 6 (Budget) |
| Why Solana? | Section 7 (Why Solana) |
| Additional Information | Section 8 (Competitive Advantage) |
| GitHub URL | https://github.com/joebower1983-a11y/send_it |
| Website | https://itsolana.io |
| Twitter | @SendItSolana420 |
| Grant Amount Requested | $12,500 USD |

---

*Document prepared February 2026. Ready to copy into the Solana Foundation grant application form.*
