use anchor_lang::prelude::*;

// ============================================================================
// SEEDS & CONSTANTS
// ============================================================================

pub const USER_ACHIEVEMENTS_SEED: &[u8] = b"user_achievements";

// Achievement bitflags
pub const FIRST_LAUNCH: u16    = 1 << 0;  // Launched their first token
pub const DIAMOND_HANDS: u16   = 1 << 1;  // Held a token 30+ days
pub const WHALE_STATUS: u16    = 1 << 2;  // >10 SOL cumulative volume
pub const DEGEN_100: u16       = 1 << 3;  // 100 trades completed
pub const EARLY_ADOPTER: u16   = 1 << 4;  // Among first 1000 users

pub const DIAMOND_HANDS_SECONDS: i64 = 30 * 24 * 60 * 60; // 30 days
pub const WHALE_VOLUME_LAMPORTS: u64 = 10 * 1_000_000_000; // 10 SOL
pub const DEGEN_TRADE_COUNT: u64 = 100;
pub const EARLY_ADOPTER_LIMIT: u64 = 1000;

// ============================================================================
// ACCOUNTS
// ============================================================================

/// Global counter for early adopter tracking.
#[account]
#[derive(Default)]
pub struct AchievementConfig {
    pub total_users: u64,
    pub authority: Pubkey,
    pub bump: u8,
}

impl AchievementConfig {
    pub const SIZE: usize = 8 + 8 + 32 + 1;
}

/// Per-user achievement state, PDA from [USER_ACHIEVEMENTS_SEED, user].
#[account]
#[derive(Default)]
pub struct UserAchievements {
    /// The user this account belongs to.
    pub user: Pubkey,
    /// Bitflags of unlocked achievements.
    pub badges: u16,
    /// Total trade count (for Degen100).
    pub trade_count: u64,
    /// Total volume in lamports (for WhaleStatus).
    pub total_volume: u64,
    /// Number of tokens launched (for FirstLaunch).
    pub tokens_launched: u64,
    /// Earliest position open timestamp (for DiamondHands tracking).
    pub earliest_hold_start: i64,
    /// Timestamp of account creation.
    pub created_at: i64,
    /// Bump seed.
    pub bump: u8,
}

impl UserAchievements {
    // 8 + 32 + 2 + 8 + 8 + 8 + 8 + 8 + 1
    pub const SIZE: usize = 8 + 32 + 2 + 8 + 8 + 8 + 8 + 8 + 1;
}

// ============================================================================
// INSTRUCTION CONTEXTS
// ============================================================================

#[derive(Accounts)]
pub struct InitializeAchievementConfig<'info> {
    #[account(
        init,
        payer = authority,
        space = AchievementConfig::SIZE,
        seeds = [b"achievement_config"],
        bump,
    )]
    pub config: Account<'info, AchievementConfig>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct InitializeUserAchievements<'info> {
    #[account(
        init,
        payer = payer,
        space = UserAchievements::SIZE,
        seeds = [USER_ACHIEVEMENTS_SEED, user.key().as_ref()],
        bump,
    )]
    pub user_achievements: Account<'info, UserAchievements>,
    #[account(
        mut,
        seeds = [b"achievement_config"],
        bump = config.bump,
    )]
    pub config: Account<'info, AchievementConfig>,
    /// CHECK: The user to create achievements for.
    pub user: AccountInfo<'info>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

/// Permissionless crank: anyone can call to update a user's stats and award badges.
#[derive(Accounts)]
pub struct CheckAndAward<'info> {
    #[account(
        mut,
        seeds = [USER_ACHIEVEMENTS_SEED, user_achievements.user.as_ref()],
        bump = user_achievements.bump,
    )]
    pub user_achievements: Account<'info, UserAchievements>,
    /// Cranker pays no rent, just signs.
    pub cranker: Signer<'info>,
}

/// Permissionless crank variant that also records new activity.
#[derive(Accounts)]
pub struct RecordActivity<'info> {
    #[account(
        mut,
        seeds = [USER_ACHIEVEMENTS_SEED, user_achievements.user.as_ref()],
        bump = user_achievements.bump,
    )]
    pub user_achievements: Account<'info, UserAchievements>,
    pub cranker: Signer<'info>,
}

