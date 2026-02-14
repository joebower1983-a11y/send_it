use anchor_lang::prelude::*;

// ============================================================================
// SEEDS & CONSTANTS
// ============================================================================

pub const CREATOR_ANALYTICS_SEED: &[u8] = b"creator_analytics";
pub const TOKEN_ANALYTICS_SNAPSHOT_SEED: &[u8] = b"token_analytics_snapshot";

/// Number of hourly slots tracked in the rolling snapshot.
pub const HOURLY_SLOTS: usize = 168; // 7 days of hourly data

// ============================================================================
// ACCOUNTS
// ============================================================================

/// Aggregate analytics for a single creator wallet.
#[account]
#[derive(Default)]
pub struct CreatorAnalytics {
    /// The creator wallet this PDA tracks.
    pub creator: Pubkey,
    /// Total number of token launches by this creator.
    pub total_launches: u32,
    /// Cumulative trading volume across all creator tokens (lamports).
    pub total_volume_generated: u64,
    /// Total fees earned by the creator (lamports).
    pub total_fees_earned: u64,
    /// Sum of unique holders across all creator tokens.
    pub total_holders_across_tokens: u32,
    /// Mint address of the creator's best-performing token (by volume).
    pub best_performing_token: Pubkey,
    /// Average time-to-graduation in seconds (0 if none graduated).
    pub avg_graduation_time: i64,
    /// PDA bump.
    pub bump: u8,
}

impl CreatorAnalytics {
    pub const SIZE: usize = 8  // discriminator
        + 32 // creator
        + 4  // total_launches
        + 8  // total_volume_generated
        + 8  // total_fees_earned
        + 4  // total_holders_across_tokens
        + 32 // best_performing_token
        + 8  // avg_graduation_time
        + 1; // bump
}

/// Per-token rolling analytics snapshot used for charts.
#[account]
#[derive(Default)]
pub struct TokenAnalyticsSnapshot {
    /// The token mint this snapshot tracks.
    pub token_mint: Pubkey,
    /// Rolling hourly volume array (lamports). Index = hour_index % HOURLY_SLOTS.
    pub hourly_volume: Vec<u64>,
    /// Rolling hourly holder growth (delta). Same indexing.
    pub holder_growth: Vec<i32>,
    /// Current write cursor (hour_index).
    pub current_slot: u16,
    /// PDA bump.
    pub bump: u8,
}

impl TokenAnalyticsSnapshot {
    pub const SIZE: usize = 8  // discriminator
        + 32 // token_mint
        + 4 + (HOURLY_SLOTS * 8) // Vec<u64> hourly_volume
        + 4 + (HOURLY_SLOTS * 4) // Vec<i32> holder_growth
        + 2  // current_slot
        + 1; // bump
}

// ============================================================================
// EVENTS
// ============================================================================

#[event]
pub struct CreatorStatsEvent {
    pub creator: Pubkey,
    pub total_launches: u32,
    pub total_volume_generated: u64,
    pub total_fees_earned: u64,
    pub total_holders_across_tokens: u32,
    pub best_performing_token: Pubkey,
    pub avg_graduation_time: i64,
}

// ============================================================================
// INSTRUCTIONS
// ============================================================================

/// Permissionless crank to update a creator's aggregate analytics.
pub fn update_creator_analytics(
    ctx: Context<UpdateCreatorAnalytics>,
    total_launches: u32,
    total_volume_generated: u64,
    total_fees_earned: u64,
    total_holders_across_tokens: u32,
    best_performing_token: Pubkey,
    avg_graduation_time: i64,
    // Optional: snapshot data for the token
    hourly_volume_entry: u64,
    holder_growth_entry: i32,
) -> Result<()> {
    // --- Creator-level analytics ---
    let analytics = &mut ctx.accounts.creator_analytics;
    analytics.creator = ctx.accounts.creator.key();
    analytics.total_launches = total_launches;
    analytics.total_volume_generated = total_volume_generated;
    analytics.total_fees_earned = total_fees_earned;
    analytics.total_holders_across_tokens = total_holders_across_tokens;
    analytics.best_performing_token = best_performing_token;
    analytics.avg_graduation_time = avg_graduation_time;
    analytics.bump = ctx.bumps.creator_analytics;

    // --- Token-level snapshot ---
    let snapshot = &mut ctx.accounts.token_analytics_snapshot;
    snapshot.token_mint = ctx.accounts.token_mint.key();
    snapshot.bump = ctx.bumps.token_analytics_snapshot;

    // Initialise vectors on first use
    if snapshot.hourly_volume.is_empty() {
        snapshot.hourly_volume = vec![0u64; HOURLY_SLOTS];
        snapshot.holder_growth = vec![0i32; HOURLY_SLOTS];
    }

    let idx = snapshot.current_slot as usize % HOURLY_SLOTS;
    snapshot.hourly_volume[idx] = hourly_volume_entry;
    snapshot.holder_growth[idx] = holder_growth_entry;
    snapshot.current_slot = snapshot.current_slot.wrapping_add(1);

    Ok(())
}

/// Emits an event with the creator's aggregate stats for frontend consumption.
pub fn get_creator_stats(ctx: Context<GetCreatorStats>) -> Result<()> {
    let a = &ctx.accounts.creator_analytics;

    emit!(CreatorStatsEvent {
        creator: a.creator,
        total_launches: a.total_launches,
        total_volume_generated: a.total_volume_generated,
        total_fees_earned: a.total_fees_earned,
        total_holders_across_tokens: a.total_holders_across_tokens,
        best_performing_token: a.best_performing_token,
        avg_graduation_time: a.avg_graduation_time,
    });

    Ok(())
}

// ============================================================================
// CONTEXT STRUCTS
// ============================================================================

#[derive(Accounts)]
pub struct UpdateCreatorAnalytics<'info> {
    #[account(
        init_if_needed,
        payer = payer,
        space = CreatorAnalytics::SIZE,
        seeds = [CREATOR_ANALYTICS_SEED, creator.key().as_ref()],
        bump,
    )]
    pub creator_analytics: Account<'info, CreatorAnalytics>,

    #[account(
        init_if_needed,
        payer = payer,
        space = TokenAnalyticsSnapshot::SIZE,
        seeds = [TOKEN_ANALYTICS_SNAPSHOT_SEED, token_mint.key().as_ref()],
        bump,
    )]
    pub token_analytics_snapshot: Account<'info, TokenAnalyticsSnapshot>,

    /// CHECK: Creator wallet — validated by seeds.
    pub creator: UncheckedAccount<'info>,

    /// CHECK: Token mint — validated by seeds.
    pub token_mint: UncheckedAccount<'info>,

    #[account(mut)]
    pub payer: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct GetCreatorStats<'info> {
    #[account(
        seeds = [CREATOR_ANALYTICS_SEED, creator.key().as_ref()],
        bump = creator_analytics.bump,
    )]
    pub creator_analytics: Account<'info, CreatorAnalytics>,

    /// CHECK: Creator wallet — validated by seeds.
    pub creator: UncheckedAccount<'info>,
}
