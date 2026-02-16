use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer};
use std::collections::BinaryHeap;

// ============================================================================
// Constants
// ============================================================================

/// Fixed-point precision: 10^6 (all prices, rates, and margins use this)
pub const PRECISION: u128 = 1_000_000;
/// Maximum leverage: 20x
pub const MAX_LEVERAGE: u64 = 20;
/// Minimum maintenance margin ratio: 2.5% (25_000 / PRECISION)
pub const DEFAULT_MAINTENANCE_MARGIN: u64 = 25_000; // 2.5%
/// Default liquidation fee: 1% to liquidator
pub const DEFAULT_LIQUIDATION_FEE: u64 = 10_000; // 1%
/// Default maker fee: 0.02%
pub const DEFAULT_MAKER_FEE: u64 = 200; // 0.02%
/// Default taker fee: 0.06%
pub const DEFAULT_TAKER_FEE: u64 = 600; // 0.06%
/// Fee split to insurance fund: 30%
pub const INSURANCE_FEE_SHARE: u64 = 300_000; // 30%
/// Fee split to SolForge vault (burn): 20%
pub const SOLFORGE_FEE_SHARE: u64 = 200_000; // 20%
/// Max orders per order book side
pub const MAX_ORDERS: usize = 256;
/// TWAP sample window
pub const TWAP_WINDOW: u64 = 3600; // 1 hour in seconds
/// Max TWAP samples stored
pub const MAX_TWAP_SAMPLES: usize = 60;
/// Default funding rate interval: 1 hour
pub const DEFAULT_FUNDING_INTERVAL: i64 = 3600;
/// Max funding rate per interval: 0.1%
pub const MAX_FUNDING_RATE: i128 = 1_000; // 0.1% of PRECISION
/// Circuit breaker: max 10% price deviation from oracle
pub const PRICE_BAND_BPS: u64 = 1000; // 10%

// ============================================================================
// Error Codes
// ============================================================================

#[error_code]
pub enum PerpError {
    #[msg("Token has not graduated to Raydium yet")]
    TokenNotGraduated,
    #[msg("Leverage exceeds maximum allowed")]
    ExcessiveLeverage,
    #[msg("Insufficient collateral for position")]
    InsufficientCollateral,
    #[msg("Position not found")]
    PositionNotFound,
    #[msg("Position is not liquidatable")]
    NotLiquidatable,
    #[msg("Order book is full")]
    OrderBookFull,
    #[msg("Order not found")]
    OrderNotFound,
    #[msg("Invalid order price")]
    InvalidPrice,
    #[msg("Invalid order size")]
    InvalidSize,
    #[msg("Arithmetic overflow")]
    MathOverflow,
    #[msg("Market is paused")]
    MarketPaused,
    #[msg("Open interest cap exceeded")]
    OpenInterestCapExceeded,
    #[msg("Position size limit exceeded")]
    PositionSizeLimitExceeded,
    #[msg("Price outside circuit breaker band")]
    CircuitBreakerTriggered,
    #[msg("Funding interval not elapsed")]
    FundingIntervalNotElapsed,
    #[msg("Unauthorized")]
    Unauthorized,
    #[msg("Insufficient insurance fund")]
    InsufficientInsuranceFund,
    #[msg("Invalid partial liquidation amount")]
    InvalidPartialLiquidation,
    #[msg("No orders to match")]
    NoOrdersToMatch,
    #[msg("Self-trade prevention")]
    SelfTrade,
    #[msg("Stale oracle price")]
    StaleOracle,
    #[msg("Invalid margin account")]
    InvalidMarginAccount,
    #[msg("Invalid oracle account — does not match market")]
    InvalidOracleAccount,
}

// ============================================================================
// Account Structs
// ============================================================================

#[account]
#[derive(Default)]
pub struct PerpMarket {
    /// Bump seed
    pub bump: u8,
    /// Authority (program or DAO)
    pub authority: Pubkey,
    /// Token mint (the graduated token)
    pub token_mint: Pubkey,
    /// Collateral mint (USDC or SOL)
    pub collateral_mint: Pubkey,
    /// Raydium AMM pool used as oracle
    pub raydium_pool: Pubkey,
    /// SolForge vault for fee burns
    pub solforge_vault: Pubkey,
    /// Insurance fund account
    pub insurance_fund: Pubkey,
    /// Collateral vault (holds all deposited collateral)
    pub collateral_vault: Pubkey,
    /// Order book account
    pub order_book: Pubkey,

    // --- Configuration ---
    /// Max leverage (stored as integer, e.g. 20 = 20x)
    pub max_leverage: u64,
    /// Maintenance margin ratio (PRECISION-based, e.g. 25_000 = 2.5%)
    pub maintenance_margin: u64,
    /// Liquidation fee (PRECISION-based)
    pub liquidation_fee: u64,
    /// Maker fee (PRECISION-based)
    pub maker_fee: u64,
    /// Taker fee (PRECISION-based)
    pub taker_fee: u64,
    /// Funding rate interval in seconds
    pub funding_interval: i64,
    /// Max open interest (in base token units)
    pub max_open_interest: u64,
    /// Max position size per user
    pub max_position_size: u64,

    // --- State ---
    /// Current mark price (PRECISION-based)
    pub mark_price: u64,
    /// Current index price from oracle (PRECISION-based)
    pub index_price: u64,
    /// Total long open interest (base units)
    pub long_open_interest: u64,
    /// Total short open interest (base units)
    pub short_open_interest: u64,
    /// Cumulative funding rate for longs (signed, PRECISION-based)
    pub cumulative_funding_long: i128,
    /// Cumulative funding rate for shorts (signed, PRECISION-based)
    pub cumulative_funding_short: i128,
    /// Last funding update timestamp
    pub last_funding_time: i64,
    /// Is market paused
    pub paused: bool,
    /// Market creation timestamp
    pub created_at: i64,

    // --- TWAP ---
    /// TWAP samples: (timestamp, price)
    pub twap_samples: Vec<TwapSample>,

    /// Reserved for future use
    pub _reserved: [u8; 128],
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Default)]
pub struct TwapSample {
    pub timestamp: i64,
    pub price: u64,
}

