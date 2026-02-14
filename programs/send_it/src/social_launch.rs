use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token, TokenAccount, MintTo};
use anchor_spl::associated_token::AssociatedToken;

use crate::{
    CurveType, PlatformConfig, TokenLaunch,
    PLATFORM_CONFIG_SEED, TOKEN_LAUNCH_SEED, TOKEN_DECIMALS,
    DEFAULT_TOTAL_SUPPLY, DEFAULT_MIGRATION_THRESHOLD,
    MAX_NAME_LEN, MAX_SYMBOL_LEN, MAX_URI_LEN,
    SendItError,
};

// ═══════════════════════════════════════════════════════════════════════════════
//  Social Launch Module for Send.it
//  Tweet-to-launch: create tokens by posting a tweet URL (inspired by Believe.app)
//
//  Flow:
//    1. Creator posts a tweet describing a token idea
//    2. Creator calls `launch_from_tweet` with the tweet URL + metadata
//    3. An oracle/verifier calls `verify_tweet` to attest tweet authenticity
//    4. Token is immediately tradeable on the bonding curve (via existing module)
// ═══════════════════════════════════════════════════════════════════════════════

declare_id!("SoCiaLLaUnCh111111111111111111111111111111");

// ── Seeds ──

pub const SOCIAL_LAUNCH_SEED: &[u8] = b"social_launch";
pub const TWEET_VERIFICATION_SEED: &[u8] = b"tweet_verification";
pub const SOCIAL_CONFIG_SEED: &[u8] = b"social_config";

// ── Limits ──

pub const MAX_TWEET_URL_LEN: usize = 280;
pub const MAX_TWEET_ID_LEN: usize = 32;
pub const MAX_AUTHOR_HANDLE_LEN: usize = 64;
pub const MAX_TWEET_CONTENT_LEN: usize = 280;
pub const MAX_TOKEN_DESCRIPTION_LEN: usize = 200;

// ═══════════════════════════════════════════════════════════════════════════════
//  ACCOUNTS
// ═══════════════════════════════════════════════════════════════════════════════

/// Global configuration for the social launch module.
#[account]
pub struct SocialLaunchConfig {
    /// Authority that can update config and designate verifiers.
    pub authority: Pubkey,
    /// The oracle/backend that is allowed to verify tweets.
    pub verifier_authority: Pubkey,
    /// Whether launches require tweet verification before trading starts.
    pub require_verification: bool,
    /// Default curve type for tweet-launched tokens.
    pub default_curve_type: CurveType,
    /// Default creator fee in basis points for social launches.
    pub default_creator_fee_bps: u16,
    /// Time (seconds) after launch before trading begins (gives verifier time).
    pub verification_grace_period: i64,
    /// Total social launches to date.
    pub total_social_launches: u64,
    pub bump: u8,
}

impl SocialLaunchConfig {
    pub const SIZE: usize = 8   // discriminator
        + 32    // authority
        + 32    // verifier_authority
        + 1     // require_verification
        + 1     // default_curve_type
        + 2     // default_creator_fee_bps
        + 8     // verification_grace_period
        + 8     // total_social_launches
        + 1;    // bump
}

/// PDA linking a tweet to a token launch. Created when a user launches from a tweet.
#[account]
pub struct SocialLaunchRecord {
    /// The wallet that initiated the launch.
    pub creator: Pubkey,
    /// The token mint created for this launch.
    pub mint: Pubkey,
    /// Full tweet URL (e.g. "https://x.com/user/status/123456").
    pub tweet_url: String,
    /// Extracted tweet ID (numeric string).
    pub tweet_id: String,
    /// Twitter/X handle of the tweet author.
    pub author_handle: String,
    /// Raw tweet content used to derive token metadata.
    pub tweet_content: String,
    /// Token name derived from tweet content.
    pub token_name: String,
    /// Token symbol derived from tweet content.
    pub token_symbol: String,
    /// Token metadata URI (off-chain JSON).
    pub token_uri: String,
    /// Optional description extracted or provided.
    pub token_description: String,
    /// Whether the linked tweet has been verified by the oracle.
    pub verified: bool,
    /// Unix timestamp of launch creation.
    pub created_at: i64,
    /// Unix timestamp of verification (0 if unverified).
    pub verified_at: i64,
    pub bump: u8,
}

