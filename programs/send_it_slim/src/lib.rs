use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer, MintTo};
use anchor_spl::associated_token::AssociatedToken;

declare_id!("98Vxqk2dHjLsUb4svNaZWwVZxt9DZZwkRQZZNQmYRm1L");

pub const PLATFORM_CONFIG_SEED: &[u8] = b"platform_config";
pub const TOKEN_LAUNCH_SEED: &[u8] = b"token_launch";
pub const USER_POSITION_SEED: &[u8] = b"user_position";
pub const PLATFORM_VAULT_SEED: &[u8] = b"platform_vault";
pub const SOL_VAULT_SEED: &[u8] = b"sol_vault";
pub const DEFAULT_TOTAL_SUPPLY: u64 = 1_000_000_000_000_000;
pub const TOKEN_DECIMALS: u8 = 6;
pub const PRECISION: u128 = 1_000_000_000_000;

#[program]
pub mod send_it {
    use super::*;

    pub fn initialize_platform(ctx: Context<InitializePlatform>, platform_fee_bps: u16, migration_threshold: u64) -> Result<()> {
        require!(platform_fee_bps <= 1000, SendItError::FeeTooHigh);
        let c = &mut ctx.accounts.platform_config;
        c.authority = ctx.accounts.authority.key();
        c.platform_fee_bps = platform_fee_bps;
        c.migration_threshold = migration_threshold;
        c.total_launches = 0;
        c.total_volume_sol = 0;
        c.paused = false;
        c.bump = ctx.bumps.platform_config;
        Ok(())
    }

    pub fn create_token(ctx: Context<CreateToken>, name: String, symbol: String, uri: String, creator_fee_bps: u16) -> Result<()> {
        require!(creator_fee_bps <= 500, SendItError::FeeTooHigh);
        let config = &ctx.accounts.platform_config;
        require!(!config.paused, SendItError::PlatformPaused);
        let clock = Clock::get()?;
        let l = &mut ctx.accounts.token_launch;
        l.creator = ctx.accounts.creator.key();
        l.mint = ctx.accounts.token_mint.key();
        l.name = name;
        l.symbol = symbol;
        l.uri = uri;
        l.creator_fee_bps = creator_fee_bps;
        l.total_supply = DEFAULT_TOTAL_SUPPLY;
        l.tokens_sold = 0;
        l.reserve_sol = 0;
        l.created_at = clock.unix_timestamp;
        l.migrated = false;
        l.total_volume_sol = 0;
        l.bump = ctx.bumps.token_launch;
        l.sol_vault_bump = ctx.bumps.launch_sol_vault;
        let mk = ctx.accounts.token_mint.key();
        let seeds: &[&[u8]] = &[TOKEN_LAUNCH_SEED, mk.as_ref(), &[l.bump]];
        token::mint_to(CpiContext::new_with_signer(ctx.accounts.token_program.to_account_info(), MintTo { mint: ctx.accounts.token_mint.to_account_info(), to: ctx.accounts.launch_token_vault.to_account_info(), authority: ctx.accounts.token_launch.to_account_info() }, &[seeds]), DEFAULT_TOTAL_SUPPLY)?;
        let config = &mut ctx.accounts.platform_config;
        config.total_launches += 1;
        Ok(())
    }