#[account]
#[derive(Default)]
pub struct OrderBook {
    pub bump: u8,
    pub market: Pubkey,
    /// Bids sorted by price descending (best bid first)
    pub bids: Vec<OrderEntry>,
    /// Asks sorted by price ascending (best ask first)
    pub asks: Vec<OrderEntry>,
    /// Monotonic order ID counter
    pub next_order_id: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Default, PartialEq)]
pub struct OrderEntry {
    pub order_id: u64,
    pub owner: Pubkey,
    pub price: u64,       // PRECISION-based
    pub size: u64,         // base token units
    pub remaining: u64,    // remaining unfilled size
    pub side: Side,
    pub order_type: OrderType,
    pub timestamp: i64,
    pub margin_account: Pubkey,
}

#[account]
#[derive(Default)]
pub struct UserMarginAccount {
    pub bump: u8,
    pub owner: Pubkey,
    /// Total deposited collateral (in collateral token units)
    pub collateral: u64,
    /// Number of open positions (for cross-margin)
    pub open_positions: u16,
    /// Cumulative realized PnL
    pub realized_pnl: i64,
    /// Created timestamp
    pub created_at: i64,
    pub _reserved: [u8; 64],
}

#[account]
pub struct Position {
    pub bump: u8,
    pub market: Pubkey,
    pub owner: Pubkey,
    pub margin_account: Pubkey,
    /// Long or Short
    pub side: Side,
    /// Position size in base units
    pub size: u64,
    /// Entry price (PRECISION-based)
    pub entry_price: u64,
    /// Collateral allocated to this position
    pub collateral: u64,
    /// Leverage used
    pub leverage: u64,
    /// Cumulative funding at time of last settlement
    pub last_cumulative_funding: i128,
    /// Unrealized funding payments owed
    pub pending_funding: i64,
    /// Timestamp opened
    pub opened_at: i64,
    /// Last updated timestamp
    pub updated_at: i64,
    pub _reserved: [u8; 64],
}

#[account]
#[derive(Default)]
pub struct InsuranceFund {
    pub bump: u8,
    pub market: Pubkey,
    pub vault: Pubkey,
    /// Total balance in collateral units
    pub balance: u64,
    /// Total payouts made
    pub total_payouts: u64,
    /// Total deposits received
    pub total_deposits: u64,
}

// ============================================================================
// Enums
// ============================================================================

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Default)]
pub enum Side {
    #[default]
    Long,
    Short,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Default)]
pub enum OrderType {
    #[default]
    Limit,
    Market,
}

// ============================================================================
// Events
// ============================================================================

#[event]
pub struct MarketCreated {
    pub market: Pubkey,
    pub token_mint: Pubkey,
    pub max_leverage: u64,
    pub timestamp: i64,
}

#[event]
pub struct PositionOpened {
    pub market: Pubkey,
    pub owner: Pubkey,
    pub side: Side,
    pub size: u64,
    pub entry_price: u64,
    pub leverage: u64,
    pub collateral: u64,
}

#[event]
pub struct PositionClosed {
    pub market: Pubkey,
    pub owner: Pubkey,
    pub side: Side,
    pub size: u64,
    pub exit_price: u64,
    pub realized_pnl: i64,
}

#[event]
pub struct PositionLiquidated {
    pub market: Pubkey,
    pub owner: Pubkey,
    pub liquidator: Pubkey,
    pub size_liquidated: u64,
    pub price: u64,
    pub liquidation_fee: u64,
}

#[event]
pub struct OrderPlaced {
    pub market: Pubkey,
    pub order_id: u64,
    pub owner: Pubkey,
    pub side: Side,
    pub order_type: OrderType,
    pub price: u64,
    pub size: u64,
}

#[event]
pub struct OrderCancelled {
    pub market: Pubkey,
    pub order_id: u64,
    pub owner: Pubkey,
}

#[event]
pub struct OrderMatched {
    pub market: Pubkey,
    pub bid_order_id: u64,
    pub ask_order_id: u64,
    pub price: u64,
    pub size: u64,
}

#[event]
pub struct FundingRateUpdated {
    pub market: Pubkey,
    pub funding_rate: i128,
    pub mark_price: u64,
    pub index_price: u64,
    pub timestamp: i64,
}

#[event]
pub struct CollateralDeposited {
    pub margin_account: Pubkey,
    pub amount: u64,
}

#[event]
pub struct CollateralWithdrawn {
    pub margin_account: Pubkey,
    pub amount: u64,
}

// ============================================================================
// Fixed-Point Math Helpers
// ============================================================================

pub mod math {
    use super::*;

    /// Multiply two PRECISION-based values: (a * b) / PRECISION
    pub fn mul_precision(a: u128, b: u128) -> Result<u128> {
        a.checked_mul(b)
            .and_then(|v| v.checked_div(PRECISION))
            .ok_or_else(|| error!(PerpError::MathOverflow))
    }

    /// Divide two PRECISION-based values: (a * PRECISION) / b
    pub fn div_precision(a: u128, b: u128) -> Result<u128> {
        if b == 0 {
            return err!(PerpError::MathOverflow);
        }
        a.checked_mul(PRECISION)
            .and_then(|v| v.checked_div(b))
            .ok_or_else(|| error!(PerpError::MathOverflow))
    }

    /// Calculate unrealized PnL for a position
    /// Long: (mark_price - entry_price) * size / PRECISION
    /// Short: (entry_price - mark_price) * size / PRECISION
    pub fn calc_unrealized_pnl(
        side: &Side,
        entry_price: u64,
        mark_price: u64,
        size: u64,
    ) -> Result<i64> {
        let pnl = match side {
            Side::Long => (mark_price as i128) - (entry_price as i128),
            Side::Short => (entry_price as i128) - (mark_price as i128),
        };
        let pnl_scaled = pnl
            .checked_mul(size as i128)
            .and_then(|v| v.checked_div(PRECISION as i128))
            .ok_or_else(|| error!(PerpError::MathOverflow))?;
        Ok(pnl_scaled as i64)
    }

    /// Calculate margin ratio = (collateral + unrealized_pnl) / notional
    /// Returns PRECISION-based ratio
    pub fn calc_margin_ratio(
        collateral: u64,
        unrealized_pnl: i64,
        notional: u64,
    ) -> Result<u64> {
        if notional == 0 {
            return Ok(u64::MAX);
        }
        let equity = (collateral as i128) + (unrealized_pnl as i128);
        if equity <= 0 {
            return Ok(0);
        }
        let ratio = (equity as u128)
            .checked_mul(PRECISION)
            .and_then(|v| v.checked_div(notional as u128))
            .ok_or_else(|| error!(PerpError::MathOverflow))?;
        Ok(ratio as u64)
    }

