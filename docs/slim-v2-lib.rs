use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer, MintTo};
use anchor_spl::associated_token::AssociatedToken;

declare_id!("98Vxqk2dHjLsUb4svNaZWwVZxt9DZZwkRQZZNQmYRm1L");

pub const PLATFORM_CONFIG_SEED: &[u8] = b"platform_config";
pub const TOKEN_LAUNCH_SEED: &[u8] = b"token_launch";
pub const USER_POSITION_SEED: &[u8] = b"user_position";
pub const PLATFORM_VAULT_SEED: &[u8] = b"platform_vault";
pub const SOL_VAULT_SEED: &[u8] = b"sol_vault";
pub const STAKE_SEED: &[u8] = b"stake";
pub const DEFAULT_TOTAL_SUPPLY: u64 = 1_000_000_000_000_000;
pub const TOKEN_DECIMALS: u8 = 6;
pub const PRECISION: u128 = 1_000_000_000_000;
pub const MIN_STAKE_DURATION: i64 = 60;
pub const MAX_STAKE_DURATION: i64 = 365 * 24 * 3600; // 1 year max
pub const MAX_NAME_LEN: usize = 32;
pub const MAX_SYMBOL_LEN: usize = 10;
pub const MAX_URI_LEN: usize = 200;
pub const RENT_EXEMPT_MIN: u64 = 890_880; // ~0.00089 SOL

pub const MPL_TOKEN_METADATA_ID: Pubkey = solana_program::pubkey!("metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s");

// ── Events ──

#[event]
pub struct TokenCreated {
    pub mint: Pubkey,
    pub creator: Pubkey,
    pub name: String,
    pub symbol: String,
    pub creator_fee_bps: u16,
    pub timestamp: i64,
}

#[event]
pub struct TokenBought {
    pub mint: Pubkey,
    pub buyer: Pubkey,
    pub sol_amount: u64,
    pub tokens_out: u64,
    pub price: u128,
}

#[event]
pub struct TokenSold {
    pub mint: Pubkey,
    pub seller: Pubkey,
    pub token_amount: u64,
    pub sol_out: u64,
}

#[event]
pub struct TokenStaked {
    pub mint: Pubkey,
    pub staker: Pubkey,
    pub amount: u64,
    pub unlock_at: i64,
}

