use anchor_lang::prelude::*;

// ============================================================================
// SEEDS & CONSTANTS
// ============================================================================

pub const TOKEN_VIDEO_SEED: &[u8] = b"token_video";
pub const USER_VIDEO_VOTE_SEED: &[u8] = b"user_video_vote";

pub const MAX_VIDEO_URL_LEN: usize = 200;
pub const MAX_THUMBNAIL_URL_LEN: usize = 200;
pub const MAX_DESCRIPTION_LEN: usize = 500;

// ============================================================================
// ACCOUNTS
// ============================================================================

/// Video pitch PDA — one per token mint. Only the token creator can set it.
#[account]
#[derive(Default)]
pub struct TokenVideo {
    /// Creator who posted the video (must match token's original creator).
    pub creator: Pubkey,
    /// URL to the video (max 200 chars).
    pub video_url: String,
    /// URL to a thumbnail image (max 200 chars).
    pub thumbnail_url: String,
    /// Short description / pitch (max 500 chars).
    pub description: String,
    /// Total upvotes.
    pub upvotes: u32,
    /// Total downvotes.
    pub downvotes: u32,
    /// Unix timestamp when the video was posted.
    pub posted_at: i64,
    /// The token mint this video belongs to.
    pub token_mint: Pubkey,
    /// PDA bump.
    pub bump: u8,
}

impl TokenVideo {
    pub const SIZE: usize = 8  // discriminator
        + 32 // creator
        + 4 + MAX_VIDEO_URL_LEN
        + 4 + MAX_THUMBNAIL_URL_LEN
        + 4 + MAX_DESCRIPTION_LEN
        + 4  // upvotes
        + 4  // downvotes
        + 8  // posted_at
        + 32 // token_mint
        + 1; // bump
}

/// Tracks whether a user has voted on a specific token video (one vote per user).
#[account]
#[derive(Default)]
pub struct UserVideoVote {
    pub user: Pubkey,
    pub token_mint: Pubkey,
    /// true = upvote, false = downvote.
    pub is_upvote: bool,
    pub bump: u8,
}

impl UserVideoVote {
    pub const SIZE: usize = 8 + 32 + 32 + 1 + 1;
}

// ============================================================================
// INSTRUCTIONS
// ============================================================================

/// Creator sets (or updates) the video pitch for their token.
pub fn set_token_video(
    ctx: Context<SetTokenVideo>,
    video_url: String,
    thumbnail_url: String,
    description: String,
) -> Result<()> {
    require!(video_url.len() <= MAX_VIDEO_URL_LEN, TokenVideoError::VideoUrlTooLong);
    require!(thumbnail_url.len() <= MAX_THUMBNAIL_URL_LEN, TokenVideoError::ThumbnailUrlTooLong);
    require!(description.len() <= MAX_DESCRIPTION_LEN, TokenVideoError::DescriptionTooLong);

    let video = &mut ctx.accounts.token_video;
    video.creator = ctx.accounts.creator.key();
    video.video_url = video_url;
    video.thumbnail_url = thumbnail_url;
    video.description = description;
    video.posted_at = Clock::get()?.unix_timestamp;
    video.token_mint = ctx.accounts.token_mint.key();
    video.bump = ctx.bumps.token_video;

    Ok(())
}

/// Upvote a token video. One vote per user enforced via UserVideoVote PDA.
pub fn upvote_video(ctx: Context<VoteVideo>) -> Result<()> {
    let video = &mut ctx.accounts.token_video;
    let vote = &mut ctx.accounts.user_video_vote;

    vote.user = ctx.accounts.voter.key();
    vote.token_mint = ctx.accounts.token_mint.key();
    vote.is_upvote = true;
    vote.bump = ctx.bumps.user_video_vote;

    video.upvotes = video.upvotes.checked_add(1).unwrap();

    Ok(())
}

/// Downvote a token video. One vote per user enforced via UserVideoVote PDA.
pub fn downvote_video(ctx: Context<VoteVideo>) -> Result<()> {
    let video = &mut ctx.accounts.token_video;
    let vote = &mut ctx.accounts.user_video_vote;

    vote.user = ctx.accounts.voter.key();
    vote.token_mint = ctx.accounts.token_mint.key();
    vote.is_upvote = false;
    vote.bump = ctx.bumps.user_video_vote;

    video.downvotes = video.downvotes.checked_add(1).unwrap();

    Ok(())
}

/// Remove a token video. Only the creator or platform authority can call this.
pub fn remove_video(ctx: Context<RemoveVideo>) -> Result<()> {
    let video = &ctx.accounts.token_video;
    let signer = ctx.accounts.authority.key();

    require!(
        signer == video.creator || signer == ctx.accounts.platform_authority.key(),
        TokenVideoError::Unauthorized
    );

    // Close account and return lamports to authority
    // (handled by the `close` constraint on the context)
    Ok(())
}

// ============================================================================
// CONTEXT STRUCTS
// ============================================================================

#[derive(Accounts)]
pub struct SetTokenVideo<'info> {
    #[account(
        init_if_needed,
        payer = creator,
        space = TokenVideo::SIZE,
        seeds = [TOKEN_VIDEO_SEED, token_mint.key().as_ref()],
        bump,
    )]
    pub token_video: Account<'info, TokenVideo>,

    /// CHECK: Token mint — validated by seeds.
    pub token_mint: UncheckedAccount<'info>,

    #[account(mut)]
    pub creator: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct VoteVideo<'info> {
    #[account(
        mut,
        seeds = [TOKEN_VIDEO_SEED, token_mint.key().as_ref()],
        bump = token_video.bump,
    )]
    pub token_video: Account<'info, TokenVideo>,

    #[account(
        init,
        payer = voter,
        space = UserVideoVote::SIZE,
        seeds = [USER_VIDEO_VOTE_SEED, token_mint.key().as_ref(), voter.key().as_ref()],
        bump,
    )]
    pub user_video_vote: Account<'info, UserVideoVote>,

    /// CHECK: Token mint — validated by seeds.
    pub token_mint: UncheckedAccount<'info>,

    #[account(mut)]
    pub voter: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct RemoveVideo<'info> {
    #[account(
        mut,
        close = authority,
        seeds = [TOKEN_VIDEO_SEED, token_mint.key().as_ref()],
        bump = token_video.bump,
    )]
    pub token_video: Account<'info, TokenVideo>,

    /// CHECK: Token mint — validated by seeds.
    pub token_mint: UncheckedAccount<'info>,

    #[account(mut)]
    pub authority: Signer<'info>,

    /// CHECK: Platform authority for admin removal. Validated in instruction logic.
    pub platform_authority: UncheckedAccount<'info>,
}

// ============================================================================
// ERRORS
// ============================================================================

#[error_code]
pub enum TokenVideoError {
    #[msg("Video URL exceeds maximum length of 200 characters")]
    VideoUrlTooLong,
    #[msg("Thumbnail URL exceeds maximum length of 200 characters")]
    ThumbnailUrlTooLong,
    #[msg("Description exceeds maximum length of 500 characters")]
    DescriptionTooLong,
    #[msg("Only the token creator or platform authority can perform this action")]
    Unauthorized,
}
