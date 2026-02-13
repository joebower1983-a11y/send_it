use anchor_lang::prelude::*;
use anchor_lang::system_program;

declare_id!("SenditHoLderRewards1111111111111111111111111");

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const PRECISION: u128 = 1_000_000_000_000; // 1e12 scaling factor
const REWARD_POOL_SEED: &[u8] = b"reward_pool";
const USER_REWARD_SEED: &[u8] = b"user_reward";
const REWARD_VAULT_SEED: &[u8] = b"reward_vault";

// ---------------------------------------------------------------------------
// Accounts
// ---------------------------------------------------------------------------

#[account]
#[derive(Default)]
pub struct RewardPool {
    /// The token mint this reward pool is for
    pub mint: Pubkey,
    /// Authority that can accrue rewards (program/trade handler)
    pub authority: Pubkey,
    /// Cumulative reward per token, scaled by 1e12
    pub reward_per_token_stored: u128,
    /// Total token supply eligible for rewards
    pub total_supply_eligible: u64,
    /// Timestamp of last reward accrual
    pub last_update_timestamp: i64,
    /// Minimum hold time in seconds before claiming (0 = disabled)
    pub min_hold_seconds: u64,
    /// Basis points of platform fee directed to this pool (e.g. 5000 = 50%)
    pub reward_fee_bps: u16,
    /// Bump seed for PDA
    pub bump: u8,
    /// Bump seed for vault PDA
    pub vault_bump: u8,
}

#[account]
#[derive(Default)]
pub struct UserRewardState {
    /// User's wallet
    pub user: Pubkey,
    /// Token mint
    pub mint: Pubkey,
    /// Snapshot of reward_per_token_stored at last update
    pub reward_per_token_paid: u128,
    /// Accumulated unclaimed rewards in lamports
    pub rewards_earned: u64,
    /// User's token balance known to the reward system
    pub balance: u64,
    /// Timestamp when user first held tokens (for min hold check)
    pub first_hold_timestamp: i64,
    /// Whether auto-compound is enabled
    pub auto_compound: bool,
    /// Bump seed for PDA
    pub bump: u8,
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

#[event]
pub struct RewardAccrued {
    pub mint: Pubkey,
    pub reward_amount: u64,
    pub new_reward_per_token: u128,
}

#[event]
pub struct RewardClaimed {
    pub user: Pubkey,
    pub mint: Pubkey,
    pub amount: u64,
}

#[event]
pub struct RewardAutoCompounded {
    pub user: Pubkey,
    pub mint: Pubkey,
    pub sol_amount: u64,
    pub tokens_received: u64,
}

#[event]
pub struct AutoCompoundToggled {
    pub user: Pubkey,
    pub mint: Pubkey,
    pub enabled: bool,
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[error_code]
pub enum RewardError {
    #[msg("Minimum hold time has not been met")]
    HoldTimeNotMet,
    #[msg("No rewards to claim")]
    NoRewardsToClaim,
    #[msg("Arithmetic overflow")]
    MathOverflow,
    #[msg("Unauthorized caller")]
    Unauthorized,
    #[msg("Invalid fee basis points (must be <= 10000)")]
    InvalidFeeBps,
}

// ---------------------------------------------------------------------------
// Instructions
// ---------------------------------------------------------------------------

/// Initialize a reward pool for a newly launched token.
pub fn initialize_reward_pool(
    ctx: Context<InitializeRewardPool>,
    reward_fee_bps: u16,
    min_hold_seconds: u64,
) -> Result<()> {
    require!(reward_fee_bps <= 10_000, RewardError::InvalidFeeBps);

    let pool = &mut ctx.accounts.reward_pool;
    pool.mint = ctx.accounts.mint.key();
    pool.authority = ctx.accounts.authority.key();
    pool.reward_per_token_stored = 0;
    pool.total_supply_eligible = 0;
    pool.last_update_timestamp = Clock::get()?.unix_timestamp;
    pool.min_hold_seconds = min_hold_seconds;
    pool.reward_fee_bps = reward_fee_bps;
    pool.bump = ctx.bumps.reward_pool;
    pool.vault_bump = ctx.bumps.reward_vault;

    Ok(())
}

/// Accrue rewards into the pool. Called by the trade instruction when fees are collected.
/// Transfers `reward_amount` lamports from the fee payer into the reward vault and
/// updates the global reward_per_token_stored.
pub fn accrue_rewards(ctx: Context<AccrueRewards>, reward_amount: u64) -> Result<()> {
    let pool = &mut ctx.accounts.reward_pool;

    // Transfer SOL from fee source to reward vault
    system_program::transfer(
        CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            system_program::Transfer {
                from: ctx.accounts.fee_payer.to_account_info(),
                to: ctx.accounts.reward_vault.to_account_info(),
            },
        ),
        reward_amount,
    )?;