impl SocialLaunchRecord {
    pub const SIZE: usize = 8   // discriminator
        + 32    // creator
        + 32    // mint
        + (4 + MAX_TWEET_URL_LEN)       // tweet_url
        + (4 + MAX_TWEET_ID_LEN)        // tweet_id
        + (4 + MAX_AUTHOR_HANDLE_LEN)   // author_handle
        + (4 + MAX_TWEET_CONTENT_LEN)   // tweet_content
        + (4 + MAX_NAME_LEN)            // token_name
        + (4 + MAX_SYMBOL_LEN)          // token_symbol
        + (4 + MAX_URI_LEN)             // token_uri
        + (4 + MAX_TOKEN_DESCRIPTION_LEN) // token_description
        + 1     // verified
        + 8     // created_at
        + 8     // verified_at
        + 1;    // bump
}

/// Standalone tweet verification PDA. Can exist independently of a launch,
/// allowing pre-verification or verification of tweets not yet launched.
#[account]
pub struct TweetVerification {
    /// The tweet ID being verified.
    pub tweet_id: String,
    /// Twitter/X handle of the tweet author.
    pub author_handle: String,
    /// Whether the tweet has been verified as authentic.
    pub verified: bool,
    /// The verifier authority that attested.
    pub verified_by: Pubkey,
    /// Unix timestamp of verification.
    pub verified_at: i64,
    /// Optional: the mint if a launch was created from this tweet.
    pub associated_mint: Option<Pubkey>,
    pub bump: u8,
}

impl TweetVerification {
    pub const SIZE: usize = 8   // discriminator
        + (4 + MAX_TWEET_ID_LEN)        // tweet_id
        + (4 + MAX_AUTHOR_HANDLE_LEN)   // author_handle
        + 1     // verified
        + 32    // verified_by
        + 8     // verified_at
        + (1 + 32) // associated_mint (Option<Pubkey>)
        + 1;    // bump
}

// ═══════════════════════════════════════════════════════════════════════════════
//  CONTEXT STRUCTS
// ═══════════════════════════════════════════════════════════════════════════════

/// Initialize the social launch module configuration.
#[derive(Accounts)]
pub struct InitializeSocialConfig<'info> {
    #[account(
        init,
        payer = authority,
        space = SocialLaunchConfig::SIZE,
        seeds = [SOCIAL_CONFIG_SEED],
        bump,
    )]
    pub social_config: Account<'info, SocialLaunchConfig>,

    #[account(
        seeds = [PLATFORM_CONFIG_SEED],
        bump = platform_config.bump,
        has_one = authority,
    )]
    pub platform_config: Account<'info, PlatformConfig>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

/// Update social launch configuration.
#[derive(Accounts)]
pub struct UpdateSocialConfig<'info> {
    #[account(
        mut,
        seeds = [SOCIAL_CONFIG_SEED],
        bump = social_config.bump,
        has_one = authority,
    )]
    pub social_config: Account<'info, SocialLaunchConfig>,

    pub authority: Signer<'info>,
}

