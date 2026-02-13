# 🚀 Send.it Telegram Bot

Telegram trading bot for **Send.it** — a Solana token launchpad with bonding curves.

## Features

- 🪙 **Launch tokens** — create new tokens with `/launch`
- 🛒 **Buy/Sell** — trade on bonding curves with quick-buy buttons (0.1 / 0.5 / 1 SOL)
- 📊 **Trending & New** — discover top tokens by volume or newest launches
- 💼 **Portfolio** — track holdings and PnL
- 🎓 **Graduation alerts** — get notified when tokens are about to migrate to Raydium
- 🎯 **Sniper mode** — auto-buy new launches with your configured amount
- ⚙️ **Settings** — adjust slippage, default buy amount, sniper config via inline buttons

## Setup

```bash
# 1. Clone and install
cd send_it/bot
npm install

# 2. Configure
cp .env.example .env
# Edit .env with your values:
#   BOT_TOKEN  — from @BotFather on Telegram
#   RPC_URL    — Solana RPC (Helius, Triton, etc.)
#   PROGRAM_ID — Send.it bonding curve program address

# 3. Run
npm start

# Dev mode (auto-restart on changes)
npm run dev
```

## Commands

| Command | Description |
|---------|-------------|
| `/start` | Welcome + wallet creation |
| `/launch <name> <symbol>` | Create a new token |
| `/buy <mint> [amount]` | Buy tokens (defaults to your configured amount) |
| `/sell <mint> [amount]` | Sell tokens (omit amount to sell all) |
| `/trending` | Top 10 tokens by 24h volume |
| `/new` | 10 most recently launched tokens |
| `/price <mint>` | Check current price & stats |
| `/portfolio` | Your holdings and PnL |
| `/settings` | Adjust slippage, buy amounts, sniper |

## Architecture

```
src/
├── index.js           # Entry point, bot setup
├── commands.js        # All slash command handlers
├── callbacks.js       # Inline button callback handlers
├── helpers.js         # Formatting utilities & keyboard builders
├── store.js           # JSON file-based data store
└── services/
    ├── solana.js       # Solana/web3 integration (wallet, buy, sell, price)
    ├── sniper.js       # Auto-buy engine for new launches
    └── alerts.js       # Graduation alert engine
```

## ⚠️ Important

The Solana integration (`services/solana.js`) contains **stub functions** for token operations. You must implement the actual program interactions:

- `launchToken()` — CPI to Send.it program to create token + bonding curve
- `buyToken()` — Buy instruction against the bonding curve
- `sellToken()` — Sell instruction against the bonding curve
- `getTokenPrice()` — Read bonding curve account state on-chain

The store uses a simple JSON file (`data/db.json`). For production, swap to Redis or a database.