    /// Calculate notional value = size * price / PRECISION
    pub fn calc_notional(size: u64, price: u64) -> Result<u64> {
        let n = (size as u128)
            .checked_mul(price as u128)
            .and_then(|v| v.checked_div(PRECISION))
            .ok_or_else(|| error!(PerpError::MathOverflow))?;
        Ok(n as u64)
    }

    /// Calculate funding payment for a position
    /// payment = size * (cumulative_funding_now - cumulative_funding_at_open) / PRECISION
    pub fn calc_funding_payment(
        size: u64,
        cumulative_now: i128,
        cumulative_at_open: i128,
    ) -> Result<i64> {
        let delta = cumulative_now
            .checked_sub(cumulative_at_open)
            .ok_or_else(|| error!(PerpError::MathOverflow))?;
        let payment = delta
            .checked_mul(size as i128)
            .and_then(|v| v.checked_div(PRECISION as i128))
            .ok_or_else(|| error!(PerpError::MathOverflow))?;
        Ok(payment as i64)
    }

    /// Calculate fee amount = notional * fee_rate / PRECISION
    pub fn calc_fee(notional: u64, fee_rate: u64) -> Result<u64> {
        let fee = (notional as u128)
            .checked_mul(fee_rate as u128)
            .and_then(|v| v.checked_div(PRECISION))
            .ok_or_else(|| error!(PerpError::MathOverflow))?;
        Ok(fee as u64)
    }

    /// Calculate liquidation price for a position
    /// Long: entry_price * (1 - 1/leverage + maintenance_margin)
    /// Short: entry_price * (1 + 1/leverage - maintenance_margin)
    pub fn calc_liquidation_price(
        entry_price: u64,
        leverage: u64,
        maintenance_margin: u64,
        side: &Side,
    ) -> Result<u64> {
        let one = PRECISION;
        let inv_lev = PRECISION / (leverage as u128);
        let mm = maintenance_margin as u128;

        let factor = match side {
            Side::Long => {
                let f = one.checked_sub(inv_lev)
                    .and_then(|v| v.checked_add(mm))
                    .ok_or_else(|| error!(PerpError::MathOverflow))?;
                f
            }
            Side::Short => {
                let f = one.checked_add(inv_lev)
                    .and_then(|v| v.checked_sub(mm))
                    .ok_or_else(|| error!(PerpError::MathOverflow))?;
                f
            }
        };

        let liq_price = (entry_price as u128)
            .checked_mul(factor)
            .and_then(|v| v.checked_div(PRECISION))
            .ok_or_else(|| error!(PerpError::MathOverflow))?;
        Ok(liq_price as u64)
    }

    /// Compute TWAP from samples within a window
    pub fn calc_twap(samples: &[TwapSample], now: i64, window: u64) -> u64 {
        let cutoff = now - (window as i64);
        let valid: Vec<&TwapSample> = samples.iter().filter(|s| s.timestamp >= cutoff).collect();
        if valid.is_empty() {
            return 0;
        }
        let sum: u128 = valid.iter().map(|s| s.price as u128).sum();
        (sum / valid.len() as u128) as u64
    }
}

// ============================================================================
// Instructions
// ============================================================================

#[program]
pub mod send_it_perps {
    use super::*;

    /// Initialize a perpetual market for a graduated token
    pub fn initialize_perp_market(
        ctx: Context<InitializePerpMarket>,
        max_leverage: u64,
        funding_interval: i64,
        maintenance_margin: u64,
        liquidation_fee: u64,
        maker_fee: u64,
        taker_fee: u64,
        max_open_interest: u64,
        max_position_size: u64,
    ) -> Result<()> {
        require!(max_leverage > 0 && max_leverage <= MAX_LEVERAGE, PerpError::ExcessiveLeverage);

        let clock = Clock::get()?;
        let market = &mut ctx.accounts.market;
        market.bump = ctx.bumps.market;
        market.authority = ctx.accounts.authority.key();
        market.token_mint = ctx.accounts.token_mint.key();
        market.collateral_mint = ctx.accounts.collateral_mint.key();
        market.raydium_pool = ctx.accounts.raydium_pool.key();
        market.solforge_vault = ctx.accounts.solforge_vault.key();
        market.insurance_fund = ctx.accounts.insurance_fund.key();
        market.collateral_vault = ctx.accounts.collateral_vault.key();
        market.order_book = ctx.accounts.order_book.key();

        market.max_leverage = max_leverage;
        market.maintenance_margin = if maintenance_margin > 0 { maintenance_margin } else { DEFAULT_MAINTENANCE_MARGIN };
        market.liquidation_fee = if liquidation_fee > 0 { liquidation_fee } else { DEFAULT_LIQUIDATION_FEE };
        market.maker_fee = if maker_fee > 0 { maker_fee } else { DEFAULT_MAKER_FEE };
        market.taker_fee = if taker_fee > 0 { taker_fee } else { DEFAULT_TAKER_FEE };
        market.funding_interval = if funding_interval > 0 { funding_interval } else { DEFAULT_FUNDING_INTERVAL };
        market.max_open_interest = max_open_interest;
        market.max_position_size = max_position_size;
        market.last_funding_time = clock.unix_timestamp;
        market.paused = false;
        market.created_at = clock.unix_timestamp;
        market.twap_samples = Vec::new();

        // Initialize order book
        let ob = &mut ctx.accounts.order_book;
        ob.bump = ctx.bumps.order_book;
        ob.market = market.key();
        ob.bids = Vec::new();
        ob.asks = Vec::new();
        ob.next_order_id = 1;

        // Initialize insurance fund
        let ins = &mut ctx.accounts.insurance_fund;
        ins.bump = ctx.bumps.insurance_fund;
        ins.market = market.key();
        ins.vault = ctx.accounts.insurance_vault.key();
        ins.balance = 0;
        ins.total_payouts = 0;
        ins.total_deposits = 0;

        emit!(MarketCreated {
            market: market.key(),
            token_mint: market.token_mint,
            max_leverage,
            timestamp: clock.unix_timestamp,
        });

        Ok(())
    }