#[derive(Accounts)]
pub struct GetAchievements<'info> {
    #[account(
        seeds = [USER_ACHIEVEMENTS_SEED, user_achievements.user.as_ref()],
        bump = user_achievements.bump,
    )]
    pub user_achievements: Account<'info, UserAchievements>,
}

// ============================================================================
// INSTRUCTIONS
// ============================================================================

pub fn handle_initialize_achievement_config(ctx: Context<InitializeAchievementConfig>) -> Result<()> {
    let config = &mut ctx.accounts.config;
    config.total_users = 0;
    config.authority = ctx.accounts.authority.key();
    config.bump = ctx.bumps.config;
    Ok(())
}

pub fn handle_initialize_user_achievements(ctx: Context<InitializeUserAchievements>) -> Result<()> {
    let clock = Clock::get()?;
    let config = &mut ctx.accounts.config;
    let acct = &mut ctx.accounts.user_achievements;

    acct.user = ctx.accounts.user.key();
    acct.badges = 0;
    acct.trade_count = 0;
    acct.total_volume = 0;
    acct.tokens_launched = 0;
    acct.earliest_hold_start = 0;
    acct.created_at = clock.unix_timestamp;
    acct.bump = ctx.bumps.user_achievements;

    config.total_users = config.total_users.checked_add(1).unwrap();

    // Award early adopter if within limit
    if config.total_users <= EARLY_ADOPTER_LIMIT {
        acct.badges |= EARLY_ADOPTER;
        emit!(AchievementUnlocked {
            user: acct.user,
            achievement: EARLY_ADOPTER,
            timestamp: clock.unix_timestamp,
        });
    }

    Ok(())
}

/// Record a trade and volume, then check/award badges.
pub fn handle_record_activity(
    ctx: Context<RecordActivity>,
    trades: u64,
    volume_lamports: u64,
    tokens_launched: u64,
    hold_start: i64,
) -> Result<()> {
    let clock = Clock::get()?;
    let acct = &mut ctx.accounts.user_achievements;

    acct.trade_count = acct.trade_count.checked_add(trades).unwrap();
    acct.total_volume = acct.total_volume.checked_add(volume_lamports).unwrap();
    acct.tokens_launched = acct.tokens_launched.checked_add(tokens_launched).unwrap();
    if hold_start > 0 && (acct.earliest_hold_start == 0 || hold_start < acct.earliest_hold_start) {
        acct.earliest_hold_start = hold_start;
    }

    check_badges(acct, clock.unix_timestamp)?;
    Ok(())
}

/// Permissionless crank: re-evaluate badges based on current stats.
pub fn handle_check_and_award(ctx: Context<CheckAndAward>) -> Result<()> {
    let clock = Clock::get()?;
    let acct = &mut ctx.accounts.user_achievements;
    check_badges(acct, clock.unix_timestamp)?;
    Ok(())
}

pub fn handle_get_achievements(ctx: Context<GetAchievements>) -> Result<u16> {
    Ok(ctx.accounts.user_achievements.badges)
}

// ============================================================================
// HELPERS
// ============================================================================

fn check_badges(acct: &mut UserAchievements, now: i64) -> Result<()> {
    let before = acct.badges;

    if acct.tokens_launched >= 1 {
        acct.badges |= FIRST_LAUNCH;
    }
    if acct.earliest_hold_start > 0 && (now - acct.earliest_hold_start) >= DIAMOND_HANDS_SECONDS {
        acct.badges |= DIAMOND_HANDS;
    }
    if acct.total_volume >= WHALE_VOLUME_LAMPORTS {
        acct.badges |= WHALE_STATUS;
    }
    if acct.trade_count >= DEGEN_TRADE_COUNT {
        acct.badges |= DEGEN_100;
    }

    // Emit events for newly awarded badges
    let newly_awarded = acct.badges & !before;
    if newly_awarded != 0 {
        emit!(AchievementUnlocked {
            user: acct.user,
            achievement: newly_awarded,
            timestamp: now,
        });
    }

    Ok(())
}

// ============================================================================
// EVENTS
// ============================================================================

#[event]
pub struct AchievementUnlocked {
    pub user: Pubkey,
    /// Bitflags of the newly awarded achievements.
    pub achievement: u16,
    pub timestamp: i64,
}

// ============================================================================
// ERRORS
// ============================================================================

#[error_code]
pub enum AchievementError {
    #[msg("User achievements account already initialized")]
    AlreadyInitialized,
    #[msg("No new achievements to award")]
    NoNewAchievements,
}