#[event]
pub struct TokenUnstaked {
    pub mint: Pubkey,
    pub staker: Pubkey,
    pub amount: u64,
}

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

    /// Update platform settings (authority only) — pause/unpause, change fees
    pub fn update_platform(ctx: Context<UpdatePlatform>, paused: Option<bool>, platform_fee_bps: Option<u16>) -> Result<()> {
        let c = &mut ctx.accounts.platform_config;
        if let Some(p) = paused {
            c.paused = p;
        }
        if let Some(fee) = platform_fee_bps {
            require!(fee <= 1000, SendItError::FeeTooHigh);
            c.platform_fee_bps = fee;
        }
        Ok(())
    }

    pub fn create_token(ctx: Context<CreateToken>, name: String, symbol: String, uri: String, creator_fee_bps: u16) -> Result<()> {
        // Validate input lengths
        require!(name.len() <= MAX_NAME_LEN, SendItError::NameTooLong);
        require!(symbol.len() <= MAX_SYMBOL_LEN, SendItError::SymbolTooLong);
        require!(uri.len() <= MAX_URI_LEN, SendItError::UriTooLong);
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
        l.total_staked = 0;
        l.reserve_sol = 0;
        l.created_at = clock.unix_timestamp;
        l.migrated = false;
        l.total_volume_sol = 0;
        l.bump = ctx.bumps.token_launch;
        l.sol_vault_bump = ctx.bumps.launch_sol_vault;

        let mk = ctx.accounts.token_mint.key();
        let seeds: &[&[u8]] = &[TOKEN_LAUNCH_SEED, mk.as_ref(), &[l.bump]];

        // Mint total supply to launch vault
        token::mint_to(CpiContext::new_with_signer(ctx.accounts.token_program.to_account_info(), MintTo {
            mint: ctx.accounts.token_mint.to_account_info(),
            to: ctx.accounts.launch_token_vault.to_account_info(),
            authority: ctx.accounts.token_launch.to_account_info(),
        }, &[seeds]), DEFAULT_TOTAL_SUPPLY)?;

        // Create Metaplex token metadata
        let ix = mpl_create_metadata_ix(
            ctx.accounts.metadata.key(),
            ctx.accounts.token_mint.key(),
            ctx.accounts.token_launch.key(),
            ctx.accounts.creator.key(),
            ctx.accounts.token_launch.key(),
            l.name.clone(), l.symbol.clone(), l.uri.clone(),
        );
        anchor_lang::solana_program::program::invoke_signed(
            &ix,
            &[
                ctx.accounts.metadata.to_account_info(),
                ctx.accounts.token_mint.to_account_info(),
                ctx.accounts.token_launch.to_account_info(),
                ctx.accounts.creator.to_account_info(),
                ctx.accounts.token_launch.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
                ctx.accounts.rent.to_account_info(),
            ],
            &[seeds],
        )?;

        emit!(TokenCreated {
            mint: ctx.accounts.token_mint.key(),
            creator: ctx.accounts.creator.key(),
            name: l.name.clone(),
            symbol: l.symbol.clone(),
            creator_fee_bps: l.creator_fee_bps,
            timestamp: clock.unix_timestamp,
        });

        let config = &mut ctx.accounts.platform_config;
        config.total_launches = config.total_launches.checked_add(1).ok_or(SendItError::MathOverflow)?;
        Ok(())
    }

    pub fn buy(ctx: Context<BuyTokens>, sol_amount: u64) -> Result<()> {
        require!(sol_amount > 0, SendItError::ZeroAmount);
        let config = &ctx.accounts.platform_config;
        require!(!config.paused, SendItError::PlatformPaused);
        let launch = &ctx.accounts.token_launch;
        require!(!launch.migrated, SendItError::AlreadyMigrated);

        // Available tokens = unsold tokens (not counting staked, which are already sold)
        let available = launch.total_supply.checked_sub(launch.tokens_sold).ok_or(SendItError::MathOverflow)?;

        let base = 1_000u128;
        let slope = PRECISION.checked_div(launch.total_supply as u128).ok_or(SendItError::MathOverflow)?;
        let cp = base.checked_add(
            slope.checked_mul(launch.tokens_sold as u128).ok_or(SendItError::MathOverflow)?
                .checked_div(PRECISION).ok_or(SendItError::MathOverflow)?
        ).ok_or(SendItError::MathOverflow)?;

        let raw_tokens = (sol_amount as u128)
            .checked_mul(1_000_000_000u128).ok_or(SendItError::MathOverflow)?
            .checked_div(cp).ok_or(SendItError::MathOverflow)?;
        let tokens_out = raw_tokens.min(available as u128) as u64;
        require!(tokens_out > 0, SendItError::InsufficientOutput);

        let pf = (sol_amount as u128)
            .checked_mul(config.platform_fee_bps as u128).ok_or(SendItError::MathOverflow)?
            .checked_div(10_000).ok_or(SendItError::MathOverflow)? as u64;
        let cf = (sol_amount as u128)
            .checked_mul(launch.creator_fee_bps as u128).ok_or(SendItError::MathOverflow)?
            .checked_div(10_000).ok_or(SendItError::MathOverflow)? as u64;
        let net = sol_amount.checked_sub(pf).ok_or(SendItError::MathOverflow)?
            .checked_sub(cf).ok_or(SendItError::MathOverflow)?;

        // Transfer SOL
        anchor_lang::system_program::transfer(CpiContext::new(ctx.accounts.system_program.to_account_info(), anchor_lang::system_program::Transfer { from: ctx.accounts.buyer.to_account_info(), to: ctx.accounts.launch_sol_vault.to_account_info() }), net)?;
        if pf > 0 { anchor_lang::system_program::transfer(CpiContext::new(ctx.accounts.system_program.to_account_info(), anchor_lang::system_program::Transfer { from: ctx.accounts.buyer.to_account_info(), to: ctx.accounts.platform_vault.to_account_info() }), pf)?; }
        if cf > 0 { anchor_lang::system_program::transfer(CpiContext::new(ctx.accounts.system_program.to_account_info(), anchor_lang::system_program::Transfer { from: ctx.accounts.buyer.to_account_info(), to: ctx.accounts.creator_wallet.to_account_info() }), cf)?; }

        // Transfer tokens
        let mk = ctx.accounts.token_mint.key();
        let seeds: &[&[u8]] = &[TOKEN_LAUNCH_SEED, mk.as_ref(), &[ctx.accounts.token_launch.bump]];
        token::transfer(CpiContext::new_with_signer(ctx.accounts.token_program.to_account_info(), Transfer { from: ctx.accounts.launch_token_vault.to_account_info(), to: ctx.accounts.buyer_token_account.to_account_info(), authority: ctx.accounts.token_launch.to_account_info() }, &[seeds]), tokens_out)?;

        let launch = &mut ctx.accounts.token_launch;
        launch.tokens_sold = launch.tokens_sold.checked_add(tokens_out).ok_or(SendItError::MathOverflow)?;
        launch.reserve_sol = launch.reserve_sol.checked_add(net).ok_or(SendItError::MathOverflow)?;
        launch.total_volume_sol = launch.total_volume_sol.saturating_add(sol_amount);

        let p = &mut ctx.accounts.user_position;
        p.owner = ctx.accounts.buyer.key();
        p.mint = ctx.accounts.token_mint.key();
        p.tokens_bought = p.tokens_bought.saturating_add(tokens_out);
        p.sol_spent = p.sol_spent.saturating_add(sol_amount);
        if p.bump == 0 { p.bump = ctx.bumps.user_position; }

        let config = &mut ctx.accounts.platform_config;
        config.total_volume_sol = config.total_volume_sol.saturating_add(sol_amount);

        emit!(TokenBought {
            mint: ctx.accounts.token_mint.key(),
            buyer: ctx.accounts.buyer.key(),
            sol_amount, tokens_out, price: cp,
        });

        Ok(())
    }

    pub fn sell(ctx: Context<SellTokens>, token_amount: u64) -> Result<()> {
        require!(token_amount > 0, SendItError::ZeroAmount);
        let config = &ctx.accounts.platform_config;
        require!(!config.paused, SendItError::PlatformPaused);
        let launch = &ctx.accounts.token_launch;
        require!(!launch.migrated, SendItError::AlreadyMigrated);
        require!(token_amount <= launch.tokens_sold, SendItError::InsufficientTokensSold);

        let ns = launch.tokens_sold.checked_sub(token_amount).ok_or(SendItError::MathOverflow)?;
        let base = 1_000u128;
        let slope = PRECISION.checked_div(launch.total_supply as u128).ok_or(SendItError::MathOverflow)?;
        let sum = (ns as u128).checked_add(launch.tokens_sold as u128).ok_or(SendItError::MathOverflow)?;
        let avg = base.checked_add(
            slope.checked_mul(sum).ok_or(SendItError::MathOverflow)?
                .checked_div(2u128.checked_mul(PRECISION).ok_or(SendItError::MathOverflow)?).ok_or(SendItError::MathOverflow)?
        ).ok_or(SendItError::MathOverflow)?;
        let sol_out = avg.checked_mul(token_amount as u128).ok_or(SendItError::MathOverflow)?
            .checked_div(1_000_000_000u128).ok_or(SendItError::MathOverflow)? as u64;
        require!(sol_out > 0, SendItError::InsufficientOutput);

        let pf = (sol_out as u128)
            .checked_mul(config.platform_fee_bps as u128).ok_or(SendItError::MathOverflow)?
            .checked_div(10_000).ok_or(SendItError::MathOverflow)? as u64;
        let cf = (sol_out as u128)
            .checked_mul(launch.creator_fee_bps as u128).ok_or(SendItError::MathOverflow)?
            .checked_div(10_000).ok_or(SendItError::MathOverflow)? as u64;
        let net = sol_out.checked_sub(pf).ok_or(SendItError::MathOverflow)?
            .checked_sub(cf).ok_or(SendItError::MathOverflow)?;
        let total_withdrawal = net.checked_add(pf).ok_or(SendItError::MathOverflow)?
            .checked_add(cf).ok_or(SendItError::MathOverflow)?;

        require!(net <= launch.reserve_sol, SendItError::InsufficientReserve);

        // Ensure vault stays above rent-exempt minimum
        let vault_balance = ctx.accounts.launch_sol_vault.lamports();
        require!(vault_balance.checked_sub(total_withdrawal).ok_or(SendItError::MathOverflow)? >= RENT_EXEMPT_MIN, SendItError::VaultBelowRentExempt);

        // Transfer tokens from seller to vault
        token::transfer(CpiContext::new(ctx.accounts.token_program.to_account_info(), Transfer { from: ctx.accounts.seller_token_account.to_account_info(), to: ctx.accounts.launch_token_vault.to_account_info(), authority: ctx.accounts.seller.to_account_info() }), token_amount)?;

        // Transfer SOL from vault
        let mk = ctx.accounts.token_mint.key();
        let vault_seeds: &[&[u8]] = &[SOL_VAULT_SEED, mk.as_ref(), &[ctx.accounts.token_launch.sol_vault_bump]];
        anchor_lang::system_program::transfer(CpiContext::new_with_signer(ctx.accounts.system_program.to_account_info(), anchor_lang::system_program::Transfer { from: ctx.accounts.launch_sol_vault.to_account_info(), to: ctx.accounts.seller.to_account_info() }, &[vault_seeds]), net)?;
        if pf > 0 {
            anchor_lang::system_program::transfer(CpiContext::new_with_signer(ctx.accounts.system_program.to_account_info(), anchor_lang::system_program::Transfer { from: ctx.accounts.launch_sol_vault.to_account_info(), to: ctx.accounts.platform_vault.to_account_info() }, &[vault_seeds]), pf)?;
        }
        if cf > 0 {
            anchor_lang::system_program::transfer(CpiContext::new_with_signer(ctx.accounts.system_program.to_account_info(), anchor_lang::system_program::Transfer { from: ctx.accounts.launch_sol_vault.to_account_info(), to: ctx.accounts.creator_wallet.to_account_info() }, &[vault_seeds]), cf)?;
        }

        let launch = &mut ctx.accounts.token_launch;
        launch.tokens_sold = launch.tokens_sold.checked_sub(token_amount).ok_or(SendItError::MathOverflow)?;
        launch.reserve_sol = launch.reserve_sol.checked_sub(sol_out).ok_or(SendItError::MathOverflow)?;
        launch.total_volume_sol = launch.total_volume_sol.saturating_add(sol_out);

        let p = &mut ctx.accounts.user_position;
        p.tokens_bought = p.tokens_bought.saturating_sub(token_amount);

        let config = &mut ctx.accounts.platform_config;
        config.total_volume_sol = config.total_volume_sol.saturating_add(sol_out);

        emit!(TokenSold {
            mint: ctx.accounts.token_mint.key(),
            seller: ctx.accounts.seller.key(),
            token_amount, sol_out,
        });

        Ok(())
    }

    /// Stake tokens — lock for a duration, tracked on-chain
    pub fn stake(ctx: Context<StakeTokens>, amount: u64, duration_seconds: i64) -> Result<()> {
        require!(amount > 0, SendItError::ZeroAmount);
        require!(duration_seconds >= MIN_STAKE_DURATION, SendItError::StakeTooShort);
        require!(duration_seconds <= MAX_STAKE_DURATION, SendItError::StakeTooLong);
        let clock = Clock::get()?;

        // Transfer tokens to the launch token vault
        token::transfer(CpiContext::new(ctx.accounts.token_program.to_account_info(), Transfer {
            from: ctx.accounts.staker_token_account.to_account_info(),
            to: ctx.accounts.launch_token_vault.to_account_info(),
            authority: ctx.accounts.staker.to_account_info(),
        }), amount)?;

        let s = &mut ctx.accounts.stake_account;
        // If re-staking (init_if_needed), require previous stake is claimed
        if s.amount > 0 {
            require!(s.claimed, SendItError::StakeAlreadyActive);
        }
        s.staker = ctx.accounts.staker.key();
        s.mint = ctx.accounts.token_mint.key();
        s.amount = amount;
        s.staked_at = clock.unix_timestamp;
        s.unlock_at = clock.unix_timestamp.checked_add(duration_seconds).ok_or(SendItError::MathOverflow)?;
        s.claimed = false;
        if s.bump == 0 { s.bump = ctx.bumps.stake_account; }

        // Track total staked on the launch
        let launch = &mut ctx.accounts.token_launch;
        launch.total_staked = launch.total_staked.checked_add(amount).ok_or(SendItError::MathOverflow)?;

        emit!(TokenStaked {
            mint: ctx.accounts.token_mint.key(),
            staker: ctx.accounts.staker.key(),
            amount,
            unlock_at: s.unlock_at,
        });

        Ok(())
    }

    /// Unstake tokens — withdraw after lock period expires
    pub fn unstake(ctx: Context<UnstakeTokens>) -> Result<()> {
        let clock = Clock::get()?;
        let s = &ctx.accounts.stake_account;
        require!(clock.unix_timestamp >= s.unlock_at, SendItError::StakeLocked);
        require!(!s.claimed, SendItError::AlreadyClaimed);
        let amount = s.amount;

        // Ensure vault has enough tokens (excluding staked tokens needed by others)
        let vault_balance = ctx.accounts.launch_token_vault.amount;
        require!(vault_balance >= amount, SendItError::InsufficientVaultBalance);

        let mk = ctx.accounts.token_mint.key();
        let seeds: &[&[u8]] = &[TOKEN_LAUNCH_SEED, mk.as_ref(), &[ctx.accounts.token_launch.bump]];
        token::transfer(CpiContext::new_with_signer(ctx.accounts.token_program.to_account_info(), Transfer {
            from: ctx.accounts.launch_token_vault.to_account_info(),
            to: ctx.accounts.staker_token_account.to_account_info(),
            authority: ctx.accounts.token_launch.to_account_info(),
        }, &[seeds]), amount)?;

        let s = &mut ctx.accounts.stake_account;
        s.claimed = true;

        // Update total staked
        let launch = &mut ctx.accounts.token_launch;
        launch.total_staked = launch.total_staked.saturating_sub(amount);

        emit!(TokenUnstaked {
            mint: ctx.accounts.token_mint.key(),
            staker: ctx.accounts.staker.key(),
            amount,
        });

        Ok(())
    }
}

