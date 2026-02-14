use anchor_lang::prelude::*;
use anchor_lang::system_program;

declare_id!("SenditFeeSp1itting11111111111111111111111111");

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const FEE_CONFIG_SEED: &[u8] = b"fee_config";
const USER_FEE_CLAIM_SEED: &[u8] = b"user_fee_claim";
const FEE_VAULT_SEED: &[u8] = b"fee_vault";

const MAX_SPLITS: usize = 5;
const MAX_SOCIAL_HANDLE_LEN: usize = 64;
const BPS_TOTAL: u16 = 10_000;

// ---------------------------------------------------------------------------
// Accounts
// ---------------------------------------------------------------------------

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Default)]
pub struct FeeSplit {
    /// Recipient wallet
    pub recipient: Pubkey,
    /// Share in basis points (out of 10000)
    pub share_bps: u16,
    /// Optional social handle for display purposes
    pub social_handle: String,
}

impl FeeSplit {
    pub const SIZE: usize = 32 + 2 + (4 + MAX_SOCIAL_HANDLE_LEN);
}

#[account]
#[derive(Default)]
pub struct FeeConfig {
    /// The token mint this fee config is for
    pub token_mint: Pubkey,
    /// Original creator who set up the config
    pub creator: Pubkey,
    /// Fee split recipients (up to 5)
    pub splits: Vec<FeeSplit>,
    /// Total fees distributed through this config (lamports)
    pub total_distributed: u64,
    /// Whether distribution has ever occurred (locks splits if true, unless creator allows updates)
    pub has_distributed: bool,
    /// Whether creator allows updating splits after first distribution
    pub allow_update_after_distribution: bool,
    /// Bump seed for PDA
    pub bump: u8,
}

impl FeeConfig {
    pub const SIZE: usize = 8 // discriminator
        + 32  // token_mint
        + 32  // creator
        + (4 + MAX_SPLITS * FeeSplit::SIZE) // splits vec
        + 8   // total_distributed
        + 1   // has_distributed
        + 1   // allow_update_after_distribution
        + 1;  // bump
}

#[account]
#[derive(Default)]
pub struct UserFeeClaim {
    /// Recipient wallet
    pub recipient: Pubkey,
    /// Token mint
    pub token_mint: Pubkey,
    /// Total amount allocated to this recipient (lamports)
    pub total_allocated: u64,
    /// Total amount claimed by this recipient (lamports)
    pub total_claimed: u64,
    /// Bump seed for PDA
    pub bump: u8,
}

impl UserFeeClaim {
    pub const SIZE: usize = 8 // discriminator
        + 32  // recipient
        + 32  // token_mint
        + 8   // total_allocated
        + 8   // total_claimed
        + 1;  // bump
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

#[event]
pub struct FeeConfigCreated {
    pub token_mint: Pubkey,
    pub creator: Pubkey,
    pub num_splits: u8,
}

#[event]
pub struct FeeSplitsUpdated {
    pub token_mint: Pubkey,
    pub creator: Pubkey,
    pub num_splits: u8,
}

#[event]
pub struct FeesDistributed {
    pub token_mint: Pubkey,
    pub total_amount: u64,
    pub num_recipients: u8,
}

#[event]
pub struct SplitFeesClaimed {
    pub token_mint: Pubkey,
    pub recipient: Pubkey,
    pub amount: u64,
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[error_code]
pub enum FeeSplittingError {
    #[msg("Too many fee splits (max 5)")]
    TooManySplits,
    #[msg("Total share basis points must equal 10000")]
    InvalidShareTotal,
    #[msg("Social handle too long (max 64 chars)")]
    SocialHandleTooLong,
    #[msg("Fee splits cannot be updated after distribution")]
    SplitsLocked,
    #[msg("Nothing to claim")]
    NothingToClaim,
    #[msg("Nothing to distribute")]
    NothingToDistribute,
    #[msg("Unauthorized — only creator can perform this action")]
    Unauthorized,
    #[msg("Recipient not found in fee splits")]
    RecipientNotFound,
    #[msg("Math overflow")]
    MathOverflow,
}

// ---------------------------------------------------------------------------
// Instructions
// ---------------------------------------------------------------------------

#[derive(Accounts)]
pub struct InitializeFeeConfig<'info> {
    #[account(
        init,
        payer = creator,
        space = FeeConfig::SIZE,
        seeds = [FEE_CONFIG_SEED, token_mint.key().as_ref()],
        bump,
    )]
    pub fee_config: Account<'info, FeeConfig>,

    /// CHECK: Token mint for which fees are configured
    pub token_mint: AccountInfo<'info>,

    #[account(mut)]
    pub creator: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UpdateFeeSplits<'info> {
    #[account(
        mut,
        seeds = [FEE_CONFIG_SEED, token_mint.key().as_ref()],
        bump = fee_config.bump,
        has_one = creator @ FeeSplittingError::Unauthorized,
    )]
    pub fee_config: Account<'info, FeeConfig>,

    /// CHECK: Token mint
    pub token_mint: AccountInfo<'info>,

    pub creator: Signer<'info>,
}