    /// Create a cross-margin account for a user
    pub fn create_margin_account(ctx: Context<CreateMarginAccount>) -> Result<()> {
        let clock = Clock::get()?;
        let account = &mut ctx.accounts.margin_account;
        account.bump = ctx.bumps.margin_account;
        account.owner = ctx.accounts.owner.key();
        account.collateral = 0;
        account.open_positions = 0;
        account.realized_pnl = 0;
        account.created_at = clock.unix_timestamp;
        Ok(())
    }

    /// Deposit collateral into margin account
    pub fn deposit_collateral(ctx: Context<DepositCollateral>, amount: u64) -> Result<()> {
        // Transfer collateral from user to vault
        let cpi_accounts = Transfer {
            from: ctx.accounts.user_token_account.to_account_info(),
            to: ctx.accounts.collateral_vault.to_account_info(),
            authority: ctx.accounts.owner.to_account_info(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.token_program.to_account_info(), cpi_accounts);
        token::transfer(cpi_ctx, amount)?;

        let margin = &mut ctx.accounts.margin_account;
        margin.collateral = margin.collateral.checked_add(amount)
            .ok_or_else(|| error!(PerpError::MathOverflow))?;

        emit!(CollateralDeposited {
            margin_account: margin.key(),
            amount,
        });

        Ok(())
    }

    /// Withdraw collateral from margin account (must maintain margin requirements)
    pub fn withdraw_collateral(ctx: Context<WithdrawCollateral>, amount: u64) -> Result<()> {
        let margin = &mut ctx.accounts.margin_account;
        require!(margin.collateral >= amount, PerpError::InsufficientCollateral);

        // TODO: Check that withdrawal doesn't violate margin requirements for open positions
        margin.collateral = margin.collateral.checked_sub(amount)
            .ok_or_else(|| error!(PerpError::MathOverflow))?;

        // Transfer from vault to user using PDA signer
        let market_key = ctx.accounts.market.key();
        let seeds = &[
            b"collateral_vault",
            market_key.as_ref(),
            &[ctx.accounts.market.bump],
        ];
        let signer_seeds = &[&seeds[..]];

        let cpi_accounts = Transfer {
            from: ctx.accounts.collateral_vault.to_account_info(),
            to: ctx.accounts.user_token_account.to_account_info(),
            authority: ctx.accounts.vault_authority.to_account_info(),
        };
        let cpi_ctx = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            cpi_accounts,
            signer_seeds,
        );
        token::transfer(cpi_ctx, amount)?;

        emit!(CollateralWithdrawn {
            margin_account: margin.key(),
            amount,
        });

        Ok(())
    }

    /// Open a new leveraged position
    pub fn open_position(
        ctx: Context<OpenPosition>,
        side: Side,
        size: u64,
        leverage: u64,
        collateral: u64,
    ) -> Result<()> {
        let market = &ctx.accounts.market;
        require!(!market.paused, PerpError::MarketPaused);
        require!(leverage > 0 && leverage <= market.max_leverage, PerpError::ExcessiveLeverage);
        require!(size > 0, PerpError::InvalidSize);
        require!(size <= market.max_position_size, PerpError::PositionSizeLimitExceeded);

        // Check open interest caps
        let new_oi = match side {
            Side::Long => market.long_open_interest.checked_add(size),
            Side::Short => market.short_open_interest.checked_add(size),
        }.ok_or_else(|| error!(PerpError::MathOverflow))?;
        require!(new_oi <= market.max_open_interest, PerpError::OpenInterestCapExceeded);

        // Use mark price as entry (in production, would use fill price from order book)
        let entry_price = market.mark_price;
        require!(entry_price > 0, PerpError::InvalidPrice);

        // Check circuit breaker
        check_price_band(entry_price, market.index_price)?;

        // Calculate required collateral: notional / leverage
        let notional = math::calc_notional(size, entry_price)?;
        let required_collateral = notional / (leverage as u64);
        require!(collateral >= required_collateral, PerpError::InsufficientCollateral);

        // Deduct collateral from margin account
        let margin = &mut ctx.accounts.margin_account;
        require!(margin.collateral >= collateral, PerpError::InsufficientCollateral);
        margin.collateral = margin.collateral.checked_sub(collateral)
            .ok_or_else(|| error!(PerpError::MathOverflow))?;
        margin.open_positions = margin.open_positions.checked_add(1)
            .ok_or_else(|| error!(PerpError::MathOverflow))?;

        // Calculate and charge taker fee
        let fee = math::calc_fee(notional, market.taker_fee)?;
        distribute_fees(&ctx.accounts.market, fee)?;

        // Initialize position
        let clock = Clock::get()?;
        let position = &mut ctx.accounts.position;
        position.bump = ctx.bumps.position;
        position.market = market.key();
        position.owner = ctx.accounts.owner.key();
        position.margin_account = margin.key();
        position.side = side;
        position.size = size;
        position.entry_price = entry_price;
        position.collateral = collateral;
        position.leverage = leverage;
        position.last_cumulative_funding = match side {
            Side::Long => market.cumulative_funding_long,
            Side::Short => market.cumulative_funding_short,
        };
        position.pending_funding = 0;
        position.opened_at = clock.unix_timestamp;
        position.updated_at = clock.unix_timestamp;

        // Update market open interest
        let market_mut = &mut ctx.accounts.market;
        match side {
            Side::Long => {
                market_mut.long_open_interest = market_mut.long_open_interest
                    .checked_add(size).ok_or_else(|| error!(PerpError::MathOverflow))?;
            }
            Side::Short => {
                market_mut.short_open_interest = market_mut.short_open_interest
                    .checked_add(size).ok_or_else(|| error!(PerpError::MathOverflow))?;
            }
        }

        emit!(PositionOpened {
            market: market_mut.key(),
            owner: ctx.accounts.owner.key(),
            side,
            size,
            entry_price,
            leverage,
            collateral,
        });

        Ok(())
    }

