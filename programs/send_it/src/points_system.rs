use anchor_lang::prelude::*;

// ============================================================================
// SEEDS & CONSTANTS
// ============================================================================

pub const POINTS_CONFIG_SEED: &[u8] = b"points_config";
pub const USER_POINTS_SEED: &[u8] = b"user_points";
pub const POINTS_LEADERBOARD_SEED: &[u8] = b"points_leaderboard";
pub const SEASON_ARCHIVE_SEED: &[u8] = b"season_archive";
pub const REWARD_CLAIM_SEED: &[u8] = b"reward_claim";

/// Maximum entries on the points leaderboard.
pub const MAX_POINTS_LEADERBOARD: usize = 100;

/// Minimum cooldown between point-earning actions (seconds).
pub const DEFAULT_ACTION_COOLDOWN: i64 = 60;

/// Default maximum points earnable per calendar day.
pub const DEFAULT_MAX_DAILY_POINTS: u64 = 10_000;

/// Streak resets if user is inactive for more than this many seconds (48h grace).
pub const STREAK_GRACE_PERIOD: i64 = 48 * 60 * 60;

/// One day in seconds.
pub const SECONDS_PER_DAY: i64 = 86_400;

// ============================================================================
// ENUMS
// ============================================================================

/// The type of action being rewarded.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
pub enum PointAction {
    Trade,
    Launch,
    Referral,
    HoldDay,
}

/// Reward types that can be claimed by spending points.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
pub enum RewardKind {
    /// Percentage fee discount (value = discount bps, e.g. 500 = 5%).
    FeeDiscount,
    /// Early access to a specific launch (value = launch index / identifier).
    EarlyAccess,
    /// Cosmetic badge or title (value = badge id).
    Badge,
}

// ============================================================================
// ACCOUNTS
// ============================================================================

/// Global points configuration, managed by the platform authority.
/// PDA: [POINTS_CONFIG_SEED]
#[account]
pub struct PointsConfig {
    /// Platform authority that can update config and award points.
    pub authority: Pubkey,
    /// Points granted per trade action.
    pub points_per_trade: u64,
    /// Points granted per token launch.
    pub points_per_launch: u64,
    /// Points granted per successful referral.
    pub points_per_referral: u64,
    /// Points granted per day of holding a token.
    pub points_per_hold_day: u64,
    /// Current season identifier. Incremented on season reset.
    pub season_id: u32,
    /// Minimum seconds between point-earning actions per user.
    pub action_cooldown: i64,
    /// Maximum points a user can earn in a single calendar day.
    pub max_daily_points: u64,
    /// Whether point accrual is paused.
    pub paused: bool,
    /// Bump seed.
    pub bump: u8,
}

impl PointsConfig {
    pub const SIZE: usize = 8  // discriminator
        + 32  // authority
        + 8   // points_per_trade
        + 8   // points_per_launch
        + 8   // points_per_referral
        + 8   // points_per_hold_day
        + 4   // season_id
        + 8   // action_cooldown
        + 8   // max_daily_points
        + 1   // paused
        + 1;  // bump

    /// Look up how many points a given action is worth.
    pub fn points_for(&self, action: PointAction) -> u64 {
        match action {
            PointAction::Trade => self.points_per_trade,
            PointAction::Launch => self.points_per_launch,
            PointAction::Referral => self.points_per_referral,
            PointAction::HoldDay => self.points_per_hold_day,
        }
    }
}

/// Per-user points account, scoped to the current season.
/// PDA: [USER_POINTS_SEED, user_pubkey, season_id (LE bytes)]
#[account]
pub struct UserPoints {
    /// The user this account belongs to.
    pub user: Pubkey,
    /// Season this account is valid for.
    pub season_id: u32,
    /// Lifetime total points earned this season.
    pub total_points: u64,
    /// Points available to spend (total minus redeemed).
    pub available_points: u64,
    /// User level, derived from total_points thresholds.
    pub level: u16,
    /// Unix timestamp of the last point-earning action.
    pub last_action_ts: i64,
    /// Consecutive days with at least one point-earning action.
    pub streak_days: u32,
    /// The calendar day (unix_ts / 86400) of the last action, for streak tracking.
    pub last_action_day: i64,
    /// Points earned today (resets each calendar day).
    pub daily_points_earned: u64,
    /// Calendar day number for `daily_points_earned` tracking.
    pub daily_reset_day: i64,
    /// Bump seed.
    pub bump: u8,
}

