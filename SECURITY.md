# Security Policy

## Reporting a Vulnerability

If you discover a security vulnerability in Send.it, please report it responsibly:

- **Email:** sendit@proton.me
- **GitHub Issues:** [github.com/joebower1983-a11y/send_it/issues](https://github.com/joebower1983-a11y/send_it/issues)

Please do **not** publicly disclose vulnerabilities until we've had a chance to address them.

## Scope

- On-chain Anchor program (`programs/send_it/`)
- Slim devnet program (`programs/send_it_slim/`)
- 5IVE DSL port (`sendit-5ive/`)
- Frontend (`app/`)

## Current Status

- **Network:** Solana Devnet (not mainnet — no real funds at risk)
- **Audit:** Pre-audit. Mainnet deployment will follow a professional security audit.
- **Bug Bounty:** Not yet active. Will launch alongside mainnet.

## Known Limitations

- Program is deployed to devnet only — not intended for production use yet
- No formal audit has been conducted
- Token-2022 integration has been internally reviewed (see `TOKEN_2022_AUDIT.md`)
