use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer, Mint};
use anchor_spl::associated_token::AssociatedToken;

declare_id!("SenditStaking11111111111111111111111111111111");

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const STAKE_POOL_SEED: &[u8] = b"stake_pool";
const USER_STAKE_SEED: &[u8] = b"user_stake";
const STAKE_VAULT_SEED: &[u8] = b"stake_vault";
const PRECISION: u128 = 1_000_000_000_000; // 1e12

// ---------------------------------------------------------------------------
// Accounts
// ---------------------------------------------------------------------------

#[account]
#[derive(Default)]
pub struct StakePool {
    /// Token mint this pool is for
    pub mint: Pubkey,
    /// Creator who initialized the pool
    pub creator: Pubkey,
    /// Total tokens currently staked
    pub total_staked: u64,
    /// Reward rate: tokens per second per staked token (scaled by PRECISION)
    pub reward_rate: u128,
    /// Cumulative reward per token stored (scaled by PRECISION)
    pub reward_per_token_stored: u128,
    /// Last time rewards were updated
    pub last_update: i64,
    /// Whether the token has graduated (required to create pool)
    pub graduated: bool,
    /// Bump seed
    pub bump: u8,
    /// Vault bump
    pub vault_bump: u8,
}

#[account]
#[derive(Default)]
pub struct UserStake {
    /// User wallet
    pub user: Pubkey,
    /// Token mint
    pub mint: Pubkey,
    /// Amount staked
    pub amount: u64,
    /// Timestamp of first stake
    pub start_time: i64,
    /// Accumulated rewards earned (unclaimed)
    pub rewards_earned: u64,
    /// Snapshot of reward_per_token_stored at last interaction
    pub reward_per_token_paid: u128,
    /// Bump seed
    pub bump: u8,
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

#[event]
pub struct StakePoolCreated {
    pub mint: Pubkey,
    pub creator: Pubkey,
    pub reward_rate: u128,
    pub timestamp: i64,
}

#[event]
pub struct TokensStaked {
    pub user: Pubkey,
    pub mint: Pubkey,
    pub amount: u64,
    pub total_staked: u64,
    pub timestamp: i64,
}

#[event]
pub struct TokensUnstaked {
    pub user: Pubkey,
    pub mint: Pubkey,
    pub amount: u64,
    pub total_staked: u64,
    pub timestamp: i64,
}

#[event]
pub struct RewardsClaimed {
    pub user: Pubkey,
    pub mint: Pubkey,
    pub amount: u64,
    pub timestamp: i64,
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[error_code]
pub enum StakingError {
    #[msg("Token has not graduated yet")]
    NotGraduated,
    #[msg("Stake pool already exists")]
    PoolAlreadyExists,
    #[msg("Amount must be greater than zero")]
    ZeroAmount,
    #[msg("Insufficient staked balance")]
    InsufficientStake,
    #[msg("No rewards to claim")]
    NoRewards,
    #[msg("Reward rate must be greater than zero")]
    InvalidRewardRate,
    #[msg("Arithmetic overflow")]
    MathOverflow,
    #[msg("Only the pool creator can update reward rate")]
    UnauthorizedCreator,
}

// ---------------------------------------------------------------------------
// Instructions
// ---------------------------------------------------------------------------

#[program]
pub mod staking {
    use super::*;

    /// Create a stake pool for a graduated token. Only the creator can call this.
    pub fn create_stake_pool(
        ctx: Context<CreateStakePool>,
        reward_rate: u128,
    ) -> Result<()> {
        require!(reward_rate > 0, StakingError::InvalidRewardRate);

        let clock = Clock::get()?;
        let pool = &mut ctx.accounts.stake_pool;
        pool.mint = ctx.accounts.mint.key();
        pool.creator = ctx.accounts.creator.key();
        pool.total_staked = 0;
        pool.reward_rate = reward_rate;
        pool.reward_per_token_stored = 0;
        pool.last_update = clock.unix_timestamp;
        pool.graduated = true;
        pool.bump = ctx.bumps.stake_pool;
        pool.vault_bump = ctx.bumps.stake_vault;

        emit!(StakePoolCreated {
            mint: pool.mint,
            creator: pool.creator,
            reward_rate,
            timestamp: clock.unix_timestamp,
        });

        Ok(())
    }

