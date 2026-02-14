use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer, Mint, MintTo, Burn};
use anchor_spl::associated_token::AssociatedToken;

declare_id!("SenditFundTkns1111111111111111111111111111111");

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

pub const FUND_CONFIG_SEED: &[u8] = b"fund_config";
pub const FUND_SHARE_MINT_SEED: &[u8] = b"fund_share_mint";
pub const FUND_VAULT_SEED: &[u8] = b"fund_vault";
pub const FUND_SOL_VAULT_SEED: &[u8] = b"fund_sol_vault";
pub const USER_FUND_POSITION_SEED: &[u8] = b"user_fund_position";

pub const MAX_FUND_NAME_LEN: usize = 32;
pub const MAX_BASKET_SIZE: usize = 10;
pub const SHARE_DECIMALS: u8 = 6;
pub const BPS_DENOMINATOR: u64 = 10_000;
pub const MAX_MANAGEMENT_FEE_BPS: u16 = 500; // 5%

// ---------------------------------------------------------------------------
// Accounts
// ---------------------------------------------------------------------------

/// On-chain configuration for a fund / index token.
///
/// Stores the basket composition (token mints + weights), fee parameters,
/// and cumulative deposit tracking. Derived as a PDA from the fund name.
#[account]
pub struct FundConfig {
    /// Human-readable fund name (unique, used in PDA seed).
    pub name: String,
    /// Creator authority – can rebalance and collect fees.
    pub creator: Pubkey,
    /// SPL token mints that make up the basket (max 10).
    pub token_mints: Vec<Pubkey>,
    /// Corresponding weights in basis points (must sum to 10 000).
    pub weights_bps: Vec<u16>,
    /// Total SOL deposited into this fund (lifetime).
    pub total_deposits_sol: u64,
    /// Annualised management fee in basis points.
    pub management_fee_bps: u16,
    /// Whether the fund is currently accepting deposits.
    pub active: bool,
    /// Share mint address (the fund-share SPL token).
    pub share_mint: Pubkey,
    /// Creation timestamp.
    pub created_at: i64,
    /// Bump for this PDA.
    pub bump: u8,
    /// Bump for the share mint PDA.
    pub share_mint_bump: u8,
}

impl FundConfig {
    pub const SIZE: usize = 8 // discriminator
        + (4 + MAX_FUND_NAME_LEN)               // name
        + 32                                      // creator
        + (4 + MAX_BASKET_SIZE * 32)             // token_mints vec
        + (4 + MAX_BASKET_SIZE * 2)              // weights_bps vec
        + 8                                       // total_deposits_sol
        + 2                                       // management_fee_bps
        + 1                                       // active
        + 32                                      // share_mint
        + 8                                       // created_at
        + 1                                       // bump
        + 1;                                      // share_mint_bump
}

/// Per-user position tracking for a specific fund.
#[account]
pub struct UserFundPosition {
    /// User wallet.
    pub user: Pubkey,
    /// Fund config address.
    pub fund: Pubkey,
    /// Total shares held (mirrors token account but useful for analytics).
    pub shares_held: u64,
    /// Cumulative SOL deposited by this user.
    pub total_deposited_sol: u64,
    /// Cumulative SOL value redeemed by this user.
    pub total_redeemed_sol: u64,
    /// Timestamp of first deposit.
    pub first_deposit_at: i64,
    /// Bump seed.
    pub bump: u8,
}

impl UserFundPosition {
    pub const SIZE: usize = 8 + 32 + 32 + 8 + 8 + 8 + 8 + 1;
}

// ---------------------------------------------------------------------------
// Instructions
// ---------------------------------------------------------------------------

