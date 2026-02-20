# Jupiter Integration — Send.Swap AMM

## How It Works
Jupiter indexes AMMs via the `jupiter-amm-interface` Rust crate. You implement the `Amm` trait and submit a PR or reach out on their Discord `#developer-support` channel.

**Repo:** https://github.com/jup-ag/jupiter-amm-interface
**Discord:** https://discord.gg/jup → #developer-support

## Steps to Get Indexed

1. **Implement `jupiter-amm-interface`** — Create a Rust crate that implements the `Amm` trait for Send.Swap's constant-product pools
2. **Test on devnet** — Verify Jupiter can route through Send.Swap pools
3. **Submit to Jupiter team** — PR to their repo or via Discord
4. **Go live on mainnet** — Once audited and deployed

## Technical Notes
- Send.Swap is a standard x*y=k AMM (like Raydium v1)
- 1% swap fee (0.3% LP, 0.7% protocol)
- Pool accounts are 122 bytes (AmmPool struct)
- Instruction: `swap(sol_amount: u64, token_amount: u64)`
- PDAs: `[amm_pool, mint]`, `[pool_sol_vault, mint]`

## Discord Message (Ready to Send)

```
Hey Jupiter team! 👋

I'm building Send.it — a token launchpad on Solana with 31 on-chain Anchor modules. Our AMM ("Send.Swap") graduates tokens from bonding curves into constant-product (x*y=k) pools.

We're live on devnet with 3 active pools and heading to mainnet audit soon. Would love to implement the jupiter-amm-interface to get Send.Swap pools indexed in the Jupiter aggregator.

Quick stats:
- 31 on-chain modules (16k lines Rust, 0 Sec3 vulnerabilities)
- Standard x*y=k AMM with 1% fee (0.3% LP, 0.7% protocol)
- Program: HTKq18cATdwCZb6XM66Mhn8JWKCFTrZqH6zU1zip88Zx (devnet)
- Demo: senditsolana.io
- GitHub: github.com/joebower1983-a11y/send_it

Questions:
1. Is the jupiter-amm-interface crate still the right path for new AMM integrations?
2. Any minimum TVL/volume requirements before you'll index pools?
3. Do you index devnet pools for testing, or mainnet only?

Happy to share our IDL or any technical details. Thanks! 🚀
```

## Twitter DM (Backup)

```
Hey @JupiterExchange! Building Send.it — 31-module token launchpad on Solana with Send.Swap AMM (x*y=k). Live on devnet, heading to mainnet.

Would love to get Send.Swap pools indexed in Jupiter. Already looking at jupiter-amm-interface. Who should I talk to? 🚀

senditsolana.io
```

## Status
- [ ] Join Jupiter Discord
- [ ] Post in #developer-support
- [ ] Start implementing jupiter-amm-interface crate
- [ ] Test integration on devnet
- [ ] Submit for mainnet indexing
