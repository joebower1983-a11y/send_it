use anchor_lang::prelude::*;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------
pub const MAX_HOURLY_SNAPSHOTS: usize = 168; // 7 days of hourly data
pub const MAX_WHALE_TRANSACTIONS: usize = 50;
pub const MAX_TOP_HOLDERS: usize = 20;
pub const WHALE_THRESHOLD_LAMPORTS: u64 = 1_000_000_000; // 1 SOL
pub const HOUR_SECONDS: i64 = 3600;

// ---------------------------------------------------------------------------
// Account structs
// ---------------------------------------------------------------------------

#[account]
pub struct TokenAnalytics {
    pub token_mint: Pubkey,
    pub total_volume: u64,
    pub total_trades: u64,
    pub holder_count: u32,
    pub last_update_slot: u64,
    pub last_snapshot_ts: i64,

    /// Ring-buffer of hourly volume snapshots
    pub snapshot_head: u16,
    pub hourly_volumes: [u64; MAX_HOURLY_SNAPSHOTS],

    /// Holder count history (parallel to hourly_volumes)
    pub hourly_holders: [u32; MAX_HOURLY_SNAPSHOTS],

    /// Recent whale transactions (> 1 SOL)
    pub whale_tx_head: u16,
    pub whale_transactions: [WhaleTransaction; MAX_WHALE_TRANSACTIONS],

    pub bump: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Default)]
pub struct WhaleTransaction {
    pub trader: Pubkey,
    pub amount_lamports: u64,
    pub is_buy: bool,
    pub timestamp: i64,
}

#[account]
pub struct WhaleTracker {
    pub token_mint: Pubkey,
    pub top_holders: [HolderEntry; MAX_TOP_HOLDERS],
    pub holder_count: u8,
    pub bump: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Default)]
pub struct HolderEntry {
    pub wallet: Pubkey,
    pub balance: u64,
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

#[event]
pub struct AnalyticsUpdated {
    pub token_mint: Pubkey,
    pub total_volume: u64,
    pub total_trades: u64,
    pub holder_count: u32,
    pub timestamp: i64,
}

#[event]
pub struct WhaleAlert {
    pub token_mint: Pubkey,
    pub trader: Pubkey,
    pub amount_lamports: u64,
    pub is_buy: bool,
    pub timestamp: i64,
}

#[event]
pub struct HolderDistribution {
    pub token_mint: Pubkey,
    pub top_holders: Vec<HolderEntry>,
    pub total_holders: u32,
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[error_code]
pub enum AnalyticsError {
    #[msg("Analytics already updated this slot")]
    AlreadyUpdatedThisSlot,
    #[msg("Invalid token mint")]
    InvalidTokenMint,
    #[msg("Arithmetic overflow")]
    Overflow,
}

// ---------------------------------------------------------------------------
// Instruction accounts
// ---------------------------------------------------------------------------

#[derive(Accounts)]
#[instruction(token_mint: Pubkey)]
pub struct InitializeAnalytics<'info> {
    #[account(
        init,
        payer = payer,
        space = 8 + std::mem::size_of::<TokenAnalytics>(),
        seeds = [b"token_analytics", token_mint.as_ref()],
        bump
    )]
    pub token_analytics: Account<'info, TokenAnalytics>,

    #[account(
        init,
        payer = payer,
        space = 8 + std::mem::size_of::<WhaleTracker>(),
        seeds = [b"whale_tracker", token_mint.as_ref()],
        bump
    )]
    pub whale_tracker: Account<'info, WhaleTracker>,

    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UpdateAnalytics<'info> {
    #[account(
        mut,
        seeds = [b"token_analytics", token_analytics.token_mint.as_ref()],
        bump = token_analytics.bump
    )]
    pub token_analytics: Account<'info, TokenAnalytics>,

    #[account(
        mut,
        seeds = [b"whale_tracker", token_analytics.token_mint.as_ref()],
        bump = whale_tracker.bump
    )]
    pub whale_tracker: Account<'info, WhaleTracker>,

    /// Permissionless crank signer
    pub crank: Signer<'info>,
}

#[derive(Accounts)]
pub struct GetHolderDistribution<'info> {
    #[account(
        seeds = [b"token_analytics", token_analytics.token_mint.as_ref()],
        bump = token_analytics.bump
    )]
    pub token_analytics: Account<'info, TokenAnalytics>,

    #[account(
        seeds = [b"whale_tracker", token_analytics.token_mint.as_ref()],
        bump = whale_tracker.bump
    )]
    pub whale_tracker: Account<'info, WhaleTracker>,
}

// ---------------------------------------------------------------------------
// Instructions
// ---------------------------------------------------------------------------

pub fn handle_initialize_analytics(
    ctx: Context<InitializeAnalytics>,
    token_mint: Pubkey,
) -> Result<()> {
    let analytics = &mut ctx.accounts.token_analytics;
    analytics.token_mint = token_mint;
    analytics.bump = ctx.bumps.token_analytics;
    analytics.last_snapshot_ts = Clock::get()?.unix_timestamp;

    let tracker = &mut ctx.accounts.whale_tracker;
    tracker.token_mint = token_mint;
    tracker.bump = ctx.bumps.whale_tracker;

    Ok(())
}