/// Launch a token from a tweet. Creates both a SocialLaunchRecord and a
/// TokenLaunch (from the core module) plus mints the token supply.
#[derive(Accounts)]
#[instruction(
    tweet_url: String,
    tweet_id: String,
    author_handle: String,
    tweet_content: String,
    token_name: String,
    token_symbol: String,
    token_uri: String,
)]
pub struct LaunchFromTweet<'info> {
    // ── Social launch record (new) ──
    #[account(
        init,
        payer = creator,
        space = SocialLaunchRecord::SIZE,
        seeds = [SOCIAL_LAUNCH_SEED, tweet_id.as_bytes()],
        bump,
    )]
    pub social_launch_record: Account<'info, SocialLaunchRecord>,

    // ── Core token launch PDA (new) ──
    #[account(
        init,
        payer = creator,
        space = TokenLaunch::SIZE,
        seeds = [TOKEN_LAUNCH_SEED, token_mint.key().as_ref()],
        bump,
    )]
    pub token_launch: Account<'info, TokenLaunch>,

    // ── Token mint (new) ──
    #[account(
        init,
        payer = creator,
        mint::decimals = TOKEN_DECIMALS,
        mint::authority = token_launch,
    )]
    pub token_mint: Account<'info, Mint>,

    // ── Launch token vault (new) ──
    #[account(
        init,
        payer = creator,
        associated_token::mint = token_mint,
        associated_token::authority = token_launch,
    )]
    pub launch_token_vault: Account<'info, TokenAccount>,

    // ── Social config ──
    #[account(
        mut,
        seeds = [SOCIAL_CONFIG_SEED],
        bump = social_config.bump,
    )]
    pub social_config: Account<'info, SocialLaunchConfig>,

    // ── Platform config ──
    #[account(
        mut,
        seeds = [PLATFORM_CONFIG_SEED],
        bump = platform_config.bump,
    )]
    pub platform_config: Account<'info, PlatformConfig>,

    #[account(mut)]
    pub creator: Signer<'info>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

/// Oracle/verifier attests that a tweet is authentic and matches the on-chain record.
#[derive(Accounts)]
#[instruction(tweet_id: String)]
pub struct VerifyTweet<'info> {
    // ── Tweet verification PDA (init or update) ──
    #[account(
        init_if_needed,
        payer = verifier,
        space = TweetVerification::SIZE,
        seeds = [TWEET_VERIFICATION_SEED, tweet_id.as_bytes()],
        bump,
    )]
    pub tweet_verification: Account<'info, TweetVerification>,

    // ── Social launch record (update verified flag) ──
    #[account(
        mut,
        seeds = [SOCIAL_LAUNCH_SEED, tweet_id.as_bytes()],
        bump = social_launch_record.bump,
    )]
    pub social_launch_record: Account<'info, SocialLaunchRecord>,

    // ── Social config (check verifier authority) ──
    #[account(
        seeds = [SOCIAL_CONFIG_SEED],
        bump = social_config.bump,
    )]
    pub social_config: Account<'info, SocialLaunchConfig>,

    /// The designated verifier oracle. Must match `social_config.verifier_authority`.
    #[account(
        mut,
        constraint = verifier.key() == social_config.verifier_authority @ SocialLaunchError::UnauthorizedVerifier,
    )]
    pub verifier: Signer<'info>,

    pub system_program: Program<'info, System>,
}

/// Revoke verification (admin only). Used if a tweet is found to be fraudulent.
#[derive(Accounts)]
#[instruction(tweet_id: String)]
pub struct RevokeVerification<'info> {
    #[account(
        mut,
        seeds = [TWEET_VERIFICATION_SEED, tweet_id.as_bytes()],
        bump = tweet_verification.bump,
    )]
    pub tweet_verification: Account<'info, TweetVerification>,

    #[account(
        mut,
        seeds = [SOCIAL_LAUNCH_SEED, tweet_id.as_bytes()],
        bump = social_launch_record.bump,
    )]
    pub social_launch_record: Account<'info, SocialLaunchRecord>,

    #[account(
        seeds = [SOCIAL_CONFIG_SEED],
        bump = social_config.bump,
        has_one = authority,
    )]
    pub social_config: Account<'info, SocialLaunchConfig>,

    pub authority: Signer<'info>,
}

// ═══════════════════════════════════════════════════════════════════════════════
//  INSTRUCTIONS
// ═══════════════════════════════════════════════════════════════════════════════

