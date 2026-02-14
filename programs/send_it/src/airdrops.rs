use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer, Mint};

// ── Seeds ──────────────────────────────────────────────────────────────────────
pub const AIRDROP_CAMPAIGN_SEED: &[u8] = b"airdrop_campaign";
pub const AIRDROP_CLAIM_SEED: &[u8] = b"airdrop_claim";
pub const AIRDROP_VAULT_SEED: &[u8] = b"airdrop_vault";

// ── Errors ─────────────────────────────────────────────────────────────────────
#[error_code]
pub enum AirdropError {
    #[msg("Invalid merkle proof")]
    InvalidProof,
    #[msg("Airdrop already claimed")]
    AlreadyClaimed,
    #[msg("All airdrop slots claimed")]
    MaxRecipientsClaimed,
    #[msg("Airdrop campaign is not active")]
    CampaignNotActive,
    #[msg("Cannot cancel before deadline")]
    CancelTooEarly,
    #[msg("Unauthorized")]
    Unauthorized,
    #[msg("Deadline must be in the future")]
    InvalidDeadline,
}

// ── Account Structs ────────────────────────────────────────────────────────────
#[account]
pub struct AirdropCampaign {
    pub campaign_id: u64,
    pub token_mint: Pubkey,
    pub creator: Pubkey,
    pub vault: Pubkey,
    pub total_amount: u64,
    pub claimed_count: u64,
    pub max_recipients: u64,
    pub snapshot_slot: u64,
    pub merkle_root: [u8; 32],
    pub deadline: i64,           // unix timestamp after which creator can cancel
    pub is_active: bool,
    pub bump: u8,
    pub vault_bump: u8,
}

impl AirdropCampaign {
    pub const SIZE: usize = 8 + 8 + 32 + 32 + 32 + 8 + 8 + 8 + 8 + 32 + 8 + 1 + 1 + 1; // 187
}

/// Receipt PDA proving a user already claimed.
#[account]
pub struct AirdropClaim {
    pub campaign: Pubkey,
    pub claimant: Pubkey,
    pub amount: u64,
    pub claimed_at: i64,
    pub bump: u8,
}

impl AirdropClaim {
    pub const SIZE: usize = 8 + 32 + 32 + 8 + 8 + 1; // 89
}

// ── Events ─────────────────────────────────────────────────────────────────────
#[event]
pub struct AirdropCreated {
    pub campaign: Pubkey,
    pub token_mint: Pubkey,
    pub creator: Pubkey,
    pub total_amount: u64,
    pub max_recipients: u64,
}

#[event]
pub struct AirdropClaimed {
    pub campaign: Pubkey,
    pub claimant: Pubkey,
    pub amount: u64,
}

#[event]
pub struct AirdropCancelled {
    pub campaign: Pubkey,
    pub reclaimed_amount: u64,
}

// ── Instructions ───────────────────────────────────────────────────────────────

pub fn create_airdrop(
    ctx: Context<CreateAirdrop>,
    campaign_id: u64,
    total_amount: u64,
    max_recipients: u64,
    snapshot_slot: u64,
    merkle_root: [u8; 32],
    deadline: i64,
) -> Result<()> {
    let clock = Clock::get()?;
    require!(deadline > clock.unix_timestamp, AirdropError::InvalidDeadline);

    // Transfer tokens from creator to vault
    token::transfer(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.creator_token_account.to_account_info(),
                to: ctx.accounts.vault.to_account_info(),
                authority: ctx.accounts.creator.to_account_info(),
            },
        ),
        total_amount,
    )?;

    let campaign = &mut ctx.accounts.campaign;
    campaign.campaign_id = campaign_id;
    campaign.token_mint = ctx.accounts.token_mint.key();
    campaign.creator = ctx.accounts.creator.key();
    campaign.vault = ctx.accounts.vault.key();
    campaign.total_amount = total_amount;
    campaign.claimed_count = 0;
    campaign.max_recipients = max_recipients;
    campaign.snapshot_slot = snapshot_slot;
    campaign.merkle_root = merkle_root;
    campaign.deadline = deadline;
    campaign.is_active = true;
    campaign.bump = ctx.bumps.campaign;
    campaign.vault_bump = ctx.bumps.vault;

    emit!(AirdropCreated {
        campaign: campaign.key(),
        token_mint: campaign.token_mint,
        creator: campaign.creator,
        total_amount,
        max_recipients,
    });

    Ok(())
}