// ── Account Structs ──

#[account]
pub struct PlatformConfig {
    pub authority: Pubkey,
    pub platform_fee_bps: u16,
    pub migration_threshold: u64,
    pub total_launches: u64,
    pub total_volume_sol: u64,
    pub paused: bool,
    pub bump: u8,
}
impl PlatformConfig { pub const SIZE: usize = 8+32+2+8+8+8+1+1; }

#[account]
pub struct TokenLaunch {
    pub creator: Pubkey,
    pub mint: Pubkey,
    pub name: String,
    pub symbol: String,
    pub uri: String,
    pub creator_fee_bps: u16,
    pub total_supply: u64,
    pub tokens_sold: u64,
    pub total_staked: u64,
    pub reserve_sol: u64,
    pub created_at: i64,
    pub migrated: bool,
    pub total_volume_sol: u64,
    pub bump: u8,
    pub sol_vault_bump: u8,
}
impl TokenLaunch { pub const SIZE: usize = 8+32+32+(4+32)+(4+10)+(4+200)+2+8+8+8+8+8+1+8+1+1; }

#[account]
pub struct UserPosition {
    pub owner: Pubkey,
    pub mint: Pubkey,
    pub tokens_bought: u64,
    pub sol_spent: u64,
    pub bump: u8,
}
impl UserPosition { pub const SIZE: usize = 8+32+32+8+8+1; }