    pub fn buy(ctx: Context<BuyTokens>, sol_amount: u64) -> Result<()> {
        require!(sol_amount > 0, SendItError::ZeroAmount);
        let config = &ctx.accounts.platform_config;
        require!(!config.paused, SendItError::PlatformPaused);
        let launch = &ctx.accounts.token_launch;
        require!(!launch.migrated, SendItError::AlreadyMigrated);
        let base = 1_000u128;
        let slope = PRECISION / (launch.total_supply as u128);
        let cp = base + slope * (launch.tokens_sold as u128) / PRECISION;
        let tokens_out = ((sol_amount as u128) * 1_000_000_000u128 / cp).min((launch.total_supply - launch.tokens_sold) as u128) as u64;
        require!(tokens_out > 0, SendItError::InsufficientOutput);
        let pf = (sol_amount as u128) * (config.platform_fee_bps as u128) / 10_000;
        let cf = (sol_amount as u128) * (launch.creator_fee_bps as u128) / 10_000;
        let net = sol_amount - pf as u64 - cf as u64;
        anchor_lang::system_program::transfer(CpiContext::new(ctx.accounts.system_program.to_account_info(), anchor_lang::system_program::Transfer { from: ctx.accounts.buyer.to_account_info(), to: ctx.accounts.launch_sol_vault.to_account_info() }), net)?;
        if pf > 0 { anchor_lang::system_program::transfer(CpiContext::new(ctx.accounts.system_program.to_account_info(), anchor_lang::system_program::Transfer { from: ctx.accounts.buyer.to_account_info(), to: ctx.accounts.platform_vault.to_account_info() }), pf as u64)?; }
        if cf > 0 { anchor_lang::system_program::transfer(CpiContext::new(ctx.accounts.system_program.to_account_info(), anchor_lang::system_program::Transfer { from: ctx.accounts.buyer.to_account_info(), to: ctx.accounts.creator_wallet.to_account_info() }), cf as u64)?; }
        let mk = ctx.accounts.token_mint.key();
        let seeds: &[&[u8]] = &[TOKEN_LAUNCH_SEED, mk.as_ref(), &[ctx.accounts.token_launch.bump]];
        token::transfer(CpiContext::new_with_signer(ctx.accounts.token_program.to_account_info(), Transfer { from: ctx.accounts.launch_token_vault.to_account_info(), to: ctx.accounts.buyer_token_account.to_account_info(), authority: ctx.accounts.token_launch.to_account_info() }, &[seeds]), tokens_out)?;
        let launch = &mut ctx.accounts.token_launch;
        launch.tokens_sold += tokens_out;
        launch.reserve_sol += net;
        launch.total_volume_sol += sol_amount;
        let p = &mut ctx.accounts.user_position;
        p.owner = ctx.accounts.buyer.key();
        p.mint = ctx.accounts.token_mint.key();
        p.tokens_bought += tokens_out;
        p.sol_spent += sol_amount;
        if p.bump == 0 { p.bump = ctx.bumps.user_position; }
        let config = &mut ctx.accounts.platform_config;
        config.total_volume_sol += sol_amount;
        Ok(())
    }