/// Create a new fund with a basket of SPL tokens and their weights.
///
/// The creator defines:
/// - A unique fund name
/// - Up to 10 SPL token mints forming the basket
/// - Corresponding weights in basis points (must sum to 10 000)
/// - A management fee (max 5%)
///
/// A fund-share SPL mint is created as a PDA owned by the fund.
pub fn create_fund(
    ctx: Context<CreateFund>,
    name: String,
    token_mints: Vec<Pubkey>,
    weights_bps: Vec<u16>,
    management_fee_bps: u16,
) -> Result<()> {
    require!(name.len() > 0 && name.len() <= MAX_FUND_NAME_LEN, FundError::InvalidName);
    require!(
        token_mints.len() > 0 && token_mints.len() <= MAX_BASKET_SIZE,
        FundError::InvalidBasketSize
    );
    require!(token_mints.len() == weights_bps.len(), FundError::MintWeightMismatch);
    require!(management_fee_bps <= MAX_MANAGEMENT_FEE_BPS, FundError::FeeTooHigh);

    // Weights must sum to 10 000 bps (100%).
    let weight_sum: u64 = weights_bps.iter().map(|w| *w as u64).sum();
    require!(weight_sum == BPS_DENOMINATOR, FundError::WeightsSumInvalid);

    // Ensure no duplicate mints.
    for i in 0..token_mints.len() {
        for j in (i + 1)..token_mints.len() {
            require!(token_mints[i] != token_mints[j], FundError::DuplicateMint);
        }
    }

    let clock = Clock::get()?;
    let fund = &mut ctx.accounts.fund_config;

    fund.name = name.clone();
    fund.creator = ctx.accounts.creator.key();
    fund.token_mints = token_mints;
    fund.weights_bps = weights_bps;
    fund.total_deposits_sol = 0;
    fund.management_fee_bps = management_fee_bps;
    fund.active = true;
    fund.share_mint = ctx.accounts.share_mint.key();
    fund.created_at = clock.unix_timestamp;
    fund.bump = ctx.bumps.fund_config;
    fund.share_mint_bump = ctx.bumps.share_mint;

    emit!(FundCreated {
        fund: fund.key(),
        name,
        creator: fund.creator,
        share_mint: fund.share_mint,
        num_tokens: fund.token_mints.len() as u8,
        management_fee_bps,
        created_at: clock.unix_timestamp,
    });

    Ok(())
}

/// Deposit SOL into a fund and receive proportional fund shares.
///
/// The deposited SOL is held in the fund's SOL vault. Shares are minted
/// 1:1 with lamports on the first deposit; subsequent deposits mint shares
/// proportional to the existing share supply vs. vault balance.
pub fn deposit_to_fund(ctx: Context<DepositToFund>, sol_amount: u64) -> Result<()> {
    require!(sol_amount > 0, FundError::ZeroAmount);

    let fund = &ctx.accounts.fund_config;
    require!(fund.active, FundError::FundInactive);

    // Calculate shares to mint.
    // If supply == 0 → shares = sol_amount (bootstrap).
    // Otherwise → shares = sol_amount * total_supply / vault_balance.
    let current_supply = ctx.accounts.share_mint.supply;
    let vault_balance = ctx.accounts.fund_sol_vault.lamports();

    let shares_to_mint: u64 = if current_supply == 0 || vault_balance == 0 {
        sol_amount
    } else {
        (sol_amount as u128)
            .checked_mul(current_supply as u128)
            .ok_or(FundError::MathOverflow)?
            .checked_div(vault_balance as u128)
            .ok_or(FundError::MathOverflow)? as u64
    };

    require!(shares_to_mint > 0, FundError::InsufficientOutput);

    // Transfer SOL from depositor to fund SOL vault.
    anchor_lang::system_program::transfer(
        CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            anchor_lang::system_program::Transfer {
                from: ctx.accounts.depositor.to_account_info(),
                to: ctx.accounts.fund_sol_vault.to_account_info(),
            },
        ),
        sol_amount,
    )?;

    // Mint fund shares to depositor.
    let fund_name = ctx.accounts.fund_config.name.as_bytes();
    let bump = ctx.accounts.fund_config.bump;
    let signer_seeds: &[&[u8]] = &[FUND_CONFIG_SEED, fund_name, &[bump]];

    token::mint_to(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            MintTo {
                mint: ctx.accounts.share_mint.to_account_info(),
                to: ctx.accounts.depositor_share_account.to_account_info(),
                authority: ctx.accounts.fund_config.to_account_info(),
            },
            &[signer_seeds],
        ),
        shares_to_mint,
    )?;

    // Update fund state.
    let fund = &mut ctx.accounts.fund_config;
    fund.total_deposits_sol = fund
        .total_deposits_sol
        .checked_add(sol_amount)
        .ok_or(FundError::MathOverflow)?;

    // Update user position.
    let clock = Clock::get()?;
    let position = &mut ctx.accounts.user_position;
    if position.first_deposit_at == 0 {
        position.user = ctx.accounts.depositor.key();
        position.fund = ctx.accounts.fund_config.key();
        position.first_deposit_at = clock.unix_timestamp;
        position.bump = ctx.bumps.user_position;
    }
    position.shares_held = position
        .shares_held
        .checked_add(shares_to_mint)
        .ok_or(FundError::MathOverflow)?;
    position.total_deposited_sol = position
        .total_deposited_sol
        .checked_add(sol_amount)
        .ok_or(FundError::MathOverflow)?;

    emit!(DepositedToFund {
        fund: ctx.accounts.fund_config.key(),
        depositor: ctx.accounts.depositor.key(),
        sol_amount,
        shares_minted: shares_to_mint,
        total_supply_after: current_supply.checked_add(shares_to_mint).unwrap_or(shares_to_mint),
    });

    Ok(())
}