    // Update reward_per_token_stored
    if pool.total_supply_eligible > 0 {
        let reward_scaled = (reward_amount as u128)
            .checked_mul(PRECISION)
            .ok_or(RewardError::MathOverflow)?;
        let increment = reward_scaled
            .checked_div(pool.total_supply_eligible as u128)
            .ok_or(RewardError::MathOverflow)?;
        pool.reward_per_token_stored = pool
            .reward_per_token_stored
            .checked_add(increment)
            .ok_or(RewardError::MathOverflow)?;
    }
    // If total_supply_eligible == 0, rewards are effectively lost (no holders).
    // In practice this shouldn't happen since trades imply holders exist.

    pool.last_update_timestamp = Clock::get()?.unix_timestamp;

    emit!(RewardAccrued {
        mint: pool.mint,
        reward_amount,
        new_reward_per_token: pool.reward_per_token_stored,
    });

    Ok(())
}

/// Update a user's reward state when their token balance changes (called on every trade).
/// This must be called BEFORE the balance actually changes, with the new_balance.
pub fn update_user_reward_state(
    ctx: Context<UpdateUserRewardState>,
    new_balance: u64,
) -> Result<()> {
    let pool = &ctx.accounts.reward_pool;
    let user_state = &mut ctx.accounts.user_reward_state;
    let clock = Clock::get()?;

    // First-time initialization
    if user_state.user == Pubkey::default() {
        user_state.user = ctx.accounts.user.key();
        user_state.mint = pool.mint;
        user_state.bump = ctx.bumps.user_reward_state;
        user_state.reward_per_token_paid = pool.reward_per_token_stored;
        user_state.first_hold_timestamp = clock.unix_timestamp;
    }

    // Calculate pending rewards for current balance
    let pending = _pending_rewards(pool.reward_per_token_stored, user_state)?;
    user_state.rewards_earned = user_state
        .rewards_earned
        .checked_add(pending)
        .ok_or(RewardError::MathOverflow)?;
    user_state.reward_per_token_paid = pool.reward_per_token_stored;

    // Update eligible supply on the pool
    let pool_mut = &mut ctx.accounts.reward_pool;
    pool_mut.total_supply_eligible = pool_mut
        .total_supply_eligible
        .checked_sub(user_state.balance)
        .ok_or(RewardError::MathOverflow)?
        .checked_add(new_balance)
        .ok_or(RewardError::MathOverflow)?;

    // Track hold timestamp
    let old_balance = user_state.balance;
    user_state.balance = new_balance;

    if old_balance == 0 && new_balance > 0 {
        user_state.first_hold_timestamp = clock.unix_timestamp;
    }
    // Reset timestamp if balance drops to zero (will be set again on re-entry)
    if new_balance == 0 {
        user_state.first_hold_timestamp = 0;
    }

    Ok(())
}

/// Claim accumulated holder rewards. Transfers SOL from the reward vault to the user.
/// If auto_compound is enabled, buys more tokens instead (requires bonding curve CPI — 
/// the actual CPI call is stubbed here; integrate with your trade module).
pub fn claim_holder_rewards(ctx: Context<ClaimHolderRewards>) -> Result<()> {
    let pool = &ctx.accounts.reward_pool;
    let user_state = &mut ctx.accounts.user_reward_state;
    let clock = Clock::get()?;

    // Settle pending rewards
    let pending = _pending_rewards(pool.reward_per_token_stored, user_state)?;
    let total_claimable = user_state
        .rewards_earned
        .checked_add(pending)
        .ok_or(RewardError::MathOverflow)?;

    require!(total_claimable > 0, RewardError::NoRewardsToClaim);

    // Check minimum hold time
    if pool.min_hold_seconds > 0 && user_state.balance > 0 {
        let held_for = clock
            .unix_timestamp
            .checked_sub(user_state.first_hold_timestamp)
            .ok_or(RewardError::MathOverflow)?;
        require!(
            held_for >= pool.min_hold_seconds as i64,
            RewardError::HoldTimeNotMet
        );
    }

    // Reset user state
    user_state.rewards_earned = 0;
    user_state.reward_per_token_paid = pool.reward_per_token_stored;

    if user_state.auto_compound {
        // --- Auto-compound path ---
        // TODO: CPI into bonding curve buy instruction with `total_claimable` lamports.
        // The buy instruction should return the number of tokens received.
        // For now we emit the event with tokens_received = 0 as a placeholder.
        //
        // After CPI, update user_state.balance += tokens_received
        // and pool.total_supply_eligible += tokens_received

        emit!(RewardAutoCompounded {
            user: user_state.user,
            mint: pool.mint,
            sol_amount: total_claimable,
            tokens_received: 0, // placeholder until CPI integrated
        });
    } else {
        // --- Direct claim path ---
        // Transfer SOL from reward vault (PDA) to user
        let mint_key = pool.mint;
        let seeds: &[&[u8]] = &[
            REWARD_VAULT_SEED,
            mint_key.as_ref(),
            &[pool.vault_bump],
        ];

        let vault_info = ctx.accounts.reward_vault.to_account_info();
        let user_info = ctx.accounts.user.to_account_info();

        **vault_info.try_borrow_mut_lamports()? -= total_claimable;
        **user_info.try_borrow_mut_lamports()? += total_claimable;

        // Note: We use direct lamport manipulation since the vault is a PDA system account.
        // The PDA signer seeds are available if CPI transfer is preferred:
        let _signer_seeds: &[&[&[u8]]] = &[seeds];

        emit!(RewardClaimed {
            user: user_state.user,
            mint: pool.mint,
            amount: total_claimable,
        });
    }

    Ok(())
}

/// Toggle auto-compound for a user.
pub fn toggle_auto_compound(ctx: Context<ToggleAutoCompound>, enabled: bool) -> Result<()> {
    let user_state = &mut ctx.accounts.user_reward_state;
    user_state.auto_compound = enabled;

    emit!(AutoCompoundToggled {
        user: user_state.user,
        mint: ctx.accounts.reward_pool.mint,
        enabled,
    });

    Ok(())
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Calculate pending (unsettled) rewards for a user.
fn _pending_rewards(
    global_reward_per_token: u128,
    user_state: &UserRewardState,
) -> Result<u64> {
    if user_state.balance == 0 {
        return Ok(0);
    }

    let delta = global_reward_per_token
        .checked_sub(user_state.reward_per_token_paid)
        .ok_or(RewardError::MathOverflow)?;

    let reward = (user_state.balance as u128)
        .checked_mul(delta)
        .ok_or(RewardError::MathOverflow)?
        .checked_div(PRECISION)
        .ok_or(RewardError::MathOverflow)?;

    // Safe truncation: reward should fit in u64 for practical amounts
    Ok(reward as u64)
}

// ---------------------------------------------------------------------------
// Context structs (Anchor accounts validation)
// ---------------------------------------------------------------------------

#[derive(Accounts)]
pub struct InitializeRewardPool<'info> {
    #[account(
        init,
        payer = authority,
        space = 8 + 32 + 32 + 16 + 8 + 8 + 8 + 2 + 1 + 1 + 32, // discriminator + fields + padding
        seeds = [REWARD_POOL_SEED, mint.key().as_ref()],
        bump,
    )]
    pub reward_pool: Account<'info, RewardPool>,

    /// CHECK: Reward vault is a PDA system account that holds SOL.
    #[account(
        mut,
        seeds = [REWARD_VAULT_SEED, mint.key().as_ref()],
        bump,
    )]
    pub reward_vault: SystemAccount<'info>,

    /// CHECK: The token mint for which we create the reward pool.
    pub mint: UncheckedAccount<'info>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct AccrueRewards<'info> {
    #[account(
        mut,
        seeds = [REWARD_POOL_SEED, reward_pool.mint.as_ref()],
        bump = reward_pool.bump,
        has_one = authority @ RewardError::Unauthorized,
    )]
    pub reward_pool: Account<'info, RewardPool>,

    /// CHECK: Reward vault PDA.
    #[account(
        mut,
        seeds = [REWARD_VAULT_SEED, reward_pool.mint.as_ref()],
        bump = reward_pool.vault_bump,
    )]
    pub reward_vault: SystemAccount<'info>,

    /// The account paying the fee (typically the trade escrow or platform fee account).
    #[account(mut)]
    pub fee_payer: Signer<'info>,

    /// Authority allowed to accrue (the program's trade handler).
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UpdateUserRewardState<'info> {
    #[account(
        mut,
        seeds = [REWARD_POOL_SEED, reward_pool.mint.as_ref()],
        bump = reward_pool.bump,
        has_one = authority @ RewardError::Unauthorized,
    )]
    pub reward_pool: Account<'info, RewardPool>,

    #[account(
        init_if_needed,
        payer = payer,
        space = 8 + 32 + 32 + 16 + 16 + 8 + 8 + 8 + 1 + 1 + 16, // discriminator + fields + padding
        seeds = [USER_REWARD_SEED, reward_pool.mint.as_ref(), user.key().as_ref()],
        bump,
    )]
    pub user_reward_state: Account<'info, UserRewardState>,

    /// CHECK: The user whose balance is changing.
    pub user: UncheckedAccount<'info>,

    /// Authority (program trade handler).
    pub authority: Signer<'info>,

    #[account(mut)]
    pub payer: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ClaimHolderRewards<'info> {
    #[account(
        seeds = [REWARD_POOL_SEED, reward_pool.mint.as_ref()],
        bump = reward_pool.bump,
    )]
    pub reward_pool: Account<'info, RewardPool>,

    #[account(
        mut,
        seeds = [USER_REWARD_SEED, reward_pool.mint.as_ref(), user.key().as_ref()],
        bump = user_reward_state.bump,
        constraint = user_reward_state.user == user.key(),
    )]
    pub user_reward_state: Account<'info, UserRewardState>,

    /// CHECK: Reward vault PDA (SOL source).
    #[account(
        mut,
        seeds = [REWARD_VAULT_SEED, reward_pool.mint.as_ref()],
        bump = reward_pool.vault_bump,
    )]
    pub reward_vault: SystemAccount<'info>,

    #[account(mut)]
    pub user: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ToggleAutoCompound<'info> {
    #[account(
        seeds = [REWARD_POOL_SEED, reward_pool.mint.as_ref()],
        bump = reward_pool.bump,
    )]
    pub reward_pool: Account<'info, RewardPool>,

    #[account(
        mut,
        seeds = [USER_REWARD_SEED, reward_pool.mint.as_ref(), user.key().as_ref()],
        bump = user_reward_state.bump,
        constraint = user_reward_state.user == user.key(),
    )]
    pub user_reward_state: Account<'info, UserRewardState>,

    pub user: Signer<'info>,
}