    pub fn sell(ctx: Context<SellTokens>, token_amount: u64) -> Result<()> {
        require!(token_amount > 0, SendItError::ZeroAmount);
        let config = &ctx.accounts.platform_config;
        require!(!config.paused, SendItError::PlatformPaused);
        let launch = &ctx.accounts.token_launch;
        require!(!launch.migrated, SendItError::AlreadyMigrated);
        require!(token_amount <= launch.tokens_sold, SendItError::InsufficientTokensSold);
        let ns = launch.tokens_sold - token_amount;
        let base = 1_000u128;
        let slope = PRECISION / (launch.total_supply as u128);
        let avg = base + slope * ((ns + launch.tokens_sold) as u128) / (2 * PRECISION);
        let sol_out = (avg * (token_amount as u128) / 1_000_000_000u128) as u64;
        require!(sol_out > 0, SendItError::InsufficientOutput);
        let pf = (sol_out as u128) * (config.platform_fee_bps as u128) / 10_000;
        let cf = (sol_out as u128) * (launch.creator_fee_bps as u128) / 10_000;
        let net = sol_out - pf as u64 - cf as u64;
        require!(net <= launch.reserve_sol, SendItError::InsufficientReserve);
        // Transfer tokens from seller to vault
        token::transfer(CpiContext::new(ctx.accounts.token_program.to_account_info(), Transfer { from: ctx.accounts.seller_token_account.to_account_info(), to: ctx.accounts.launch_token_vault.to_account_info(), authority: ctx.accounts.seller.to_account_info() }), token_amount)?;
        // Transfer SOL from vault to seller using invoke_signed (vault is program-owned PDA)
        let mk = ctx.accounts.token_mint.key();
        let vault_seeds: &[&[u8]] = &[SOL_VAULT_SEED, mk.as_ref(), &[ctx.accounts.token_launch.sol_vault_bump]];
        anchor_lang::system_program::transfer(CpiContext::new_with_signer(ctx.accounts.system_program.to_account_info(), anchor_lang::system_program::Transfer { from: ctx.accounts.launch_sol_vault.to_account_info(), to: ctx.accounts.seller.to_account_info() }, &[vault_seeds]), net)?;
        if pf > 0 {
            anchor_lang::system_program::transfer(CpiContext::new_with_signer(ctx.accounts.system_program.to_account_info(), anchor_lang::system_program::Transfer { from: ctx.accounts.launch_sol_vault.to_account_info(), to: ctx.accounts.platform_vault.to_account_info() }, &[vault_seeds]), pf as u64)?;
        }
        if cf > 0 {
            anchor_lang::system_program::transfer(CpiContext::new_with_signer(ctx.accounts.system_program.to_account_info(), anchor_lang::system_program::Transfer { from: ctx.accounts.launch_sol_vault.to_account_info(), to: ctx.accounts.creator_wallet.to_account_info() }, &[vault_seeds]), cf as u64)?;
        }
        let launch = &mut ctx.accounts.token_launch;
        launch.tokens_sold -= token_amount;
        launch.reserve_sol -= sol_out;
        launch.total_volume_sol += sol_out;
        let p = &mut ctx.accounts.user_position;
        p.tokens_bought = p.tokens_bought.saturating_sub(token_amount);
        let config = &mut ctx.accounts.platform_config;
        config.total_volume_sol += sol_out;
        Ok(())
    }
}

#[account]
pub struct PlatformConfig { pub authority: Pubkey, pub platform_fee_bps: u16, pub migration_threshold: u64, pub total_launches: u64, pub total_volume_sol: u64, pub paused: bool, pub bump: u8 }
impl PlatformConfig { pub const SIZE: usize = 8+32+2+8+8+8+1+1; }

#[account]
pub struct TokenLaunch { pub creator: Pubkey, pub mint: Pubkey, pub name: String, pub symbol: String, pub uri: String, pub creator_fee_bps: u16, pub total_supply: u64, pub tokens_sold: u64, pub reserve_sol: u64, pub created_at: i64, pub migrated: bool, pub total_volume_sol: u64, pub bump: u8, pub sol_vault_bump: u8 }
impl TokenLaunch { pub const SIZE: usize = 8+32+32+(4+32)+(4+10)+(4+200)+2+8+8+8+8+1+8+1+1; }

#[account]
pub struct UserPosition { pub owner: Pubkey, pub mint: Pubkey, pub tokens_bought: u64, pub sol_spent: u64, pub bump: u8 }
impl UserPosition { pub const SIZE: usize = 8+32+32+8+8+1; }