/// Redeem (burn) fund shares and receive proportional SOL from the vault.
///
/// The SOL returned is: `share_amount * vault_balance / total_supply`.
/// A management fee is deducted and sent to the creator.
pub fn redeem_shares(ctx: Context<RedeemShares>, share_amount: u64) -> Result<()> {
    require!(share_amount > 0, FundError::ZeroAmount);

    let total_supply = ctx.accounts.share_mint.supply;
    require!(total_supply > 0, FundError::NoSharesOutstanding);

    let vault_balance = ctx.accounts.fund_sol_vault.lamports();

    // Gross SOL entitlement.
    let gross_sol: u64 = (share_amount as u128)
        .checked_mul(vault_balance as u128)
        .ok_or(FundError::MathOverflow)?
        .checked_div(total_supply as u128)
        .ok_or(FundError::MathOverflow)? as u64;

    require!(gross_sol > 0, FundError::InsufficientOutput);

    // Management fee.
    let fee = (gross_sol as u128)
        .checked_mul(ctx.accounts.fund_config.management_fee_bps as u128)
        .ok_or(FundError::MathOverflow)?
        .checked_div(BPS_DENOMINATOR as u128)
        .ok_or(FundError::MathOverflow)? as u64;

    let net_sol = gross_sol.checked_sub(fee).ok_or(FundError::MathOverflow)?;

    // Burn shares from redeemer.
    token::burn(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            Burn {
                mint: ctx.accounts.share_mint.to_account_info(),
                from: ctx.accounts.redeemer_share_account.to_account_info(),
                authority: ctx.accounts.redeemer.to_account_info(),
            },
        ),
        share_amount,
    )?;

    // Transfer SOL from vault to redeemer (lamport manipulation – vault is PDA).
    **ctx
        .accounts
        .fund_sol_vault
        .to_account_info()
        .try_borrow_mut_lamports()? -= gross_sol;
    **ctx
        .accounts
        .redeemer
        .to_account_info()
        .try_borrow_mut_lamports()? += net_sol;

    // Fee to creator.
    if fee > 0 {
        **ctx
            .accounts
            .creator_wallet
            .to_account_info()
            .try_borrow_mut_lamports()? += fee;
    }

    // Update user position.
    let position = &mut ctx.accounts.user_position;
    position.shares_held = position.shares_held.saturating_sub(share_amount);
    position.total_redeemed_sol = position
        .total_redeemed_sol
        .checked_add(net_sol)
        .ok_or(FundError::MathOverflow)?;

    emit!(SharesRedeemed {
        fund: ctx.accounts.fund_config.key(),
        redeemer: ctx.accounts.redeemer.key(),
        shares_burned: share_amount,
        sol_received: net_sol,
        fee_collected: fee,
        total_supply_after: total_supply.checked_sub(share_amount).unwrap_or(0),
    });

    Ok(())
}

