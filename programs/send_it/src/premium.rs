use anchor_lang::prelude::*;
use anchor_lang::system_program;

// ============================================================================
// SEEDS & CONSTANTS
// ============================================================================

pub const PREMIUM_LISTING_SEED: &[u8] = b"premium_listing";
pub const PREMIUM_CONFIG_SEED: &[u8] = b"premium_config";

pub const LAMPORTS_PER_SOL: u64 = 1_000_000_000;

// ============================================================================
// ENUMS
// ============================================================================

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
pub enum PremiumTier {
    /// Basic promotion — appears in "Promoted" section.
    Promoted,
    /// Mid-tier — featured on homepage carousel.
    Featured,
    /// Top tier — spotlight banner placement.
    Spotlight,
}

impl PremiumTier {
    /// Price per hour in lamports.
    pub fn price_per_hour(&self, config: &PremiumConfig) -> u64 {
        match self {
            PremiumTier::Promoted => config.promoted_price_per_hour,
            PremiumTier::Featured => config.featured_price_per_hour,
            PremiumTier::Spotlight => config.spotlight_price_per_hour,
        }
    }
}

// ============================================================================
// ACCOUNTS
// ============================================================================

/// Global premium listing configuration.
#[account]
pub struct PremiumConfig {
    pub authority: Pubkey,
    pub treasury: Pubkey,
    /// Price per hour in lamports for each tier.
    pub promoted_price_per_hour: u64,
    pub featured_price_per_hour: u64,
    pub spotlight_price_per_hour: u64,
    pub bump: u8,
}

impl PremiumConfig {
    // 8 + 32 + 32 + 8 + 8 + 8 + 1
    pub const SIZE: usize = 8 + 32 + 32 + 8 + 8 + 8 + 1;
}

/// Per-token premium listing, PDA from [PREMIUM_LISTING_SEED, token_mint].
#[account]
pub struct PremiumListing {
    /// The token mint this listing is for.
    pub token_mint: Pubkey,
    /// Who purchased the premium listing.
    pub purchaser: Pubkey,
    /// Premium tier.
    pub tier: PremiumTier,
    /// Unix timestamp when the listing started.
    pub start_time: i64,
    /// Duration in seconds.
    pub duration: i64,
    /// Total SOL paid (lamports).
    pub amount_paid: u64,
    /// Whether the listing is currently active.
    pub active: bool,
    /// Bump seed.
    pub bump: u8,
}

impl PremiumListing {
    // 8 + 32 + 32 + 1 + 8 + 8 + 8 + 1 + 1
    pub const SIZE: usize = 8 + 32 + 32 + 1 + 8 + 8 + 8 + 1 + 1;
}

// ============================================================================
// INSTRUCTION CONTEXTS
// ============================================================================

#[derive(Accounts)]
pub struct InitializePremiumConfig<'info> {
    #[account(
        init,
        payer = authority,
        space = PremiumConfig::SIZE,
        seeds = [PREMIUM_CONFIG_SEED],
        bump,
    )]
    pub config: Account<'info, PremiumConfig>,
    /// CHECK: Treasury to receive premium payments.
    pub treasury: AccountInfo<'info>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct PurchasePremium<'info> {
    #[account(
        init_if_needed,
        payer = purchaser,
        space = PremiumListing::SIZE,
        seeds = [PREMIUM_LISTING_SEED, token_mint.key().as_ref()],
        bump,
    )]
    pub premium_listing: Account<'info, PremiumListing>,
    #[account(
        seeds = [PREMIUM_CONFIG_SEED],
        bump = config.bump,
    )]
    pub config: Account<'info, PremiumConfig>,
    /// CHECK: The token mint to feature.
    pub token_mint: AccountInfo<'info>,
    /// CHECK: Treasury receives payment. Validated against config.
    #[account(
        mut,
        constraint = treasury.key() == config.treasury @ PremiumError::InvalidTreasury,
    )]
    pub treasury: AccountInfo<'info>,
    #[account(mut)]
    pub purchaser: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct CheckPremiumStatus<'info> {
    #[account(
        mut,
        seeds = [PREMIUM_LISTING_SEED, premium_listing.token_mint.as_ref()],
        bump = premium_listing.bump,
    )]
    pub premium_listing: Account<'info, PremiumListing>,
}