    /// Stake tokens into the pool.
    pub fn stake_tokens(ctx: Context<StakeTokens>, amount: u64) -> Result<()> {
        require!(amount > 0, StakingError::ZeroAmount);

        let clock = Clock::get()?;
        let pool = &mut ctx.accounts.stake_pool;
        let user_stake = &mut ctx.accounts.user_stake;

        // Update global reward accumulator
        update_reward_per_token(pool, clock.unix_timestamp)?;

        // Settle pending user rewards
        settle_user_rewards(pool, user_stake)?;

        // Transfer tokens from user to vault
        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.user_token_account.to_account_info(),
                    to: ctx.accounts.stake_vault.to_account_info(),
                    authority: ctx.accounts.user.to_account_info(),
                },
            ),
            amount,
        )?;

        // Update state
        user_stake.amount = user_stake.amount.checked_add(amount).ok_or(StakingError::MathOverflow)?;
        if user_stake.start_time == 0 {
            user_stake.start_time = clock.unix_timestamp;
        }
        pool.total_staked = pool.total_staked.checked_add(amount).ok_or(StakingError::MathOverflow)?;

        emit!(TokensStaked {
            user: ctx.accounts.user.key(),
            mint: pool.mint,
            amount,
            total_staked: pool.total_staked,
            timestamp: clock.unix_timestamp,
        });

        Ok(())
    }

    /// Unstake tokens from the pool.
    pub fn unstake_tokens(ctx: Context<UnstakeTokens>, amount: u64) -> Result<()> {
        require!(amount > 0, StakingError::ZeroAmount);

        let clock = Clock::get()?;
        let pool = &mut ctx.accounts.stake_pool;
        let user_stake = &mut ctx.accounts.user_stake;

        require!(user_stake.amount >= amount, StakingError::InsufficientStake);

        // Update rewards
        update_reward_per_token(pool, clock.unix_timestamp)?;
        settle_user_rewards(pool, user_stake)?;

        // Transfer tokens back from vault to user
        let mint_key = pool.mint;
        let seeds = &[
            STAKE_VAULT_SEED,
            mint_key.as_ref(),
            &[pool.vault_bump],
        ];
        let signer_seeds = &[&seeds[..]];

        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.stake_vault.to_account_info(),
                    to: ctx.accounts.user_token_account.to_account_info(),
                    authority: ctx.accounts.stake_vault.to_account_info(),
                },
                signer_seeds,
            ),
            amount,
        )?;

        user_stake.amount = user_stake.amount.checked_sub(amount).ok_or(StakingError::MathOverflow)?;
        pool.total_staked = pool.total_staked.checked_sub(amount).ok_or(StakingError::MathOverflow)?;

        emit!(TokensUnstaked {
            user: ctx.accounts.user.key(),
            mint: pool.mint,
            amount,
            total_staked: pool.total_staked,
            timestamp: clock.unix_timestamp,
        });

        Ok(())
    }

    /// Claim accumulated staking rewards.
    pub fn claim_staking_rewards(ctx: Context<ClaimStakingRewards>) -> Result<()> {
        let clock = Clock::get()?;
        let pool = &mut ctx.accounts.stake_pool;
        let user_stake = &mut ctx.accounts.user_stake;

        update_reward_per_token(pool, clock.unix_timestamp)?;
        settle_user_rewards(pool, user_stake)?;

        let rewards = user_stake.rewards_earned;
        require!(rewards > 0, StakingError::NoRewards);

        // Transfer reward tokens from vault
        let mint_key = pool.mint;
        let seeds = &[
            STAKE_VAULT_SEED,
            mint_key.as_ref(),
            &[pool.vault_bump],
        ];
        let signer_seeds = &[&seeds[..]];

        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.reward_vault.to_account_info(),
                    to: ctx.accounts.user_token_account.to_account_info(),
                    authority: ctx.accounts.reward_vault.to_account_info(),
                },
                signer_seeds,
            ),
            rewards,
        )?;

        user_stake.rewards_earned = 0;

        emit!(RewardsClaimed {
            user: ctx.accounts.user.key(),
            mint: pool.mint,
            amount: rewards,
            timestamp: clock.unix_timestamp,
        });

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn update_reward_per_token(pool: &mut StakePool, now: i64) -> Result<()> {
    if pool.total_staked > 0 {
        let elapsed = (now - pool.last_update) as u128;
        let additional = elapsed
            .checked_mul(pool.reward_rate)
            .ok_or(StakingError::MathOverflow)?
            .checked_div(pool.total_staked as u128)
            .ok_or(StakingError::MathOverflow)?;
        pool.reward_per_token_stored = pool
            .reward_per_token_stored
            .checked_add(additional)
            .ok_or(StakingError::MathOverflow)?;
    }
    pool.last_update = now;
    Ok(())
}