/// Rebalance fund weights. Creator-only.
///
/// Allows the fund creator to adjust the target allocation weights
/// without changing the underlying basket mints. Weights must still
/// sum to 10 000 bps.
pub fn rebalance_fund(ctx: Context<RebalanceFund>, new_weights_bps: Vec<u16>) -> Result<()> {
    let fund = &ctx.accounts.fund_config;

    require!(
        new_weights_bps.len() == fund.token_mints.len(),
        FundError::MintWeightMismatch
    );

    let weight_sum: u64 = new_weights_bps.iter().map(|w| *w as u64).sum();
    require!(weight_sum == BPS_DENOMINATOR, FundError::WeightsSumInvalid);

    let old_weights = fund.weights_bps.clone();

    let fund = &mut ctx.accounts.fund_config;
    fund.weights_bps = new_weights_bps.clone();

    emit!(FundRebalanced {
        fund: fund.key(),
        creator: ctx.accounts.creator.key(),
        old_weights_bps: old_weights,
        new_weights_bps,
    });

    Ok(())
}

/// Toggle fund active status. Creator-only.
pub fn set_fund_active(ctx: Context<RebalanceFund>, active: bool) -> Result<()> {
    let fund = &mut ctx.accounts.fund_config;
    fund.active = active;

    emit!(FundStatusChanged {
        fund: fund.key(),
        active,
    });

    Ok(())
}

// ---------------------------------------------------------------------------
// Context structs (Accounts)
// ---------------------------------------------------------------------------

#[derive(Accounts)]
#[instruction(name: String)]
pub struct CreateFund<'info> {
    #[account(
        init,
        payer = creator,
        space = FundConfig::SIZE,
        seeds = [FUND_CONFIG_SEED, name.as_bytes()],
        bump,
    )]
    pub fund_config: Account<'info, FundConfig>,

    /// Fund-share SPL mint. Authority = fund_config PDA.
    #[account(
        init,
        payer = creator,
        mint::decimals = SHARE_DECIMALS,
        mint::authority = fund_config,
        seeds = [FUND_SHARE_MINT_SEED, fund_config.key().as_ref()],
        bump,
    )]
    pub share_mint: Account<'info, Mint>,

    #[account(mut)]
    pub creator: Signer<'info>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct DepositToFund<'info> {
    #[account(
        mut,
        seeds = [FUND_CONFIG_SEED, fund_config.name.as_bytes()],
        bump = fund_config.bump,
    )]
    pub fund_config: Account<'info, FundConfig>,

    #[account(
        mut,
        seeds = [FUND_SHARE_MINT_SEED, fund_config.key().as_ref()],
        bump = fund_config.share_mint_bump,
    )]
    pub share_mint: Account<'info, Mint>,

    /// CHECK: Fund SOL vault PDA.
    #[account(
        mut,
        seeds = [FUND_SOL_VAULT_SEED, fund_config.key().as_ref()],
        bump,
    )]
    pub fund_sol_vault: AccountInfo<'info>,

    /// Depositor's associated token account for fund shares.
    #[account(
        init_if_needed,
        payer = depositor,
        associated_token::mint = share_mint,
        associated_token::authority = depositor,
    )]
    pub depositor_share_account: Account<'info, TokenAccount>,

    /// Per-user position tracker.
    #[account(
        init_if_needed,
        payer = depositor,
        space = UserFundPosition::SIZE,
        seeds = [USER_FUND_POSITION_SEED, depositor.key().as_ref(), fund_config.key().as_ref()],
        bump,
    )]
    pub user_position: Account<'info, UserFundPosition>,

    #[account(mut)]
    pub depositor: Signer<'info>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct RedeemShares<'info> {
    #[account(
        seeds = [FUND_CONFIG_SEED, fund_config.name.as_bytes()],
        bump = fund_config.bump,
    )]
    pub fund_config: Account<'info, FundConfig>,

    #[account(
        mut,
        seeds = [FUND_SHARE_MINT_SEED, fund_config.key().as_ref()],
        bump = fund_config.share_mint_bump,
    )]
    pub share_mint: Account<'info, Mint>,

    /// CHECK: Fund SOL vault PDA.
    #[account(
        mut,
        seeds = [FUND_SOL_VAULT_SEED, fund_config.key().as_ref()],
        bump,
    )]
    pub fund_sol_vault: AccountInfo<'info>,

    /// Redeemer's fund share token account.
    #[account(
        mut,
        associated_token::mint = share_mint,
        associated_token::authority = redeemer,
    )]
    pub redeemer_share_account: Account<'info, TokenAccount>,

    /// Per-user position tracker.
    #[account(
        mut,
        seeds = [USER_FUND_POSITION_SEED, redeemer.key().as_ref(), fund_config.key().as_ref()],
        bump = user_position.bump,
    )]
    pub user_position: Account<'info, UserFundPosition>,

    /// CHECK: Creator wallet to receive management fee.
    #[account(
        mut,
        constraint = creator_wallet.key() == fund_config.creator @ FundError::InvalidCreator,
    )]
    pub creator_wallet: AccountInfo<'info>,

    #[account(mut)]
    pub redeemer: Signer<'info>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct RebalanceFund<'info> {
    #[account(
        mut,
        seeds = [FUND_CONFIG_SEED, fund_config.name.as_bytes()],
        bump = fund_config.bump,
        has_one = creator,
    )]
    pub fund_config: Account<'info, FundConfig>,

    pub creator: Signer<'info>,
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

