use anchor_lang::prelude::*;

// ============================================================================
// SEEDS & CONSTANTS
// ============================================================================

pub const SHARE_CARD_SEED: &[u8] = b"share_card";

pub const MAX_TOKEN_NAME_LEN: usize = 32;
pub const MAX_SYMBOL_LEN: usize = 10;

// ============================================================================
// ACCOUNTS
// ============================================================================

/// Share card PDA — one per token mint. Stores data needed to render
/// auto-generated share/embed cards on the frontend.
#[account]
#[derive(Default)]
pub struct ShareCard {
    /// Human-readable token name (max 32 bytes).
    pub token_name: String,
    /// Token ticker symbol (max 10 bytes).
    pub symbol: String,
    /// Current price in lamports per whole token.
    pub current_price: u64,
    /// Market cap in lamports.
    pub market_cap: u64,
    /// Rolling 24-hour volume in lamports.
    pub volume_24h: u64,
    /// Number of distinct holders.
    pub holder_count: u32,
    /// Original token creator.
    pub creator: Pubkey,
    /// Migration / bonding-curve progress in basis points (0–10_000).
    pub migration_progress_bps: u16,
    /// Unix timestamp of last update.
    pub last_updated: i64,
    /// The token mint this card belongs to.
    pub token_mint: Pubkey,
    /// PDA bump.
    pub bump: u8,
}

impl ShareCard {
    pub const SIZE: usize = 8  // discriminator
        + 4 + MAX_TOKEN_NAME_LEN  // String (len prefix + data)
        + 4 + MAX_SYMBOL_LEN
        + 8  // current_price
        + 8  // market_cap
        + 8  // volume_24h
        + 4  // holder_count
        + 32 // creator
        + 2  // migration_progress_bps
        + 8  // last_updated
        + 32 // token_mint
        + 1; // bump
}

// ============================================================================
// EVENTS
// ============================================================================

#[event]
pub struct ShareCardDataEvent {
    pub token_mint: Pubkey,
    pub token_name: String,
    pub symbol: String,
    pub current_price: u64,
    pub market_cap: u64,
    pub volume_24h: u64,
    pub holder_count: u32,
    pub creator: Pubkey,
    pub migration_progress_bps: u16,
    pub last_updated: i64,
}

// ============================================================================
// INSTRUCTIONS
// ============================================================================

/// Permissionless crank — anyone can call this after trades to refresh the
/// share card with the latest on-chain data.
pub fn update_share_card(
    ctx: Context<UpdateShareCard>,
    token_name: String,
    symbol: String,
    current_price: u64,
    market_cap: u64,
    volume_24h: u64,
    holder_count: u32,
    creator: Pubkey,
    migration_progress_bps: u16,
) -> Result<()> {
    require!(token_name.len() <= MAX_TOKEN_NAME_LEN, ShareCardError::NameTooLong);
    require!(symbol.len() <= MAX_SYMBOL_LEN, ShareCardError::SymbolTooLong);
    require!(migration_progress_bps <= 10_000, ShareCardError::InvalidBps);

    let card = &mut ctx.accounts.share_card;
    card.token_name = token_name;
    card.symbol = symbol;
    card.current_price = current_price;
    card.market_cap = market_cap;
    card.volume_24h = volume_24h;
    card.holder_count = holder_count;
    card.creator = creator;
    card.migration_progress_bps = migration_progress_bps;
    card.last_updated = Clock::get()?.unix_timestamp;
    card.token_mint = ctx.accounts.token_mint.key();
    card.bump = ctx.bumps.share_card;

    Ok(())
}

/// Emits an event with the full share card data so frontends can render it.
pub fn get_share_card_data(ctx: Context<GetShareCardData>) -> Result<()> {
    let card = &ctx.accounts.share_card;

    emit!(ShareCardDataEvent {
        token_mint: card.token_mint,
        token_name: card.token_name.clone(),
        symbol: card.symbol.clone(),
        current_price: card.current_price,
        market_cap: card.market_cap,
        volume_24h: card.volume_24h,
        holder_count: card.holder_count,
        creator: card.creator,
        migration_progress_bps: card.migration_progress_bps,
        last_updated: card.last_updated,
    });

    Ok(())
}

// ============================================================================
// CONTEXT STRUCTS
// ============================================================================

#[derive(Accounts)]
pub struct UpdateShareCard<'info> {
    #[account(
        init_if_needed,
        payer = payer,
        space = ShareCard::SIZE,
        seeds = [SHARE_CARD_SEED, token_mint.key().as_ref()],
        bump,
    )]
    pub share_card: Account<'info, ShareCard>,

    /// CHECK: Token mint — validated by seeds.
    pub token_mint: UncheckedAccount<'info>,

    #[account(mut)]
    pub payer: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct GetShareCardData<'info> {
    #[account(
        seeds = [SHARE_CARD_SEED, token_mint.key().as_ref()],
        bump = share_card.bump,
    )]
    pub share_card: Account<'info, ShareCard>,

    /// CHECK: Token mint — validated by seeds.
    pub token_mint: UncheckedAccount<'info>,
}

// ============================================================================
// ERRORS
// ============================================================================

#[error_code]
pub enum ShareCardError {
    #[msg("Token name exceeds maximum length of 32 bytes")]
    NameTooLong,
    #[msg("Symbol exceeds maximum length of 10 bytes")]
    SymbolTooLong,
    #[msg("Migration progress BPS must be 0–10000")]
    InvalidBps,
}
