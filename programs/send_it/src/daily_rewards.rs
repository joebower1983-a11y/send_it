use anchor_lang::prelude::*;

// ── Seeds ──────────────────────────────────────────────────────────────────────
pub const DAILY_REWARDS_CONFIG_SEED: &[u8] = b"daily_rewards_config";
pub const USER_DAILY_REWARDS_SEED: &[u8] = b"user_daily_rewards";

// ── Constants ──────────────────────────────────────────────────────────────────
const SECONDS_PER_DAY: i64 = 86_400;

// ── Errors ─────────────────────────────────────────────────────────────────────
#[error_code]
pub enum DailyRewardsError {
    #[msg("Already checked in today")]
    AlreadyCheckedIn,
    #[msg("Insufficient points to redeem")]
    InsufficientPoints,
    #[msg("Unauthorized")]
    Unauthorized,
    #[msg("Invalid reward tier")]
    InvalidTier,
}

// ── Enums ──────────────────────────────────────────────────────────────────────
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
pub enum RewardTier {
    Bronze,   // 0-99 points
    Silver,   // 100-499
    Gold,     // 500-1999
    Platinum, // 2000-9999
    Diamond,  // 10000+
}

impl RewardTier {
    pub fn from_points(points: u64) -> Self {
        match points {
            0..=99 => RewardTier::Bronze,
            100..=499 => RewardTier::Silver,
            500..=1999 => RewardTier::Gold,
            2000..=9999 => RewardTier::Platinum,
            _ => RewardTier::Diamond,
        }
    }
}

// ── Account Structs ────────────────────────────────────────────────────────────
#[account]
pub struct DailyRewardsConfig {
    pub authority: Pubkey,
    pub points_per_checkin: u64,
    pub streak_multiplier: u64,     // basis points (100 = 1x, 150 = 1.5x)
    pub points_per_trade_sol: u64,  // points awarded per SOL traded
    pub total_checkins: u64,
    pub bump: u8,
}

impl DailyRewardsConfig {
    pub const SIZE: usize = 8 + 32 + 8 + 8 + 8 + 8 + 1; // 73
}

#[account]
pub struct UserDailyRewards {
    pub user: Pubkey,
    pub current_streak: u64,
    pub longest_streak: u64,
    pub last_checkin_day: i64,   // day number (unix_ts / 86400)
    pub total_points: u64,
    pub tier: RewardTier,
    pub total_redeemed: u64,
    pub bump: u8,
}

impl UserDailyRewards {
    pub const SIZE: usize = 8 + 32 + 8 + 8 + 8 + 8 + 1 + 8 + 1; // 82
}

// ── Events ─────────────────────────────────────────────────────────────────────
#[event]
pub struct DailyCheckin {
    pub user: Pubkey,
    pub points_awarded: u64,
    pub current_streak: u64,
    pub total_points: u64,
    pub tier: RewardTier,
}

#[event]
pub struct TradeRewardRecorded {
    pub user: Pubkey,
    pub trade_sol: u64,
    pub points_awarded: u64,
}

#[event]
pub struct PointsRedeemed {
    pub user: Pubkey,
    pub points_spent: u64,
    pub remaining: u64,
}

// ── Instructions ───────────────────────────────────────────────────────────────

pub fn initialize_daily_rewards(
    ctx: Context<InitializeDailyRewards>,
    points_per_checkin: u64,
    streak_multiplier: u64,
    points_per_trade_sol: u64,
) -> Result<()> {
    let config = &mut ctx.accounts.config;
    config.authority = ctx.accounts.authority.key();
    config.points_per_checkin = points_per_checkin;
    config.streak_multiplier = streak_multiplier;
    config.points_per_trade_sol = points_per_trade_sol;
    config.total_checkins = 0;
    config.bump = ctx.bumps.config;
    Ok(())
}

