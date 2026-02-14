use anchor_lang::prelude::*;

declare_id!("SenditContentC1aims111111111111111111111111");

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const CONTENT_CLAIM_SEED: &[u8] = b"content_claim";
const CLAIM_VERIFY_SEED: &[u8] = b"claim_verify";

const MAX_URL_LEN: usize = 200;

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Default)]
pub enum ClaimStatus {
    #[default]
    Unclaimed,
    Pending,
    Verified,
    Rejected,
}

// ---------------------------------------------------------------------------
// Accounts
// ---------------------------------------------------------------------------

#[account]
#[derive(Default)]
pub struct ContentClaim {
    /// The token mint this claim is for
    pub token_mint: Pubkey,
    /// Original creator who launched the token
    pub original_creator: Pubkey,
    /// Content owner who claimed it (None if unclaimed)
    pub claimed_by: Option<Pubkey>,
    /// Link to original content
    pub content_url: String,
    /// Current claim status
    pub claim_status: ClaimStatus,
    /// Timestamp when claimed
    pub claimed_at: Option<i64>,
    /// Basis points of creator fee redirected to content owner after verification (default 5000 = 50%)
    pub fee_redirect_bps: u16,
    /// Bump seed for PDA
    pub bump: u8,
}

impl ContentClaim {
    pub const SIZE: usize = 8  // discriminator
        + 32  // token_mint
        + 32  // original_creator
        + (1 + 32) // claimed_by Option<Pubkey>
        + (4 + MAX_URL_LEN)  // content_url
        + 1   // claim_status
        + (1 + 8) // claimed_at Option<i64>
        + 2   // fee_redirect_bps
        + 1;  // bump
}

#[account]
#[derive(Default)]
pub struct ClaimVerification {
    /// Token mint
    pub token_mint: Pubkey,
    /// The claimant submitting the claim
    pub claimant: Pubkey,
    /// URL to proof of content ownership
    pub proof_url: String,
    /// Timestamp of submission
    pub submitted_at: i64,
    /// Whether this verification has been resolved
    pub resolved: bool,
    /// Bump seed for PDA
    pub bump: u8,
}

