use anchor_lang::prelude::*;

declare_id!("SenditWidgets11111111111111111111111111111111");

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const WIDGET_SEED: &[u8] = b"widget";

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Default)]
pub enum WidgetType {
    #[default]
    PriceBadge,
    TradingCard,
    LeaderboardBadge,
    MiniChart,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Default)]
pub enum WidgetTheme {
    #[default]
    Dark,
    Light,
    Custom,
}

// ---------------------------------------------------------------------------
// Accounts
// ---------------------------------------------------------------------------

#[account]
#[derive(Default)]
pub struct WidgetConfig {
    /// The token mint this widget is for
    pub token_mint: Pubkey,
    /// Creator who owns this widget config
    pub creator: Pubkey,
    /// Type of widget
    pub widget_type: WidgetType,
    /// Theme
    pub theme: WidgetTheme,
    /// Custom RGB color (only used when theme = Custom)
    pub custom_color: Option<[u8; 3]>,
    /// Display flags
    pub show_price: bool,
    pub show_volume: bool,
    pub show_holders: bool,
    pub show_market_cap: bool,
    /// Whether the widget is enabled
    pub enabled: bool,
    /// Total views tracked on-chain for analytics
    pub views: u64,
    /// Bump seed for PDA
    pub bump: u8,
}

impl WidgetConfig {
    pub const SIZE: usize = 8  // discriminator
        + 32  // token_mint
        + 32  // creator
        + 1   // widget_type
        + 1   // theme
        + (1 + 3) // custom_color Option<[u8; 3]>
        + 1   // show_price
        + 1   // show_volume
        + 1   // show_holders
        + 1   // show_market_cap
        + 1   // enabled
        + 8   // views
        + 1;  // bump
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

#[event]
pub struct WidgetCreated {
    pub token_mint: Pubkey,
    pub creator: Pubkey,
    pub widget_type: WidgetType,
    pub theme: WidgetTheme,
}

#[event]
pub struct WidgetUpdated {
    pub token_mint: Pubkey,
    pub creator: Pubkey,
    pub widget_type: WidgetType,
    pub theme: WidgetTheme,
}

#[event]
pub struct WidgetViewed {
    pub token_mint: Pubkey,
    pub views: u64,
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[error_code]
pub enum WidgetError {
    #[msg("Unauthorized — only creator can modify widget")]
    Unauthorized,
    #[msg("Widget is disabled")]
    WidgetDisabled,
    #[msg("Custom color required when theme is Custom")]
    CustomColorRequired,
}

// ---------------------------------------------------------------------------
// Instructions
// ---------------------------------------------------------------------------

#[derive(Accounts)]
pub struct CreateWidgetConfig<'info> {
    #[account(
        init,
        payer = creator,
        space = WidgetConfig::SIZE,
        seeds = [WIDGET_SEED, token_mint.key().as_ref()],
        bump,
    )]
    pub widget_config: Account<'info, WidgetConfig>,

    /// CHECK: Token mint
    pub token_mint: AccountInfo<'info>,

    #[account(mut)]
    pub creator: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UpdateWidgetConfig<'info> {
    #[account(
        mut,
        seeds = [WIDGET_SEED, token_mint.key().as_ref()],
        bump = widget_config.bump,
        has_one = creator @ WidgetError::Unauthorized,
    )]
    pub widget_config: Account<'info, WidgetConfig>,

    /// CHECK: Token mint
    pub token_mint: AccountInfo<'info>,

    pub creator: Signer<'info>,
}

#[derive(Accounts)]
pub struct RecordWidgetView<'info> {
    #[account(
        mut,
        seeds = [WIDGET_SEED, token_mint.key().as_ref()],
        bump = widget_config.bump,
    )]
    pub widget_config: Account<'info, WidgetConfig>,

    /// CHECK: Token mint
    pub token_mint: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct DisableWidget<'info> {
    #[account(
        mut,
        seeds = [WIDGET_SEED, token_mint.key().as_ref()],
        bump = widget_config.bump,
    )]
    pub widget_config: Account<'info, WidgetConfig>,

    /// CHECK: Token mint
    pub token_mint: AccountInfo<'info>,

    /// Creator or platform authority
    pub authority: Signer<'info>,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

pub fn handle_create_widget_config(
    ctx: Context<CreateWidgetConfig>,
    widget_type: WidgetType,
    theme: WidgetTheme,
    custom_color: Option<[u8; 3]>,
    show_price: bool,
    show_volume: bool,
    show_holders: bool,
    show_market_cap: bool,
) -> Result<()> {
    if theme == WidgetTheme::Custom {
        require!(custom_color.is_some(), WidgetError::CustomColorRequired);
    }

    let config = &mut ctx.accounts.widget_config;
    config.token_mint = ctx.accounts.token_mint.key();
    config.creator = ctx.accounts.creator.key();
    config.widget_type = widget_type;
    config.theme = theme;
    config.custom_color = custom_color;
    config.show_price = show_price;
    config.show_volume = show_volume;
    config.show_holders = show_holders;
    config.show_market_cap = show_market_cap;
    config.enabled = true;
    config.views = 0;
    config.bump = ctx.bumps.widget_config;

    emit!(WidgetCreated {
        token_mint: ctx.accounts.token_mint.key(),
        creator: ctx.accounts.creator.key(),
        widget_type,
        theme,
    });

    Ok(())
}

pub fn handle_update_widget_config(
    ctx: Context<UpdateWidgetConfig>,
    widget_type: Option<WidgetType>,
    theme: Option<WidgetTheme>,
    custom_color: Option<Option<[u8; 3]>>,
    show_price: Option<bool>,
    show_volume: Option<bool>,
    show_holders: Option<bool>,
    show_market_cap: Option<bool>,
) -> Result<()> {
    let config = &mut ctx.accounts.widget_config;

    if let Some(wt) = widget_type {
        config.widget_type = wt;
    }
    if let Some(t) = theme {
        config.theme = t;
    }
    if let Some(cc) = custom_color {
        config.custom_color = cc;
    }
    if let Some(v) = show_price {
        config.show_price = v;
    }
    if let Some(v) = show_volume {
        config.show_volume = v;
    }
    if let Some(v) = show_holders {
        config.show_holders = v;
    }
    if let Some(v) = show_market_cap {
        config.show_market_cap = v;
    }

    // Validate custom color if theme is Custom
    if config.theme == WidgetTheme::Custom {
        require!(config.custom_color.is_some(), WidgetError::CustomColorRequired);
    }

    emit!(WidgetUpdated {
        token_mint: ctx.accounts.token_mint.key(),
        creator: ctx.accounts.creator.key(),
        widget_type: config.widget_type,
        theme: config.theme,
    });

    Ok(())
}

pub fn handle_record_widget_view(ctx: Context<RecordWidgetView>) -> Result<()> {
    let config = &mut ctx.accounts.widget_config;
    require!(config.enabled, WidgetError::WidgetDisabled);

    config.views = config.views.saturating_add(1);

    emit!(WidgetViewed {
        token_mint: ctx.accounts.token_mint.key(),
        views: config.views,
    });

    Ok(())
}

pub fn handle_disable_widget(ctx: Context<DisableWidget>) -> Result<()> {
    let config = &ctx.accounts.widget_config;
    // Allow creator or any authority (for platform admin, check in caller)
    require!(
        ctx.accounts.authority.key() == config.creator,
        WidgetError::Unauthorized
    );

    let config = &mut ctx.accounts.widget_config;
    config.enabled = false;

    Ok(())
}
