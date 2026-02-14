use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer, Mint};

declare_id!("SenditStable1111111111111111111111111111111");

// ============================================================================
// CONSTANTS
// ============================================================================

const STABLE_PAIR_SEED: &[u8] = b"stable_pair";
const LP_POSITION_SEED: &[u8] = b"lp_position";
const POOL_TOKEN_VAULT_SEED: &[u8] = b"sp_token_vault";
const POOL_STABLE_VAULT_SEED: &[u8] = b"sp_stable_vault";
const TREASURY_SEED: &[u8] = b"sp_treasury";

const BPS_DENOMINATOR: u64 = 10_000;
const MAX_FEE_BPS: u16 = 500; // 5%
const MIN_LIQUIDITY: u64 = 1_000; // minimum initial deposit to avoid zero-division

// ============================================================================
// ACCOUNTS
// ============================================================================

/// Configuration PDA for a token/stablecoin trading pair.
///
/// Seeds: `["stable_pair", token_mint, stable_mint]`
#[account]
pub struct StablePairConfig {
    /// The non-stable token mint (e.g. project token).
    pub token_mint: Pubkey,
    /// The stablecoin mint (USDC, USDT, etc.).
    pub stable_mint: Pubkey,
    /// Current token reserve held in the pool.
    pub pool_token_reserve: u64,
    /// Current stablecoin reserve held in the pool.
    pub pool_stable_reserve: u64,
    /// Trading fee in basis points charged on every swap.
    pub fee_bps: u16,
    /// The wallet that created this pair.
    pub creator: Pubkey,
    /// Total LP shares outstanding (virtual — no LP mint, tracked via LPPosition PDAs).
    pub total_lp_shares: u128,
    /// Whether the pair is paused.
    pub paused: bool,
    /// Bump seed.
    pub bump: u8,
    /// Token vault bump.
    pub token_vault_bump: u8,
    /// Stable vault bump.
    pub stable_vault_bump: u8,
}

impl StablePairConfig {
    pub const SIZE: usize = 8   // discriminator
        + 32    // token_mint
        + 32    // stable_mint
        + 8     // pool_token_reserve
        + 8     // pool_stable_reserve
        + 2     // fee_bps
        + 32    // creator
        + 16    // total_lp_shares
        + 1     // paused
        + 1     // bump
        + 1     // token_vault_bump
        + 1;    // stable_vault_bump
}

/// Per-user LP position tracking the user's share of a stable pair pool.
///
/// Seeds: `["lp_position", stable_pair, user]`
#[account]
pub struct LPPosition {
    /// The stable pair this position belongs to.
    pub pair: Pubkey,
    /// The user who owns this LP position.
    pub owner: Pubkey,
    /// LP shares held by this user.
    pub lp_shares: u128,
    /// Bump seed.
    pub bump: u8,
}

impl LPPosition {
    pub const SIZE: usize = 8 + 32 + 32 + 16 + 1;
}

// ============================================================================
// EVENTS
// ============================================================================

#[event]
pub struct StablePairCreated {
    pub token_mint: Pubkey,
    pub stable_mint: Pubkey,
    pub creator: Pubkey,
    pub fee_bps: u16,
    pub timestamp: i64,
}

#[event]
pub struct SwapExecuted {
    pub pair: Pubkey,
    pub user: Pubkey,
    /// True = token→stable, False = stable→token.
    pub token_to_stable: bool,
    pub amount_in: u64,
    pub amount_out: u64,
    pub fee_amount: u64,
    pub timestamp: i64,
}

#[event]
pub struct LiquidityAdded {
    pub pair: Pubkey,
    pub provider: Pubkey,
    pub token_amount: u64,
    pub stable_amount: u64,
    pub lp_shares_minted: u128,
    pub timestamp: i64,
}

#[event]
pub struct LiquidityRemoved {
    pub pair: Pubkey,
    pub provider: Pubkey,
    pub token_amount: u64,
    pub stable_amount: u64,
    pub lp_shares_burned: u128,
    pub timestamp: i64,
}

// ============================================================================
// ERRORS
// ============================================================================