#[derive(Accounts)]
pub struct InitializePlatform<'info> {
    #[account(init, payer=authority, space=PlatformConfig::SIZE, seeds=[PLATFORM_CONFIG_SEED], bump)]
    pub platform_config: Account<'info, PlatformConfig>,
    #[account(mut)] pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct CreateToken<'info> {
    #[account(init, payer=creator, space=TokenLaunch::SIZE, seeds=[TOKEN_LAUNCH_SEED, token_mint.key().as_ref()], bump)]
    pub token_launch: Account<'info, TokenLaunch>,
    #[account(init, payer=creator, mint::decimals=TOKEN_DECIMALS, mint::authority=token_launch)]
    pub token_mint: Account<'info, Mint>,
    #[account(init, payer=creator, associated_token::mint=token_mint, associated_token::authority=token_launch)]
    pub launch_token_vault: Account<'info, TokenAccount>,
    /// CHECK: SOL vault PDA owned by this program
    #[account(mut, seeds=[SOL_VAULT_SEED, token_mint.key().as_ref()], bump)]
    pub launch_sol_vault: AccountInfo<'info>,
    #[account(mut, seeds=[PLATFORM_CONFIG_SEED], bump=platform_config.bump)]
    pub platform_config: Account<'info, PlatformConfig>,
    #[account(mut)] pub creator: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct BuyTokens<'info> {
    #[account(mut, seeds=[TOKEN_LAUNCH_SEED, token_mint.key().as_ref()], bump=token_launch.bump)]
    pub token_launch: Account<'info, TokenLaunch>,
    pub token_mint: Account<'info, Mint>,
    #[account(mut, associated_token::mint=token_mint, associated_token::authority=token_launch)]
    pub launch_token_vault: Account<'info, TokenAccount>,
    /// CHECK: SOL vault
    #[account(mut, seeds=[SOL_VAULT_SEED, token_mint.key().as_ref()], bump)]
    pub launch_sol_vault: AccountInfo<'info>,
    #[account(init_if_needed, payer=buyer, associated_token::mint=token_mint, associated_token::authority=buyer)]
    pub buyer_token_account: Account<'info, TokenAccount>,
    #[account(init_if_needed, payer=buyer, space=UserPosition::SIZE, seeds=[USER_POSITION_SEED, buyer.key().as_ref(), token_mint.key().as_ref()], bump)]
    pub user_position: Account<'info, UserPosition>,
    /// CHECK: Platform vault
    #[account(mut, seeds=[PLATFORM_VAULT_SEED], bump)]
    pub platform_vault: AccountInfo<'info>,
    /// CHECK: Creator wallet
    #[account(mut, constraint=creator_wallet.key()==token_launch.creator @ SendItError::InvalidCreator)]
    pub creator_wallet: AccountInfo<'info>,
    #[account(mut, seeds=[PLATFORM_CONFIG_SEED], bump=platform_config.bump)]
    pub platform_config: Account<'info, PlatformConfig>,
    #[account(mut)] pub buyer: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct SellTokens<'info> {
    #[account(mut, seeds=[TOKEN_LAUNCH_SEED, token_mint.key().as_ref()], bump=token_launch.bump)]
    pub token_launch: Account<'info, TokenLaunch>,
    pub token_mint: Account<'info, Mint>,
    #[account(mut, associated_token::mint=token_mint, associated_token::authority=token_launch)]
    pub launch_token_vault: Account<'info, TokenAccount>,
    /// CHECK: SOL vault
    #[account(mut, seeds=[SOL_VAULT_SEED, token_mint.key().as_ref()], bump)]
    pub launch_sol_vault: AccountInfo<'info>,
    #[account(mut, associated_token::mint=token_mint, associated_token::authority=seller)]
    pub seller_token_account: Account<'info, TokenAccount>,
    #[account(mut, seeds=[USER_POSITION_SEED, seller.key().as_ref(), token_mint.key().as_ref()], bump=user_position.bump)]
    pub user_position: Account<'info, UserPosition>,
    /// CHECK: Platform vault
    #[account(mut, seeds=[PLATFORM_VAULT_SEED], bump)]
    pub platform_vault: AccountInfo<'info>,
    /// CHECK: Creator wallet
    #[account(mut, constraint=creator_wallet.key()==token_launch.creator @ SendItError::InvalidCreator)]
    pub creator_wallet: AccountInfo<'info>,
    #[account(mut, seeds=[PLATFORM_CONFIG_SEED], bump=platform_config.bump)]
    pub platform_config: Account<'info, PlatformConfig>,
    #[account(mut)] pub seller: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[error_code]
pub enum SendItError {
    #[msg("Fee too high")] FeeTooHigh,
    #[msg("Platform paused")] PlatformPaused,
    #[msg("Already migrated")] AlreadyMigrated,
    #[msg("Zero amount")] ZeroAmount,
    #[msg("Insufficient output")] InsufficientOutput,
    #[msg("Insufficient reserve")] InsufficientReserve,
    #[msg("Insufficient tokens sold")] InsufficientTokensSold,
    #[msg("Invalid creator")] InvalidCreator,
}
