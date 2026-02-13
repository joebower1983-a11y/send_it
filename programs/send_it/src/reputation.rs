use anchor_lang::prelude::*;

// ═══════════════════════════════════════════════
//  FairScale Reputation Module for Send.it
//  On-chain reputation gating via FairScore oracle
// ═══════════════════════════════════════════════

declare_id!("REPuTaT1oNxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx");

#[account]
#[derive(Default)]
pub struct ReputationConfig {
    pub authority: Pubkey,
    pub oracle_authority: Pubkey,
    pub min_score_to_launch: u8,           // default 30
    pub min_score_premium_launch: u8,      // default 60
    pub strict_vesting_threshold: u8,      // below this → 2x vesting
    pub fee_discount_bronze_bps: u16,      // 0 (0%)
    pub fee_discount_silver_bps: u16,      // 500 (5%)
    pub fee_discount_gold_bps: u16,        // 1000 (10%)
    pub fee_discount_platinum_bps: u16,    // 2000 (20%)
    pub bump: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum ReputationTier {
    Unscored,
    Bronze,
    Silver,
    Gold,
    Platinum,
}

impl Default for ReputationTier {
    fn default() -> Self { ReputationTier::Unscored }
}

#[account]
#[derive(Default)]
pub struct ReputationAttestation {
    pub wallet: Pubkey,
    pub fairscore: u8,               // 0-100
    pub tier: ReputationTier,
    pub last_updated: i64,           // unix timestamp
    pub attested_by: Pubkey,         // oracle authority that submitted
    pub bump: u8,
}

// ─── Contexts ───

#[derive(Accounts)]
pub struct InitializeReputationConfig<'info> {
    #[account(
        init,
        payer = authority,
        space = 8 + 32 + 32 + 1 + 1 + 1 + 2 + 2 + 2 + 2 + 1,
        seeds = [b"reputation_config"],
        bump,
    )]
    pub config: Account<'info, ReputationConfig>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UpdateReputationConfig<'info> {
    #[account(
        mut,
        seeds = [b"reputation_config"],
        bump = config.bump,
        has_one = authority,
    )]
    pub config: Account<'info, ReputationConfig>,
    pub authority: Signer<'info>,
}

#[derive(Accounts)]
#[instruction(wallet: Pubkey)]
pub struct UpdateReputation<'info> {
    #[account(
        init_if_needed,
        payer = oracle_authority,
        space = 8 + 32 + 1 + 1 + 8 + 32 + 1,
        seeds = [b"reputation", wallet.as_ref()],
        bump,
    )]
    pub attestation: Account<'info, ReputationAttestation>,
    #[account(
        seeds = [b"reputation_config"],
        bump = config.bump,
        constraint = config.oracle_authority == oracle_authority.key(),
    )]
    pub config: Account<'info, ReputationConfig>,
    #[account(mut)]
    pub oracle_authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct CheckLaunchEligibility<'info> {
    #[account(
        seeds = [b"reputation", attestation.wallet.as_ref()],
        bump = attestation.bump,
    )]
    pub attestation: Account<'info, ReputationAttestation>,
    #[account(
        seeds = [b"reputation_config"],
        bump = config.bump,
    )]
    pub config: Account<'info, ReputationConfig>,
}

#[derive(Accounts)]
pub struct GetFeeDiscount<'info> {
    #[account(
        seeds = [b"reputation", attestation.wallet.as_ref()],
        bump = attestation.bump,
    )]
    pub attestation: Account<'info, ReputationAttestation>,
    #[account(
        seeds = [b"reputation_config"],
        bump = config.bump,
    )]
    pub config: Account<'info, ReputationConfig>,
}

// ─── Instructions ───

pub fn initialize_reputation_config(
    ctx: Context<InitializeReputationConfig>,
    oracle_authority: Pubkey,
) -> Result<()> {
    let config = &mut ctx.accounts.config;
    config.authority = ctx.accounts.authority.key();
    config.oracle_authority = oracle_authority;
    config.min_score_to_launch = 30;
    config.min_score_premium_launch = 60;
    config.strict_vesting_threshold = 40;
    config.fee_discount_bronze_bps = 0;
    config.fee_discount_silver_bps = 500;
    config.fee_discount_gold_bps = 1000;
    config.fee_discount_platinum_bps = 2000;
    config.bump = ctx.bumps.config;
    Ok(())
}