#[error_code]
pub enum StablePairError {
    #[msg("Fee exceeds maximum (5%)")]
    FeeTooHigh,
    #[msg("Amount must be greater than zero")]
    ZeroAmount,
    #[msg("Pair is paused")]
    Paused,
    #[msg("Insufficient output amount")]
    InsufficientOutput,
    #[msg("Slippage tolerance exceeded")]
    SlippageExceeded,
    #[msg("Insufficient LP shares")]
    InsufficientShares,
    #[msg("Initial liquidity too low")]
    InitialLiquidityTooLow,
    #[msg("Math overflow")]
    MathOverflow,
    #[msg("Pool is empty")]
    EmptyPool,
    #[msg("Unauthorized")]
    Unauthorized,
}

// ============================================================================
// INSTRUCTIONS (HANDLERS)
// ============================================================================

/// Create a new token/stablecoin pair.
pub fn handle_create_stable_pair(
    ctx: Context<CreateStablePair>,
    fee_bps: u16,
) -> Result<()> {
    require!(fee_bps <= MAX_FEE_BPS, StablePairError::FeeTooHigh);

    let clock = Clock::get()?;
    let pair = &mut ctx.accounts.stable_pair;
    pair.token_mint = ctx.accounts.token_mint.key();
    pair.stable_mint = ctx.accounts.stable_mint.key();
    pair.pool_token_reserve = 0;
    pair.pool_stable_reserve = 0;
    pair.fee_bps = fee_bps;
    pair.creator = ctx.accounts.creator.key();
    pair.total_lp_shares = 0;
    pair.paused = false;
    pair.bump = ctx.bumps.stable_pair;
    pair.token_vault_bump = ctx.bumps.token_vault;
    pair.stable_vault_bump = ctx.bumps.stable_vault;

    emit!(StablePairCreated {
        token_mint: pair.token_mint,
        stable_mint: pair.stable_mint,
        creator: pair.creator,
        fee_bps,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

/// Swap project token → stablecoin.
pub fn handle_swap_token_for_stable(
    ctx: Context<SwapTokenForStable>,
    amount_in: u64,
    min_amount_out: u64,
) -> Result<()> {
    require!(amount_in > 0, StablePairError::ZeroAmount);
    let pair = &ctx.accounts.stable_pair;
    require!(!pair.paused, StablePairError::Paused);

    let (amount_out, fee) = compute_swap_output(
        amount_in,
        pair.pool_token_reserve,
        pair.pool_stable_reserve,
        pair.fee_bps,
    )?;
    require!(amount_out > 0, StablePairError::InsufficientOutput);
    require!(amount_out >= min_amount_out, StablePairError::SlippageExceeded);

    // Transfer tokens in: user → pool token vault
    token::transfer(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.user_token_account.to_account_info(),
                to: ctx.accounts.token_vault.to_account_info(),
                authority: ctx.accounts.user.to_account_info(),
            },
        ),
        amount_in,
    )?;

    // Transfer stables out: pool stable vault → user  (PDA signer)
    let token_mint_key = ctx.accounts.stable_pair.token_mint;
    let stable_mint_key = ctx.accounts.stable_pair.stable_mint;
    let pair_seeds: &[&[u8]] = &[
        STABLE_PAIR_SEED,
        token_mint_key.as_ref(),
        stable_mint_key.as_ref(),
        &[ctx.accounts.stable_pair.bump],
    ];
    let signer = &[pair_seeds];

    // Send amount_out minus platform fee share to user, fee to treasury
    let treasury_fee = fee / 2; // 50% of fee to platform treasury
    let user_receives = amount_out; // fee already deducted in compute_swap_output

    token::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.stable_vault.to_account_info(),
                to: ctx.accounts.user_stable_account.to_account_info(),
                authority: ctx.accounts.stable_pair.to_account_info(),
            },
            signer,
        ),
        user_receives,
    )?;

    // Transfer treasury fee (in stablecoins) to treasury vault
    if treasury_fee > 0 {
        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.stable_vault.to_account_info(),
                    to: ctx.accounts.treasury_stable_account.to_account_info(),
                    authority: ctx.accounts.stable_pair.to_account_info(),
                },
                signer,
            ),
            treasury_fee,
        )?;
    }

    // Update reserves
    let pair = &mut ctx.accounts.stable_pair;
    pair.pool_token_reserve = pair.pool_token_reserve
        .checked_add(amount_in)
        .ok_or(StablePairError::MathOverflow)?;
    pair.pool_stable_reserve = pair.pool_stable_reserve
        .checked_sub(amount_out)
        .ok_or(StablePairError::MathOverflow)?
        .checked_sub(treasury_fee)
        .ok_or(StablePairError::MathOverflow)?;

    let clock = Clock::get()?;
    emit!(SwapExecuted {
        pair: pair.key(),
        user: ctx.accounts.user.key(),
        token_to_stable: true,
        amount_in,
        amount_out: user_receives,
        fee_amount: fee,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

/// Swap stablecoin → project token.
pub fn handle_swap_stable_for_token(
    ctx: Context<SwapStableForToken>,
    amount_in: u64,
    min_amount_out: u64,
) -> Result<()> {
    require!(amount_in > 0, StablePairError::ZeroAmount);
    let pair = &ctx.accounts.stable_pair;
    require!(!pair.paused, StablePairError::Paused);

    let (amount_out, fee) = compute_swap_output(
        amount_in,
        pair.pool_stable_reserve,
        pair.pool_token_reserve,
        pair.fee_bps,
    )?;
    require!(amount_out > 0, StablePairError::InsufficientOutput);
    require!(amount_out >= min_amount_out, StablePairError::SlippageExceeded);

    // Transfer stables in: user → pool stable vault
    token::transfer(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.user_stable_account.to_account_info(),
                to: ctx.accounts.stable_vault.to_account_info(),
                authority: ctx.accounts.user.to_account_info(),
            },
        ),
        amount_in,
    )?;

    // Transfer tokens out: pool token vault → user (PDA signer)
    let token_mint_key = ctx.accounts.stable_pair.token_mint;
    let stable_mint_key = ctx.accounts.stable_pair.stable_mint;
    let pair_seeds: &[&[u8]] = &[
        STABLE_PAIR_SEED,
        token_mint_key.as_ref(),
        stable_mint_key.as_ref(),
        &[ctx.accounts.stable_pair.bump],
    ];
    let signer = &[pair_seeds];

    let treasury_fee = fee / 2;
    let user_receives = amount_out;

    token::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.token_vault.to_account_info(),
                to: ctx.accounts.user_token_account.to_account_info(),
                authority: ctx.accounts.stable_pair.to_account_info(),
            },
            signer,
        ),
        user_receives,
    )?;

    // Treasury fee in tokens
    if treasury_fee > 0 {
        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.token_vault.to_account_info(),
                    to: ctx.accounts.treasury_token_account.to_account_info(),
                    authority: ctx.accounts.stable_pair.to_account_info(),
                },
                signer,
            ),
            treasury_fee,
        )?;
    }

    // Update reserves
    let pair = &mut ctx.accounts.stable_pair;
    pair.pool_stable_reserve = pair.pool_stable_reserve
        .checked_add(amount_in)
        .ok_or(StablePairError::MathOverflow)?;
    pair.pool_token_reserve = pair.pool_token_reserve
        .checked_sub(amount_out)
        .ok_or(StablePairError::MathOverflow)?
        .checked_sub(treasury_fee)
        .ok_or(StablePairError::MathOverflow)?;

    let clock = Clock::get()?;
    emit!(SwapExecuted {
        pair: pair.key(),
        user: ctx.accounts.user.key(),
        token_to_stable: false,
        amount_in,
        amount_out: user_receives,
        fee_amount: fee,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

/// Add liquidity to a stable pair. Deposits both tokens proportionally
/// (or arbitrary amounts on first deposit) and mints virtual LP shares.
pub fn handle_add_liquidity(
    ctx: Context<AddLiquidity>,
    token_amount: u64,
    stable_amount: u64,
    min_lp_shares: u128,
) -> Result<()> {
    require!(token_amount > 0 && stable_amount > 0, StablePairError::ZeroAmount);
    let pair = &ctx.accounts.stable_pair;
    require!(!pair.paused, StablePairError::Paused);

    // Calculate LP shares to mint
    let lp_shares: u128 = if pair.total_lp_shares == 0 {
        // First deposit — shares = sqrt(token_amount * stable_amount)
        let product = (token_amount as u128)
            .checked_mul(stable_amount as u128)
            .ok_or(StablePairError::MathOverflow)?;
        let shares = isqrt(product);
        require!(shares >= MIN_LIQUIDITY as u128, StablePairError::InitialLiquidityTooLow);
        shares
    } else {
        // Proportional deposit: shares = min(dT/T, dS/S) * total_shares
        let share_by_token = (token_amount as u128)
            .checked_mul(pair.total_lp_shares)
            .ok_or(StablePairError::MathOverflow)?
            .checked_div(pair.pool_token_reserve as u128)
            .ok_or(StablePairError::EmptyPool)?;
        let share_by_stable = (stable_amount as u128)
            .checked_mul(pair.total_lp_shares)
            .ok_or(StablePairError::MathOverflow)?
            .checked_div(pair.pool_stable_reserve as u128)
            .ok_or(StablePairError::EmptyPool)?;
        share_by_token.min(share_by_stable)
    };
    require!(lp_shares >= min_lp_shares, StablePairError::SlippageExceeded);

    // Transfer tokens in
    token::transfer(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.user_token_account.to_account_info(),
                to: ctx.accounts.token_vault.to_account_info(),
                authority: ctx.accounts.provider.to_account_info(),
            },
        ),
        token_amount,
    )?;

    // Transfer stables in
    token::transfer(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.user_stable_account.to_account_info(),
                to: ctx.accounts.stable_vault.to_account_info(),
                authority: ctx.accounts.provider.to_account_info(),
            },
        ),
        stable_amount,
    )?;

    // Update pair state
    let pair = &mut ctx.accounts.stable_pair;
    pair.pool_token_reserve = pair.pool_token_reserve
        .checked_add(token_amount)
        .ok_or(StablePairError::MathOverflow)?;
    pair.pool_stable_reserve = pair.pool_stable_reserve
        .checked_add(stable_amount)
        .ok_or(StablePairError::MathOverflow)?;
    pair.total_lp_shares = pair.total_lp_shares
        .checked_add(lp_shares)
        .ok_or(StablePairError::MathOverflow)?;

    // Update user LP position
    let lp_pos = &mut ctx.accounts.lp_position;
    lp_pos.pair = ctx.accounts.stable_pair.key();
    lp_pos.owner = ctx.accounts.provider.key();
    lp_pos.lp_shares = lp_pos.lp_shares
        .checked_add(lp_shares)
        .ok_or(StablePairError::MathOverflow)?;
    if lp_pos.bump == 0 {
        lp_pos.bump = ctx.bumps.lp_position;
    }

    let clock = Clock::get()?;
    emit!(LiquidityAdded {
        pair: ctx.accounts.stable_pair.key(),
        provider: ctx.accounts.provider.key(),
        token_amount,
        stable_amount,
        lp_shares_minted: lp_shares,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

/// Remove liquidity from a stable pair by burning LP shares and
/// receiving proportional reserves back.
pub fn handle_remove_liquidity(
    ctx: Context<RemoveLiquidity>,
    lp_shares_to_burn: u128,
    min_token_out: u64,
    min_stable_out: u64,
) -> Result<()> {
    require!(lp_shares_to_burn > 0, StablePairError::ZeroAmount);

    let lp_pos = &ctx.accounts.lp_position;
    require!(lp_pos.lp_shares >= lp_shares_to_burn, StablePairError::InsufficientShares);

    let pair = &ctx.accounts.stable_pair;

    // Calculate proportional amounts
    let token_out = (pair.pool_token_reserve as u128)
        .checked_mul(lp_shares_to_burn)
        .ok_or(StablePairError::MathOverflow)?
        .checked_div(pair.total_lp_shares)
        .ok_or(StablePairError::EmptyPool)? as u64;

    let stable_out = (pair.pool_stable_reserve as u128)
        .checked_mul(lp_shares_to_burn)
        .ok_or(StablePairError::MathOverflow)?
        .checked_div(pair.total_lp_shares)
        .ok_or(StablePairError::EmptyPool)? as u64;

    require!(token_out >= min_token_out, StablePairError::SlippageExceeded);
    require!(stable_out >= min_stable_out, StablePairError::SlippageExceeded);

    // PDA signer
    let token_mint_key = pair.token_mint;
    let stable_mint_key = pair.stable_mint;
    let pair_seeds: &[&[u8]] = &[
        STABLE_PAIR_SEED,
        token_mint_key.as_ref(),
        stable_mint_key.as_ref(),
        &[pair.bump],
    ];
    let signer = &[pair_seeds];

    // Transfer tokens out
    token::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.token_vault.to_account_info(),
                to: ctx.accounts.user_token_account.to_account_info(),
                authority: ctx.accounts.stable_pair.to_account_info(),
            },
            signer,
        ),
        token_out,
    )?;

    // Transfer stables out
    token::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.stable_vault.to_account_info(),
                to: ctx.accounts.user_stable_account.to_account_info(),
                authority: ctx.accounts.stable_pair.to_account_info(),
            },
            signer,
        ),
        stable_out,
    )?;

    // Update pair state
    let pair = &mut ctx.accounts.stable_pair;
    pair.pool_token_reserve = pair.pool_token_reserve
        .checked_sub(token_out)
        .ok_or(StablePairError::MathOverflow)?;
    pair.pool_stable_reserve = pair.pool_stable_reserve
        .checked_sub(stable_out)
        .ok_or(StablePairError::MathOverflow)?;
    pair.total_lp_shares = pair.total_lp_shares
        .checked_sub(lp_shares_to_burn)
        .ok_or(StablePairError::MathOverflow)?;

    // Update user LP position
    let lp_pos = &mut ctx.accounts.lp_position;
    lp_pos.lp_shares = lp_pos.lp_shares
        .checked_sub(lp_shares_to_burn)
        .ok_or(StablePairError::MathOverflow)?;

    let clock = Clock::get()?;
    emit!(LiquidityRemoved {
        pair: ctx.accounts.stable_pair.key(),
        provider: ctx.accounts.provider.key(),
        token_amount: token_out,
        stable_amount: stable_out,
        lp_shares_burned: lp_shares_to_burn,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

/// Pause or unpause a stable pair (creator only).
pub fn handle_set_pair_paused(
    ctx: Context<PairAdmin>,
    paused: bool,
) -> Result<()> {
    ctx.accounts.stable_pair.paused = paused;
    Ok(())
}

/// Update fee on a stable pair (creator only).
pub fn handle_update_pair_fee(
    ctx: Context<PairAdmin>,
    new_fee_bps: u16,
) -> Result<()> {
    require!(new_fee_bps <= MAX_FEE_BPS, StablePairError::FeeTooHigh);
    ctx.accounts.stable_pair.fee_bps = new_fee_bps;
    Ok(())
}

// ============================================================================
// AMM MATH
// ============================================================================

/// Constant product swap: given `amount_in` of asset A, compute output of
/// asset B after fee deduction.
///
/// Formula: `amount_out = (reserve_out * amount_in_after_fee) / (reserve_in + amount_in_after_fee)`
///
/// Fee is taken from `amount_in` before the swap calculation.
/// Returns `(amount_out, fee_amount)`.
fn compute_swap_output(
    amount_in: u64,
    reserve_in: u64,
    reserve_out: u64,
    fee_bps: u16,
) -> Result<(u64, u64)> {
    require!(reserve_in > 0 && reserve_out > 0, StablePairError::EmptyPool);

    let fee = (amount_in as u128)
        .checked_mul(fee_bps as u128)
        .ok_or(StablePairError::MathOverflow)?
        .checked_div(BPS_DENOMINATOR as u128)
        .ok_or(StablePairError::MathOverflow)? as u64;

    let amount_in_after_fee = (amount_in as u128)
        .checked_sub(fee as u128)
        .ok_or(StablePairError::MathOverflow)?;

    let numerator = (reserve_out as u128)
        .checked_mul(amount_in_after_fee)
        .ok_or(StablePairError::MathOverflow)?;

    let denominator = (reserve_in as u128)
        .checked_add(amount_in_after_fee)
        .ok_or(StablePairError::MathOverflow)?;

    let amount_out = numerator
        .checked_div(denominator)
        .ok_or(StablePairError::MathOverflow)? as u64;

    Ok((amount_out, fee))
}

/// Integer square root via Newton's method.
fn isqrt(n: u128) -> u128 {
    if n == 0 {
        return 0;
    }
    let mut x = n;
    let mut y = (x + 1) / 2;
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x
}

// ============================================================================
// CONTEXT STRUCTS
// ============================================================================

#[derive(Accounts)]
pub struct CreateStablePair<'info> {
    #[account(
        init,
        payer = creator,
        space = StablePairConfig::SIZE,
        seeds = [STABLE_PAIR_SEED, token_mint.key().as_ref(), stable_mint.key().as_ref()],
        bump,
    )]
    pub stable_pair: Account<'info, StablePairConfig>,

    pub token_mint: Account<'info, Mint>,
    pub stable_mint: Account<'info, Mint>,

    /// Token vault owned by the pair PDA.
    #[account(
        init,
        payer = creator,
        token::mint = token_mint,
        token::authority = stable_pair,
        seeds = [POOL_TOKEN_VAULT_SEED, stable_pair.key().as_ref()],
        bump,
    )]
    pub token_vault: Account<'info, TokenAccount>,

    /// Stablecoin vault owned by the pair PDA.
    #[account(
        init,
        payer = creator,
        token::mint = stable_mint,
        token::authority = stable_pair,
        seeds = [POOL_STABLE_VAULT_SEED, stable_pair.key().as_ref()],
        bump,
    )]
    pub stable_vault: Account<'info, TokenAccount>,

    #[account(mut)]
    pub creator: Signer<'info>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct SwapTokenForStable<'info> {
    #[account(
        mut,
        seeds = [STABLE_PAIR_SEED, stable_pair.token_mint.as_ref(), stable_pair.stable_mint.as_ref()],
        bump = stable_pair.bump,
    )]
    pub stable_pair: Account<'info, StablePairConfig>,

    #[account(
        mut,
        seeds = [POOL_TOKEN_VAULT_SEED, stable_pair.key().as_ref()],
        bump = stable_pair.token_vault_bump,
    )]
    pub token_vault: Account<'info, TokenAccount>,

    #[account(
        mut,
        seeds = [POOL_STABLE_VAULT_SEED, stable_pair.key().as_ref()],
        bump = stable_pair.stable_vault_bump,
    )]
    pub stable_vault: Account<'info, TokenAccount>,

    /// User's token account (source of tokens being sold).
    #[account(
        mut,
        constraint = user_token_account.mint == stable_pair.token_mint,
    )]
    pub user_token_account: Account<'info, TokenAccount>,

    /// User's stablecoin account (receives stablecoins).
    #[account(
        mut,
        constraint = user_stable_account.mint == stable_pair.stable_mint,
    )]
    pub user_stable_account: Account<'info, TokenAccount>,

    /// Platform treasury stablecoin account for fee collection.
    #[account(
        mut,
        constraint = treasury_stable_account.mint == stable_pair.stable_mint,
    )]
    pub treasury_stable_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub user: Signer<'info>,

    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct SwapStableForToken<'info> {
    #[account(
        mut,
        seeds = [STABLE_PAIR_SEED, stable_pair.token_mint.as_ref(), stable_pair.stable_mint.as_ref()],
        bump = stable_pair.bump,
    )]
    pub stable_pair: Account<'info, StablePairConfig>,

    #[account(
        mut,
        seeds = [POOL_TOKEN_VAULT_SEED, stable_pair.key().as_ref()],
        bump = stable_pair.token_vault_bump,
    )]
    pub token_vault: Account<'info, TokenAccount>,

    #[account(
        mut,
        seeds = [POOL_STABLE_VAULT_SEED, stable_pair.key().as_ref()],
        bump = stable_pair.stable_vault_bump,
    )]
    pub stable_vault: Account<'info, TokenAccount>,

    /// User's stablecoin account (source).
    #[account(
        mut,
        constraint = user_stable_account.mint == stable_pair.stable_mint,
    )]
    pub user_stable_account: Account<'info, TokenAccount>,

    /// User's token account (receives tokens).
    #[account(
        mut,
        constraint = user_token_account.mint == stable_pair.token_mint,
    )]
    pub user_token_account: Account<'info, TokenAccount>,

    /// Platform treasury token account for fee collection.
    #[account(
        mut,
        constraint = treasury_token_account.mint == stable_pair.token_mint,
    )]
    pub treasury_token_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub user: Signer<'info>,

    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct AddLiquidity<'info> {
    #[account(
        mut,
        seeds = [STABLE_PAIR_SEED, stable_pair.token_mint.as_ref(), stable_pair.stable_mint.as_ref()],
        bump = stable_pair.bump,
    )]
    pub stable_pair: Account<'info, StablePairConfig>,

    #[account(
        mut,
        seeds = [POOL_TOKEN_VAULT_SEED, stable_pair.key().as_ref()],
        bump = stable_pair.token_vault_bump,
    )]
    pub token_vault: Account<'info, TokenAccount>,

    #[account(
        mut,
        seeds = [POOL_STABLE_VAULT_SEED, stable_pair.key().as_ref()],
        bump = stable_pair.stable_vault_bump,
    )]
    pub stable_vault: Account<'info, TokenAccount>,

    #[account(
        init_if_needed,
        payer = provider,
        space = LPPosition::SIZE,
        seeds = [LP_POSITION_SEED, stable_pair.key().as_ref(), provider.key().as_ref()],
        bump,
    )]
    pub lp_position: Account<'info, LPPosition>,

    #[account(
        mut,
        constraint = user_token_account.mint == stable_pair.token_mint,
    )]
    pub user_token_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        constraint = user_stable_account.mint == stable_pair.stable_mint,
    )]
    pub user_stable_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub provider: Signer<'info>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct RemoveLiquidity<'info> {
    #[account(
        mut,
        seeds = [STABLE_PAIR_SEED, stable_pair.token_mint.as_ref(), stable_pair.stable_mint.as_ref()],
        bump = stable_pair.bump,
    )]
    pub stable_pair: Account<'info, StablePairConfig>,

    #[account(
        mut,
        seeds = [POOL_TOKEN_VAULT_SEED, stable_pair.key().as_ref()],
        bump = stable_pair.token_vault_bump,
    )]
    pub token_vault: Account<'info, TokenAccount>,

    #[account(
        mut,
        seeds = [POOL_STABLE_VAULT_SEED, stable_pair.key().as_ref()],
        bump = stable_pair.stable_vault_bump,
    )]
    pub stable_vault: Account<'info, TokenAccount>,

    #[account(
        mut,
        seeds = [LP_POSITION_SEED, stable_pair.key().as_ref(), provider.key().as_ref()],
        bump = lp_position.bump,
        constraint = lp_position.owner == provider.key(),
    )]
    pub lp_position: Account<'info, LPPosition>,

    #[account(
        mut,
        constraint = user_token_account.mint == stable_pair.token_mint,
    )]
    pub user_token_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        constraint = user_stable_account.mint == stable_pair.stable_mint,
    )]
    pub user_stable_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub provider: Signer<'info>,

    pub token_program: Program<'info, Token>,
}

/// Admin context — only the pair creator can call.
#[derive(Accounts)]
pub struct PairAdmin<'info> {
    #[account(
        mut,
        seeds = [STABLE_PAIR_SEED, stable_pair.token_mint.as_ref(), stable_pair.stable_mint.as_ref()],
        bump = stable_pair.bump,
        constraint = stable_pair.creator == authority.key() @ StablePairError::Unauthorized,
    )]
    pub stable_pair: Account<'info, StablePairConfig>,

    pub authority: Signer<'info>,
}