/// Initialize the social launch module. Called once by platform admin.
pub fn handle_initialize_social_config(
    ctx: Context<InitializeSocialConfig>,
    verifier_authority: Pubkey,
    require_verification: bool,
    default_curve_type: CurveType,
    default_creator_fee_bps: u16,
    verification_grace_period: i64,
) -> Result<()> {
    let config = &mut ctx.accounts.social_config;
    config.authority = ctx.accounts.authority.key();
    config.verifier_authority = verifier_authority;
    config.require_verification = require_verification;
    config.default_curve_type = default_curve_type;
    config.default_creator_fee_bps = default_creator_fee_bps;
    config.verification_grace_period = verification_grace_period;
    config.total_social_launches = 0;
    config.bump = ctx.bumps.social_config;

    emit!(SocialConfigInitialized {
        authority: config.authority,
        verifier_authority,
        require_verification,
    });

    Ok(())
}

/// Update social launch configuration parameters.
pub fn handle_update_social_config(
    ctx: Context<UpdateSocialConfig>,
    new_verifier: Option<Pubkey>,
    new_require_verification: Option<bool>,
    new_default_curve: Option<CurveType>,
    new_default_fee_bps: Option<u16>,
    new_grace_period: Option<i64>,
) -> Result<()> {
    let config = &mut ctx.accounts.social_config;

    if let Some(v) = new_verifier {
        config.verifier_authority = v;
    }
    if let Some(rv) = new_require_verification {
        config.require_verification = rv;
    }
    if let Some(c) = new_default_curve {
        config.default_curve_type = c;
    }
    if let Some(fee) = new_default_fee_bps {
        require!(fee <= 500, SendItError::FeeTooHigh);
        config.default_creator_fee_bps = fee;
    }
    if let Some(gp) = new_grace_period {
        config.verification_grace_period = gp;
    }

    Ok(())
}