impl ClaimVerification {
    pub const SIZE: usize = 8  // discriminator
        + 32  // token_mint
        + 32  // claimant
        + (4 + MAX_URL_LEN)  // proof_url
        + 8   // submitted_at
        + 1   // resolved
        + 1;  // bump
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

#[event]
pub struct ContentRegistered {
    pub token_mint: Pubkey,
    pub creator: Pubkey,
    pub content_url: String,
}

#[event]
pub struct ClaimSubmitted {
    pub token_mint: Pubkey,
    pub claimant: Pubkey,
    pub proof_url: String,
}

#[event]
pub struct ClaimVerifiedEvent {
    pub token_mint: Pubkey,
    pub claimant: Pubkey,
    pub fee_redirect_bps: u16,
}

#[event]
pub struct ClaimRejected {
    pub token_mint: Pubkey,
    pub claimant: Pubkey,
}

#[event]
pub struct FeesRedirected {
    pub token_mint: Pubkey,
    pub content_owner: Pubkey,
    pub amount: u64,
    pub fee_redirect_bps: u16,
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[error_code]
pub enum ContentClaimError {
    #[msg("Content URL too long (max 200 chars)")]
    UrlTooLong,
    #[msg("Content has already been claimed")]
    AlreadyClaimed,
    #[msg("Invalid claim status transition")]
    InvalidStatusTransition,
    #[msg("Unauthorized — only platform authority can verify/reject")]
    Unauthorized,
    #[msg("Claim is not in Pending status")]
    NotPending,
    #[msg("Claim is not Verified — cannot redirect fees")]
    NotVerified,
    #[msg("Fee redirect bps too high (max 10000)")]
    InvalidRedirectBps,
    #[msg("Proof URL too long (max 200 chars)")]
    ProofUrlTooLong,
    #[msg("Math overflow")]
    MathOverflow,
}

// ---------------------------------------------------------------------------
// Instructions
// ---------------------------------------------------------------------------

#[derive(Accounts)]
pub struct RegisterContent<'info> {
    #[account(
        init,
        payer = creator,
        space = ContentClaim::SIZE,
        seeds = [CONTENT_CLAIM_SEED, token_mint.key().as_ref()],
        bump,
    )]
    pub content_claim: Account<'info, ContentClaim>,

    /// CHECK: Token mint
    pub token_mint: AccountInfo<'info>,

    #[account(mut)]
    pub creator: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct SubmitClaim<'info> {
    #[account(
        mut,
        seeds = [CONTENT_CLAIM_SEED, token_mint.key().as_ref()],
        bump = content_claim.bump,
    )]
    pub content_claim: Account<'info, ContentClaim>,

    #[account(
        init,
        payer = claimant,
        space = ClaimVerification::SIZE,
        seeds = [CLAIM_VERIFY_SEED, token_mint.key().as_ref(), claimant.key().as_ref()],
        bump,
    )]
    pub claim_verification: Account<'info, ClaimVerification>,

    /// CHECK: Token mint
    pub token_mint: AccountInfo<'info>,

    #[account(mut)]
    pub claimant: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct VerifyClaim<'info> {
    #[account(
        mut,
        seeds = [CONTENT_CLAIM_SEED, token_mint.key().as_ref()],
        bump = content_claim.bump,
    )]
    pub content_claim: Account<'info, ContentClaim>,

    #[account(
        mut,
        seeds = [CLAIM_VERIFY_SEED, token_mint.key().as_ref(), claimant.key().as_ref()],
        bump = claim_verification.bump,
    )]
    pub claim_verification: Account<'info, ClaimVerification>,

    /// CHECK: Token mint
    pub token_mint: AccountInfo<'info>,

    /// CHECK: The claimant whose claim is being verified
    pub claimant: AccountInfo<'info>,

    /// Platform authority that can verify claims
    pub authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct RejectClaim<'info> {
    #[account(
        mut,
        seeds = [CONTENT_CLAIM_SEED, token_mint.key().as_ref()],
        bump = content_claim.bump,
    )]
    pub content_claim: Account<'info, ContentClaim>,

    #[account(
        mut,
        seeds = [CLAIM_VERIFY_SEED, token_mint.key().as_ref(), claimant.key().as_ref()],
        bump = claim_verification.bump,
    )]
    pub claim_verification: Account<'info, ClaimVerification>,

    /// CHECK: Token mint
    pub token_mint: AccountInfo<'info>,

    /// CHECK: The claimant whose claim is being rejected
    pub claimant: AccountInfo<'info>,

    /// Platform authority
    pub authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct RedirectFees<'info> {
    #[account(
        seeds = [CONTENT_CLAIM_SEED, token_mint.key().as_ref()],
        bump = content_claim.bump,
    )]
    pub content_claim: Account<'info, ContentClaim>,

    /// CHECK: Token mint
    pub token_mint: AccountInfo<'info>,

    /// CHECK: Content owner receiving redirected fees
    #[account(
        mut,
        constraint = Some(content_owner.key()) == content_claim.claimed_by @ ContentClaimError::NotVerified,
    )]
    pub content_owner: AccountInfo<'info>,

    /// CHECK: Creator fee source (vault or creator wallet)
    #[account(mut)]
    pub fee_source: AccountInfo<'info>,

    /// Permissionless crank
    #[account(mut)]
    pub payer: Signer<'info>,

    pub system_program: Program<'info, System>,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

pub fn handle_register_content(
    ctx: Context<RegisterContent>,
    content_url: String,
    fee_redirect_bps: u16,
) -> Result<()> {
    require!(content_url.len() <= MAX_URL_LEN, ContentClaimError::UrlTooLong);
    require!(fee_redirect_bps <= 10_000, ContentClaimError::InvalidRedirectBps);

    let claim = &mut ctx.accounts.content_claim;
    claim.token_mint = ctx.accounts.token_mint.key();
    claim.original_creator = ctx.accounts.creator.key();
    claim.claimed_by = None;
    claim.content_url = content_url.clone();
    claim.claim_status = ClaimStatus::Unclaimed;
    claim.claimed_at = None;
    claim.fee_redirect_bps = if fee_redirect_bps == 0 { 5000 } else { fee_redirect_bps };
    claim.bump = ctx.bumps.content_claim;

    emit!(ContentRegistered {
        token_mint: ctx.accounts.token_mint.key(),
        creator: ctx.accounts.creator.key(),
        content_url,
    });

    Ok(())
}