pub fn handle_update_analytics(
    ctx: Context<UpdateAnalytics>,
    trade_volume_lamports: u64,
    is_buy: bool,
    trader: Pubkey,
    current_holder_count: u32,
) -> Result<()> {
    let analytics = &mut ctx.accounts.token_analytics;
    let clock = Clock::get()?;
    let current_slot = clock.slot;

    require!(
        current_slot > analytics.last_update_slot,
        AnalyticsError::AlreadyUpdatedThisSlot
    );

    // Update totals
    analytics.total_volume = analytics
        .total_volume
        .checked_add(trade_volume_lamports)
        .ok_or(AnalyticsError::Overflow)?;
    analytics.total_trades = analytics
        .total_trades
        .checked_add(1)
        .ok_or(AnalyticsError::Overflow)?;
    analytics.holder_count = current_holder_count;
    analytics.last_update_slot = current_slot;

    // Rotate hourly snapshot if needed
    let now = clock.unix_timestamp;
    if now - analytics.last_snapshot_ts >= HOUR_SECONDS {
        let head = analytics.snapshot_head as usize;
        analytics.hourly_volumes[head] = trade_volume_lamports;
        analytics.hourly_holders[head] = current_holder_count;
        analytics.snapshot_head = ((head + 1) % MAX_HOURLY_SNAPSHOTS) as u16;
        analytics.last_snapshot_ts = now;
    } else {
        // Accumulate into current bucket
        let head = if analytics.snapshot_head == 0 {
            MAX_HOURLY_SNAPSHOTS - 1
        } else {
            (analytics.snapshot_head - 1) as usize
        };
        analytics.hourly_volumes[head] = analytics.hourly_volumes[head]
            .checked_add(trade_volume_lamports)
            .ok_or(AnalyticsError::Overflow)?;
        analytics.hourly_holders[head] = current_holder_count;
    }

    // Track whale transactions
    if trade_volume_lamports >= WHALE_THRESHOLD_LAMPORTS {
        let wh = analytics.whale_tx_head as usize;
        analytics.whale_transactions[wh] = WhaleTransaction {
            trader,
            amount_lamports: trade_volume_lamports,
            is_buy,
            timestamp: now,
        };
        analytics.whale_tx_head = ((wh + 1) % MAX_WHALE_TRANSACTIONS) as u16;

        emit!(WhaleAlert {
            token_mint: analytics.token_mint,
            trader,
            amount_lamports: trade_volume_lamports,
            is_buy,
            timestamp: now,
        });
    }

    // Update whale tracker top holders
    let tracker = &mut ctx.accounts.whale_tracker;
    update_top_holders(tracker, trader, trade_volume_lamports, is_buy);

    emit!(AnalyticsUpdated {
        token_mint: analytics.token_mint,
        total_volume: analytics.total_volume,
        total_trades: analytics.total_trades,
        holder_count: analytics.holder_count,
        timestamp: now,
    });

    Ok(())
}

pub fn handle_get_holder_distribution(ctx: Context<GetHolderDistribution>) -> Result<()> {
    let analytics = &ctx.accounts.token_analytics;
    let tracker = &ctx.accounts.whale_tracker;

    let count = tracker.holder_count as usize;
    let holders: Vec<HolderEntry> = tracker.top_holders[..count].to_vec();

    emit!(HolderDistribution {
        token_mint: analytics.token_mint,
        top_holders: holders,
        total_holders: analytics.holder_count,
    });

    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn update_top_holders(tracker: &mut WhaleTracker, wallet: Pubkey, amount: u64, is_buy: bool) {
    let count = tracker.holder_count as usize;

    // Check if wallet already in list
    for i in 0..count {
        if tracker.top_holders[i].wallet == wallet {
            if is_buy {
                tracker.top_holders[i].balance = tracker.top_holders[i]
                    .balance
                    .saturating_add(amount);
            } else {
                tracker.top_holders[i].balance = tracker.top_holders[i]
                    .balance
                    .saturating_sub(amount);
            }
            sort_holders(tracker);
            return;
        }
    }

    // New entry — only add on buys
    if !is_buy {
        return;
    }

    if count < MAX_TOP_HOLDERS {
        tracker.top_holders[count] = HolderEntry {
            wallet,
            balance: amount,
        };
        tracker.holder_count += 1;
    } else {
        // Replace smallest if new amount is larger
        let min_idx = (0..MAX_TOP_HOLDERS)
            .min_by_key(|&i| tracker.top_holders[i].balance)
            .unwrap();
        if amount > tracker.top_holders[min_idx].balance {
            tracker.top_holders[min_idx] = HolderEntry {
                wallet,
                balance: amount,
            };
        }
    }
    sort_holders(tracker);
}

fn sort_holders(tracker: &mut WhaleTracker) {
    let count = tracker.holder_count as usize;
    // Simple insertion sort (max 20 elements)
    for i in 1..count {
        let mut j = i;
        while j > 0 && tracker.top_holders[j].balance > tracker.top_holders[j - 1].balance {
            let tmp = tracker.top_holders[j];
            tracker.top_holders[j] = tracker.top_holders[j - 1];
            tracker.top_holders[j - 1] = tmp;
            j -= 1;
        }
    }
}