/// Launch a token from a tweet URL.
///
/// The creator supplies tweet metadata (URL, ID, author, content) along with
/// desired token name/symbol/URI. A `SocialLaunchRecord` is created linking the
/// tweet to the mint, and a standard `TokenLaunch` is initialized on the bonding
/// curve. The full token supply is minted to the launch vault.
///
/// If `require_verification` is enabled, trading start is delayed by
/// `verification_grace_period` to allow the oracle to verify the tweet.
pub fn handle_launch_from_tweet(
    ctx: Context<LaunchFromTweet>,
    tweet_url: String,
    tweet_id: String,
    author_handle: String,
    tweet_content: String,
    token_name: String,
    token_symbol: String,
    token_uri: String,
    token_description: String,
    curve_type_override: Option<CurveType>,
    creator_fee_bps_override: Option<u16>,
) -> Result<()> {
    // ── Validate inputs ──
    require!(tweet_url.len() <= MAX_TWEET_URL_LEN, SocialLaunchError::TweetUrlTooLong);
    require!(tweet_id.len() <= MAX_TWEET_ID_LEN, SocialLaunchError::TweetIdTooLong);
    require!(author_handle.len() <= MAX_AUTHOR_HANDLE_LEN, SocialLaunchError::AuthorHandleTooLong);
    require!(tweet_content.len() <= MAX_TWEET_CONTENT_LEN, SocialLaunchError::TweetContentTooLong);
    require!(token_name.len() <= MAX_NAME_LEN, SendItError::NameTooLong);
    require!(token_symbol.len() <= MAX_SYMBOL_LEN, SendItError::SymbolTooLong);
    require!(token_uri.len() <= MAX_URI_LEN, SendItError::UriTooLong);
    require!(
        token_description.len() <= MAX_TOKEN_DESCRIPTION_LEN,
        SocialLaunchError::DescriptionTooLong
    );
    require!(!tweet_url.is_empty(), SocialLaunchError::EmptyTweetUrl);
    require!(!tweet_id.is_empty(), SocialLaunchError::EmptyTweetId);

    let platform_config = &ctx.accounts.platform_config;
    require!(!platform_config.paused, SendItError::PlatformPaused);

    let social_config = &ctx.accounts.social_config;
    let clock = Clock::get()?;

    // Resolve curve type and fee (use overrides or social config defaults)
    let curve_type = curve_type_override.unwrap_or(social_config.default_curve_type);
    let creator_fee_bps = creator_fee_bps_override.unwrap_or(social_config.default_creator_fee_bps);
    require!(creator_fee_bps <= 500, SendItError::FeeTooHigh);

    // Determine trading start: if verification required, delay by grace period
    let trading_starts_at = if social_config.require_verification {
        clock.unix_timestamp + social_config.verification_grace_period
    } else {
        clock.unix_timestamp // immediate
    };

    // ── Populate social launch record ──
    let record = &mut ctx.accounts.social_launch_record;
    record.creator = ctx.accounts.creator.key();
    record.mint = ctx.accounts.token_mint.key();
    record.tweet_url = tweet_url.clone();
    record.tweet_id = tweet_id.clone();
    record.author_handle = author_handle.clone();
    record.tweet_content = tweet_content.clone();
    record.token_name = token_name.clone();
    record.token_symbol = token_symbol.clone();
    record.token_uri = token_uri.clone();
    record.token_description = token_description;
    record.verified = false;
    record.created_at = clock.unix_timestamp;
    record.verified_at = 0;
    record.bump = ctx.bumps.social_launch_record;

    // ── Populate core TokenLaunch (integrates with bonding_curve.rs) ──
    let launch = &mut ctx.accounts.token_launch;
    launch.creator = ctx.accounts.creator.key();
    launch.mint = ctx.accounts.token_mint.key();
    launch.name = token_name.clone();
    launch.symbol = token_symbol.clone();
    launch.uri = token_uri.clone();
    launch.curve_type = curve_type;
    launch.creator_fee_bps = creator_fee_bps;
    launch.total_supply = DEFAULT_TOTAL_SUPPLY;
    launch.tokens_sold = 0;
    launch.reserve_sol = 0;
    launch.created_at = clock.unix_timestamp;
    launch.trading_starts_at = trading_starts_at;
    launch.snipe_window_end = trading_starts_at + 30; // 30s anti-snipe window
    launch.max_buy_during_snipe = DEFAULT_TOTAL_SUPPLY / 100; // 1% during snipe
    launch.lock_period_end = clock.unix_timestamp; // no lock for social launches
    launch.migrated = false;
    launch.paused = false;
    launch.total_volume_sol = 0;
    launch.bump = ctx.bumps.token_launch;

    // ── Mint total supply to launch vault ──
    let mint_key = ctx.accounts.token_mint.key();
    let launch_seeds: &[&[u8]] = &[
        TOKEN_LAUNCH_SEED,
        mint_key.as_ref(),
        &[launch.bump],
    ];
    let signer_seeds = &[launch_seeds];

    anchor_spl::token::mint_to(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            MintTo {
                mint: ctx.accounts.token_mint.to_account_info(),
                to: ctx.accounts.launch_token_vault.to_account_info(),
                authority: ctx.accounts.token_launch.to_account_info(),
            },
            signer_seeds,
        ),
        DEFAULT_TOTAL_SUPPLY,
    )?;

    // ── Update counters ──
    let social_config = &mut ctx.accounts.social_config;
    social_config.total_social_launches += 1;

    let platform_config = &mut ctx.accounts.platform_config;
    platform_config.total_launches += 1;

    emit!(SocialLaunchCreated {
        mint: launch.mint,
        creator: launch.creator,
        tweet_id,
        tweet_url,
        author_handle,
        token_name,
        token_symbol,
        curve_type,
        trading_starts_at,
        created_at: clock.unix_timestamp,
    });

    Ok(())
}