    /// Close an entire position
    pub fn close_position(ctx: Context<ClosePosition>) -> Result<()> {
        let market = &ctx.accounts.market;
        let position = &ctx.accounts.position;
        let exit_price = market.mark_price;

        // Check circuit breaker
        check_price_band(exit_price, market.index_price)?;

        // Settle funding
        let funding_payment = settle_funding(position, market)?;

        // Calculate PnL
        let unrealized_pnl = math::calc_unrealized_pnl(
            &position.side, position.entry_price, exit_price, position.size,
        )?;
        let total_pnl = unrealized_pnl + funding_payment;

        // Calculate fee
        let notional = math::calc_notional(position.size, exit_price)?;
        let fee = math::calc_fee(notional, market.taker_fee)?;

        // Return collateral +/- PnL - fees to margin account
        let margin = &mut ctx.accounts.margin_account;
        let return_amount = (position.collateral as i64) + total_pnl - (fee as i64);
        if return_amount > 0 {
            margin.collateral = margin.collateral
                .checked_add(return_amount as u64)
                .ok_or_else(|| error!(PerpError::MathOverflow))?;
        }
        // If return_amount <= 0, collateral is lost (insurance fund covers if needed)
        margin.realized_pnl = margin.realized_pnl
            .checked_add(total_pnl)
            .ok_or_else(|| error!(PerpError::MathOverflow))?;
        margin.open_positions = margin.open_positions.saturating_sub(1);

        // Update market OI
        let market_mut = &mut ctx.accounts.market;
        match position.side {
            Side::Long => {
                market_mut.long_open_interest = market_mut.long_open_interest
                    .saturating_sub(position.size);
            }
            Side::Short => {
                market_mut.short_open_interest = market_mut.short_open_interest
                    .saturating_sub(position.size);
            }
        }

        distribute_fees(market_mut, fee)?;

        emit!(PositionClosed {
            market: market_mut.key(),
            owner: ctx.accounts.owner.key(),
            side: position.side,
            size: position.size,
            exit_price,
            realized_pnl: total_pnl,
        });

        Ok(())
    }

    /// Increase an existing position's size
    pub fn increase_position(
        ctx: Context<ModifyPosition>,
        additional_size: u64,
        additional_collateral: u64,
    ) -> Result<()> {
        let market = &ctx.accounts.market;
        require!(!market.paused, PerpError::MarketPaused);

        let position = &mut ctx.accounts.position;
        let new_size = position.size.checked_add(additional_size)
            .ok_or_else(|| error!(PerpError::MathOverflow))?;
        require!(new_size <= market.max_position_size, PerpError::PositionSizeLimitExceeded);

        let current_price = market.mark_price;
        check_price_band(current_price, market.index_price)?;

        // Weighted average entry price
        let old_notional = (position.size as u128) * (position.entry_price as u128);
        let new_notional = (additional_size as u128) * (current_price as u128);
        let avg_entry = (old_notional + new_notional) / (new_size as u128);

        // Deduct collateral
        let margin = &mut ctx.accounts.margin_account;
        require!(margin.collateral >= additional_collateral, PerpError::InsufficientCollateral);
        margin.collateral = margin.collateral.checked_sub(additional_collateral)
            .ok_or_else(|| error!(PerpError::MathOverflow))?;

        position.size = new_size;
        position.entry_price = avg_entry as u64;
        position.collateral = position.collateral.checked_add(additional_collateral)
            .ok_or_else(|| error!(PerpError::MathOverflow))?;
        position.updated_at = Clock::get()?.unix_timestamp;

        // Update OI
        let market_mut = &mut ctx.accounts.market;
        match position.side {
            Side::Long => {
                market_mut.long_open_interest = market_mut.long_open_interest
                    .checked_add(additional_size).ok_or_else(|| error!(PerpError::MathOverflow))?;
            }
            Side::Short => {
                market_mut.short_open_interest = market_mut.short_open_interest
                    .checked_add(additional_size).ok_or_else(|| error!(PerpError::MathOverflow))?;
            }
        }

        Ok(())
    }

    /// Decrease an existing position's size (partial close)
    pub fn decrease_position(
        ctx: Context<ModifyPosition>,
        decrease_size: u64,
    ) -> Result<()> {
        let market = &ctx.accounts.market;
        let position = &mut ctx.accounts.position;
        require!(decrease_size > 0 && decrease_size < position.size, PerpError::InvalidSize);

        let exit_price = market.mark_price;
        check_price_band(exit_price, market.index_price)?;

        // Calculate PnL on closed portion
        let pnl = math::calc_unrealized_pnl(&position.side, position.entry_price, exit_price, decrease_size)?;

        // Return proportional collateral + PnL
        let collateral_fraction = (position.collateral as u128)
            .checked_mul(decrease_size as u128)
            .and_then(|v| v.checked_div(position.size as u128))
            .ok_or_else(|| error!(PerpError::MathOverflow))? as u64;

        let margin = &mut ctx.accounts.margin_account;
        let return_amount = (collateral_fraction as i64) + pnl;
        if return_amount > 0 {
            margin.collateral = margin.collateral
                .checked_add(return_amount as u64)
                .ok_or_else(|| error!(PerpError::MathOverflow))?;
        }
        margin.realized_pnl = margin.realized_pnl.checked_add(pnl)
            .ok_or_else(|| error!(PerpError::MathOverflow))?;

        position.size = position.size.checked_sub(decrease_size)
            .ok_or_else(|| error!(PerpError::MathOverflow))?;
        position.collateral = position.collateral.checked_sub(collateral_fraction)
            .ok_or_else(|| error!(PerpError::MathOverflow))?;
        position.updated_at = Clock::get()?.unix_timestamp;

        // Update OI
        let market_mut = &mut ctx.accounts.market;
        match position.side {
            Side::Long => {
                market_mut.long_open_interest = market_mut.long_open_interest
                    .saturating_sub(decrease_size);
            }
            Side::Short => {
                market_mut.short_open_interest = market_mut.short_open_interest
                    .saturating_sub(decrease_size);
            }
        }

        Ok(())
    }