pub fn update_reputation_config(
    ctx: Context<UpdateReputationConfig>,
    min_score_to_launch: Option<u8>,
    min_score_premium_launch: Option<u8>,
    strict_vesting_threshold: Option<u8>,
    fee_discount_bronze_bps: Option<u16>,
    fee_discount_silver_bps: Option<u16>,
    fee_discount_gold_bps: Option<u16>,
    fee_discount_platinum_bps: Option<u16>,
    oracle_authority: Option<Pubkey>,
) -> Result<()> {
    let config = &mut ctx.accounts.config;
    if let Some(v) = min_score_to_launch { config.min_score_to_launch = v; }
    if let Some(v) = min_score_premium_launch { config.min_score_premium_launch = v; }
    if let Some(v) = strict_vesting_threshold { config.strict_vesting_threshold = v; }
    if let Some(v) = fee_discount_bronze_bps { config.fee_discount_bronze_bps = v; }
    if let Some(v) = fee_discount_silver_bps { config.fee_discount_silver_bps = v; }
    if let Some(v) = fee_discount_gold_bps { config.fee_discount_gold_bps = v; }
    if let Some(v) = fee_discount_platinum_bps { config.fee_discount_platinum_bps = v; }
    if let Some(v) = oracle_authority { config.oracle_authority = v; }
    Ok(())
}

pub fn update_reputation(
    ctx: Context<UpdateReputation>,
    wallet: Pubkey,
    fairscore: u8,
    tier: ReputationTier,
) -> Result<()> {
    require!(fairscore <= 100, ReputationError::InvalidScore);

    let attestation = &mut ctx.accounts.attestation;
    attestation.wallet = wallet;
    attestation.fairscore = fairscore;
    attestation.tier = tier;
    attestation.last_updated = Clock::get()?.unix_timestamp;
    attestation.attested_by = ctx.accounts.oracle_authority.key();
    attestation.bump = ctx.bumps.attestation;

    msg!("Reputation updated: wallet={}, score={}, tier={:?}", wallet, fairscore, tier);
    Ok(())
}

pub fn check_launch_eligibility(
    ctx: Context<CheckLaunchEligibility>,
    premium: bool,
) -> Result<bool> {
    let attestation = &ctx.accounts.attestation;
    let config = &ctx.accounts.config;

    let min_score = if premium {
        config.min_score_premium_launch
    } else {
        config.min_score_to_launch
    };

    let eligible = attestation.fairscore >= min_score;
    msg!(
        "Launch eligibility: score={}, min={}, eligible={}",
        attestation.fairscore, min_score, eligible
    );
    Ok(eligible)
}

pub fn get_fee_discount(ctx: Context<GetFeeDiscount>) -> Result<u16> {
    let attestation = &ctx.accounts.attestation;
    let config = &ctx.accounts.config;

    let discount_bps = match attestation.tier {
        ReputationTier::Platinum => config.fee_discount_platinum_bps,
        ReputationTier::Gold => config.fee_discount_gold_bps,
        ReputationTier::Silver => config.fee_discount_silver_bps,
        ReputationTier::Bronze => config.fee_discount_bronze_bps,
        ReputationTier::Unscored => 0,
    };

    msg!("Fee discount: tier={:?}, discount_bps={}", attestation.tier, discount_bps);
    Ok(discount_bps)
}

/// Check if creator needs extended vesting (2x) due to low reputation
pub fn needs_extended_vesting(
    attestation: &ReputationAttestation,
    config: &ReputationConfig,
) -> bool {
    attestation.fairscore < config.strict_vesting_threshold
}

/// Calculate vesting multiplier: 2x for low rep, 1x for normal
pub fn vesting_multiplier(
    attestation: &ReputationAttestation,
    config: &ReputationConfig,
) -> u8 {
    if needs_extended_vesting(attestation, config) { 2 } else { 1 }
}

// ─── Errors ───

#[error_code]
pub enum ReputationError {
    #[msg("FairScore must be between 0 and 100")]
    InvalidScore,
    #[msg("Wallet does not meet minimum reputation score to launch")]
    InsufficientReputation,
    #[msg("Reputation attestation is stale (>24h old)")]
    StaleAttestation,
}