/// Verify a tweet's authenticity. Called by the designated oracle/verifier.
///
/// Sets the `verified` flag on both the `TweetVerification` PDA and the
/// associated `SocialLaunchRecord`. Once verified, the token is eligible
/// for trading (if verification gating is enabled).
pub fn handle_verify_tweet(
    ctx: Context<VerifyTweet>,
    tweet_id: String,
    author_handle: String,
    verified: bool,
) -> Result<()> {
    require!(tweet_id.len() <= MAX_TWEET_ID_LEN, SocialLaunchError::TweetIdTooLong);
    require!(
        author_handle.len() <= MAX_AUTHOR_HANDLE_LEN,
        SocialLaunchError::AuthorHandleTooLong
    );

    let clock = Clock::get()?;

    // ── Populate / update TweetVerification PDA ──
    let verification = &mut ctx.accounts.tweet_verification;
    verification.tweet_id = tweet_id.clone();
    verification.author_handle = author_handle.clone();
    verification.verified = verified;
    verification.verified_by = ctx.accounts.verifier.key();
    verification.verified_at = clock.unix_timestamp;
    verification.associated_mint = Some(ctx.accounts.social_launch_record.mint);
    if verification.bump == 0 {
        verification.bump = ctx.bumps.tweet_verification;
    }

    // ── Update the social launch record ──
    let record = &mut ctx.accounts.social_launch_record;
    record.verified = verified;
    record.verified_at = clock.unix_timestamp;

    // Validate that the tweet metadata matches
    require!(
        record.tweet_id == tweet_id,
        SocialLaunchError::TweetIdMismatch
    );

    emit!(TweetVerified {
        tweet_id,
        author_handle,
        verified,
        mint: record.mint,
        verifier: ctx.accounts.verifier.key(),
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

/// Revoke tweet verification (admin only).
///
/// Used when a tweet is discovered to be fraudulent, deleted, or otherwise
/// invalid after initial verification. This pauses the associated launch.
pub fn handle_revoke_verification(
    ctx: Context<RevokeVerification>,
    tweet_id: String,
) -> Result<()> {
    let verification = &mut ctx.accounts.tweet_verification;
    verification.verified = false;

    let record = &mut ctx.accounts.social_launch_record;
    record.verified = false;

    emit!(TweetVerificationRevoked {
        tweet_id,
        mint: record.mint,
        revoked_by: ctx.accounts.authority.key(),
        timestamp: Clock::get()?.unix_timestamp,
    });

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════════
//  EVENTS
// ═══════════════════════════════════════════════════════════════════════════════

#[event]
pub struct SocialConfigInitialized {
    pub authority: Pubkey,
    pub verifier_authority: Pubkey,
    pub require_verification: bool,
}

#[event]
pub struct SocialLaunchCreated {
    pub mint: Pubkey,
    pub creator: Pubkey,
    pub tweet_id: String,
    pub tweet_url: String,
    pub author_handle: String,
    pub token_name: String,
    pub token_symbol: String,
    pub curve_type: CurveType,
    pub trading_starts_at: i64,
    pub created_at: i64,
}

#[event]
pub struct TweetVerified {
    pub tweet_id: String,
    pub author_handle: String,
    pub verified: bool,
    pub mint: Pubkey,
    pub verifier: Pubkey,
    pub timestamp: i64,
}

#[event]
pub struct TweetVerificationRevoked {
    pub tweet_id: String,
    pub mint: Pubkey,
    pub revoked_by: Pubkey,
    pub timestamp: i64,
}

// ═══════════════════════════════════════════════════════════════════════════════
//  ERRORS
// ═══════════════════════════════════════════════════════════════════════════════

#[error_code]
pub enum SocialLaunchError {
    #[msg("Tweet URL exceeds maximum length")]
    TweetUrlTooLong,
    #[msg("Tweet ID exceeds maximum length")]
    TweetIdTooLong,
    #[msg("Author handle exceeds maximum length")]
    AuthorHandleTooLong,
    #[msg("Tweet content exceeds maximum length")]
    TweetContentTooLong,
    #[msg("Token description exceeds maximum length")]
    DescriptionTooLong,
    #[msg("Tweet URL cannot be empty")]
    EmptyTweetUrl,
    #[msg("Tweet ID cannot be empty")]
    EmptyTweetId,
    #[msg("Unauthorized verifier")]
    UnauthorizedVerifier,
    #[msg("Tweet ID does not match the launch record")]
    TweetIdMismatch,
    #[msg("Tweet has not been verified")]
    TweetNotVerified,
    #[msg("Tweet has already been used for a launch")]
    TweetAlreadyUsed,
}