    /// Place a limit or market order on the order book
    pub fn place_order(
        ctx: Context<PlaceOrder>,
        side: Side,
        order_type: OrderType,
        price: u64,
        size: u64,
    ) -> Result<()> {
        let market = &ctx.accounts.market;
        require!(!market.paused, PerpError::MarketPaused);
        require!(size > 0, PerpError::InvalidSize);

        if order_type == OrderType::Limit {
            require!(price > 0, PerpError::InvalidPrice);
            check_price_band(price, market.index_price)?;
        }

        let ob = &mut ctx.accounts.order_book;
        let orders = match side {
            Side::Long => &ob.bids,
            Side::Short => &ob.asks,
        };
        require!(orders.len() < MAX_ORDERS, PerpError::OrderBookFull);

        let clock = Clock::get()?;
        let order_id = ob.next_order_id;
        ob.next_order_id = ob.next_order_id.checked_add(1)
            .ok_or_else(|| error!(PerpError::MathOverflow))?;

        let entry = OrderEntry {
            order_id,
            owner: ctx.accounts.owner.key(),
            price: if order_type == OrderType::Market { 0 } else { price },
            size,
            remaining: size,
            side,
            order_type,
            timestamp: clock.unix_timestamp,
            margin_account: ctx.accounts.margin_account.key(),
        };

        match side {
            Side::Long => {
                // Insert maintaining descending price order
                let pos = ob.bids.iter().position(|o| o.price < price).unwrap_or(ob.bids.len());
                ob.bids.insert(pos, entry);
            }
            Side::Short => {
                // Insert maintaining ascending price order
                let pos = ob.asks.iter().position(|o| o.price > price).unwrap_or(ob.asks.len());
                ob.asks.insert(pos, entry);
            }
        }

        emit!(OrderPlaced {
            market: market.key(),
            order_id,
            owner: ctx.accounts.owner.key(),
            side,
            order_type,
            price,
            size,
        });

        Ok(())
    }

    /// Cancel an existing order
    pub fn cancel_order(ctx: Context<CancelOrder>, order_id: u64) -> Result<()> {
        let ob = &mut ctx.accounts.order_book;
        let owner = ctx.accounts.owner.key();

        // Search bids
        if let Some(pos) = ob.bids.iter().position(|o| o.order_id == order_id && o.owner == owner) {
            ob.bids.remove(pos);
            emit!(OrderCancelled { market: ob.market, order_id, owner });
            return Ok(());
        }

        // Search asks
        if let Some(pos) = ob.asks.iter().position(|o| o.order_id == order_id && o.owner == owner) {
            ob.asks.remove(pos);
            emit!(OrderCancelled { market: ob.market, order_id, owner });
            return Ok(());
        }

        err!(PerpError::OrderNotFound)
    }

    /// Permissionless crank: match crossing orders
    pub fn match_orders(ctx: Context<MatchOrders>, max_matches: u8) -> Result<()> {
        let ob = &mut ctx.accounts.order_book;
        let market = &mut ctx.accounts.market;
        require!(!market.paused, PerpError::MarketPaused);

        let mut matches_done: u8 = 0;

        while matches_done < max_matches && !ob.bids.is_empty() && !ob.asks.is_empty() {
            let best_bid = &ob.bids[0];
            let best_ask = &ob.asks[0];

            // Market orders always cross; limit orders cross when bid >= ask
            let crosses = best_bid.order_type == OrderType::Market
                || best_ask.order_type == OrderType::Market
                || best_bid.price >= best_ask.price;

            if !crosses {
                break;
            }

            // Self-trade prevention
            if best_bid.owner == best_ask.owner {
                // Remove the newer order
                if best_bid.timestamp > best_ask.timestamp {
                    ob.bids.remove(0);
                } else {
                    ob.asks.remove(0);
                }
                continue;
            }

            // Match at the resting order's price (price-time priority)
            let fill_price = if best_bid.timestamp < best_ask.timestamp {
                best_bid.price
            } else {
                best_ask.price
            };
            let fill_size = best_bid.remaining.min(best_ask.remaining);

            // Update mark price
            if fill_price > 0 {
                market.mark_price = fill_price;
                // Add TWAP sample
                let clock = Clock::get()?;
                market.twap_samples.push(TwapSample {
                    timestamp: clock.unix_timestamp,
                    price: fill_price,
                });
                if market.twap_samples.len() > MAX_TWAP_SAMPLES {
                    market.twap_samples.remove(0);
                }
            }

            emit!(OrderMatched {
                market: market.key(),
                bid_order_id: ob.bids[0].order_id,
                ask_order_id: ob.asks[0].order_id,
                price: fill_price,
                size: fill_size,
            });

            // Update remaining sizes
            ob.bids[0].remaining = ob.bids[0].remaining.saturating_sub(fill_size);
            ob.asks[0].remaining = ob.asks[0].remaining.saturating_sub(fill_size);

            // Remove fully filled orders
            if ob.bids[0].remaining == 0 {
                ob.bids.remove(0);
            }
            if !ob.asks.is_empty() && ob.asks[0].remaining == 0 {
                ob.asks.remove(0);
            }

            matches_done += 1;
        }

        require!(matches_done > 0, PerpError::NoOrdersToMatch);
        Ok(())
    }

    /// Permissionless crank: update funding rate
    pub fn update_funding_rate(ctx: Context<UpdateFundingRate>) -> Result<()> {
        let market = &mut ctx.accounts.market;
        let clock = Clock::get()?;

        require!(
            clock.unix_timestamp >= market.last_funding_time + market.funding_interval,
            PerpError::FundingIntervalNotElapsed
        );

        // Funding rate = (mark_price - index_price) / index_price, clamped
        // Positive = longs pay shorts, Negative = shorts pay longs
        let mark = market.mark_price as i128;
        let index = market.index_price as i128;

        let raw_rate = if index > 0 {
            ((mark - index) * PRECISION as i128) / index
        } else {
            0
        };

        let clamped_rate = raw_rate.max(-MAX_FUNDING_RATE).min(MAX_FUNDING_RATE);

        // Update cumulative funding
        market.cumulative_funding_long = market.cumulative_funding_long
            .checked_add(clamped_rate)
            .ok_or_else(|| error!(PerpError::MathOverflow))?;
        market.cumulative_funding_short = market.cumulative_funding_short
            .checked_sub(clamped_rate)
            .ok_or_else(|| error!(PerpError::MathOverflow))?;

        market.last_funding_time = clock.unix_timestamp;

        emit!(FundingRateUpdated {
            market: market.key(),
            funding_rate: clamped_rate,
            mark_price: market.mark_price,
            index_price: market.index_price,
            timestamp: clock.unix_timestamp,
        });

        Ok(())
    }

