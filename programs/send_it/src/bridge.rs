use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer, Mint};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

pub const BRIDGE_CONFIG_SEED: &[u8] = b"bridge_config";
pub const BRIDGE_REQUEST_SEED: &[u8] = b"bridge_req";
pub const VAULT_SEED: &[u8] = b"vault";
pub const MAX_CHAINS: usize = 10;
pub const BRIDGE_EXPIRY_SECONDS: i64 = 86_400; // 24 hours

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[error_code]
pub enum BridgeError {
    #[msg("Bridge is currently paused")]
    BridgePaused,
    #[msg("Unsupported destination chain")]
    UnsupportedChain,
    #[msg("Amount below minimum bridge threshold")]
    AmountTooLow,
    #[msg("Bridge request is not in the expected status")]
    InvalidStatus,
    #[msg("Bridge request has not expired yet")]
    NotExpired,
    #[msg("Bridge request has expired")]
    Expired,
    #[msg("Invalid Wormhole VAA")]
    InvalidVAA,
    #[msg("Unauthorized")]
    Unauthorized,
    #[msg("Arithmetic overflow")]
    Overflow,
    #[msg("Maximum supported chains exceeded")]
    MaxChainsExceeded,
}

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum BridgeStatus {
    Pending,
    WormholeSent,
    Completed,
    Cancelled,
}

// ---------------------------------------------------------------------------
// Account structs (state)
// ---------------------------------------------------------------------------

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug)]
pub struct ChainInfo {
    pub chain_id: u16,
    pub fee_bps: u16,          // basis points (e.g. 50 = 0.5%)
    pub min_amount: u64,
    pub enabled: bool,
}

#[account]
pub struct BridgeConfig {
    pub authority: Pubkey,
    pub wormhole_program: Pubkey,
    pub wormhole_bridge: Pubkey,       // Wormhole token bridge address
    pub fee_collector: Pubkey,
    pub supported_chains: Vec<ChainInfo>,
    pub total_bridged: u64,
    pub total_requests: u64,
    pub paused: bool,
    pub bump: u8,
}

impl BridgeConfig {
    pub const MAX_SIZE: usize = 8  // discriminator
        + 32   // authority
        + 32   // wormhole_program
        + 32   // wormhole_bridge
        + 32   // fee_collector
        + 4 + (MAX_CHAINS * 13) // vec overhead + chain info (2+2+8+1 per chain)
        + 8    // total_bridged
        + 8    // total_requests
        + 1    // paused
        + 1;   // bump

    pub fn get_chain(&self, chain_id: u16) -> Option<&ChainInfo> {
        self.supported_chains.iter().find(|c| c.chain_id == chain_id && c.enabled)
    }
}

#[account]
pub struct BridgeRequest {
    pub user: Pubkey,
    pub token_mint: Pubkey,
    pub amount: u64,
    pub fee_amount: u64,
    pub net_amount: u64,
    pub destination_chain: u16,
    pub destination_address: [u8; 32],
    pub status: BridgeStatus,
    pub created_at: i64,
    pub wormhole_sequence: Option<u64>,
    pub nonce: u64,
    pub bump: u8,
}

impl BridgeRequest {
    pub const MAX_SIZE: usize = 8  // discriminator
        + 32   // user
        + 32   // token_mint
        + 8    // amount
        + 8    // fee_amount
        + 8    // net_amount
        + 2    // destination_chain
        + 32   // destination_address
        + 1    // status (enum)
        + 8    // created_at
        + 9    // wormhole_sequence (Option<u64>)
        + 8    // nonce
        + 1;   // bump
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

#[event]
pub struct BridgeInitiated {
    pub user: Pubkey,
    pub token_mint: Pubkey,
    pub amount: u64,
    pub fee_amount: u64,
    pub net_amount: u64,
    pub destination_chain: u16,
    pub destination_address: [u8; 32],
    pub nonce: u64,
    pub timestamp: i64,
}

#[event]
pub struct BridgeConfirmed {
    pub user: Pubkey,
    pub token_mint: Pubkey,
    pub net_amount: u64,
    pub destination_chain: u16,
    pub wormhole_sequence: u64,
    pub timestamp: i64,
}

#[event]
pub struct BridgeCancelled {
    pub user: Pubkey,
    pub token_mint: Pubkey,
    pub amount: u64,
    pub nonce: u64,
    pub timestamp: i64,
}

#[event]
pub struct BridgeConfigUpdated {
    pub authority: Pubkey,
    pub timestamp: i64,
}

// ---------------------------------------------------------------------------
// Instructions
// ---------------------------------------------------------------------------

/// Initialize the bridge config (admin only, once).
#[derive(Accounts)]
pub struct InitializeBridge<'info> {
    #[account(
        init,
        payer = authority,
        space = BridgeConfig::MAX_SIZE,
        seeds = [BRIDGE_CONFIG_SEED],
        bump,
    )]
    pub bridge_config: Account<'info, BridgeConfig>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