#[derive(Accounts)]
pub struct DistributeFees<'info> {
    #[account(
        mut,
        seeds = [FEE_CONFIG_SEED, token_mint.key().as_ref()],
        bump = fee_config.bump,
    )]
    pub fee_config: Account<'info, FeeConfig>,

    /// CHECK: Token mint
    pub token_mint: AccountInfo<'info>,

    /// CHECK: Fee vault PDA holding accumulated creator fees
    #[account(
        mut,
        seeds = [FEE_VAULT_SEED, token_mint.key().as_ref()],
        bump,
    )]
    pub fee_vault: AccountInfo<'info>,

    /// CHECK: Permissionless crank caller
    #[account(mut)]
    pub payer: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ClaimSplitFees<'info> {
    #[account(
        seeds = [FEE_CONFIG_SEED, token_mint.key().as_ref()],
        bump = fee_config.bump,
    )]
    pub fee_config: Account<'info, FeeConfig>,

    #[account(
        mut,
        seeds = [USER_FEE_CLAIM_SEED, token_mint.key().as_ref(), recipient.key().as_ref()],
        bump = user_fee_claim.bump,
        has_one = recipient,
    )]
    pub user_fee_claim: Account<'info, UserFeeClaim>,

    /// CHECK: Token mint
    pub token_mint: AccountInfo<'info>,

    /// CHECK: Fee vault PDA
    #[account(
        mut,
        seeds = [FEE_VAULT_SEED, token_mint.key().as_ref()],
        bump,
    )]
    pub fee_vault: AccountInfo<'info>,

    #[account(mut)]
    pub recipient: Signer<'info>,

    pub system_program: Program<'info, System>,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

pub fn handle_initialize_fee_config(
    ctx: Context<InitializeFeeConfig>,
    splits: Vec<FeeSplit>,
    allow_update_after_distribution: bool,
) -> Result<()> {
    require!(splits.len() <= MAX_SPLITS, FeeSplittingError::TooManySplits);

    // Validate social handles
    for split in &splits {
        require!(
            split.social_handle.len() <= MAX_SOCIAL_HANDLE_LEN,
            FeeSplittingError::SocialHandleTooLong
        );
    }

    // Validate total bps = 10000
    let total_bps: u16 = splits.iter().map(|s| s.share_bps).sum();
    if splits.is_empty() {
        // Default: 100% to creator — we'll store a single split
        let config = &mut ctx.accounts.fee_config;
        config.token_mint = ctx.accounts.token_mint.key();
        config.creator = ctx.accounts.creator.key();
        config.splits = vec![FeeSplit {
            recipient: ctx.accounts.creator.key(),
            share_bps: BPS_TOTAL,
            social_handle: String::new(),
        }];
        config.total_distributed = 0;
        config.has_distributed = false;
        config.allow_update_after_distribution = allow_update_after_distribution;
        config.bump = ctx.bumps.fee_config;
    } else {
        require!(total_bps == BPS_TOTAL, FeeSplittingError::InvalidShareTotal);

        let config = &mut ctx.accounts.fee_config;
        config.token_mint = ctx.accounts.token_mint.key();
        config.creator = ctx.accounts.creator.key();
        config.splits = splits;
        config.total_distributed = 0;
        config.has_distributed = false;
        config.allow_update_after_distribution = allow_update_after_distribution;
        config.bump = ctx.bumps.fee_config;
    }

    emit!(FeeConfigCreated {
        token_mint: ctx.accounts.token_mint.key(),
        creator: ctx.accounts.creator.key(),
        num_splits: ctx.accounts.fee_config.splits.len() as u8,
    });

    Ok(())
}