    /// Liquidate an under-margined position (full or partial)
    pub fn liquidate_position(
        ctx: Context<LiquidatePosition>,
        liquidation_size: u64,
    ) -> Result<()> {
        let market = &ctx.accounts.market;
        let position = &ctx.accounts.position;

        // Settle pending funding
        let funding_payment = settle_funding(position, market)?;

        // Check if position is liquidatable
        let unrealized_pnl = math::calc_unrealized_pnl(
            &position.side, position.entry_price, market.mark_price, position.size,
        )?;
        let effective_pnl = unrealized_pnl + funding_payment;
        let notional = math::calc_notional(position.size, market.mark_price)?;
        let margin_ratio = math::calc_margin_ratio(position.collateral, effective_pnl, notional)?;

        require!(margin_ratio < market.maintenance_margin, PerpError::NotLiquidatable);

        // Determine liquidation amount (partial or full)
        let liq_size = if liquidation_size > 0 && liquidation_size < position.size {
            liquidation_size
        } else {
            position.size
        };

        // Calculate liquidation fee
        let liq_notional = math::calc_notional(liq_size, market.mark_price)?;
        let liq_fee = math::calc_fee(liq_notional, market.liquidation_fee)?;

        // Proportional collateral for liquidated portion
        let collateral_fraction = (position.collateral as u128)
            .checked_mul(liq_size as u128)
            .and_then(|v| v.checked_div(position.size as u128))
            .ok_or_else(|| error!(PerpError::MathOverflow))? as u64;

        // PnL on liquidated portion
        let liq_pnl = math::calc_unrealized_pnl(
            &position.side, position.entry_price, market.mark_price, liq_size,
        )?;

        // Remaining after PnL: if negative PnL exceeds collateral, insurance fund absorbs
        let remainder = (collateral_fraction as i64) + liq_pnl - (liq_fee as i64);

        // Pay liquidator fee
        // (In production: transfer liq_fee to liquidator's token account)

        // Update insurance fund if there's a shortfall
        if remainder < 0 {
            let shortfall = (-remainder) as u64;
            let ins = &mut ctx.accounts.insurance_fund;
            if ins.balance >= shortfall {
                ins.balance = ins.balance.saturating_sub(shortfall);
                ins.total_payouts = ins.total_payouts.checked_add(shortfall)
                    .ok_or_else(|| error!(PerpError::MathOverflow))?;
            }
            // If insurance fund insufficient, socialized loss (not implemented here)
        }

        // Update position
        let position_mut = &mut ctx.accounts.position;
        let margin = &mut ctx.accounts.position_margin_account;

        if liq_size >= position_mut.size {
            // Full liquidation — close position
            position_mut.size = 0;
            position_mut.collateral = 0;
            margin.open_positions = margin.open_positions.saturating_sub(1);
        } else {
            // Partial liquidation
            position_mut.size = position_mut.size.checked_sub(liq_size)
                .ok_or_else(|| error!(PerpError::MathOverflow))?;
            position_mut.collateral = position_mut.collateral.checked_sub(collateral_fraction)
                .ok_or_else(|| error!(PerpError::MathOverflow))?;
        }
        position_mut.updated_at = Clock::get()?.unix_timestamp;

        // Update market OI
        let market_mut = &mut ctx.accounts.market;
        match position_mut.side {
            Side::Long => {
                market_mut.long_open_interest = market_mut.long_open_interest.saturating_sub(liq_size);
            }
            Side::Short => {
                market_mut.short_open_interest = market_mut.short_open_interest.saturating_sub(liq_size);
            }
        }

        emit!(PositionLiquidated {
            market: market_mut.key(),
            owner: position_mut.owner,
            liquidator: ctx.accounts.liquidator.key(),
            size_liquidated: liq_size,
            price: market_mut.mark_price,
            liquidation_fee: liq_fee,
        });

        Ok(())
    }

    /// Update oracle price from Raydium pool (permissionless crank)
    pub fn update_oracle_price(ctx: Context<UpdateOraclePrice>, price: u64) -> Result<()> {
        // In production, this would read from the Raydium AMM pool account
        // and extract the current spot price. For now, the caller passes price
        // which should be validated against the Raydium pool state.
        let market = &mut ctx.accounts.market;
        require!(price > 0, PerpError::InvalidPrice);

        market.index_price = price;

        // Update TWAP
        let clock = Clock::get()?;
        market.twap_samples.push(TwapSample {
            timestamp: clock.unix_timestamp,
            price,
        });
        if market.twap_samples.len() > MAX_TWAP_SAMPLES {
            market.twap_samples.remove(0);
        }

        Ok(())
    }