pub fn claim_airdrop(
    ctx: Context<ClaimAirdrop>,
    amount: u64,
    proof: Vec<[u8; 32]>,
) -> Result<()> {
    let campaign = &ctx.accounts.campaign;
    require!(campaign.is_active, AirdropError::CampaignNotActive);
    require!(
        campaign.claimed_count < campaign.max_recipients,
        AirdropError::MaxRecipientsClaimed
    );

    // Verify merkle proof
    let leaf = anchor_lang::solana_program::keccak::hashv(&[
        &ctx.accounts.claimant.key().to_bytes(),
        &amount.to_le_bytes(),
    ]);
    let mut computed = leaf.0;
    for node in proof.iter() {
        if computed <= *node {
            computed = anchor_lang::solana_program::keccak::hashv(&[&computed, node]).0;
        } else {
            computed = anchor_lang::solana_program::keccak::hashv(&[node, &computed]).0;
        }
    }
    require!(computed == campaign.merkle_root, AirdropError::InvalidProof);

    // Transfer from vault to claimant
    let campaign_key = campaign.campaign_id.to_le_bytes();
    let seeds = &[
        AIRDROP_VAULT_SEED,
        campaign_key.as_ref(),
        &[campaign.vault_bump],
    ];
    token::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.vault.to_account_info(),
                to: ctx.accounts.claimant_token_account.to_account_info(),
                authority: ctx.accounts.vault.to_account_info(),
            },
            &[seeds],
        ),
        amount,
    )?;

    // Update campaign
    let campaign = &mut ctx.accounts.campaign;
    campaign.claimed_count = campaign.claimed_count.checked_add(1).unwrap();

    // Write claim receipt
    let claim = &mut ctx.accounts.claim_receipt;
    claim.campaign = campaign.key();
    claim.claimant = ctx.accounts.claimant.key();
    claim.amount = amount;
    claim.claimed_at = Clock::get()?.unix_timestamp;
    claim.bump = ctx.bumps.claim_receipt;

    emit!(AirdropClaimed {
        campaign: campaign.key(),
        claimant: ctx.accounts.claimant.key(),
        amount,
    });

    Ok(())
}

pub fn cancel_airdrop(ctx: Context<CancelAirdrop>) -> Result<()> {
    let campaign = &ctx.accounts.campaign;
    require!(campaign.is_active, AirdropError::CampaignNotActive);
    require!(
        ctx.accounts.creator.key() == campaign.creator,
        AirdropError::Unauthorized
    );
    let clock = Clock::get()?;
    require!(
        clock.unix_timestamp >= campaign.deadline,
        AirdropError::CancelTooEarly
    );

    let remaining = ctx.accounts.vault.amount;

    let campaign_key = campaign.campaign_id.to_le_bytes();
    let seeds = &[
        AIRDROP_VAULT_SEED,
        campaign_key.as_ref(),
        &[campaign.vault_bump],
    ];
    token::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.vault.to_account_info(),
                to: ctx.accounts.creator_token_account.to_account_info(),
                authority: ctx.accounts.vault.to_account_info(),
            },
            &[seeds],
        ),
        remaining,
    )?;

    let campaign = &mut ctx.accounts.campaign;
    campaign.is_active = false;

    emit!(AirdropCancelled {
        campaign: campaign.key(),
        reclaimed_amount: remaining,
    });

    Ok(())
}

// ── Contexts ───────────────────────────────────────────────────────────────────

#[derive(Accounts)]
#[instruction(campaign_id: u64)]
pub struct CreateAirdrop<'info> {
    #[account(
        init,
        payer = creator,
        space = AirdropCampaign::SIZE,
        seeds = [AIRDROP_CAMPAIGN_SEED, creator.key().as_ref(), &campaign_id.to_le_bytes()],
        bump,
    )]
    pub campaign: Account<'info, AirdropCampaign>,

    #[account(
        init,
        payer = creator,
        token::mint = token_mint,
        token::authority = vault,
        seeds = [AIRDROP_VAULT_SEED, &campaign_id.to_le_bytes()],
        bump,
    )]
    pub vault: Account<'info, TokenAccount>,

    pub token_mint: Account<'info, Mint>,

    #[account(mut, token::mint = token_mint, token::authority = creator)]
    pub creator_token_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub creator: Signer<'info>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct ClaimAirdrop<'info> {
    #[account(
        mut,
        seeds = [AIRDROP_CAMPAIGN_SEED, campaign.creator.as_ref(), &campaign.campaign_id.to_le_bytes()],
        bump = campaign.bump,
    )]
    pub campaign: Account<'info, AirdropCampaign>,

    #[account(
        init,
        payer = claimant,
        space = AirdropClaim::SIZE,
        seeds = [AIRDROP_CLAIM_SEED, campaign.key().as_ref(), claimant.key().as_ref()],
        bump,
    )]
    pub claim_receipt: Account<'info, AirdropClaim>,

    #[account(
        mut,
        seeds = [AIRDROP_VAULT_SEED, &campaign.campaign_id.to_le_bytes()],
        bump = campaign.vault_bump,
    )]
    pub vault: Account<'info, TokenAccount>,

    #[account(mut, token::mint = campaign.token_mint)]
    pub claimant_token_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub claimant: Signer<'info>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct CancelAirdrop<'info> {
    #[account(
        mut,
        seeds = [AIRDROP_CAMPAIGN_SEED, campaign.creator.as_ref(), &campaign.campaign_id.to_le_bytes()],
        bump = campaign.bump,
    )]
    pub campaign: Account<'info, AirdropCampaign>,

    #[account(
        mut,
        seeds = [AIRDROP_VAULT_SEED, &campaign.campaign_id.to_le_bytes()],
        bump = campaign.vault_bump,
    )]
    pub vault: Account<'info, TokenAccount>,

    #[account(mut, token::mint = campaign.token_mint, token::authority = creator)]
    pub creator_token_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub creator: Signer<'info>,

    pub token_program: Program<'info, Token>,
}
