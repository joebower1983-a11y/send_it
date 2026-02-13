use anchor_lang::prelude::*;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------
pub const MAX_ALERTS_PER_CHECK: usize = 10;

// ---------------------------------------------------------------------------
// Account structs
// ---------------------------------------------------------------------------

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
pub enum PriceDirection {
    Above,
    Below,
}

#[account]
pub struct AlertSubscription {
    pub owner: Pubkey,
    pub token_mint: Pubkey,
    pub target_price: u64,     // price in lamports per token (scaled)
    pub direction: PriceDirection,
    pub active: bool,
    pub created_at: i64,
    pub triggered_at: i64,     // 0 if not yet triggered
    pub alert_id: u64,
    pub bump: u8,
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

#[event]
pub struct AlertCreated {
    pub owner: Pubkey,
    pub token_mint: Pubkey,
    pub alert_id: u64,
    pub target_price: u64,
    pub direction: PriceDirection,
}

#[event]
pub struct AlertCancelled {
    pub owner: Pubkey,
    pub token_mint: Pubkey,
    pub alert_id: u64,
}

#[event]
pub struct AlertTriggered {
    pub owner: Pubkey,
    pub token_mint: Pubkey,
    pub alert_id: u64,
    pub target_price: u64,
    pub current_price: u64,
    pub direction: PriceDirection,
    pub timestamp: i64,
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[error_code]
pub enum PriceAlertError {
    #[msg("Alert is not active")]
    AlertNotActive,
    #[msg("Alert is already active")]
    AlertAlreadyActive,
    #[msg("Unauthorized — only the alert owner can cancel")]
    Unauthorized,
    #[msg("Target price must be greater than zero")]
    InvalidTargetPrice,
    #[msg("Price condition not met")]
    PriceConditionNotMet,
}

// ---------------------------------------------------------------------------
// Instruction accounts
// ---------------------------------------------------------------------------

#[derive(Accounts)]
#[instruction(alert_id: u64, token_mint: Pubkey)]
pub struct CreateAlert<'info> {
    #[account(
        init,
        payer = owner,
        space = 8 + std::mem::size_of::<AlertSubscription>(),
        seeds = [b"alert", owner.key().as_ref(), token_mint.as_ref(), alert_id.to_le_bytes().as_ref()],
        bump
    )]
    pub alert: Account<'info, AlertSubscription>,

    #[account(mut)]
    pub owner: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct CancelAlert<'info> {
    #[account(
        mut,
        seeds = [b"alert", alert.owner.as_ref(), alert.token_mint.as_ref(), alert.alert_id.to_le_bytes().as_ref()],
        bump = alert.bump,
        has_one = owner @ PriceAlertError::Unauthorized
    )]
    pub alert: Account<'info, AlertSubscription>,

    pub owner: Signer<'info>,
}

#[derive(Accounts)]
pub struct CheckAlerts<'info> {
    #[account(
        mut,
        seeds = [b"alert", alert.owner.as_ref(), alert.token_mint.as_ref(), alert.alert_id.to_le_bytes().as_ref()],
        bump = alert.bump
    )]
    pub alert: Account<'info, AlertSubscription>,

    /// Permissionless crank signer
    pub crank: Signer<'info>,
}

// ---------------------------------------------------------------------------
// Instructions
// ---------------------------------------------------------------------------

pub fn handle_create_alert(
    ctx: Context<CreateAlert>,
    alert_id: u64,
    token_mint: Pubkey,
    target_price: u64,
    direction: PriceDirection,
) -> Result<()> {
    require!(target_price > 0, PriceAlertError::InvalidTargetPrice);

    let alert = &mut ctx.accounts.alert;
    let clock = Clock::get()?;

    alert.owner = ctx.accounts.owner.key();
    alert.token_mint = token_mint;
    alert.target_price = target_price;
    alert.direction = direction;
    alert.active = true;
    alert.created_at = clock.unix_timestamp;
    alert.triggered_at = 0;
    alert.alert_id = alert_id;
    alert.bump = ctx.bumps.alert;

    emit!(AlertCreated {
        owner: alert.owner,
        token_mint,
        alert_id,
        target_price,
        direction,
    });

    Ok(())
}

pub fn handle_cancel_alert(ctx: Context<CancelAlert>) -> Result<()> {
    let alert = &mut ctx.accounts.alert;
    require!(alert.active, PriceAlertError::AlertNotActive);

    alert.active = false;

    emit!(AlertCancelled {
        owner: alert.owner,
        token_mint: alert.token_mint,
        alert_id: alert.alert_id,
    });

    Ok(())
}

pub fn handle_check_alerts(
    ctx: Context<CheckAlerts>,
    current_price: u64,
) -> Result<()> {
    let alert = &mut ctx.accounts.alert;
    require!(alert.active, PriceAlertError::AlertNotActive);

    let triggered = match alert.direction {
        PriceDirection::Above => current_price >= alert.target_price,
        PriceDirection::Below => current_price <= alert.target_price,
    };

    require!(triggered, PriceAlertError::PriceConditionNotMet);

    let clock = Clock::get()?;
    alert.active = false;
    alert.triggered_at = clock.unix_timestamp;

    emit!(AlertTriggered {
        owner: alert.owner,
        token_mint: alert.token_mint,
        alert_id: alert.alert_id,
        target_price: alert.target_price,
        current_price,
        direction: alert.direction,
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}
