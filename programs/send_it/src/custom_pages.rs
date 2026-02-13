use anchor_lang::prelude::*;

use crate::{
    TokenLaunch, PlatformConfig, SendItError,
    TOKEN_LAUNCH_SEED, PLATFORM_CONFIG_SEED, PLATFORM_VAULT_SEED,
    LAMPORTS_PER_SOL,
};

// ============================================================================
// CONSTANTS
// ============================================================================

pub const CUSTOM_PAGE_SEED: &[u8] = b"custom_page";

pub const MAX_HEADER_IMAGE_URL: usize = 256;
pub const MAX_THEME_COLOR: usize = 7;         // #RRGGBB
pub const MAX_DESCRIPTION_LONG: usize = 2000;
pub const MAX_SOCIAL_LINKS: usize = 512;      // JSON string of social links
pub const MAX_CSS_HASH: usize = 64;           // SHA256 hex of custom CSS

// Tier pricing in lamports
pub const TIER_BASIC_PRICE: u64 = 0;
pub const TIER_PRO_PRICE: u64 = 100_000_000;      // 0.1 SOL
pub const TIER_ULTRA_PRICE: u64 = 500_000_000;     // 0.5 SOL

// ============================================================================
// ACCOUNTS
// ============================================================================

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
pub enum PageTier {
    Basic,  // free: theme_color only
    Pro,    // 0.1 SOL: + header image, long description
    Ultra,  // 0.5 SOL: + social links, custom CSS hash
}

#[account]
pub struct CustomPage {
    pub token_launch: Pubkey,          // associated TokenLaunch PDA
    pub mint: Pubkey,                  // token mint
    pub creator: Pubkey,               // page owner (token creator)
    pub tier: PageTier,                // current tier level
    pub header_image_url: String,      // Pro+: header/banner image URL
    pub theme_color: String,           // Basic+: hex color code
    pub description_long: String,      // Pro+: extended description
    pub social_links: String,          // Ultra: JSON encoded social links
    pub custom_css_hash: String,       // Ultra: hash of approved custom CSS
    pub last_updated: i64,             // timestamp of last update
    pub bump: u8,
}

impl CustomPage {
    pub const SIZE: usize = 8          // discriminator
        + 32                            // token_launch
        + 32                            // mint
        + 32                            // creator
        + 1                             // tier
        + (4 + MAX_HEADER_IMAGE_URL)    // header_image_url
        + (4 + MAX_THEME_COLOR)         // theme_color
        + (4 + MAX_DESCRIPTION_LONG)    // description_long
        + (4 + MAX_SOCIAL_LINKS)        // social_links
        + (4 + MAX_CSS_HASH)            // custom_css_hash
        + 8                             // last_updated
        + 1;                            // bump
}

// ============================================================================
// INSTRUCTIONS
// ============================================================================

/// Update (or initialize) a custom page for a token launch.
/// Creator pays the tier fee in SOL to the platform vault.
/// Upgrading tier pays the difference. Downgrading is not refunded.
pub fn update_custom_page(
    ctx: Context<UpdateCustomPage>,
    tier: PageTier,
    header_image_url: Option<String>,
    theme_color: Option<String>,
    description_long: Option<String>,
    social_links: Option<String>,
    custom_css_hash: Option<String>,
) -> Result<()> {
    let config = &ctx.accounts.platform_config;
    require!(!config.paused, SendItError::PlatformPaused);

    let launch = &ctx.accounts.token_launch;
    require!(launch.creator == ctx.accounts.creator.key(), SendItError::InvalidCreator);

    // Validate field lengths
    if let Some(ref url) = header_image_url {
        require!(url.len() <= MAX_HEADER_IMAGE_URL, CustomPageError::FieldTooLong);
    }
    if let Some(ref color) = theme_color {
        require!(color.len() <= MAX_THEME_COLOR, CustomPageError::FieldTooLong);
        // Basic hex color validation
        require!(
            color.starts_with('#') && color.len() == 7,
            CustomPageError::InvalidColor
        );
    }
    if let Some(ref desc) = description_long {
        require!(desc.len() <= MAX_DESCRIPTION_LONG, CustomPageError::FieldTooLong);
    }
    if let Some(ref links) = social_links {
        require!(links.len() <= MAX_SOCIAL_LINKS, CustomPageError::FieldTooLong);
    }
    if let Some(ref hash) = custom_css_hash {
        require!(hash.len() <= MAX_CSS_HASH, CustomPageError::FieldTooLong);
    }

    // Enforce tier-based field access
    match tier {
        PageTier::Basic => {
            // Basic only allows theme_color
            require!(header_image_url.is_none(), CustomPageError::TierTooLow);
            require!(description_long.is_none(), CustomPageError::TierTooLow);
            require!(social_links.is_none(), CustomPageError::TierTooLow);
            require!(custom_css_hash.is_none(), CustomPageError::TierTooLow);
        }
        PageTier::Pro => {
            // Pro allows theme_color + header_image + description
            require!(social_links.is_none(), CustomPageError::TierTooLow);
            require!(custom_css_hash.is_none(), CustomPageError::TierTooLow);
        }
        PageTier::Ultra => {
            // Ultra allows everything
        }
    }

    // Calculate fee: pay the tier price (upgrade difference if already paid a lower tier)
    let page = &ctx.accounts.custom_page;
    let current_tier_price = tier_price(page.tier);
    let new_tier_price = tier_price(tier);
    let fee = if new_tier_price > current_tier_price {
        new_tier_price - current_tier_price
    } else {
        0 // No refund on downgrade, no extra charge on same tier
    };

    if fee > 0 {
        anchor_lang::system_program::transfer(
            CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.creator.to_account_info(),
                    to: ctx.accounts.platform_vault.to_account_info(),
                },
            ),
            fee,
        )?;
    }

    // Update page fields
    let clock = Clock::get()?;
    let page = &mut ctx.accounts.custom_page;

    // First-time init fields
    if page.bump == 0 {
        page.token_launch = ctx.accounts.token_launch.key();
        page.mint = launch.mint;
        page.creator = ctx.accounts.creator.key();
        page.bump = ctx.bumps.custom_page;
    }

    page.tier = tier;
    page.last_updated = clock.unix_timestamp;

    if let Some(color) = theme_color {
        page.theme_color = color;
    }
    if let Some(url) = header_image_url {
        page.header_image_url = url;
    }
    if let Some(desc) = description_long {
        page.description_long = desc;
    }
    if let Some(links) = social_links {
        page.social_links = links;
    }
    if let Some(hash) = custom_css_hash {
        page.custom_css_hash = hash;
    }

    emit!(CustomPageUpdated {
        page: page.key(),
        mint: page.mint,
        tier,
        fee_paid: fee,
    });

    Ok(())
}

