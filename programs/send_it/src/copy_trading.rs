use anchor_lang::prelude::*;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------
pub const MAX_FOLLOWERS: u32 = 10_000;
pub const MIN_ALLOCATION_LAMPORTS: u64 = 100_000_000; // 0.1 SOL

// ---------------------------------------------------------------------------
// Account structs
// ---------------------------------------------------------------------------

#[account]
pub struct TraderProfile {
    pub trader: Pubkey,
    pub total_pnl: i64,          // net PnL in lamports (signed)
    pub total_trades: u64,
    pub winning_trades: u64,
    pub followers_count: u32,
    pub created_at: i64,
    pub last_trade_at: i64,
    pub active: bool,
    pub bump: u8,
}

impl TraderProfile {
    /// Win rate in basis points (0-10000)
    pub fn win_rate_bps(&self) -> u16 {
        if self.total_trades == 0 {
            return 0;
        }
        ((self.winning_trades as u128 * 10_000) / self.total_trades as u128) as u16
    }
}

#[account]
pub struct CopyPosition {
    pub follower: Pubkey,
    pub leader: Pubkey,
    pub max_allocation: u64,     // max lamports to allocate per copy trade
    pub used_allocation: u64,    // currently deployed
    pub active: bool,
    pub created_at: i64,
    pub total_copied_trades: u64,
    pub copy_pnl: i64,          // follower's PnL from this copy relationship
    pub bump: u8,
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

#[event]
pub struct TraderProfileCreated {
    pub trader: Pubkey,
    pub timestamp: i64,
}

#[event]
pub struct TraderFollowed {
    pub follower: Pubkey,
    pub leader: Pubkey,
    pub max_allocation: u64,
}

#[event]
pub struct TraderUnfollowed {
    pub follower: Pubkey,
    pub leader: Pubkey,
}

#[event]
pub struct CopyTradeExecuted {
    pub leader: Pubkey,
    pub follower: Pubkey,
    pub token_mint: Pubkey,
    pub leader_amount: u64,
    pub follower_amount: u64,
    pub is_buy: bool,
    pub timestamp: i64,
}

#[event]
pub struct TraderStatsUpdated {
    pub trader: Pubkey,
    pub total_pnl: i64,
    pub win_rate_bps: u16,
    pub total_trades: u64,
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[error_code]
pub enum CopyTradingError {
    #[msg("Trader profile already exists")]
    ProfileAlreadyExists,
    #[msg("Trader profile is not active")]
    ProfileNotActive,
    #[msg("Cannot follow yourself")]
    CannotFollowSelf,
    #[msg("Already following this trader")]
    AlreadyFollowing,
    #[msg("Not following this trader")]
    NotFollowing,
    #[msg("Max followers reached for this trader")]
    MaxFollowersReached,
    #[msg("Allocation below minimum")]
    AllocationTooLow,
    #[msg("Insufficient remaining allocation")]
    InsufficientAllocation,
    #[msg("Copy position is not active")]
    PositionNotActive,
    #[msg("Unauthorized")]
    Unauthorized,
    #[msg("Arithmetic overflow")]
    Overflow,
}

// ---------------------------------------------------------------------------
// Instruction accounts
// ---------------------------------------------------------------------------

#[derive(Accounts)]
pub struct CreateTraderProfile<'info> {
    #[account(
        init,
        payer = trader,
        space = 8 + std::mem::size_of::<TraderProfile>(),
        seeds = [b"trader_profile", trader.key().as_ref()],
        bump
    )]
    pub profile: Account<'info, TraderProfile>,

    #[account(mut)]
    pub trader: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct FollowTrader<'info> {
    #[account(
        mut,
        seeds = [b"trader_profile", leader_profile.trader.as_ref()],
        bump = leader_profile.bump
    )]
    pub leader_profile: Account<'info, TraderProfile>,

    #[account(
        init,
        payer = follower,
        space = 8 + std::mem::size_of::<CopyPosition>(),
        seeds = [b"copy_position", follower.key().as_ref(), leader_profile.trader.as_ref()],
        bump
    )]
    pub copy_position: Account<'info, CopyPosition>,

    #[account(mut)]
    pub follower: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UnfollowTrader<'info> {
    #[account(
        mut,
        seeds = [b"trader_profile", leader_profile.trader.as_ref()],
        bump = leader_profile.bump
    )]
    pub leader_profile: Account<'info, TraderProfile>,

    #[account(
        mut,
        seeds = [b"copy_position", follower.key().as_ref(), leader_profile.trader.as_ref()],
        bump = copy_position.bump,
        has_one = follower @ CopyTradingError::Unauthorized
    )]
    pub copy_position: Account<'info, CopyPosition>,

    pub follower: Signer<'info>,
}

#[derive(Accounts)]
pub struct ExecuteCopyTrade<'info> {
    #[account(
        mut,
        seeds = [b"trader_profile", leader_profile.trader.as_ref()],
        bump = leader_profile.bump
    )]
    pub leader_profile: Account<'info, TraderProfile>,

    #[account(
        mut,
        seeds = [b"copy_position", copy_position.follower.as_ref(), leader_profile.trader.as_ref()],
        bump = copy_position.bump
    )]
    pub copy_position: Account<'info, CopyPosition>,

    /// The crank or leader who triggers the copy
    pub executor: Signer<'info>,
}

// ---------------------------------------------------------------------------
// Instructions
// ---------------------------------------------------------------------------

