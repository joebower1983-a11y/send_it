# Send.it Perpetual Exchange — Design Document

> Percolator-inspired on-chain perpetual futures for graduated tokens on Solana.

## Overview

The Send.it perps module enables leveraged perpetual trading on tokens **after** they graduate from the bonding curve and migrate to Raydium. This creates a complete lifecycle: launch → bonding curve → Raydium graduation → perpetual futures.

## Architecture

```
┌─────────────────────────────────────────────────┐
│                  Send.it Perps                   │
├──────────┬──────────┬───────────┬───────────────┤
│ Position │  Order   │  Funding  │  Liquidation  │
│ Manager  │  Book    │  Engine   │  Engine       │
├──────────┴──────────┴───────────┴───────────────┤
│              Cross-Margin System                 │
├─────────────────────────────────────────────────┤
│          Oracle (Raydium Pool TWAP)              │
├──────────┬──────────────────────┬───────────────┤
│ Insurance│   Fee Distribution   │  SolForge     │
│ Fund     │                      │  Burn Vault   │
└──────────┴──────────────────────┴───────────────┘
```

### Key Accounts

| Account | Purpose |
|---------|---------|
| `PerpMarket` | Per-token market config and state (OI, funding, TWAP) |
| `OrderBook` | On-chain bid/ask arrays with price-time priority |
| `Position` | Individual leveraged position (per user per market) |
| `UserMarginAccount` | Cross-margin collateral pool per user |
| `InsuranceFund` | Per-market fund absorbing liquidation shortfalls |

## Graduation Flow Integration

1. Token completes bonding curve on Send.it
2. Liquidity migrates to Raydium (graduation)
3. Authority calls `initialize_perp_market` with the Raydium pool as oracle
4. Traders create margin accounts, deposit collateral, and trade perps

**Requirement:** `initialize_perp_market` should validate that the token has a live Raydium pool (the `raydium_pool` account is checked on-chain).

## Position Management

### Opening

```
open_position(market, side, size, leverage, collateral)
```

- Validates leverage ≤ `max_leverage` (default 20x)
- Checks open interest caps and position size limits
- Deducts collateral from `UserMarginAccount`
- Entry price = current mark price
- Charges taker fee on notional value

### Closing

Full close returns `collateral ± PnL - fees` to margin account. Partial close (`decrease_position`) returns proportional collateral + PnL on the closed portion.

### Cross-Margin

All positions share a single `UserMarginAccount` per user. Collateral is pooled — gains on one position offset losses on another. The `open_positions` counter tracks active positions.

## Order Book

Simplified on-chain CLOB with two sorted `Vec<OrderEntry>`:

- **Bids**: descending by price (best bid first)
- **Asks**: ascending by price (best ask first)
- Max 256 orders per side
- Price-time priority matching

### Order Types

- **Limit**: Rests on book at specified price
- **Market**: Fills against resting orders (price=0, crosses immediately)

### Matching (`match_orders`)

Permissionless crank instruction. Anyone can call it to earn priority fees:

1. Check if best bid ≥ best ask (or either is market order)
2. Self-trade prevention: remove newer order if same owner
3. Fill at resting order's price
4. Update mark price and TWAP on each fill
5. Remove fully filled orders

## Oracle & TWAP

### Index Price

Derived from the Raydium AMM pool spot price. Updated via `update_oracle_price` crank that reads the pool's token reserves and computes `reserve_quote / reserve_base`.

### Mark Price

Updated on each order match. Represents the last traded price on the perps order book.

### TWAP

Time-weighted average price over a 1-hour window, stored as up to 60 samples. Used for funding rate calculation to reduce manipulation.

```
TWAP = Σ(sample_price) / count(samples in window)
```

## Funding Rate

Periodic payments between longs and shorts to anchor mark price to index:

```
funding_rate = clamp((mark_price - index_price) / index_price, -0.1%, +0.1%)
```

- **Positive rate** → longs pay shorts (mark > index, too bullish)
- **Negative rate** → shorts pay longs (mark < index, too bearish)
- Default interval: 1 hour
- Cumulative tracking: each position stores its `last_cumulative_funding` at open/settlement

### Funding Payment

