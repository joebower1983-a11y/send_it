use anchor_lang::prelude::*;
use anchor_lang::system_program;

// ============================================================================
// SEEDS & CONSTANTS
// ============================================================================

pub const REFERRAL_ACCOUNT_SEED: &[u8] = b"referral";
pub const REFERRAL_VAULT_SEED: &[u8] = b"referral_vault";

/// Referrer gets 25% of the platform fee by default (configurable).
pub const DEFAULT_REFERRAL_FEE_BPS: u16 = 2500; // 25% of platform fee

// ============================================================================
// ACCOUNTS
// ============================================================================

/// Global referral configuration.
#[account]
pub struct ReferralConfig {
    pub authority: Pubkey,
    /// Basis points of platform fee allocated to referrer (e.g. 2500 = 25%).
    pub referral_fee_bps: u16,
    /// Platform treasury that receives the remainder.
    pub treasury: Pubkey,
    pub bump: u8,
}

impl ReferralConfig {
    pub const SIZE: usize = 8 + 32 + 2 + 32 + 1;
}

/// Per-user referral account, PDA from [REFERRAL_ACCOUNT_SEED, user].
#[account]
pub struct ReferralAccount {
    /// The user this referral account belongs to.
    pub user: Pubkey,
    /// The user's referrer (Pubkey::default() if none).
    pub referrer: Pubkey,
    /// How many users this user has referred.
    pub total_referred: u64,
    /// Total lamports earned from referrals (pending + claimed).
    pub total_earned: u64,
    /// Lamports available to claim.
    pub claimable: u64,
    /// Timestamp of registration.
    pub registered_at: i64,
    /// Bump seed.
    pub bump: u8,
}

impl ReferralAccount {
    // 8 + 32 + 32 + 8 + 8 + 8 + 8 + 1
    pub const SIZE: usize = 8 + 32 + 32 + 8 + 8 + 8 + 8 + 1;
}

// ============================================================================
// INSTRUCTION CONTEXTS
// ============================================================================

#[derive(Accounts)]
pub struct InitializeReferralConfig<'info> {
    #[account(
        init,
        payer = authority,
        space = ReferralConfig::SIZE,
        seeds = [b"referral_config"],
        bump,
    )]
    pub config: Account<'info, ReferralConfig>,
    /// CHECK: Treasury wallet.
    pub treasury: AccountInfo<'info>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

/// Register a new referral account. If `referrer_account` is provided, link the referral.
#[derive(Accounts)]
pub struct RegisterReferral<'info> {
    #[account(
        init,
        payer = user,
        space = ReferralAccount::SIZE,
        seeds = [REFERRAL_ACCOUNT_SEED, user.key().as_ref()],
        bump,
    )]
    pub referral_account: Account<'info, ReferralAccount>,
    /// The referrer's existing referral account (optional — can be None if no referrer).
    #[account(
        mut,
        seeds = [REFERRAL_ACCOUNT_SEED, referrer_account.user.as_ref()],
        bump = referrer_account.bump,
    )]
    pub referrer_account: Option<Account<'info, ReferralAccount>>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}

/// Called during a trade to credit referral rewards.
#[derive(Accounts)]
pub struct CreditReferralReward<'info> {
    /// The trading user's referral account (must have a referrer set).
    #[account(
        seeds = [REFERRAL_ACCOUNT_SEED, trader_referral.user.as_ref()],
        bump = trader_referral.bump,
    )]
    pub trader_referral: Account<'info, ReferralAccount>,
    /// The referrer's account to credit.
    #[account(
        mut,
        seeds = [REFERRAL_ACCOUNT_SEED, referrer_referral.user.as_ref()],
        bump = referrer_referral.bump,
        constraint = referrer_referral.user == trader_referral.referrer @ ReferralError::ReferrerMismatch,
    )]
    pub referrer_referral: Account<'info, ReferralAccount>,
    #[account(
        seeds = [b"referral_config"],
        bump = config.bump,
    )]
    pub config: Account<'info, ReferralConfig>,
    /// The platform fee payer (program authority / CPI caller).
    #[account(mut)]
    pub fee_payer: Signer<'info>,
    /// CHECK: Referral vault PDA that holds reward lamports.
    #[account(
        mut,
        seeds = [REFERRAL_VAULT_SEED],
        bump,
    )]
    pub referral_vault: AccountInfo<'info>,
    pub system_program: Program<'info, System>,
}

/// Claim accumulated referral rewards.
#[derive(Accounts)]
pub struct ClaimReferralRewards<'info> {
    #[account(
        mut,
        seeds = [REFERRAL_ACCOUNT_SEED, user.key().as_ref()],
        bump = referral_account.bump,
        constraint = referral_account.user == user.key() @ ReferralError::Unauthorized,
    )]
    pub referral_account: Account<'info, ReferralAccount>,
    #[account(mut)]
    pub user: Signer<'info>,
    /// CHECK: Referral vault PDA that holds reward lamports.
    #[account(
        mut,
        seeds = [REFERRAL_VAULT_SEED],
        bump,
    )]
    pub referral_vault: AccountInfo<'info>,
    pub system_program: Program<'info, System>,
}