impl UserPoints {
    pub const SIZE: usize = 8  // discriminator
        + 32  // user
        + 4   // season_id
        + 8   // total_points
        + 8   // available_points
        + 2   // level
        + 8   // last_action_ts
        + 4   // streak_days
        + 8   // last_action_day
        + 8   // daily_points_earned
        + 8   // daily_reset_day
        + 1;  // bump

    /// Compute the user level from total points.
    /// Thresholds: 0→L1, 100→L2, 500→L3, 2000→L4, 5000→L5, 10000→L6, 25000→L7, 50000→L8, 100000→L9, 250000→L10
    pub fn compute_level(total_points: u64) -> u16 {
        match total_points {
            0..=99 => 1,
            100..=499 => 2,
            500..=1_999 => 3,
            2_000..=4_999 => 4,
            5_000..=9_999 => 5,
            10_000..=24_999 => 6,
            25_000..=49_999 => 7,
            50_000..=99_999 => 8,
            100_000..=249_999 => 9,
            _ => 10,
        }
    }
}

/// Leaderboard entry for the points system.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Default)]
pub struct PointsLeaderboardEntry {
    pub user: Pubkey,
    pub total_points: u64,
    pub level: u16,
}

impl PointsLeaderboardEntry {
    pub const SIZE: usize = 32 + 8 + 2;
}

/// On-chain leaderboard holding the top point holders for the current season.
/// PDA: [POINTS_LEADERBOARD_SEED, season_id (LE bytes)]
#[account]
pub struct PointsLeaderboardState {
    /// Season this leaderboard belongs to.
    pub season_id: u32,
    /// Sorted descending by total_points.
    pub entries: Vec<PointsLeaderboardEntry>,
    /// Bump seed.
    pub bump: u8,
}

impl PointsLeaderboardState {
    pub const SIZE: usize = 8  // discriminator
        + 4   // season_id
        + (4 + MAX_POINTS_LEADERBOARD * PointsLeaderboardEntry::SIZE) // entries vec
        + 1;  // bump
}

/// Archived leaderboard snapshot from a previous season.
/// PDA: [SEASON_ARCHIVE_SEED, season_id (LE bytes)]
#[account]
pub struct SeasonArchive {
    pub season_id: u32,
    pub ended_at: i64,
    pub top_entries: Vec<PointsLeaderboardEntry>,
    pub bump: u8,
}

impl SeasonArchive {
    pub const SIZE: usize = 8  // discriminator
        + 4   // season_id
        + 8   // ended_at
        + (4 + MAX_POINTS_LEADERBOARD * PointsLeaderboardEntry::SIZE)
        + 1;  // bump
}

// ============================================================================
// CONTEXT STRUCTS
// ============================================================================

/// Initialize the global points configuration. Called once by admin.
#[derive(Accounts)]
pub struct InitializePointsConfig<'info> {
    #[account(
        init,
        payer = authority,
        space = PointsConfig::SIZE,
        seeds = [POINTS_CONFIG_SEED],
        bump,
    )]
    pub points_config: Account<'info, PointsConfig>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

/// Update points configuration values. Authority only.
#[derive(Accounts)]
pub struct UpdatePointsConfig<'info> {
    #[account(
        mut,
        seeds = [POINTS_CONFIG_SEED],
        bump = points_config.bump,
        has_one = authority,
    )]
    pub points_config: Account<'info, PointsConfig>,

    pub authority: Signer<'info>,
}

/// Award points to a user. Called by program authority (backend / CPI).
#[derive(Accounts)]
#[instruction(action: PointAction)]
pub struct AwardPoints<'info> {
    #[account(
        seeds = [POINTS_CONFIG_SEED],
        bump = points_config.bump,
        has_one = authority,
    )]
    pub points_config: Account<'info, PointsConfig>,

    #[account(
        init_if_needed,
        payer = authority,
        space = UserPoints::SIZE,
        seeds = [
            USER_POINTS_SEED,
            user.key().as_ref(),
            &points_config.season_id.to_le_bytes(),
        ],
        bump,
    )]
    pub user_points: Account<'info, UserPoints>,

    #[account(
        mut,
        seeds = [
            POINTS_LEADERBOARD_SEED,
            &points_config.season_id.to_le_bytes(),
        ],
        bump = leaderboard.bump,
    )]
    pub leaderboard: Account<'info, PointsLeaderboardState>,

    /// CHECK: The user receiving points. Does not need to sign.
    pub user: AccountInfo<'info>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

