use anchor_lang::prelude::*;
use anchor_lang::system_program;

// ── Seeds ──────────────────────────────────────────────────────────────────────
pub const SEASON_SEED: &[u8] = b"season";
pub const SEASON_PASS_SEED: &[u8] = b"season_pass";
pub const SEASON_REWARD_SEED: &[u8] = b"season_reward";

// ── Errors ─────────────────────────────────────────────────────────────────────
#[error_code]
pub enum SeasonError {
    #[msg("Season is not currently active")]
    SeasonNotActive,
    #[msg("Season has not ended yet")]
    SeasonNotEnded,
    #[msg("Season already ended")]
    SeasonAlreadyEnded,
    #[msg("Unauthorized")]
    Unauthorized,
    #[msg("Already joined this season")]
    AlreadyJoined,
    #[msg("Insufficient XP for this reward level")]
    InsufficientXP,
    #[msg("Reward already claimed for this level")]
    RewardAlreadyClaimed,
    #[msg("Invalid time range")]
    InvalidTimeRange,
    #[msg("Season end time not reached")]
    SeasonStillActive,
}

// ── Enums ──────────────────────────────────────────────────────────────────────
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
pub enum RewardType {
    Lamports,
    FeeDiscount,     // basis points discount
    PriorityAccess,
    BadgeNFT,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
pub enum XPSource {
    TradeVolume,
    TokenLaunch,
    HoldDuration,
    Referral,
}

// ── Bitflags for achievements ──────────────────────────────────────────────────
pub const ACHIEVEMENT_FIRST_TRADE: u64    = 1 << 0;
pub const ACHIEVEMENT_10_TRADES: u64      = 1 << 1;
pub const ACHIEVEMENT_100_TRADES: u64     = 1 << 2;
pub const ACHIEVEMENT_LAUNCH_TOKEN: u64   = 1 << 3;
pub const ACHIEVEMENT_1_SOL_VOLUME: u64   = 1 << 4;
pub const ACHIEVEMENT_10_SOL_VOLUME: u64  = 1 << 5;
pub const ACHIEVEMENT_100_SOL_VOLUME: u64 = 1 << 6;
pub const ACHIEVEMENT_REFERRAL_5: u64     = 1 << 7;
pub const ACHIEVEMENT_DIAMOND_HANDS: u64  = 1 << 8;  // held >7 days
pub const ACHIEVEMENT_STREAK_7: u64       = 1 << 9;

// ── Account Structs ────────────────────────────────────────────────────────────
#[account]
pub struct Season {
    pub authority: Pubkey,
    pub season_number: u64,
    pub start_time: i64,
    pub end_time: i64,
    pub total_participants: u64,
    pub prize_pool_lamports: u64,
    pub is_active: bool,
    pub is_finalized: bool,
    pub bump: u8,
}

impl Season {
    pub const SIZE: usize = 8 + 32 + 8 + 8 + 8 + 8 + 8 + 1 + 1 + 1; // 83
}

#[account]
pub struct SeasonPass {
    pub season: Pubkey,
    pub user: Pubkey,
    pub xp: u64,
    pub level: u32,
    pub trades_count: u64,
    pub volume: u64,               // in lamports
    pub achievements_unlocked: u64, // bitflag
    pub rewards_claimed_mask: u64,  // bitflag per level (up to 64 levels)
    pub joined_at: i64,
    pub bump: u8,
}

impl SeasonPass {
    pub const SIZE: usize = 8 + 32 + 32 + 8 + 4 + 8 + 8 + 8 + 8 + 8 + 1; // 125
}

#[account]
pub struct SeasonReward {
    pub season: Pubkey,
    pub level: u32,
    pub min_xp: u64,
    pub reward_type: RewardType,
    pub reward_amount: u64,
    pub bump: u8,
}

impl SeasonReward {
    pub const SIZE: usize = 8 + 32 + 4 + 8 + 1 + 8 + 1; // 62
}

// ── Events ─────────────────────────────────────────────────────────────────────
#[event]
pub struct SeasonStarted {
    pub season_number: u64,
    pub start_time: i64,
    pub end_time: i64,
}

#[event]
pub struct SeasonJoined {
    pub season_number: u64,
    pub user: Pubkey,
}

#[event]
pub struct XPRecorded {
    pub season_number: u64,
    pub user: Pubkey,
    pub xp_gained: u64,
    pub source: XPSource,
    pub new_total_xp: u64,
    pub new_level: u32,
}

#[event]
pub struct SeasonRewardClaimed {
    pub season_number: u64,
    pub user: Pubkey,
    pub level: u32,
    pub reward_type: RewardType,
    pub reward_amount: u64,
}

#[event]
pub struct SeasonEnded {
    pub season_number: u64,
    pub total_participants: u64,
    pub prize_pool_lamports: u64,
}

// ── Instructions ───────────────────────────────────────────────────────────────

pub fn start_season(
    ctx: Context<StartSeason>,
    season_number: u64,
    start_time: i64,
    end_time: i64,
) -> Result<()> {
    require!(end_time > start_time, SeasonError::InvalidTimeRange);

    let season = &mut ctx.accounts.season;
    season.authority = ctx.accounts.authority.key();
    season.season_number = season_number;
    season.start_time = start_time;
    season.end_time = end_time;
    season.total_participants = 0;
    season.prize_pool_lamports = 0;
    season.is_active = true;
    season.is_finalized = false;
    season.bump = ctx.bumps.season;

    emit!(SeasonStarted {
        season_number,
        start_time,
        end_time,
    });

    Ok(())
}

/// Add a reward tier for a season level.
pub fn add_season_reward(
    ctx: Context<AddSeasonReward>,
    level: u32,
    min_xp: u64,
    reward_type: RewardType,
    reward_amount: u64,
) -> Result<()> {
    let season = &ctx.accounts.season;
    require!(
        ctx.accounts.authority.key() == season.authority,
        SeasonError::Unauthorized
    );

    let reward = &mut ctx.accounts.season_reward;
    reward.season = season.key();
    reward.level = level;
    reward.min_xp = min_xp;
    reward.reward_type = reward_type;
    reward.reward_amount = reward_amount;
    reward.bump = ctx.bumps.season_reward;

    Ok(())
}

/// Join a season (free).
pub fn join_season(ctx: Context<JoinSeason>) -> Result<()> {
    let season = &ctx.accounts.season;
    require!(season.is_active, SeasonError::SeasonNotActive);

    let clock = Clock::get()?;
    let pass = &mut ctx.accounts.season_pass;
    pass.season = season.key();
    pass.user = ctx.accounts.user.key();
    pass.xp = 0;
    pass.level = 0;
    pass.trades_count = 0;
    pass.volume = 0;
    pass.achievements_unlocked = 0;
    pass.rewards_claimed_mask = 0;
    pass.joined_at = clock.unix_timestamp;
    pass.bump = ctx.bumps.season_pass;

    let season = &mut ctx.accounts.season;
    season.total_participants = season.total_participants.checked_add(1).unwrap();

    emit!(SeasonJoined {
        season_number: season.season_number,
        user: ctx.accounts.user.key(),
    });

    Ok(())
}

/// Record XP from trades, launches, holding, referrals.
pub fn record_season_xp(
    ctx: Context<RecordSeasonXP>,
    xp_amount: u64,
    source: XPSource,
    trade_volume_lamports: u64,
) -> Result<()> {
    let season = &ctx.accounts.season;
    require!(season.is_active, SeasonError::SeasonNotActive);

    let pass = &mut ctx.accounts.season_pass;
    pass.xp = pass.xp.checked_add(xp_amount).unwrap();

    // Update trade stats if applicable
    if source == XPSource::TradeVolume {
        pass.trades_count = pass.trades_count.checked_add(1).unwrap();
        pass.volume = pass.volume.checked_add(trade_volume_lamports).unwrap();

        // Check trade achievements
        if pass.trades_count >= 1 {
            pass.achievements_unlocked |= ACHIEVEMENT_FIRST_TRADE;
        }
        if pass.trades_count >= 10 {
            pass.achievements_unlocked |= ACHIEVEMENT_10_TRADES;
        }
        if pass.trades_count >= 100 {
            pass.achievements_unlocked |= ACHIEVEMENT_100_TRADES;
        }
        // Volume achievements (in SOL)
        let volume_sol = pass.volume / 1_000_000_000;
        if volume_sol >= 1 {
            pass.achievements_unlocked |= ACHIEVEMENT_1_SOL_VOLUME;
        }
        if volume_sol >= 10 {
            pass.achievements_unlocked |= ACHIEVEMENT_10_SOL_VOLUME;
        }
        if volume_sol >= 100 {
            pass.achievements_unlocked |= ACHIEVEMENT_100_SOL_VOLUME;
        }
    } else if source == XPSource::TokenLaunch {
        pass.achievements_unlocked |= ACHIEVEMENT_LAUNCH_TOKEN;
    }

    // Level calculation: level = sqrt(xp / 100), simple curve
    pass.level = ((pass.xp / 100) as f64).sqrt() as u32;

    emit!(XPRecorded {
        season_number: season.season_number,
        user: ctx.accounts.user.key(),
        xp_gained: xp_amount,
        source,
        new_total_xp: pass.xp,
        new_level: pass.level,
    });

    Ok(())
}

/// Claim a reward for reaching a specific level.
pub fn claim_season_reward(ctx: Context<ClaimSeasonReward>) -> Result<()> {
    let reward = &ctx.accounts.season_reward;
    let pass = &mut ctx.accounts.season_pass;

    require!(pass.xp >= reward.min_xp, SeasonError::InsufficientXP);

    // Check if already claimed for this level
    let level_bit = 1u64 << (reward.level as u64 % 64);
    require!(
        pass.rewards_claimed_mask & level_bit == 0,
        SeasonError::RewardAlreadyClaimed
    );
    pass.rewards_claimed_mask |= level_bit;

    // Distribute reward based on type
    if reward.reward_type == RewardType::Lamports && reward.reward_amount > 0 {
        // Transfer from season PDA (prize pool) to user
        let season = &ctx.accounts.season;
        let season_number_bytes = season.season_number.to_le_bytes();
        let seeds = &[
            SEASON_SEED,
            season_number_bytes.as_ref(),
            &[season.bump],
        ];

        system_program::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.system_program.to_account_info(),
                system_program::Transfer {
                    from: ctx.accounts.season.to_account_info(),
                    to: ctx.accounts.user.to_account_info(),
                },
                &[seeds],
            ),
            reward.reward_amount,
        )?;
    }
    // FeeDiscount, PriorityAccess, BadgeNFT handled off-chain or by other modules

    emit!(SeasonRewardClaimed {
        season_number: ctx.accounts.season.season_number,
        user: ctx.accounts.user.key(),
        level: reward.level,
        reward_type: reward.reward_type,
        reward_amount: reward.reward_amount,
    });

    Ok(())
}