pub fn handle_submit_claim(
    ctx: Context<SubmitClaim>,
    proof_url: String,
) -> Result<()> {
    require!(proof_url.len() <= MAX_URL_LEN, ContentClaimError::ProofUrlTooLong);

    let content_claim = &ctx.accounts.content_claim;
    require!(
        content_claim.claim_status == ClaimStatus::Unclaimed,
        ContentClaimError::AlreadyClaimed
    );

    // Update content claim to Pending
    let content_claim = &mut ctx.accounts.content_claim;
    content_claim.claim_status = ClaimStatus::Pending;
    content_claim.claimed_by = Some(ctx.accounts.claimant.key());

    let clock = Clock::get()?;

    // Initialize verification record
    let verification = &mut ctx.accounts.claim_verification;
    verification.token_mint = ctx.accounts.token_mint.key();
    verification.claimant = ctx.accounts.claimant.key();
    verification.proof_url = proof_url.clone();
    verification.submitted_at = clock.unix_timestamp;
    verification.resolved = false;
    verification.bump = ctx.bumps.claim_verification;

    emit!(ClaimSubmitted {
        token_mint: ctx.accounts.token_mint.key(),
        claimant: ctx.accounts.claimant.key(),
        proof_url,
    });

    Ok(())
}

pub fn handle_verify_claim(ctx: Context<VerifyClaim>) -> Result<()> {
    let content_claim = &ctx.accounts.content_claim;
    require!(
        content_claim.claim_status == ClaimStatus::Pending,
        ContentClaimError::NotPending
    );

    let clock = Clock::get()?;

    let content_claim = &mut ctx.accounts.content_claim;
    content_claim.claim_status = ClaimStatus::Verified;
    content_claim.claimed_at = Some(clock.unix_timestamp);

    let verification = &mut ctx.accounts.claim_verification;
    verification.resolved = true;

    emit!(ClaimVerifiedEvent {
        token_mint: ctx.accounts.token_mint.key(),
        claimant: ctx.accounts.claimant.key(),
        fee_redirect_bps: content_claim.fee_redirect_bps,
    });

    Ok(())
}

pub fn handle_reject_claim(ctx: Context<RejectClaim>) -> Result<()> {
    let content_claim = &ctx.accounts.content_claim;
    require!(
        content_claim.claim_status == ClaimStatus::Pending,
        ContentClaimError::NotPending
    );

    let content_claim = &mut ctx.accounts.content_claim;
    content_claim.claim_status = ClaimStatus::Rejected;
    content_claim.claimed_by = None;

    let verification = &mut ctx.accounts.claim_verification;
    verification.resolved = true;

    emit!(ClaimRejected {
        token_mint: ctx.accounts.token_mint.key(),
        claimant: ctx.accounts.claimant.key(),
    });

    Ok(())
}

pub fn handle_redirect_fees(
    ctx: Context<RedirectFees>,
    amount: u64,
) -> Result<()> {
    let content_claim = &ctx.accounts.content_claim;
    require!(
        content_claim.claim_status == ClaimStatus::Verified,
        ContentClaimError::NotVerified
    );

    // Calculate redirect amount
    let redirect_amount = (amount as u128)
        .checked_mul(content_claim.fee_redirect_bps as u128)
        .ok_or(ContentClaimError::MathOverflow)?
        .checked_div(10_000u128)
        .ok_or(ContentClaimError::MathOverflow)? as u64;

    if redirect_amount > 0 {
        // Transfer from fee source to content owner
        **ctx.accounts.fee_source.try_borrow_mut_lamports()? -= redirect_amount;
        **ctx.accounts.content_owner.try_borrow_mut_lamports()? += redirect_amount;
    }

    emit!(FeesRedirected {
        token_mint: ctx.accounts.token_mint.key(),
        content_owner: ctx.accounts.content_owner.key(),
        amount: redirect_amount,
        fee_redirect_bps: content_claim.fee_redirect_bps,
    });

    Ok(())
}