#[account]
pub struct StakeAccount {
    pub staker: Pubkey,
    pub mint: Pubkey,
    pub amount: u64,
    pub staked_at: i64,
    pub unlock_at: i64,
    pub claimed: bool,
    pub bump: u8,
}
impl StakeAccount { pub const SIZE: usize = 8+32+32+8+8+8+1+1; }

// ── Contexts ──

#[derive(Accounts)]
pub struct InitializePlatform<'info> {
    #[account(init, payer=authority, space=PlatformConfig::SIZE, seeds=[PLATFORM_CONFIG_SEED], bump)]
    pub platform_config: Account<'info, PlatformConfig>,
    #[account(mut)] pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UpdatePlatform<'info> {
    #[account(mut, seeds=[PLATFORM_CONFIG_SEED], bump=platform_config.bump, has_one=authority)]
    pub platform_config: Account<'info, PlatformConfig>,
    pub authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct CreateToken<'info> {
    #[account(init, payer=creator, space=TokenLaunch::SIZE, seeds=[TOKEN_LAUNCH_SEED, token_mint.key().as_ref()], bump)]
    pub token_launch: Account<'info, TokenLaunch>,
    #[account(init, payer=creator, mint::decimals=TOKEN_DECIMALS, mint::authority=token_launch)]
    pub token_mint: Account<'info, Mint>,
    #[account(init, payer=creator, associated_token::mint=token_mint, associated_token::authority=token_launch)]
    pub launch_token_vault: Account<'info, TokenAccount>,
    /// CHECK: SOL vault PDA
    #[account(mut, seeds=[SOL_VAULT_SEED, token_mint.key().as_ref()], bump)]
    pub launch_sol_vault: AccountInfo<'info>,
    /// CHECK: Metaplex metadata PDA — validated by Metaplex program CPI
    #[account(mut, seeds=[b"metadata", MPL_TOKEN_METADATA_ID.as_ref(), token_mint.key().as_ref()], bump, seeds::program = MPL_TOKEN_METADATA_ID)]
    pub metadata: AccountInfo<'info>,
    #[account(mut, seeds=[PLATFORM_CONFIG_SEED], bump=platform_config.bump)]
    pub platform_config: Account<'info, PlatformConfig>,
    #[account(mut)] pub creator: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
    /// CHECK: Metaplex Token Metadata program
    #[account(address = MPL_TOKEN_METADATA_ID)]
    pub token_metadata_program: AccountInfo<'info>,
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