/// User spends points to claim a reward.
#[derive(Accounts)]
#[instruction(reward_kind: RewardKind, reward_value: u64)]
pub struct ClaimReward<'info> {
    #[account(
        seeds = [POINTS_CONFIG_SEED],
        bump = points_config.bump,
    )]
    pub points_config: Account<'info, PointsConfig>,

    #[account(
        mut,
        seeds = [
            USER_POINTS_SEED,
            user.key().as_ref(),
            &points_config.season_id.to_le_bytes(),
        ],
        bump = user_points.bump,
        constraint = user_points.user == user.key() @ PointsError::Unauthorized,
    )]
    pub user_points: Account<'info, UserPoints>,

    #[account(mut)]
    pub user: Signer<'info>,
}

/// Initialize a leaderboard for a given season. Called by admin.
#[derive(Accounts)]
pub struct InitializePointsLeaderboard<'info> {
    #[account(
        init,
        payer = authority,
        space = PointsLeaderboardState::SIZE,
        seeds = [
            POINTS_LEADERBOARD_SEED,
            &points_config.season_id.to_le_bytes(),
        ],
        bump,
    )]
    pub leaderboard: Account<'info, PointsLeaderboardState>,

    #[account(
        seeds = [POINTS_CONFIG_SEED],
        bump = points_config.bump,
        has_one = authority,
    )]
    pub points_config: Account<'info, PointsConfig>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

/// End the current season: archive the leaderboard and bump season_id.
#[derive(Accounts)]
pub struct EndSeason<'info> {
    #[account(
        mut,
        seeds = [POINTS_CONFIG_SEED],
        bump = points_config.bump,
        has_one = authority,
    )]
    pub points_config: Account<'info, PointsConfig>,

    #[account(
        seeds = [
            POINTS_LEADERBOARD_SEED,
            &points_config.season_id.to_le_bytes(),
        ],
        bump = current_leaderboard.bump,
    )]
    pub current_leaderboard: Account<'info, PointsLeaderboardState>,

    #[account(
        init,
        payer = authority,
        space = SeasonArchive::SIZE,
        seeds = [
            SEASON_ARCHIVE_SEED,
            &points_config.season_id.to_le_bytes(),
        ],
        bump,
    )]
    pub season_archive: Account<'info, SeasonArchive>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

// ============================================================================
// INSTRUCTIONS (free functions called from lib.rs #[program] block)
// ============================================================================

/// Initialize the global points configuration.
pub fn handle_initialize_points_config(
    ctx: Context<InitializePointsConfig>,
    points_per_trade: u64,
    points_per_launch: u64,
    points_per_referral: u64,
    points_per_hold_day: u64,
) -> Result<()> {
    let config = &mut ctx.accounts.points_config;
    config.authority = ctx.accounts.authority.key();
    config.points_per_trade = points_per_trade;
    config.points_per_launch = points_per_launch;
    config.points_per_referral = points_per_referral;
    config.points_per_hold_day = points_per_hold_day;
    config.season_id = 1;
    config.action_cooldown = DEFAULT_ACTION_COOLDOWN;
    config.max_daily_points = DEFAULT_MAX_DAILY_POINTS;
    config.paused = false;
    config.bump = ctx.bumps.points_config;

    emit!(PointsConfigInitialized {
        authority: config.authority,
        season_id: config.season_id,
        points_per_trade,
        points_per_launch,
        points_per_referral,
        points_per_hold_day,
    });

    Ok(())
}

/// Update one or more fields on the points config.
pub fn handle_update_points_config(
    ctx: Context<UpdatePointsConfig>,
    points_per_trade: Option<u64>,
    points_per_launch: Option<u64>,
    points_per_referral: Option<u64>,
    points_per_hold_day: Option<u64>,
    action_cooldown: Option<i64>,
    max_daily_points: Option<u64>,
    paused: Option<bool>,
) -> Result<()> {
    let config = &mut ctx.accounts.points_config;

    if let Some(v) = points_per_trade { config.points_per_trade = v; }
    if let Some(v) = points_per_launch { config.points_per_launch = v; }
    if let Some(v) = points_per_referral { config.points_per_referral = v; }
    if let Some(v) = points_per_hold_day { config.points_per_hold_day = v; }
    if let Some(v) = action_cooldown {
        require!(v >= 0, PointsError::InvalidCooldown);
        config.action_cooldown = v;
    }
    if let Some(v) = max_daily_points { config.max_daily_points = v; }
    if let Some(v) = paused { config.paused = v; }

    emit!(PointsConfigUpdated {
        authority: config.authority,
        season_id: config.season_id,
    });

    Ok(())
}