/// End a season (authority only). Finalizes and marks inactive.
pub fn end_season(ctx: Context<EndSeason>) -> Result<()> {
    let season = &mut ctx.accounts.season;
    require!(
        ctx.accounts.authority.key() == season.authority,
        SeasonError::Unauthorized
    );
    require!(season.is_active, SeasonError::SeasonAlreadyEnded);

    let clock = Clock::get()?;
    require!(
        clock.unix_timestamp >= season.end_time,
        SeasonError::SeasonStillActive
    );

    season.is_active = false;
    season.is_finalized = true;

    emit!(SeasonEnded {
        season_number: season.season_number,
        total_participants: season.total_participants,
        prize_pool_lamports: season.prize_pool_lamports,
    });

    Ok(())
}

/// Fund the season prize pool.
pub fn fund_season(ctx: Context<FundSeason>, lamports: u64) -> Result<()> {
    system_program::transfer(
        CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            system_program::Transfer {
                from: ctx.accounts.funder.to_account_info(),
                to: ctx.accounts.season.to_account_info(),
            },
        ),
        lamports,
    )?;

    let season = &mut ctx.accounts.season;
    season.prize_pool_lamports = season.prize_pool_lamports.checked_add(lamports).unwrap();

    Ok(())
}

// ── Contexts ───────────────────────────────────────────────────────────────────