fn settle_user_rewards(pool: &StakePool, user_stake: &mut UserStake) -> Result<()> {
    if user_stake.amount > 0 {
        let pending = (user_stake.amount as u128)
            .checked_mul(
                pool.reward_per_token_stored
                    .checked_sub(user_stake.reward_per_token_paid)
                    .ok_or(StakingError::MathOverflow)?,
            )
            .ok_or(StakingError::MathOverflow)?
            .checked_div(PRECISION)
            .ok_or(StakingError::MathOverflow)? as u64;
        user_stake.rewards_earned = user_stake
            .rewards_earned
            .checked_add(pending)
            .ok_or(StakingError::MathOverflow)?;
    }
    user_stake.reward_per_token_paid = pool.reward_per_token_stored;
    Ok(())
}

// ---------------------------------------------------------------------------
// Context Structs
// ---------------------------------------------------------------------------

#[derive(Accounts)]
pub struct CreateStakePool<'info> {
    #[account(mut)]
    pub creator: Signer<'info>,

    pub mint: Account<'info, Mint>,

    #[account(
        init,
        payer = creator,
        space = 8 + std::mem::size_of::<StakePool>(),
        seeds = [STAKE_POOL_SEED, mint.key().as_ref()],
        bump,
    )]
    pub stake_pool: Account<'info, StakePool>,

    #[account(
        init,
        payer = creator,
        token::mint = mint,
        token::authority = stake_vault,
        seeds = [STAKE_VAULT_SEED, mint.key().as_ref()],
        bump,
    )]
    pub stake_vault: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct StakeTokens<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        seeds = [STAKE_POOL_SEED, stake_pool.mint.as_ref()],
        bump = stake_pool.bump,
    )]
    pub stake_pool: Account<'info, StakePool>,

    #[account(
        init_if_needed,
        payer = user,
        space = 8 + std::mem::size_of::<UserStake>(),
        seeds = [USER_STAKE_SEED, stake_pool.mint.as_ref(), user.key().as_ref()],
        bump,
    )]
    pub user_stake: Account<'info, UserStake>,

    #[account(
        mut,
        token::mint = stake_pool.mint,
        token::authority = user,
    )]
    pub user_token_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        seeds = [STAKE_VAULT_SEED, stake_pool.mint.as_ref()],
        bump = stake_pool.vault_bump,
    )]
    pub stake_vault: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UnstakeTokens<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        seeds = [STAKE_POOL_SEED, stake_pool.mint.as_ref()],
        bump = stake_pool.bump,
    )]
    pub stake_pool: Account<'info, StakePool>,

    #[account(
        mut,
        seeds = [USER_STAKE_SEED, stake_pool.mint.as_ref(), user.key().as_ref()],
        bump = user_stake.bump,
        has_one = user,
    )]
    pub user_stake: Account<'info, UserStake>,

    #[account(
        mut,
        token::mint = stake_pool.mint,
        token::authority = user,
    )]
    pub user_token_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        seeds = [STAKE_VAULT_SEED, stake_pool.mint.as_ref()],
        bump = stake_pool.vault_bump,
    )]
    pub stake_vault: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct ClaimStakingRewards<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        seeds = [STAKE_POOL_SEED, stake_pool.mint.as_ref()],
        bump = stake_pool.bump,
    )]
    pub stake_pool: Account<'info, StakePool>,

    #[account(
        mut,
        seeds = [USER_STAKE_SEED, stake_pool.mint.as_ref(), user.key().as_ref()],
        bump = user_stake.bump,
        has_one = user,
    )]
    pub user_stake: Account<'info, UserStake>,

    #[account(
        mut,
        token::mint = stake_pool.mint,
        token::authority = user,
    )]
    pub user_token_account: Account<'info, TokenAccount>,

    /// Separate reward vault (could be same as stake vault depending on design)
    #[account(mut)]
    pub reward_vault: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}