#[derive(Accounts)]
pub struct StakeTokens<'info> {
    #[account(init_if_needed, payer=staker, space=StakeAccount::SIZE, seeds=[STAKE_SEED, staker.key().as_ref(), token_mint.key().as_ref()], bump)]
    pub stake_account: Account<'info, StakeAccount>,
    #[account(mut, seeds=[TOKEN_LAUNCH_SEED, token_mint.key().as_ref()], bump=token_launch.bump)]
    pub token_launch: Account<'info, TokenLaunch>,
    pub token_mint: Account<'info, Mint>,
    #[account(mut, associated_token::mint=token_mint, associated_token::authority=staker)]
    pub staker_token_account: Account<'info, TokenAccount>,
    #[account(mut, associated_token::mint=token_mint, associated_token::authority=token_launch)]
    pub launch_token_vault: Account<'info, TokenAccount>,
    #[account(mut)] pub staker: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UnstakeTokens<'info> {
    #[account(mut, seeds=[STAKE_SEED, staker.key().as_ref(), token_mint.key().as_ref()], bump=stake_account.bump, has_one=staker)]
    pub stake_account: Account<'info, StakeAccount>,
    #[account(mut, seeds=[TOKEN_LAUNCH_SEED, token_mint.key().as_ref()], bump=token_launch.bump)]
    pub token_launch: Account<'info, TokenLaunch>,
    pub token_mint: Account<'info, Mint>,
    #[account(mut, associated_token::mint=token_mint, associated_token::authority=staker)]
    pub staker_token_account: Account<'info, TokenAccount>,
    #[account(mut, associated_token::mint=token_mint, associated_token::authority=token_launch)]
    pub launch_token_vault: Account<'info, TokenAccount>,
    #[account(mut)] pub staker: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