/// Reset a custom page back to defaults (Basic tier).
/// Only the creator can reset. No refund.
pub fn reset_page(ctx: Context<ResetPage>) -> Result<()> {
    let page = &mut ctx.accounts.custom_page;
    require!(page.creator == ctx.accounts.creator.key(), SendItError::InvalidCreator);

    let clock = Clock::get()?;

    page.tier = PageTier::Basic;
    page.header_image_url = String::new();
    page.theme_color = String::from("#00ff88"); // default neon green
    page.description_long = String::new();
    page.social_links = String::new();
    page.custom_css_hash = String::new();
    page.last_updated = clock.unix_timestamp;

    emit!(CustomPageReset {
        page: page.key(),
        mint: page.mint,
    });

    Ok(())
}

// ============================================================================
// HELPERS
// ============================================================================

fn tier_price(tier: PageTier) -> u64 {
    match tier {
        PageTier::Basic => TIER_BASIC_PRICE,
        PageTier::Pro => TIER_PRO_PRICE,
        PageTier::Ultra => TIER_ULTRA_PRICE,
    }
}

// ============================================================================
// CONTEXT STRUCTS
// ============================================================================

#[derive(Accounts)]
pub struct UpdateCustomPage<'info> {
    #[account(
        init_if_needed,
        payer = creator,
        space = CustomPage::SIZE,
        seeds = [CUSTOM_PAGE_SEED, token_launch.mint.as_ref()],
        bump,
    )]
    pub custom_page: Account<'info, CustomPage>,

    #[account(
        seeds = [TOKEN_LAUNCH_SEED, token_launch.mint.as_ref()],
        bump = token_launch.bump,
        has_one = creator,
    )]
    pub token_launch: Account<'info, TokenLaunch>,

    #[account(
        seeds = [PLATFORM_CONFIG_SEED],
        bump = platform_config.bump,
    )]
    pub platform_config: Account<'info, PlatformConfig>,

    /// CHECK: Platform vault receives tier fees
    #[account(
        mut,
        seeds = [PLATFORM_VAULT_SEED],
        bump,
    )]
    pub platform_vault: AccountInfo<'info>,

    #[account(mut)]
    pub creator: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ResetPage<'info> {
    #[account(
        mut,
        seeds = [CUSTOM_PAGE_SEED, custom_page.mint.as_ref()],
        bump = custom_page.bump,
        constraint = custom_page.creator == creator.key() @ SendItError::InvalidCreator,
    )]
    pub custom_page: Account<'info, CustomPage>,

    pub creator: Signer<'info>,
}

// ============================================================================
// EVENTS
// ============================================================================

#[event]
pub struct CustomPageUpdated {
    pub page: Pubkey,
    pub mint: Pubkey,
    pub tier: PageTier,
    pub fee_paid: u64,
}

#[event]
pub struct CustomPageReset {
    pub page: Pubkey,
    pub mint: Pubkey,
}

// ============================================================================
// ERRORS
// ============================================================================

#[error_code]
pub enum CustomPageError {
    #[msg("Field exceeds maximum length")]
    FieldTooLong,
    #[msg("Invalid hex color format (use #RRGGBB)")]
    InvalidColor,
    #[msg("Feature requires a higher tier")]
    TierTooLow,
}