#[event]
pub struct FundCreated {
    pub fund: Pubkey,
    pub name: String,
    pub creator: Pubkey,
    pub share_mint: Pubkey,
    pub num_tokens: u8,
    pub management_fee_bps: u16,
    pub created_at: i64,
}

#[event]
pub struct DepositedToFund {
    pub fund: Pubkey,
    pub depositor: Pubkey,
    pub sol_amount: u64,
    pub shares_minted: u64,
    pub total_supply_after: u64,
}

#[event]
pub struct SharesRedeemed {
    pub fund: Pubkey,
    pub redeemer: Pubkey,
    pub shares_burned: u64,
    pub sol_received: u64,
    pub fee_collected: u64,
    pub total_supply_after: u64,
}

#[event]
pub struct FundRebalanced {
    pub fund: Pubkey,
    pub creator: Pubkey,
    pub old_weights_bps: Vec<u16>,
    pub new_weights_bps: Vec<u16>,
}

#[event]
pub struct FundStatusChanged {
    pub fund: Pubkey,
    pub active: bool,
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[error_code]
pub enum FundError {
    #[msg("Fund name must be 1-32 characters")]
    InvalidName,
    #[msg("Basket must contain 1-10 token mints")]
    InvalidBasketSize,
    #[msg("Number of mints must match number of weights")]
    MintWeightMismatch,
    #[msg("Weights must sum to 10000 (100%)")]
    WeightsSumInvalid,
    #[msg("Duplicate mint in basket")]
    DuplicateMint,
    #[msg("Management fee too high (max 5%)")]
    FeeTooHigh,
    #[msg("Amount must be greater than zero")]
    ZeroAmount,
    #[msg("Fund is not active")]
    FundInactive,
    #[msg("Insufficient output amount")]
    InsufficientOutput,
    #[msg("No shares outstanding")]
    NoSharesOutstanding,
    #[msg("Invalid creator")]
    InvalidCreator,
    #[msg("Math overflow")]
    MathOverflow,
}