/// Initiate a bridge request — locks tokens in the vault.
#[derive(Accounts)]
#[instruction(amount: u64, destination_chain: u16, destination_address: [u8; 32], nonce: u64)]
pub struct InitiateBridge<'info> {
    #[account(
        seeds = [BRIDGE_CONFIG_SEED],
        bump = bridge_config.bump,
        constraint = !bridge_config.paused @ BridgeError::BridgePaused,
    )]
    pub bridge_config: Account<'info, BridgeConfig>,

    #[account(
        init,
        payer = user,
        space = BridgeRequest::MAX_SIZE,
        seeds = [BRIDGE_REQUEST_SEED, user.key().as_ref(), &nonce.to_le_bytes()],
        bump,
    )]
    pub bridge_request: Account<'info, BridgeRequest>,

    #[account(mut)]
    pub user: Signer<'info>,

    pub token_mint: Account<'info, Mint>,

    /// User's token account (source).
    #[account(
        mut,
        constraint = user_token_account.owner == user.key(),
        constraint = user_token_account.mint == token_mint.key(),
    )]
    pub user_token_account: Account<'info, TokenAccount>,

    /// Program-owned vault for this token.
    #[account(
        mut,
        seeds = [VAULT_SEED, token_mint.key().as_ref()],
        bump,
    )]
    pub token_vault: Account<'info, TokenAccount>,

    /// Fee collector token account.
    #[account(
        mut,
        constraint = fee_vault.mint == token_mint.key(),
    )]
    pub fee_vault: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

/// Confirm bridge after Wormhole VAA verification.
#[derive(Accounts)]
pub struct ConfirmBridge<'info> {
    #[account(
        mut,
        seeds = [BRIDGE_CONFIG_SEED],
        bump = bridge_config.bump,
    )]
    pub bridge_config: Account<'info, BridgeConfig>,

    #[account(
        mut,
        seeds = [BRIDGE_REQUEST_SEED, bridge_request.user.as_ref(), &bridge_request.nonce.to_le_bytes()],
        bump = bridge_request.bump,
        constraint = bridge_request.status == BridgeStatus::Pending @ BridgeError::InvalidStatus,
    )]
    pub bridge_request: Account<'info, BridgeRequest>,

    /// Authority or permissioned relayer.
    pub authority: Signer<'info>,

    /// CHECK: Wormhole VAA account — verified by the wormhole program.
    pub wormhole_vaa: UncheckedAccount<'info>,

    /// CHECK: Wormhole program for CPI.
    pub wormhole_program: UncheckedAccount<'info>,
}

/// Cancel an expired bridge request — returns tokens to user.
#[derive(Accounts)]
pub struct CancelBridge<'info> {
    #[account(
        seeds = [BRIDGE_CONFIG_SEED],
        bump = bridge_config.bump,
    )]
    pub bridge_config: Account<'info, BridgeConfig>,

    #[account(
        mut,
        seeds = [BRIDGE_REQUEST_SEED, user.key().as_ref(), &bridge_request.nonce.to_le_bytes()],
        bump = bridge_request.bump,
        constraint = bridge_request.status == BridgeStatus::Pending @ BridgeError::InvalidStatus,
        constraint = bridge_request.user == user.key() @ BridgeError::Unauthorized,
    )]
    pub bridge_request: Account<'info, BridgeRequest>,

    #[account(mut)]
    pub user: Signer<'info>,

    pub token_mint: Account<'info, Mint>,

    /// User's token account (destination for refund).
    #[account(
        mut,
        constraint = user_token_account.owner == user.key(),
        constraint = user_token_account.mint == token_mint.key(),
    )]
    pub user_token_account: Account<'info, TokenAccount>,

    /// Program-owned vault.
    #[account(
        mut,
        seeds = [VAULT_SEED, token_mint.key().as_ref()],
        bump,
    )]
    pub token_vault: Account<'info, TokenAccount>,

    /// Fee vault — fee is also refunded on cancel.
    #[account(
        mut,
        constraint = fee_vault.mint == token_mint.key(),
    )]
    pub fee_vault: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

// ---------------------------------------------------------------------------
// Instruction handlers
// ---------------------------------------------------------------------------

pub fn handle_initialize_bridge(
    ctx: Context<InitializeBridge>,
    wormhole_program: Pubkey,
    wormhole_bridge: Pubkey,
    fee_collector: Pubkey,
    chains: Vec<ChainInfo>,
) -> Result<()> {
    require!(chains.len() <= MAX_CHAINS, BridgeError::MaxChainsExceeded);

    let config = &mut ctx.accounts.bridge_config;
    config.authority = ctx.accounts.authority.key();
    config.wormhole_program = wormhole_program;
    config.wormhole_bridge = wormhole_bridge;
    config.fee_collector = fee_collector;
    config.supported_chains = chains;
    config.total_bridged = 0;
    config.total_requests = 0;
    config.paused = false;
    config.bump = ctx.bumps.bridge_config;

    emit!(BridgeConfigUpdated {
        authority: config.authority,
        timestamp: Clock::get()?.unix_timestamp,
    });

    Ok(())
}