    /// Pause or unpause a market (authority only)
    pub fn set_market_paused(ctx: Context<AdminAction>, paused: bool) -> Result<()> {
        require!(
            ctx.accounts.authority.key() == ctx.accounts.market.authority,
            PerpError::Unauthorized
        );
        ctx.accounts.market.paused = paused;
        Ok(())
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

fn check_price_band(price: u64, index_price: u64) -> Result<()> {
    if index_price == 0 {
        return Ok(()); // No oracle yet, skip band check
    }
    let deviation = if price > index_price {
        ((price - index_price) as u128) * PRECISION / (index_price as u128)
    } else {
        ((index_price - price) as u128) * PRECISION / (index_price as u128)
    };
    require!(
        deviation <= (PRICE_BAND_BPS as u128) * PRECISION / 10_000,
        PerpError::CircuitBreakerTriggered
    );
    Ok(())
}

fn settle_funding(position: &Position, market: &PerpMarket) -> Result<i64> {
    let cumulative_now = match position.side {
        Side::Long => market.cumulative_funding_long,
        Side::Short => market.cumulative_funding_short,
    };
    math::calc_funding_payment(position.size, cumulative_now, position.last_cumulative_funding)
}

fn distribute_fees(_market: &PerpMarket, _fee: u64) -> Result<()> {
    // Fee distribution:
    // - INSURANCE_FEE_SHARE (30%) → insurance fund
    // - SOLFORGE_FEE_SHARE (20%) → SolForge vault for token burn
    // - Remaining 50% → protocol revenue
    //
    // In production, this performs CPI token transfers.
    // Insurance fund portion:
    //   let ins_amount = (fee as u128 * INSURANCE_FEE_SHARE as u128 / PRECISION) as u64;
    // SolForge burn portion:
    //   let burn_amount = (fee as u128 * SOLFORGE_FEE_SHARE as u128 / PRECISION) as u64;
    // Protocol portion:
    //   let protocol_amount = fee - ins_amount - burn_amount;
    Ok(())
}

// ============================================================================
// Account Contexts
// ============================================================================

#[derive(Accounts)]
pub struct InitializePerpMarket<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        init,
        payer = authority,
        space = 8 + 2048, // generous allocation
        seeds = [b"perp_market", token_mint.key().as_ref()],
        bump,
    )]
    pub market: Account<'info, PerpMarket>,

    #[account(
        init,
        payer = authority,
        space = 8 + 32768, // order book needs room
        seeds = [b"order_book", market.key().as_ref()],
        bump,
    )]
    pub order_book: Account<'info, OrderBook>,

    #[account(
        init,
        payer = authority,
        space = 8 + 256,
        seeds = [b"insurance_fund", market.key().as_ref()],
        bump,
    )]
    pub insurance_fund: Account<'info, InsuranceFund>,

    /// CHECK: Token mint of the graduated token
    pub token_mint: AccountInfo<'info>,
    /// CHECK: Collateral token mint
    pub collateral_mint: AccountInfo<'info>,
    /// CHECK: Raydium AMM pool for oracle — stored on-chain during init, validated in subsequent use
    pub raydium_pool: AccountInfo<'info>,
    /// CHECK: SolForge vault for burns
    pub solforge_vault: AccountInfo<'info>,
    /// CHECK: Collateral vault token account
    pub collateral_vault: AccountInfo<'info>,
    /// CHECK: Insurance vault token account
    pub insurance_vault: AccountInfo<'info>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct CreateMarginAccount<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        init,
        payer = owner,
        space = 8 + 256,
        seeds = [b"margin_account", owner.key().as_ref()],
        bump,
    )]
    pub margin_account: Account<'info, UserMarginAccount>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct DepositCollateral<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        mut,
        seeds = [b"margin_account", owner.key().as_ref()],
        bump = margin_account.bump,
        has_one = owner,
    )]
    pub margin_account: Account<'info, UserMarginAccount>,

    #[account(mut)]
    pub user_token_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub collateral_vault: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct WithdrawCollateral<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        mut,
        seeds = [b"margin_account", owner.key().as_ref()],
        bump = margin_account.bump,
        has_one = owner,
    )]
    pub margin_account: Account<'info, UserMarginAccount>,

    pub market: Account<'info, PerpMarket>,

    #[account(mut)]
    pub user_token_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub collateral_vault: Account<'info, TokenAccount>,

    /// CHECK: PDA vault authority
    pub vault_authority: AccountInfo<'info>,

    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct OpenPosition<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(mut)]
    pub market: Account<'info, PerpMarket>,

    #[account(
        mut,
        seeds = [b"margin_account", owner.key().as_ref()],
        bump = margin_account.bump,
        has_one = owner,
    )]
    pub margin_account: Account<'info, UserMarginAccount>,

    #[account(
        init,
        payer = owner,
        space = 8 + 512,
        seeds = [b"position", market.key().as_ref(), owner.key().as_ref(), &market.long_open_interest.to_le_bytes()],
        bump,
    )]
    pub position: Account<'info, Position>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ClosePosition<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(mut)]
    pub market: Account<'info, PerpMarket>,

    #[account(
        mut,
        has_one = owner,
        has_one = market,
        close = owner,
    )]
    pub position: Account<'info, Position>,

    #[account(
        mut,
        seeds = [b"margin_account", owner.key().as_ref()],
        bump = margin_account.bump,
        has_one = owner,
    )]
    pub margin_account: Account<'info, UserMarginAccount>,
}

#[derive(Accounts)]
pub struct ModifyPosition<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(mut)]
    pub market: Account<'info, PerpMarket>,

    #[account(
        mut,
        has_one = owner,
        has_one = market,
    )]
    pub position: Account<'info, Position>,

    #[account(
        mut,
        seeds = [b"margin_account", owner.key().as_ref()],
        bump = margin_account.bump,
        has_one = owner,
    )]
    pub margin_account: Account<'info, UserMarginAccount>,
}

#[derive(Accounts)]
pub struct PlaceOrder<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    pub market: Account<'info, PerpMarket>,

    #[account(
        mut,
        has_one = market,
    )]
    pub order_book: Account<'info, OrderBook>,

    #[account(
        seeds = [b"margin_account", owner.key().as_ref()],
        bump = margin_account.bump,
        has_one = owner,
    )]
    pub margin_account: Account<'info, UserMarginAccount>,
}

#[derive(Accounts)]
pub struct CancelOrder<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(mut)]
    pub order_book: Account<'info, OrderBook>,
}

#[derive(Accounts)]
pub struct MatchOrders<'info> {
    /// Anyone can crank
    pub cranker: Signer<'info>,

    #[account(mut)]
    pub market: Account<'info, PerpMarket>,

    #[account(
        mut,
        has_one = market,
    )]
    pub order_book: Account<'info, OrderBook>,
}

#[derive(Accounts)]
pub struct UpdateFundingRate<'info> {
    /// Anyone can crank
    pub cranker: Signer<'info>,

    #[account(mut)]
    pub market: Account<'info, PerpMarket>,
}

#[derive(Accounts)]
pub struct LiquidatePosition<'info> {
    #[account(mut)]
    pub liquidator: Signer<'info>,

    #[account(mut)]
    pub market: Account<'info, PerpMarket>,

    #[account(
        mut,
        has_one = market,
    )]
    pub position: Account<'info, Position>,

    #[account(
        mut,
        constraint = position_margin_account.key() == position.margin_account @ PerpError::InvalidMarginAccount,
    )]
    pub position_margin_account: Account<'info, UserMarginAccount>,

    #[account(
        mut,
        has_one = market,
    )]
    pub insurance_fund: Account<'info, InsuranceFund>,
}

#[derive(Accounts)]
pub struct UpdateOraclePrice<'info> {
    /// Anyone can crank (but should validate against Raydium pool)
    pub cranker: Signer<'info>,

    #[account(mut)]
    pub market: Account<'info, PerpMarket>,

    /// CHECK: Raydium pool account — validated by constraint matching market's stored pool key
    #[account(
        constraint = raydium_pool.key() == market.raydium_pool @ PerpError::InvalidOracleAccount
    )]
    pub raydium_pool: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct AdminAction<'info> {
    pub authority: Signer<'info>,

    #[account(mut)]
    pub market: Account<'info, PerpMarket>,
}