#[derive(Accounts)]
#[instruction(season_number: u64)]
pub struct StartSeason<'info> {
    #[account(
        init,
        payer = authority,
        space = Season::SIZE,
        seeds = [SEASON_SEED, &season_number.to_le_bytes()],
        bump,
    )]
    pub season: Account<'info, Season>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(level: u32)]
pub struct AddSeasonReward<'info> {
    #[account(
        seeds = [SEASON_SEED, &season.season_number.to_le_bytes()],
        bump = season.bump,
    )]
    pub season: Account<'info, Season>,

    #[account(
        init,
        payer = authority,
        space = SeasonReward::SIZE,
        seeds = [SEASON_REWARD_SEED, season.key().as_ref(), &level.to_le_bytes()],
        bump,
    )]
    pub season_reward: Account<'info, SeasonReward>,

    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct JoinSeason<'info> {
    #[account(
        mut,
        seeds = [SEASON_SEED, &season.season_number.to_le_bytes()],
        bump = season.bump,
    )]
    pub season: Account<'info, Season>,

    #[account(
        init,
        payer = user,
        space = SeasonPass::SIZE,
        seeds = [SEASON_PASS_SEED, season.key().as_ref(), user.key().as_ref()],
        bump,
    )]
    pub season_pass: Account<'info, SeasonPass>,

    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct RecordSeasonXP<'info> {
    #[account(
        seeds = [SEASON_SEED, &season.season_number.to_le_bytes()],
        bump = season.bump,
    )]
    pub season: Account<'info, Season>,

    #[account(
        mut,
        seeds = [SEASON_PASS_SEED, season.key().as_ref(), user.key().as_ref()],
        bump = season_pass.bump,
        has_one = user,
    )]
    pub season_pass: Account<'info, SeasonPass>,

    pub user: Signer<'info>,
}