// ── Errors ──

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
    #[msg("Stake still locked")] StakeLocked,
    #[msg("Already claimed")] AlreadyClaimed,
    #[msg("Stake too short")] StakeTooShort,
    #[msg("Stake too long")] StakeTooLong,
    #[msg("Math overflow")] MathOverflow,
    #[msg("Name too long (max 32)")] NameTooLong,
    #[msg("Symbol too long (max 10)")] SymbolTooLong,
    #[msg("URI too long (max 200)")] UriTooLong,
    #[msg("Vault below rent-exempt minimum")] VaultBelowRentExempt,
    #[msg("Stake already active")] StakeAlreadyActive,
    #[msg("Insufficient vault balance")] InsufficientVaultBalance,
}

// ── Metaplex CPI Helper ──

fn mpl_create_metadata_ix(
    metadata_account: Pubkey, mint: Pubkey, mint_authority: Pubkey,
    payer: Pubkey, update_authority: Pubkey,
    name: String, symbol: String, uri: String,
) -> anchor_lang::solana_program::instruction::Instruction {
    use anchor_lang::solana_program::instruction::{AccountMeta, Instruction};
    let mut data = vec![33u8]; // CreateMetadataAccountV3
    let nb = name.as_bytes();
    data.extend_from_slice(&(nb.len() as u32).to_le_bytes());
    data.extend_from_slice(nb);
    let sb = symbol.as_bytes();
    data.extend_from_slice(&(sb.len() as u32).to_le_bytes());
    data.extend_from_slice(sb);
    let ub = uri.as_bytes();
    data.extend_from_slice(&(ub.len() as u32).to_le_bytes());
    data.extend_from_slice(ub);
    data.extend_from_slice(&0u16.to_le_bytes()); // seller_fee_basis_points
    data.push(0); // creators: None
    data.push(0); // collection: None
    data.push(0); // uses: None
    data.push(1); // is_mutable: true
    data.push(0); // collection_details: None
    Instruction {
        program_id: MPL_TOKEN_METADATA_ID,
        accounts: vec![
            AccountMeta::new(metadata_account, false),
            AccountMeta::new_readonly(mint, false),
            AccountMeta::new_readonly(mint_authority, true),
            AccountMeta::new(payer, true),
            AccountMeta::new_readonly(update_authority, false),
            AccountMeta::new_readonly(anchor_lang::solana_program::system_program::ID, false),
            AccountMeta::new_readonly(anchor_lang::solana_program::sysvar::rent::ID, false),
        ],
        data,
    }
}