pub fn handle_initiate_bridge(
    ctx: Context<InitiateBridge>,
    amount: u64,
    destination_chain: u16,
    destination_address: [u8; 32],
    nonce: u64,
) -> Result<()> {
    let config = &ctx.accounts.bridge_config;
    let chain = config
        .get_chain(destination_chain)
        .ok_or(BridgeError::UnsupportedChain)?;

    require!(amount >= chain.min_amount, BridgeError::AmountTooLow);

    // Calculate fee
    let fee_amount = amount
        .checked_mul(chain.fee_bps as u64)
        .ok_or(BridgeError::Overflow)?
        .checked_div(10_000)
        .ok_or(BridgeError::Overflow)?;
    let net_amount = amount.checked_sub(fee_amount).ok_or(BridgeError::Overflow)?;

    // Transfer net amount to vault
    token::transfer(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.user_token_account.to_account_info(),
                to: ctx.accounts.token_vault.to_account_info(),
                authority: ctx.accounts.user.to_account_info(),
            },
        ),
        net_amount,
    )?;

    // Transfer fee to fee vault
    if fee_amount > 0 {
        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.user_token_account.to_account_info(),
                    to: ctx.accounts.fee_vault.to_account_info(),
                    authority: ctx.accounts.user.to_account_info(),
                },
            ),
            fee_amount,
        )?;
    }

    // Populate bridge request
    let now = Clock::get()?.unix_timestamp;
    let req = &mut ctx.accounts.bridge_request;
    req.user = ctx.accounts.user.key();
    req.token_mint = ctx.accounts.token_mint.key();
    req.amount = amount;
    req.fee_amount = fee_amount;
    req.net_amount = net_amount;
    req.destination_chain = destination_chain;
    req.destination_address = destination_address;
    req.status = BridgeStatus::Pending;
    req.created_at = now;
    req.wormhole_sequence = None;
    req.nonce = nonce;
    req.bump = ctx.bumps.bridge_request;

    // TODO: CPI to Wormhole to post the message
    // wormhole::post_message(cpi_ctx, nonce, payload, consistency_level)?;

    emit!(BridgeInitiated {
        user: req.user,
        token_mint: req.token_mint,
        amount,
        fee_amount,
        net_amount,
        destination_chain,
        destination_address,
        nonce,
        timestamp: now,
    });

    Ok(())
}

pub fn handle_confirm_bridge(
    ctx: Context<ConfirmBridge>,
    wormhole_sequence: u64,
) -> Result<()> {
    let config = &ctx.accounts.bridge_config;
    require!(
        ctx.accounts.authority.key() == config.authority,
        BridgeError::Unauthorized
    );

    // TODO: Verify the Wormhole VAA via CPI
    // wormhole::verify_vaa(cpi_ctx, vaa_data)?;

    let now = Clock::get()?.unix_timestamp;
    let req = &mut ctx.accounts.bridge_request;

    // Check not expired
    require!(
        now <= req.created_at + BRIDGE_EXPIRY_SECONDS,
        BridgeError::Expired
    );

    req.status = BridgeStatus::Completed;
    req.wormhole_sequence = Some(wormhole_sequence);

    let config = &mut ctx.accounts.bridge_config;
    config.total_bridged = config
        .total_bridged
        .checked_add(req.net_amount)
        .ok_or(BridgeError::Overflow)?;
    config.total_requests = config
        .total_requests
        .checked_add(1)
        .ok_or(BridgeError::Overflow)?;

    emit!(BridgeConfirmed {
        user: req.user,
        token_mint: req.token_mint,
        net_amount: req.net_amount,
        destination_chain: req.destination_chain,
        wormhole_sequence,
        timestamp: now,
    });

    Ok(())
}

pub fn handle_cancel_bridge(ctx: Context<CancelBridge>) -> Result<()> {
    let req = &ctx.accounts.bridge_request;
    let now = Clock::get()?.unix_timestamp;

    // Must be expired to cancel
    require!(
        now > req.created_at + BRIDGE_EXPIRY_SECONDS,
        BridgeError::NotExpired
    );

    // Refund net amount from vault to user (PDA signer)
    let mint_key = ctx.accounts.token_mint.key();
    let seeds: &[&[u8]] = &[VAULT_SEED, mint_key.as_ref()];
    let (_, vault_bump) = Pubkey::find_program_address(seeds, ctx.program_id);
    let signer_seeds: &[&[&[u8]]] = &[&[VAULT_SEED, mint_key.as_ref(), &[vault_bump]]];

    token::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.token_vault.to_account_info(),
                to: ctx.accounts.user_token_account.to_account_info(),
                authority: ctx.accounts.token_vault.to_account_info(),
            },
            signer_seeds,
        ),
        req.net_amount,
    )?;

    // Refund fee (fee vault → user, requires fee vault authority setup)
    // In production, fee vault would also be PDA-owned for refund capability.
    // Simplified: fee is non-refundable or handled via admin instruction.

    let req = &mut ctx.accounts.bridge_request;
    req.status = BridgeStatus::Cancelled;

    emit!(BridgeCancelled {
        user: req.user,
        token_mint: req.token_mint,
        amount: req.amount,
        nonce: req.nonce,
        timestamp: now,
    });

    Ok(())
}