pub fn handle_update_fee_splits(
    ctx: Context<UpdateFeeSplits>,
    new_splits: Vec<FeeSplit>,
) -> Result<()> {
    let config = &ctx.accounts.fee_config;

    // Check if updates are allowed
    if config.has_distributed && !config.allow_update_after_distribution {
        return Err(FeeSplittingError::SplitsLocked.into());
    }

    require!(new_splits.len() <= MAX_SPLITS, FeeSplittingError::TooManySplits);

    for split in &new_splits {
        require!(
            split.social_handle.len() <= MAX_SOCIAL_HANDLE_LEN,
            FeeSplittingError::SocialHandleTooLong
        );
    }

    let total_bps: u16 = new_splits.iter().map(|s| s.share_bps).sum();
    require!(total_bps == BPS_TOTAL, FeeSplittingError::InvalidShareTotal);

    let config = &mut ctx.accounts.fee_config;
    config.splits = new_splits;

    emit!(FeeSplitsUpdated {
        token_mint: ctx.accounts.token_mint.key(),
        creator: ctx.accounts.creator.key(),
        num_splits: config.splits.len() as u8,
    });

    Ok(())
}

pub fn handle_distribute_fees(ctx: Context<DistributeFees>) -> Result<()> {
    let fee_vault = &ctx.accounts.fee_vault;
    let rent = Rent::get()?;
    let min_balance = rent.minimum_balance(0);
    let available = fee_vault
        .lamports()
        .checked_sub(min_balance)
        .unwrap_or(0);

    require!(available > 0, FeeSplittingError::NothingToDistribute);

    let config = &ctx.accounts.fee_config;
    let splits = &config.splits;

    // Distribute proportionally to each split recipient via remaining_accounts
    // Each remaining account corresponds to a UserFeeClaim PDA in order
    let remaining = &ctx.remaining_accounts;
    require!(
        remaining.len() == splits.len(),
        FeeSplittingError::RecipientNotFound
    );

    for (i, split) in splits.iter().enumerate() {
        let share_amount = (available as u128)
            .checked_mul(split.share_bps as u128)
            .ok_or(FeeSplittingError::MathOverflow)?
            .checked_div(BPS_TOTAL as u128)
            .ok_or(FeeSplittingError::MathOverflow)? as u64;

        if share_amount > 0 {
            // Update the UserFeeClaim account
            let claim_info = &remaining[i];
            let mut claim_data = claim_info.try_borrow_mut_data()?;
            // Skip 8-byte discriminator, deserialize
            let mut claim: UserFeeClaim =
                UserFeeClaim::try_deserialize(&mut &claim_data[..])?;
            claim.total_allocated = claim
                .total_allocated
                .checked_add(share_amount)
                .ok_or(FeeSplittingError::MathOverflow)?;
            // Serialize back
            let mut writer = &mut claim_data[8..];
            claim.try_serialize(&mut writer)?;
        }
    }

    // Transfer total from vault
    let mint_key = ctx.accounts.token_mint.key();
    let vault_seeds: &[&[u8]] = &[
        FEE_VAULT_SEED,
        mint_key.as_ref(),
        &[ctx.bumps.fee_vault],
    ];

    **ctx.accounts.fee_vault.try_borrow_mut_lamports()? -= available;
    // Distribute lamports to fee_config PDA temporarily (claims pull from vault)
    // Actually we keep in vault and let claims pull — just update allocations above

    // Re-add lamports since we only updated allocations
    **ctx.accounts.fee_vault.try_borrow_mut_lamports()? += available;

    let config = &mut ctx.accounts.fee_config;
    config.total_distributed = config
        .total_distributed
        .checked_add(available)
        .ok_or(FeeSplittingError::MathOverflow)?;
    config.has_distributed = true;

    emit!(FeesDistributed {
        token_mint: ctx.accounts.token_mint.key(),
        total_amount: available,
        num_recipients: splits.len() as u8,
    });

    Ok(())
}

pub fn handle_claim_split_fees(ctx: Context<ClaimSplitFees>) -> Result<()> {
    let claim = &ctx.accounts.user_fee_claim;
    let claimable = claim
        .total_allocated
        .checked_sub(claim.total_claimed)
        .unwrap_or(0);

    require!(claimable > 0, FeeSplittingError::NothingToClaim);

    // Transfer from fee vault to recipient
    let mint_key = ctx.accounts.token_mint.key();
    let vault_seeds: &[&[u8]] = &[
        FEE_VAULT_SEED,
        mint_key.as_ref(),
        &[ctx.bumps.fee_vault],
    ];

    **ctx.accounts.fee_vault.try_borrow_mut_lamports()? -= claimable;
    **ctx.accounts.recipient.to_account_info().try_borrow_mut_lamports()? += claimable;

    let claim = &mut ctx.accounts.user_fee_claim;
    claim.total_claimed = claim
        .total_claimed
        .checked_add(claimable)
        .ok_or(FeeSplittingError::MathOverflow)?;

    emit!(SplitFeesClaimed {
        token_mint: ctx.accounts.token_mint.key(),
        recipient: ctx.accounts.recipient.key(),
        amount: claimable,
    });

    Ok(())
}