// ============================================================================
// INSTRUCTIONS
// ============================================================================

pub fn handle_initialize_referral_config(
    ctx: Context<InitializeReferralConfig>,
    referral_fee_bps: u16,
) -> Result<()> {
    require!(referral_fee_bps <= 10_000, ReferralError::InvalidFeeBps);
    let config = &mut ctx.accounts.config;
    config.authority = ctx.accounts.authority.key();
    config.referral_fee_bps = referral_fee_bps;
    config.treasury = ctx.accounts.treasury.key();
    config.bump = ctx.bumps.config;
    Ok(())
}

pub fn handle_register_referral(ctx: Context<RegisterReferral>) -> Result<()> {
    let clock = Clock::get()?;
    let acct = &mut ctx.accounts.referral_account;

    acct.user = ctx.accounts.user.key();
    acct.total_referred = 0;
    acct.total_earned = 0;
    acct.claimable = 0;
    acct.registered_at = clock.unix_timestamp;
    acct.bump = ctx.bumps.referral_account;

    if let Some(referrer) = &mut ctx.accounts.referrer_account {
        // Can't refer yourself
        require!(referrer.user != acct.user, ReferralError::SelfReferral);
        acct.referrer = referrer.user;
        referrer.total_referred = referrer.total_referred.checked_add(1).unwrap();

        emit!(ReferralRegistered {
            user: acct.user,
            referrer: referrer.user,
            timestamp: clock.unix_timestamp,
        });
    } else {
        acct.referrer = Pubkey::default();
    }

    Ok(())
}

/// Credit referral reward during a trade. Called via CPI from the trade instruction.
pub fn handle_credit_referral_reward(
    ctx: Context<CreditReferralReward>,
    platform_fee_lamports: u64,
) -> Result<()> {
    let config = &ctx.accounts.config;
    let referrer = &mut ctx.accounts.referrer_referral;

    // Calculate referrer's share of the platform fee
    let referral_reward = platform_fee_lamports
        .checked_mul(config.referral_fee_bps as u64)
        .unwrap()
        .checked_div(10_000)
        .unwrap();

    if referral_reward == 0 {
        return Ok(());
    }

    // Transfer reward lamports to the vault
    system_program::transfer(
        CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            system_program::Transfer {
                from: ctx.accounts.fee_payer.to_account_info(),
                to: ctx.accounts.referral_vault.to_account_info(),
            },
        ),
        referral_reward,
    )?;

    referrer.total_earned = referrer.total_earned.checked_add(referral_reward).unwrap();
    referrer.claimable = referrer.claimable.checked_add(referral_reward).unwrap();

    emit!(ReferralRewardCredited {
        referrer: referrer.user,
        trader: ctx.accounts.trader_referral.user,
        amount: referral_reward,
    });

    Ok(())
}

pub fn handle_claim_referral_rewards(ctx: Context<ClaimReferralRewards>) -> Result<()> {
    let acct = &mut ctx.accounts.referral_account;
    let amount = acct.claimable;
    require!(amount > 0, ReferralError::NothingToClaim);

    acct.claimable = 0;

    // Transfer from vault PDA to user
    let vault_bump = ctx.bumps.referral_vault;
    let seeds: &[&[u8]] = &[REFERRAL_VAULT_SEED, &[vault_bump]];
    let signer_seeds = &[seeds];

    **ctx.accounts.referral_vault.try_borrow_mut_lamports()? -= amount;
    **ctx.accounts.user.try_borrow_mut_lamports()? += amount;

    // Verify we used PDA correctly (the seeds are validated by account constraint)
    let _ = signer_seeds;

    emit!(ReferralRewardsClaimed {
        user: acct.user,
        amount,
    });

    Ok(())
}

// ============================================================================
// EVENTS
// ============================================================================

#[event]
pub struct ReferralRegistered {
    pub user: Pubkey,
    pub referrer: Pubkey,
    pub timestamp: i64,
}

#[event]
pub struct ReferralRewardCredited {
    pub referrer: Pubkey,
    pub trader: Pubkey,
    pub amount: u64,
}

#[event]
pub struct ReferralRewardsClaimed {
    pub user: Pubkey,
    pub amount: u64,
}

// ============================================================================
// ERRORS
// ============================================================================

#[error_code]
pub enum ReferralError {
    #[msg("Cannot refer yourself")]
    SelfReferral,
    #[msg("No referral rewards to claim")]
    NothingToClaim,
    #[msg("Referrer account mismatch")]
    ReferrerMismatch,
    #[msg("Unauthorized")]
    Unauthorized,
    #[msg("Invalid fee basis points (must be <= 10000)")]
    InvalidFeeBps,
}