#[derive(Accounts)]
pub struct ClaimSeasonReward<'info> {
    #[account(
        mut,
        seeds = [SEASON_SEED, &season.season_number.to_le_bytes()],
        bump = season.bump,
    )]
    pub season: Account<'info, Season>,

    #[account(
        mut,
        seeds = [SEASON_PASS_SEED, season.key().as_ref(), user.key().as_ref()],
        bump = season_pass.bump,
        has_one = user,
    )]
    pub season_pass: Account<'info, SeasonPass>,

    #[account(
        seeds = [SEASON_REWARD_SEED, season.key().as_ref(), &season_reward.level.to_le_bytes()],
        bump = season_reward.bump,
    )]
    pub season_reward: Account<'info, SeasonReward>,

    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct EndSeason<'info> {
    #[account(
        mut,
        seeds = [SEASON_SEED, &season.season_number.to_le_bytes()],
        bump = season.bump,
    )]
    pub season: Account<'info, Season>,
    pub authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct FundSeason<'info> {
    #[account(
        mut,
        seeds = [SEASON_SEED, &season.season_number.to_le_bytes()],
        bump = season.bump,
    )]
    pub season: Account<'info, Season>,
    #[account(mut)]
    pub funder: Signer<'info>,
    pub system_program: Program<'info, System>,
}