```
payment = position_size × (cumulative_funding_now - cumulative_funding_at_open) / PRECISION
```

Settled on close or liquidation. Applied as adjustment to PnL.

## Fixed-Point Math

All prices, rates, and ratios use **6-decimal fixed point** (PRECISION = 10^6):

| Value | Representation |
|-------|---------------|
| $1.00 | 1,000,000 |
| 0.06% fee | 600 |
| 2.5% margin | 25,000 |
| 20x leverage | 20 (integer) |

### Key Formulas

**Notional Value:**
```
notional = size × price / PRECISION
```

**Unrealized PnL:**
```
Long:  (mark_price - entry_price) × size / PRECISION
Short: (entry_price - mark_price) × size / PRECISION
```

**Margin Ratio:**
```
margin_ratio = (collateral + unrealized_pnl) / notional
```

**Liquidation Price:**
```
Long:  entry_price × (1 - 1/leverage + maintenance_margin)
Short: entry_price × (1 + 1/leverage - maintenance_margin)
```

## Liquidation

### Trigger

Position is liquidatable when `margin_ratio < maintenance_margin` (default 2.5%).

### Process

1. Anyone calls `liquidate_position(position, size)`
2. Verify margin ratio is below maintenance
3. Settle pending funding
4. Calculate liquidation fee (default 1% of liquidated notional)
5. Liquidator receives the fee
6. If collateral + PnL < 0 (shortfall), insurance fund covers
7. Position size reduced (partial) or account closed (full)

### Partial Liquidation

Callers can specify a `liquidation_size` less than the full position. This brings the position back toward healthy margin without fully closing it.

## Fee Structure

| Fee | Default | Description |
|-----|---------|-------------|
| Maker | 0.02% | Orders that add liquidity (resting on book) |
| Taker | 0.06% | Orders that remove liquidity (crossing) |
| Liquidation | 1.00% | Paid to liquidator on liquidated notional |

### Fee Distribution

```
Trading Fees
├── 30% → Insurance Fund
├── 20% → SolForge Vault (token burn)
└── 50% → Protocol Revenue
```

The SolForge vault integration creates a deflationary flywheel: more trading → more fees → more burns → token appreciation.

## Insurance Fund

Per-market fund that:

- Receives 30% of all trading fees
- Absorbs negative PnL from liquidations (when collateral doesn't cover losses)
- Tracks total deposits and payouts for transparency

If the insurance fund is depleted, the system would need socialized loss (not yet implemented — a future upgrade path).

## Risk Management

### Circuit Breakers

Orders and positions are rejected if price deviates > 10% from oracle index price. This prevents manipulation via stale oracle or flash crashes.

### Open Interest Caps

Per-market configurable maximum total open interest. Prevents excessive risk concentration.

### Position Size Limits

Per-user maximum position size to prevent single-actor domination.

### Leverage Cap

Hard maximum of 20x leverage. Configurable per market (could be lower for volatile tokens).

## Crank Instructions

Three permissionless crank instructions anyone can call:

| Instruction | Purpose | Frequency |
|------------|---------|-----------|
| `match_orders` | Match crossing bid/ask orders | Every block (if orders exist) |
| `update_funding_rate` | Settle funding payments | Every funding interval (1h) |
| `update_oracle_price` | Refresh index price from Raydium | Every few seconds |

Crankers are incentivized by Solana priority fees and (in the case of liquidators) liquidation fees.

## Security Considerations

- All math uses checked arithmetic to prevent overflow
- PDA seeds prevent account collision
- `has_one` constraints validate account relationships
- Authority-gated admin functions (pause, config changes)
- Self-trade prevention on order matching
- Oracle staleness should be checked (TODO: add max age check)

## Future Enhancements

- **ADL (Auto-Deleveraging)**: When insurance fund is empty, deleverage profitable positions
- **Socialized Loss**: Distribute losses across all positions if insurance depleted
- **Advanced Order Types**: Stop-loss, take-profit, trailing stop
- **Multi-collateral**: Accept multiple tokens as collateral with haircuts
- **On-chain Pyth/Switchboard oracle**: More robust oracle beyond Raydium TWAP
- **LP Vault**: Allow passive LPs to provide liquidity and earn fees
- **Referral System**: Fee rebates for referrers