// ============================================================================
// INSTRUCTIONS
// ============================================================================

pub fn handle_initialize_premium_config(
    ctx: Context<InitializePremiumConfig>,
    promoted_price_per_hour: u64,
    featured_price_per_hour: u64,
    spotlight_price_per_hour: u64,
) -> Result<()> {
    let config = &mut ctx.accounts.config;
    config.authority = ctx.accounts.authority.key();
    config.treasury = ctx.accounts.treasury.key();
    config.promoted_price_per_hour = promoted_price_per_hour;
    config.featured_price_per_hour = featured_price_per_hour;
    config.spotlight_price_per_hour = spotlight_price_per_hour;
    config.bump = ctx.bumps.config;
    Ok(())
}

pub fn handle_purchase_premium(
    ctx: Context<PurchasePremium>,
    tier: PremiumTier,
    duration_hours: u64,
) -> Result<()> {
    require!(duration_hours > 0, PremiumError::InvalidDuration);
    require!(duration_hours <= 720, PremiumError::InvalidDuration); // Max 30 days

    let clock = Clock::get()?;
    let config = &ctx.accounts.config;
    let listing = &mut ctx.accounts.premium_listing;

    // If an existing listing is still active, extend it
    let current_end = if listing.active {
        let end = listing.start_time + listing.duration;
        if clock.unix_timestamp < end {
            end
        } else {
            clock.unix_timestamp
        }
    } else {
        clock.unix_timestamp
    };

    let price_per_hour = tier.price_per_hour(config);
    let total_cost = price_per_hour.checked_mul(duration_hours).ok_or(PremiumError::Overflow)?;

    // Transfer SOL to treasury
    system_program::transfer(
        CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            system_program::Transfer {
                from: ctx.accounts.purchaser.to_account_info(),
                to: ctx.accounts.treasury.to_account_info(),
            },
        ),
        total_cost,
    )?;

    let duration_seconds = (duration_hours as i64).checked_mul(3600).unwrap();

    listing.token_mint = ctx.accounts.token_mint.key();
    listing.purchaser = ctx.accounts.purchaser.key();
    listing.tier = tier;
    listing.start_time = current_end;
    listing.duration = if listing.active {
        listing.duration.checked_add(duration_seconds).unwrap()
    } else {
        duration_seconds
    };
    listing.amount_paid = listing.amount_paid.checked_add(total_cost).unwrap();
    listing.active = true;
    listing.bump = ctx.bumps.premium_listing;

    emit!(PremiumPurchased {
        token_mint: listing.token_mint,
        purchaser: listing.purchaser,
        tier,
        duration_hours,
        amount_paid: total_cost,
        expires_at: listing.start_time + listing.duration,
    });

    Ok(())
}

/// Check if a premium listing is still active; deactivate if expired.
pub fn handle_check_premium_status(ctx: Context<CheckPremiumStatus>) -> Result<bool> {
    let clock = Clock::get()?;
    let listing = &mut ctx.accounts.premium_listing;

    if !listing.active {
        return Ok(false);
    }

    let expires_at = listing.start_time + listing.duration;
    if clock.unix_timestamp >= expires_at {
        listing.active = false;

        emit!(PremiumExpired {
            token_mint: listing.token_mint,
            expired_at: clock.unix_timestamp,
        });

        return Ok(false);
    }

    Ok(true)
}

// ============================================================================
// EVENTS
// ============================================================================

#[event]
pub struct PremiumPurchased {
    pub token_mint: Pubkey,
    pub purchaser: Pubkey,
    pub tier: PremiumTier,
    pub duration_hours: u64,
    pub amount_paid: u64,
    pub expires_at: i64,
}

#[event]
pub struct PremiumExpired {
    pub token_mint: Pubkey,
    pub expired_at: i64,
}

// ============================================================================
// ERRORS
// ============================================================================

#[error_code]
pub enum PremiumError {
    #[msg("Invalid duration (must be 1-720 hours)")]
    InvalidDuration,
    #[msg("Treasury account does not match config")]
    InvalidTreasury,
    #[msg("Arithmetic overflow")]
    Overflow,
    #[msg("Premium listing is not active")]
    NotActive,
}