/// Initialize the leaderboard for the current season.
pub fn handle_initialize_points_leaderboard(
    ctx: Context<InitializePointsLeaderboard>,
) -> Result<()> {
    let lb = &mut ctx.accounts.leaderboard;
    lb.season_id = ctx.accounts.points_config.season_id;
    lb.entries = Vec::new();
    lb.bump = ctx.bumps.leaderboard;
    Ok(())
}

/// Award points to a user for a specific action.
///
/// Anti-gaming enforced:
/// - Cooldown between actions
/// - Daily points cap
/// - Streak tracking with grace period
pub fn handle_award_points(
    ctx: Context<AwardPoints>,
    action: PointAction,
    multiplier: Option<u64>,
) -> Result<()> {
    let config = &ctx.accounts.points_config;
    require!(!config.paused, PointsError::PointsPaused);

    let clock = Clock::get()?;
    let now = clock.unix_timestamp;
    let today = now / SECONDS_PER_DAY;

    let user_points = &mut ctx.accounts.user_points;

    // First-time init
    if user_points.user == Pubkey::default() {
        user_points.user = ctx.accounts.user.key();
        user_points.season_id = config.season_id;
        user_points.bump = ctx.bumps.user_points;
        user_points.streak_days = 0;
        user_points.last_action_day = 0;
        user_points.daily_reset_day = today;
    }

    // --- Anti-gaming: cooldown ---
    let elapsed = now.saturating_sub(user_points.last_action_ts);
    require!(
        elapsed >= config.action_cooldown,
        PointsError::CooldownActive
    );

    // --- Daily cap reset ---
    if today != user_points.daily_reset_day {
        user_points.daily_points_earned = 0;
        user_points.daily_reset_day = today;
    }

    // Calculate points to award
    let base_points = config.points_for(action);
    let mult = multiplier.unwrap_or(1).max(1);
    let raw_points = base_points.checked_mul(mult).ok_or(PointsError::MathOverflow)?;

    // --- Anti-gaming: daily cap ---
    let headroom = config.max_daily_points.saturating_sub(user_points.daily_points_earned);
    require!(headroom > 0, PointsError::DailyCapReached);
    let capped_points = raw_points.min(headroom);

    // --- Streak logic ---
    let last_day = user_points.last_action_day;
    if last_day == 0 {
        // First ever action
        user_points.streak_days = 1;
    } else if today == last_day {
        // Same day, streak unchanged
    } else if today == last_day + 1 {
        // Consecutive day
        user_points.streak_days = user_points.streak_days.saturating_add(1);
    } else {
        // Check grace period (48h from last action timestamp)
        let since_last = now.saturating_sub(user_points.last_action_ts);
        if since_last <= STREAK_GRACE_PERIOD {
            user_points.streak_days = user_points.streak_days.saturating_add(1);
        } else {
            // Streak broken
            user_points.streak_days = 1;
        }
    }

    // Apply streak bonus: +1% per streak day, capped at +50%
    let streak_bonus_pct = (user_points.streak_days as u64).min(50);
    let bonus = capped_points
        .checked_mul(streak_bonus_pct)
        .ok_or(PointsError::MathOverflow)?
        / 100;
    let final_points = capped_points
        .checked_add(bonus)
        .ok_or(PointsError::MathOverflow)?;

    // Re-check daily cap after bonus
    let final_points = final_points.min(
        config.max_daily_points.saturating_sub(user_points.daily_points_earned),
    );

    // Update user state
    user_points.total_points = user_points
        .total_points
        .checked_add(final_points)
        .ok_or(PointsError::MathOverflow)?;
    user_points.available_points = user_points
        .available_points
        .checked_add(final_points)
        .ok_or(PointsError::MathOverflow)?;
    user_points.daily_points_earned = user_points
        .daily_points_earned
        .checked_add(final_points)
        .ok_or(PointsError::MathOverflow)?;
    user_points.last_action_ts = now;
    user_points.last_action_day = today;
    user_points.level = UserPoints::compute_level(user_points.total_points);

    // --- Update leaderboard ---
    let lb = &mut ctx.accounts.leaderboard;
    update_points_leaderboard(
        &mut lb.entries,
        PointsLeaderboardEntry {
            user: ctx.accounts.user.key(),
            total_points: user_points.total_points,
            level: user_points.level,
        },
    );

    emit!(PointsAwarded {
        user: ctx.accounts.user.key(),
        action,
        base_points,
        streak_bonus: bonus,
        final_points,
        new_total: user_points.total_points,
        level: user_points.level,
        streak_days: user_points.streak_days,
        season_id: config.season_id,
    });

    Ok(())
}