pub fn daily_checkin(ctx: Context<DailyCheckinCtx>) -> Result<()> {
    let clock = Clock::get()?;
    let today = clock.unix_timestamp / SECONDS_PER_DAY;

    let rewards = &mut ctx.accounts.user_rewards;
    let config = &ctx.accounts.config;

    // First-time init
    if rewards.user == Pubkey::default() {
        rewards.user = ctx.accounts.user.key();
        rewards.bump = ctx.bumps.user_rewards;
    }

    require!(rewards.last_checkin_day < today, DailyRewardsError::AlreadyCheckedIn);

    // Update streak
    if rewards.last_checkin_day == today - 1 {
        rewards.current_streak = rewards.current_streak.checked_add(1).unwrap();
    } else {
        rewards.current_streak = 1;
    }
    if rewards.current_streak > rewards.longest_streak {
        rewards.longest_streak = rewards.current_streak;
    }
    rewards.last_checkin_day = today;

    // Calculate points: base * (1 + streak_bonus)
    // streak_multiplier is in bps per streak day, capped at 3x
    let multiplier = std::cmp::min(
        100_u64.checked_add(
            config.streak_multiplier.checked_mul(rewards.current_streak).unwrap_or(u64::MAX)
        ).unwrap_or(300),
        300,
    );
    let points = config
        .points_per_checkin
        .checked_mul(multiplier)
        .unwrap()
        .checked_div(100)
        .unwrap();

    rewards.total_points = rewards.total_points.checked_add(points).unwrap();
    rewards.tier = RewardTier::from_points(rewards.total_points);

    // Update global counter
    let config = &mut ctx.accounts.config;
    config.total_checkins = config.total_checkins.checked_add(1).unwrap();

    emit!(DailyCheckin {
        user: ctx.accounts.user.key(),
        points_awarded: points,
        current_streak: rewards.current_streak,
        total_points: rewards.total_points,
        tier: rewards.tier,
    });

    Ok(())
}

/// Called from the trade execution flow to award points for trading.
pub fn record_trade_reward(
    ctx: Context<RecordTradeReward>,
    trade_sol_amount: u64, // in lamports
) -> Result<()> {
    let config = &ctx.accounts.config;
    // Points = (lamports / 1e9) * points_per_trade_sol
    let points = trade_sol_amount
        .checked_mul(config.points_per_trade_sol)
        .unwrap()
        .checked_div(1_000_000_000)
        .unwrap_or(0);

    if points > 0 {
        let rewards = &mut ctx.accounts.user_rewards;
        if rewards.user == Pubkey::default() {
            rewards.user = ctx.accounts.user.key();
            rewards.bump = ctx.bumps.user_rewards;
        }
        rewards.total_points = rewards.total_points.checked_add(points).unwrap();
        rewards.tier = RewardTier::from_points(rewards.total_points);

        emit!(TradeRewardRecorded {
            user: ctx.accounts.user.key(),
            trade_sol: trade_sol_amount,
            points_awarded: points,
        });
    }

    Ok(())
}

/// Redeem points for fee discounts or priority access.
pub fn redeem_points(ctx: Context<RedeemPoints>, points_to_spend: u64) -> Result<()> {
    let rewards = &mut ctx.accounts.user_rewards;
    require!(
        rewards.total_points >= points_to_spend,
        DailyRewardsError::InsufficientPoints
    );

    rewards.total_points = rewards.total_points.checked_sub(points_to_spend).unwrap();
    rewards.total_redeemed = rewards.total_redeemed.checked_add(points_to_spend).unwrap();
    rewards.tier = RewardTier::from_points(rewards.total_points);

    emit!(PointsRedeemed {
        user: ctx.accounts.user.key(),
        points_spent: points_to_spend,
        remaining: rewards.total_points,
    });

    Ok(())
}

// ── Contexts ───────────────────────────────────────────────────────────────────

#[derive(Accounts)]
pub struct InitializeDailyRewards<'info> {
    #[account(
        init,
        payer = authority,
        space = DailyRewardsConfig::SIZE,
        seeds = [DAILY_REWARDS_CONFIG_SEED],
        bump,
    )]
    pub config: Account<'info, DailyRewardsConfig>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct DailyCheckinCtx<'info> {
    #[account(
        mut,
        seeds = [DAILY_REWARDS_CONFIG_SEED],
        bump = config.bump,
    )]
    pub config: Account<'info, DailyRewardsConfig>,

    #[account(
        init_if_needed,
        payer = user,
        space = UserDailyRewards::SIZE,
        seeds = [USER_DAILY_REWARDS_SEED, user.key().as_ref()],
        bump,
    )]
    pub user_rewards: Account<'info, UserDailyRewards>,

    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct RecordTradeReward<'info> {
    #[account(
        seeds = [DAILY_REWARDS_CONFIG_SEED],
        bump = config.bump,
    )]
    pub config: Account<'info, DailyRewardsConfig>,

    #[account(
        init_if_needed,
        payer = user,
        space = UserDailyRewards::SIZE,
        seeds = [USER_DAILY_REWARDS_SEED, user.key().as_ref()],
        bump,
    )]
    pub user_rewards: Account<'info, UserDailyRewards>,

    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct RedeemPoints<'info> {
    #[account(
        mut,
        seeds = [USER_DAILY_REWARDS_SEED, user.key().as_ref()],
        bump = user_rewards.bump,
        has_one = user,
    )]
    pub user_rewards: Account<'info, UserDailyRewards>,

    pub user: Signer<'info>,
}