pub fn handle_create_trader_profile(ctx: Context<CreateTraderProfile>) -> Result<()> {
    let profile = &mut ctx.accounts.profile;
    let clock = Clock::get()?;

    profile.trader = ctx.accounts.trader.key();
    profile.total_pnl = 0;
    profile.total_trades = 0;
    profile.winning_trades = 0;
    profile.followers_count = 0;
    profile.created_at = clock.unix_timestamp;
    profile.last_trade_at = 0;
    profile.active = true;
    profile.bump = ctx.bumps.profile;

    emit!(TraderProfileCreated {
        trader: profile.trader,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

pub fn handle_follow_trader(
    ctx: Context<FollowTrader>,
    max_allocation: u64,
) -> Result<()> {
    let leader = &mut ctx.accounts.leader_profile;
    let follower_key = ctx.accounts.follower.key();

    require!(leader.active, CopyTradingError::ProfileNotActive);
    require!(follower_key != leader.trader, CopyTradingError::CannotFollowSelf);
    require!(leader.followers_count < MAX_FOLLOWERS, CopyTradingError::MaxFollowersReached);
    require!(max_allocation >= MIN_ALLOCATION_LAMPORTS, CopyTradingError::AllocationTooLow);

    leader.followers_count = leader
        .followers_count
        .checked_add(1)
        .ok_or(CopyTradingError::Overflow)?;

    let pos = &mut ctx.accounts.copy_position;
    let clock = Clock::get()?;

    pos.follower = follower_key;
    pos.leader = leader.trader;
    pos.max_allocation = max_allocation;
    pos.used_allocation = 0;
    pos.active = true;
    pos.created_at = clock.unix_timestamp;
    pos.total_copied_trades = 0;
    pos.copy_pnl = 0;
    pos.bump = ctx.bumps.copy_position;

    emit!(TraderFollowed {
        follower: follower_key,
        leader: leader.trader,
        max_allocation,
    });

    Ok(())
}

pub fn handle_unfollow_trader(ctx: Context<UnfollowTrader>) -> Result<()> {
    let leader = &mut ctx.accounts.leader_profile;
    let pos = &mut ctx.accounts.copy_position;

    require!(pos.active, CopyTradingError::PositionNotActive);

    pos.active = false;
    leader.followers_count = leader.followers_count.saturating_sub(1);

    emit!(TraderUnfollowed {
        follower: pos.follower,
        leader: leader.trader,
    });

    Ok(())
}

pub fn handle_execute_copy_trade(
    ctx: Context<ExecuteCopyTrade>,
    token_mint: Pubkey,
    leader_trade_amount: u64,
    leader_total_balance: u64,
    is_buy: bool,
    trade_pnl: i64,
) -> Result<()> {
    let leader = &mut ctx.accounts.leader_profile;
    let pos = &mut ctx.accounts.copy_position;
    let clock = Clock::get()?;

    require!(leader.active, CopyTradingError::ProfileNotActive);
    require!(pos.active, CopyTradingError::PositionNotActive);

    // Calculate proportional follower amount
    // follower_amount = leader_trade_amount * (follower_max_allocation / leader_total_balance)
    let follower_amount = if leader_total_balance > 0 {
        ((leader_trade_amount as u128)
            .checked_mul(pos.max_allocation as u128)
            .ok_or(CopyTradingError::Overflow)?
        )
        .checked_div(leader_total_balance as u128)
        .ok_or(CopyTradingError::Overflow)? as u64
    } else {
        0u64
    };

    let remaining = pos
        .max_allocation
        .saturating_sub(pos.used_allocation);
    require!(
        !is_buy || follower_amount <= remaining,
        CopyTradingError::InsufficientAllocation
    );

    // Update copy position
    if is_buy {
        pos.used_allocation = pos
            .used_allocation
            .checked_add(follower_amount)
            .ok_or(CopyTradingError::Overflow)?;
    } else {
        pos.used_allocation = pos.used_allocation.saturating_sub(follower_amount);
    }

    pos.total_copied_trades = pos
        .total_copied_trades
        .checked_add(1)
        .ok_or(CopyTradingError::Overflow)?;

    // Scale PnL proportionally for follower
    let follower_pnl = if leader_total_balance > 0 {
        ((trade_pnl as i128)
            .checked_mul(pos.max_allocation as i128)
            .ok_or(CopyTradingError::Overflow)?
        )
        .checked_div(leader_total_balance as i128)
        .ok_or(CopyTradingError::Overflow)? as i64
    } else {
        0i64
    };
    pos.copy_pnl = pos
        .copy_pnl
        .checked_add(follower_pnl)
        .ok_or(CopyTradingError::Overflow)?;

    // Update leader stats
    leader.total_trades = leader
        .total_trades
        .checked_add(1)
        .ok_or(CopyTradingError::Overflow)?;
    leader.total_pnl = leader
        .total_pnl
        .checked_add(trade_pnl)
        .ok_or(CopyTradingError::Overflow)?;
    if trade_pnl > 0 {
        leader.winning_trades = leader
            .winning_trades
            .checked_add(1)
            .ok_or(CopyTradingError::Overflow)?;
    }
    leader.last_trade_at = clock.unix_timestamp;

    emit!(CopyTradeExecuted {
        leader: leader.trader,
        follower: pos.follower,
        token_mint,
        leader_amount: leader_trade_amount,
        follower_amount,
        is_buy,
        timestamp: clock.unix_timestamp,
    });

    emit!(TraderStatsUpdated {
        trader: leader.trader,
        total_pnl: leader.total_pnl,
        win_rate_bps: leader.win_rate_bps(),
        total_trades: leader.total_trades,
    });

    Ok(())
}