/// Spend points to claim a reward (fee discount, early access, badge, etc.).
pub fn handle_claim_reward(
    ctx: Context<ClaimReward>,
    reward_kind: RewardKind,
    reward_value: u64,
    points_cost: u64,
) -> Result<()> {
    require!(points_cost > 0, PointsError::ZeroCost);

    let user_points = &mut ctx.accounts.user_points;
    require!(
        user_points.available_points >= points_cost,
        PointsError::InsufficientPoints
    );

    user_points.available_points = user_points
        .available_points
        .checked_sub(points_cost)
        .ok_or(PointsError::MathOverflow)?;

    emit!(RewardClaimed {
        user: ctx.accounts.user.key(),
        reward_kind,
        reward_value,
        points_spent: points_cost,
        remaining_points: user_points.available_points,
        season_id: ctx.accounts.points_config.season_id,
    });

    Ok(())
}

/// End the current season: archive leaderboard, bump season_id.
pub fn handle_end_season(ctx: Context<EndSeason>) -> Result<()> {
    let clock = Clock::get()?;
    let config = &ctx.accounts.points_config;
    let current_lb = &ctx.accounts.current_leaderboard;

    // Write archive
    let archive = &mut ctx.accounts.season_archive;
    archive.season_id = config.season_id;
    archive.ended_at = clock.unix_timestamp;
    archive.top_entries = current_lb.entries.clone();
    archive.bump = ctx.bumps.season_archive;

    let old_season = config.season_id;

    // Bump season
    let config = &mut ctx.accounts.points_config;
    config.season_id = config
        .season_id
        .checked_add(1)
        .ok_or(PointsError::MathOverflow)?;

    emit!(SeasonEnded {
        season_id: old_season,
        new_season_id: config.season_id,
        ended_at: clock.unix_timestamp,
        top_user: if current_lb.entries.is_empty() {
            Pubkey::default()
        } else {
            current_lb.entries[0].user
        },
    });

    Ok(())
}

// ============================================================================
// HELPERS
// ============================================================================

/// Insert or update a user on the leaderboard, keep sorted desc, truncate to max.
fn update_points_leaderboard(
    entries: &mut Vec<PointsLeaderboardEntry>,
    entry: PointsLeaderboardEntry,
) {
    if let Some(existing) = entries.iter_mut().find(|e| e.user == entry.user) {
        existing.total_points = entry.total_points;
        existing.level = entry.level;
    } else {
        entries.push(entry);
    }
    entries.sort_by(|a, b| b.total_points.cmp(&a.total_points));
    entries.truncate(MAX_POINTS_LEADERBOARD);
}

// ============================================================================
// EVENTS
// ============================================================================

#[event]
pub struct PointsConfigInitialized {
    pub authority: Pubkey,
    pub season_id: u32,
    pub points_per_trade: u64,
    pub points_per_launch: u64,
    pub points_per_referral: u64,
    pub points_per_hold_day: u64,
}

#[event]
pub struct PointsConfigUpdated {
    pub authority: Pubkey,
    pub season_id: u32,
}

#[event]
pub struct PointsAwarded {
    pub user: Pubkey,
    pub action: PointAction,
    pub base_points: u64,
    pub streak_bonus: u64,
    pub final_points: u64,
    pub new_total: u64,
    pub level: u16,
    pub streak_days: u32,
    pub season_id: u32,
}

#[event]
pub struct RewardClaimed {
    pub user: Pubkey,
    pub reward_kind: RewardKind,
    pub reward_value: u64,
    pub points_spent: u64,
    pub remaining_points: u64,
    pub season_id: u32,
}

#[event]
pub struct SeasonEnded {
    pub season_id: u32,
    pub new_season_id: u32,
    pub ended_at: i64,
    pub top_user: Pubkey,
}

// ============================================================================
// ERRORS
// ============================================================================

#[error_code]
pub enum PointsError {
    #[msg("Points system is paused")]
    PointsPaused,
    #[msg("Action cooldown still active")]
    CooldownActive,
    #[msg("Daily points cap reached")]
    DailyCapReached,
    #[msg("Insufficient points to claim reward")]
    InsufficientPoints,
    #[msg("Reward cost must be greater than zero")]
    ZeroCost,
    #[msg("Unauthorized")]
    Unauthorized,
    #[msg("Invalid cooldown value")]
    InvalidCooldown,
    #[msg("Math overflow")]
    MathOverflow,
}
