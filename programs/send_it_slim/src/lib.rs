use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer, MintTo, Burn};
use anchor_spl::associated_token::AssociatedToken;

declare_id!("HTKq18cATdwCZb6XM66Mhn8JWKCFTrZqH6zU1zip88Zx");

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
pub const REWARD_RATE_BPS_PER_YEAR: u64 = 1000; // 10% APY in basis points
pub const SECONDS_PER_YEAR: u64 = 365 * 24 * 3600;

pub const MPL_TOKEN_METADATA_ID: Pubkey = solana_program::pubkey!("metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s");

// ── AMM Constants ──
pub const POOL_SEED: &[u8] = b"amm_pool";
pub const POOL_SOL_VAULT_SEED: &[u8] = b"pool_sol_vault";
pub const LP_MINT_SEED: &[u8] = b"lp_mint";
pub const SWAP_FEE_BPS: u64 = 100; // 1% swap fee
pub const LP_FEE_BPS: u64 = 30; // 0.3% to LP holders
pub const PROTOCOL_FEE_BPS: u64 = 70; // 0.7% to protocol
pub const MIN_LIQUIDITY: u64 = 1000; // minimum LP tokens locked forever

// ── Governance (Realms) Constants ──
pub const GOV_BRIDGE_SEED: &[u8] = b"gov_bridge";
pub const PROPOSAL_SEED: &[u8] = b"proposal";
pub const VOTE_SEED: &[u8] = b"vote_record";
pub const MAX_TITLE_LEN: usize = 64;
pub const MAX_DESC_LEN: usize = 256;

// ── Tapestry (Social) Constants ──
pub const SOCIAL_PROFILE_SEED: &[u8] = b"social_profile";
pub const SOCIAL_POST_SEED: &[u8] = b"social_post";
pub const FOLLOW_SEED: &[u8] = b"follow";
pub const LIKE_SEED: &[u8] = b"like";
pub const MAX_DISPLAY_NAME_LEN: usize = 32;
pub const MAX_BIO_LEN: usize = 160;
pub const MAX_POST_LEN: usize = 280;

// ── Torque (Loyalty) Constants ──
pub const LOYALTY_CONFIG_SEED: &[u8] = b"loyalty_config";
pub const LOYALTY_ACCOUNT_SEED: &[u8] = b"loyalty_account";

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
    pub reward: u64,
}

#[event]
pub struct PoolCreated {
    pub mint: Pubkey,
    pub pool: Pubkey,
    pub initial_token: u64,
    pub initial_sol: u64,
    pub lp_minted: u64,
}

#[event]
pub struct Swapped {
    pub pool: Pubkey,
    pub user: Pubkey,
    pub sol_in: u64,
    pub token_in: u64,
    pub sol_out: u64,
    pub token_out: u64,
    pub fee: u64,
}

#[event]
pub struct LiquidityAdded {
    pub pool: Pubkey,
    pub user: Pubkey,
    pub sol_amount: u64,
    pub token_amount: u64,
    pub lp_minted: u64,
}

#[event]
pub struct LiquidityRemoved {
    pub pool: Pubkey,
    pub user: Pubkey,
    pub sol_amount: u64,
    pub token_amount: u64,
    pub lp_burned: u64,
}

#[event]
pub struct GovernanceBridgeInitialized {
    pub mint: Pubkey,
    pub realm: Pubkey,
    pub authority: Pubkey,
}

#[event]
pub struct ProposalCreated {
    pub proposal_id: u64,
    pub mint: Pubkey,
    pub proposer: Pubkey,
    pub title: String,
}

#[event]
pub struct VoteCast {
    pub proposal_id: u64,
    pub voter: Pubkey,
    pub approve: bool,
    pub weight: u64,
}

#[event]
pub struct SocialProfileCreated {
    pub owner: Pubkey,
    pub display_name: String,
}

#[event]
pub struct SocialProfileUpdated {
    pub owner: Pubkey,
    pub display_name: String,
}

#[event]
pub struct UserFollowed {
    pub follower: Pubkey,
    pub followee: Pubkey,
}

#[event]
pub struct SocialPostCreated {
    pub author: Pubkey,
    pub post_id: u64,
    pub content: String,
}

#[event]
pub struct PostLiked {
    pub liker: Pubkey,
    pub post_author: Pubkey,
    pub post_id: u64,
}

#[event]
pub struct LoyaltyConfigInitialized {
    pub mint: Pubkey,
    pub authority: Pubkey,
    pub points_per_sol: u64,
}

#[event]
pub struct LoyaltyPointsAwarded {
    pub user: Pubkey,
    pub mint: Pubkey,
    pub points: u64,
    pub total_points: u64,
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
        let bump = ctx.bumps.token_launch;
        let sol_vault_bump = ctx.bumps.launch_sol_vault;
        let mk = ctx.accounts.token_mint.key();
        let launch_key = ctx.accounts.token_launch.key();
        let creator_key = ctx.accounts.creator.key();

        // Initialize launch account fields
        {
            let l = &mut ctx.accounts.token_launch;
            l.creator = creator_key;
            l.mint = mk;
            l.name = name.clone();
            l.symbol = symbol.clone();
            l.uri = uri.clone();
            l.creator_fee_bps = creator_fee_bps;
            l.total_supply = DEFAULT_TOTAL_SUPPLY;
            l.tokens_sold = 0;
            l.total_staked = 0;
            l.reserve_sol = 0;
            l.created_at = clock.unix_timestamp;
            l.migrated = false;
            l.total_volume_sol = 0;
            l.bump = bump;
            l.sol_vault_bump = sol_vault_bump;
        }
        // Mutable borrow dropped — safe to use ctx.accounts immutably now

        let seeds: &[&[u8]] = &[TOKEN_LAUNCH_SEED, mk.as_ref(), &[bump]];

        // Mint total supply to launch vault
        token::mint_to(CpiContext::new_with_signer(ctx.accounts.token_program.to_account_info(), MintTo {
            mint: ctx.accounts.token_mint.to_account_info(),
            to: ctx.accounts.launch_token_vault.to_account_info(),
            authority: ctx.accounts.token_launch.to_account_info(),
        }, &[seeds]), DEFAULT_TOTAL_SUPPLY)?;

        // Create Metaplex token metadata
        let ix = mpl_create_metadata_ix(
            ctx.accounts.metadata.key(),
            mk,
            launch_key,
            creator_key,
            launch_key,
            name.clone(), symbol.clone(), uri,
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
            mint: mk,
            creator: creator_key,
            name,
            symbol,
            creator_fee_bps,
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

    /// Unstake tokens — withdraw after lock period expires, with rewards
    pub fn unstake(ctx: Context<UnstakeTokens>) -> Result<()> {
        let clock = Clock::get()?;
        let s = &ctx.accounts.stake_account;
        require!(clock.unix_timestamp >= s.unlock_at, SendItError::StakeLocked);
        require!(!s.claimed, SendItError::AlreadyClaimed);
        let amount = s.amount;

        // Calculate reward: amount * rate * duration / (10000 * seconds_per_year)
        let duration_secs = (clock.unix_timestamp - s.staked_at) as u64;
        let reward = (amount as u128)
            .checked_mul(REWARD_RATE_BPS_PER_YEAR as u128).ok_or(SendItError::MathOverflow)?
            .checked_mul(duration_secs as u128).ok_or(SendItError::MathOverflow)?
            .checked_div(10_000u128.checked_mul(SECONDS_PER_YEAR as u128).ok_or(SendItError::MathOverflow)?).ok_or(SendItError::MathOverflow)? as u64;

        // Cap reward at available unsold tokens in vault (minus staked tokens)
        let vault_balance = ctx.accounts.launch_token_vault.amount;
        let launch = &ctx.accounts.token_launch;
        let available_for_rewards = vault_balance.saturating_sub(launch.total_staked);
        let actual_reward = reward.min(available_for_rewards);
        let total_withdrawal = amount.checked_add(actual_reward).ok_or(SendItError::MathOverflow)?;

        require!(vault_balance >= total_withdrawal, SendItError::InsufficientVaultBalance);

        let mk = ctx.accounts.token_mint.key();
        let seeds: &[&[u8]] = &[TOKEN_LAUNCH_SEED, mk.as_ref(), &[ctx.accounts.token_launch.bump]];
        token::transfer(CpiContext::new_with_signer(ctx.accounts.token_program.to_account_info(), Transfer {
            from: ctx.accounts.launch_token_vault.to_account_info(),
            to: ctx.accounts.staker_token_account.to_account_info(),
            authority: ctx.accounts.token_launch.to_account_info(),
        }, &[seeds]), total_withdrawal)?;

        let s = &mut ctx.accounts.stake_account;
        s.claimed = true;

        // Update total staked
        let launch = &mut ctx.accounts.token_launch;
        launch.total_staked = launch.total_staked.saturating_sub(amount);

        emit!(TokenUnstaked {
            mint: ctx.accounts.token_mint.key(),
            staker: ctx.accounts.staker.key(),
            amount,
            reward: actual_reward,
        });

        Ok(())
    }

    // ══════════════════════════════════════════════════════════════
    // AMM — PumpSwap-style constant product liquidity pools
    // ══════════════════════════════════════════════════════════════

    /// Create a liquidity pool when a token graduates from the bonding curve.
    /// Seeds the pool with remaining unsold tokens + accumulated SOL.
    pub fn create_pool(ctx: Context<CreatePool>) -> Result<()> {
        let launch = &ctx.accounts.token_launch;
        require!(launch.migrated == false, SendItError::AlreadyMigrated);

        let config = &ctx.accounts.platform_config;
        require!(!config.paused, SendItError::PlatformPaused);

        // Check graduation threshold
        let reserve = launch.reserve_sol;
        require!(reserve >= config.migration_threshold, SendItError::NotGraduated);

        let mk = ctx.accounts.token_mint.key();
        let launch_bump = launch.bump;
        let sol_vault_bump = launch.sol_vault_bump;

        // Calculate initial pool liquidity
        let vault_token_balance = ctx.accounts.launch_token_vault.amount;
        let staked = launch.total_staked;
        let initial_tokens = vault_token_balance.saturating_sub(staked); // unsold tokens
        let initial_sol = reserve; // all bonding curve SOL

        require!(initial_tokens > 0, SendItError::ZeroAmount);
        require!(initial_sol > 0, SendItError::ZeroAmount);

        // Transfer tokens from launch vault to pool token vault
        let launch_seeds: &[&[u8]] = &[TOKEN_LAUNCH_SEED, mk.as_ref(), &[launch_bump]];
        token::transfer(CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.launch_token_vault.to_account_info(),
                to: ctx.accounts.pool_token_vault.to_account_info(),
                authority: ctx.accounts.token_launch.to_account_info(),
            },
            &[launch_seeds],
        ), initial_tokens)?;

        // Transfer SOL from launch sol vault to pool sol vault
        let vault_seeds: &[&[u8]] = &[SOL_VAULT_SEED, mk.as_ref(), &[sol_vault_bump]];
        let sol_to_transfer = initial_sol.saturating_sub(RENT_EXEMPT_MIN); // keep rent-exempt in old vault
        anchor_lang::system_program::transfer(CpiContext::new_with_signer(
            ctx.accounts.system_program.to_account_info(),
            anchor_lang::system_program::Transfer {
                from: ctx.accounts.launch_sol_vault.to_account_info(),
                to: ctx.accounts.pool_sol_vault.to_account_info(),
            },
            &[vault_seeds],
        ), sol_to_transfer)?;

        // Mint initial LP tokens: sqrt(initial_sol * initial_tokens)
        let lp_amount = isqrt((initial_sol as u128).checked_mul(initial_tokens as u128).ok_or(SendItError::MathOverflow)?);
        require!(lp_amount > MIN_LIQUIDITY as u128, SendItError::InsufficientOutput);

        // Mint LP tokens to creator (minus MIN_LIQUIDITY locked forever)
        let pool_bump = ctx.bumps.amm_pool;
        let pool_seeds: &[&[u8]] = &[POOL_SEED, mk.as_ref(), &[pool_bump]];
        let lp_to_creator = (lp_amount as u64).checked_sub(MIN_LIQUIDITY).ok_or(SendItError::MathOverflow)?;
        token::mint_to(CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            MintTo {
                mint: ctx.accounts.lp_mint.to_account_info(),
                to: ctx.accounts.creator_lp_account.to_account_info(),
                authority: ctx.accounts.amm_pool.to_account_info(),
            },
            &[pool_seeds],
        ), lp_to_creator)?;

        // Initialize pool state
        let pool = &mut ctx.accounts.amm_pool;
        pool.mint = mk;
        pool.token_reserve = initial_tokens;
        pool.sol_reserve = sol_to_transfer;
        pool.lp_mint = ctx.accounts.lp_mint.key();
        pool.lp_supply = lp_amount as u64;
        pool.total_fees_sol = 0;
        pool.total_fees_token = 0;
        pool.created_at = Clock::get()?.unix_timestamp;
        pool.bump = pool_bump;
        pool.pool_sol_vault_bump = ctx.bumps.pool_sol_vault;

        // Mark launch as migrated
        let launch = &mut ctx.accounts.token_launch;
        launch.migrated = true;

        emit!(PoolCreated {
            mint: mk,
            pool: ctx.accounts.amm_pool.key(),
            initial_token: initial_tokens,
            initial_sol: sol_to_transfer,
            lp_minted: lp_amount as u64,
        });

        Ok(())
    }

    /// Swap SOL for tokens or tokens for SOL through the AMM pool.
    /// Pass sol_amount > 0 to buy tokens, or token_amount > 0 to sell tokens.
    pub fn swap(ctx: Context<Swap>, sol_amount: u64, token_amount: u64) -> Result<()> {
        require!((sol_amount > 0) != (token_amount > 0), SendItError::InvalidSwap);
        let pool = &ctx.accounts.amm_pool;
        let mk = pool.mint;

        if sol_amount > 0 {
            // Buy tokens with SOL
            let fee = sol_amount.checked_mul(SWAP_FEE_BPS).ok_or(SendItError::MathOverflow)?
                .checked_div(10_000).ok_or(SendItError::MathOverflow)?;
            let net_sol = sol_amount.checked_sub(fee).ok_or(SendItError::MathOverflow)?;

            // Constant product: token_out = token_reserve - (token_reserve * sol_reserve) / (sol_reserve + net_sol)
            let new_sol_reserve = (pool.sol_reserve as u128).checked_add(net_sol as u128).ok_or(SendItError::MathOverflow)?;
            let token_out = (pool.token_reserve as u128)
                .checked_sub(
                    (pool.token_reserve as u128).checked_mul(pool.sol_reserve as u128).ok_or(SendItError::MathOverflow)?
                        .checked_div(new_sol_reserve).ok_or(SendItError::MathOverflow)?
                ).ok_or(SendItError::MathOverflow)? as u64;
            require!(token_out > 0, SendItError::InsufficientOutput);

            // Transfer SOL in
            anchor_lang::system_program::transfer(CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.user.to_account_info(),
                    to: ctx.accounts.pool_sol_vault.to_account_info(),
                },
            ), sol_amount)?;

            // Protocol fee to platform vault
            let protocol_fee = fee.checked_mul(PROTOCOL_FEE_BPS).ok_or(SendItError::MathOverflow)?
                .checked_div(SWAP_FEE_BPS).ok_or(SendItError::MathOverflow)?;
            if protocol_fee > 0 {
                let psvb = pool.pool_sol_vault_bump;
                let psv_seeds: &[&[u8]] = &[POOL_SOL_VAULT_SEED, mk.as_ref(), &[psvb]];
                anchor_lang::system_program::transfer(CpiContext::new_with_signer(
                    ctx.accounts.system_program.to_account_info(),
                    anchor_lang::system_program::Transfer {
                        from: ctx.accounts.pool_sol_vault.to_account_info(),
                        to: ctx.accounts.platform_vault.to_account_info(),
                    },
                    &[psv_seeds],
                ), protocol_fee)?;
            }

            // Transfer tokens out
            let pool_bump = ctx.accounts.amm_pool.bump;
            let pool_seeds: &[&[u8]] = &[POOL_SEED, mk.as_ref(), &[pool_bump]];
            token::transfer(CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.pool_token_vault.to_account_info(),
                    to: ctx.accounts.user_token_account.to_account_info(),
                    authority: ctx.accounts.amm_pool.to_account_info(),
                },
                &[pool_seeds],
            ), token_out)?;

            let pool = &mut ctx.accounts.amm_pool;
            pool.sol_reserve = pool.sol_reserve.checked_add(net_sol).ok_or(SendItError::MathOverflow)?;
            pool.token_reserve = pool.token_reserve.checked_sub(token_out).ok_or(SendItError::MathOverflow)?;
            pool.total_fees_sol = pool.total_fees_sol.saturating_add(fee);

            emit!(Swapped {
                pool: ctx.accounts.amm_pool.key(),
                user: ctx.accounts.user.key(),
                sol_in: sol_amount, token_in: 0, sol_out: 0, token_out, fee,
            });
        } else {
            // Sell tokens for SOL
            let fee_tokens = token_amount.checked_mul(SWAP_FEE_BPS).ok_or(SendItError::MathOverflow)?
                .checked_div(10_000).ok_or(SendItError::MathOverflow)?;
            let net_tokens = token_amount.checked_sub(fee_tokens).ok_or(SendItError::MathOverflow)?;

            // Constant product: sol_out = sol_reserve - (sol_reserve * token_reserve) / (token_reserve + net_tokens)
            let new_token_reserve = (pool.token_reserve as u128).checked_add(net_tokens as u128).ok_or(SendItError::MathOverflow)?;
            let sol_out = (pool.sol_reserve as u128)
                .checked_sub(
                    (pool.sol_reserve as u128).checked_mul(pool.token_reserve as u128).ok_or(SendItError::MathOverflow)?
                        .checked_div(new_token_reserve).ok_or(SendItError::MathOverflow)?
                ).ok_or(SendItError::MathOverflow)? as u64;
            require!(sol_out > 0, SendItError::InsufficientOutput);

            // Transfer tokens in
            token::transfer(CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.user_token_account.to_account_info(),
                    to: ctx.accounts.pool_token_vault.to_account_info(),
                    authority: ctx.accounts.user.to_account_info(),
                },
            ), token_amount)?;

            // Transfer SOL out
            let psvb = pool.pool_sol_vault_bump;
            let psv_seeds: &[&[u8]] = &[POOL_SOL_VAULT_SEED, mk.as_ref(), &[psvb]];
            anchor_lang::system_program::transfer(CpiContext::new_with_signer(
                ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.pool_sol_vault.to_account_info(),
                    to: ctx.accounts.user.to_account_info(),
                },
                &[psv_seeds],
            ), sol_out)?;

            let pool = &mut ctx.accounts.amm_pool;
            pool.token_reserve = pool.token_reserve.checked_add(net_tokens).ok_or(SendItError::MathOverflow)?;
            pool.sol_reserve = pool.sol_reserve.checked_sub(sol_out).ok_or(SendItError::MathOverflow)?;
            pool.total_fees_token = pool.total_fees_token.saturating_add(fee_tokens);

            emit!(Swapped {
                pool: ctx.accounts.amm_pool.key(),
                user: ctx.accounts.user.key(),
                sol_in: 0, token_in: token_amount, sol_out, token_out: 0, fee: fee_tokens,
            });
        }

        Ok(())
    }

    /// Add liquidity — deposit SOL + tokens proportionally, receive LP tokens
    pub fn add_liquidity(ctx: Context<AddLiquidity>, sol_amount: u64) -> Result<()> {
        require!(sol_amount > 0, SendItError::ZeroAmount);
        let pool = &ctx.accounts.amm_pool;
        let mk = pool.mint;

        // Calculate proportional token amount
        let token_amount = (sol_amount as u128)
            .checked_mul(pool.token_reserve as u128).ok_or(SendItError::MathOverflow)?
            .checked_div(pool.sol_reserve as u128).ok_or(SendItError::MathOverflow)? as u64;
        require!(token_amount > 0, SendItError::ZeroAmount);

        // Calculate LP tokens to mint: lp_supply * sol_amount / sol_reserve
        let lp_mint_amount = (pool.lp_supply as u128)
            .checked_mul(sol_amount as u128).ok_or(SendItError::MathOverflow)?
            .checked_div(pool.sol_reserve as u128).ok_or(SendItError::MathOverflow)? as u64;
        require!(lp_mint_amount > 0, SendItError::InsufficientOutput);

        // Transfer SOL
        anchor_lang::system_program::transfer(CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            anchor_lang::system_program::Transfer {
                from: ctx.accounts.user.to_account_info(),
                to: ctx.accounts.pool_sol_vault.to_account_info(),
            },
        ), sol_amount)?;

        // Transfer tokens
        token::transfer(CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.user_token_account.to_account_info(),
                to: ctx.accounts.pool_token_vault.to_account_info(),
                authority: ctx.accounts.user.to_account_info(),
            },
        ), token_amount)?;

        // Mint LP tokens
        let pool_bump = ctx.accounts.amm_pool.bump;
        let pool_seeds: &[&[u8]] = &[POOL_SEED, mk.as_ref(), &[pool_bump]];
        token::mint_to(CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            MintTo {
                mint: ctx.accounts.lp_mint.to_account_info(),
                to: ctx.accounts.user_lp_account.to_account_info(),
                authority: ctx.accounts.amm_pool.to_account_info(),
            },
            &[pool_seeds],
        ), lp_mint_amount)?;

        let pool = &mut ctx.accounts.amm_pool;
        pool.sol_reserve = pool.sol_reserve.checked_add(sol_amount).ok_or(SendItError::MathOverflow)?;
        pool.token_reserve = pool.token_reserve.checked_add(token_amount).ok_or(SendItError::MathOverflow)?;
        pool.lp_supply = pool.lp_supply.checked_add(lp_mint_amount).ok_or(SendItError::MathOverflow)?;

        emit!(LiquidityAdded {
            pool: ctx.accounts.amm_pool.key(),
            user: ctx.accounts.user.key(),
            sol_amount, token_amount, lp_minted: lp_mint_amount,
        });

        Ok(())
    }

    /// Remove liquidity — burn LP tokens, withdraw proportional SOL + tokens
    pub fn remove_liquidity(ctx: Context<RemoveLiquidity>, lp_amount: u64) -> Result<()> {
        require!(lp_amount > 0, SendItError::ZeroAmount);
        let pool = &ctx.accounts.amm_pool;
        let mk = pool.mint;

        // Calculate proportional amounts
        let sol_amount = (lp_amount as u128)
            .checked_mul(pool.sol_reserve as u128).ok_or(SendItError::MathOverflow)?
            .checked_div(pool.lp_supply as u128).ok_or(SendItError::MathOverflow)? as u64;
        let token_amount = (lp_amount as u128)
            .checked_mul(pool.token_reserve as u128).ok_or(SendItError::MathOverflow)?
            .checked_div(pool.lp_supply as u128).ok_or(SendItError::MathOverflow)? as u64;
        require!(sol_amount > 0 && token_amount > 0, SendItError::InsufficientOutput);

        // Burn LP tokens
        token::burn(CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            token::Burn {
                mint: ctx.accounts.lp_mint.to_account_info(),
                from: ctx.accounts.user_lp_account.to_account_info(),
                authority: ctx.accounts.user.to_account_info(),
            },
        ), lp_amount)?;

        // Transfer SOL out
        let psvb = pool.pool_sol_vault_bump;
        let psv_seeds: &[&[u8]] = &[POOL_SOL_VAULT_SEED, mk.as_ref(), &[psvb]];
        anchor_lang::system_program::transfer(CpiContext::new_with_signer(
            ctx.accounts.system_program.to_account_info(),
            anchor_lang::system_program::Transfer {
                from: ctx.accounts.pool_sol_vault.to_account_info(),
                to: ctx.accounts.user.to_account_info(),
            },
            &[psv_seeds],
        ), sol_amount)?;

        // Transfer tokens out
        let pool_bump = ctx.accounts.amm_pool.bump;
        let pool_seeds: &[&[u8]] = &[POOL_SEED, mk.as_ref(), &[pool_bump]];
        token::transfer(CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.pool_token_vault.to_account_info(),
                to: ctx.accounts.user_token_account.to_account_info(),
                authority: ctx.accounts.amm_pool.to_account_info(),
            },
            &[pool_seeds],
        ), token_amount)?;

        let pool = &mut ctx.accounts.amm_pool;
        pool.sol_reserve = pool.sol_reserve.checked_sub(sol_amount).ok_or(SendItError::MathOverflow)?;
        pool.token_reserve = pool.token_reserve.checked_sub(token_amount).ok_or(SendItError::MathOverflow)?;
        pool.lp_supply = pool.lp_supply.checked_sub(lp_amount).ok_or(SendItError::MathOverflow)?;

        emit!(LiquidityRemoved {
            pool: ctx.accounts.amm_pool.key(),
            user: ctx.accounts.user.key(),
            sol_amount, token_amount, lp_burned: lp_amount,
        });

        Ok(())
    }

    // ══════════════════════════════════════════════════════════════
    // REALMS — Governance Bridge
    // ══════════════════════════════════════════════════════════════

    /// Initialize a governance bridge linking a token launch to a Realms DAO
    pub fn init_governance_bridge(ctx: Context<InitGovernanceBridge>, realm: Pubkey) -> Result<()> {
        let bridge = &mut ctx.accounts.gov_bridge;
        bridge.mint = ctx.accounts.token_mint.key();
        bridge.realm = realm;
        bridge.authority = ctx.accounts.authority.key();
        bridge.proposal_count = 0;
        bridge.created_at = Clock::get()?.unix_timestamp;
        bridge.bump = ctx.bumps.gov_bridge;

        emit!(GovernanceBridgeInitialized {
            mint: bridge.mint,
            realm,
            authority: bridge.authority,
        });
        Ok(())
    }

    /// Create a governance proposal
    pub fn create_realms_proposal(ctx: Context<CreateRealmsProposal>, title: String, description: String) -> Result<()> {
        require!(title.len() <= MAX_TITLE_LEN, SendItError::TitleTooLong);
        require!(description.len() <= MAX_DESC_LEN, SendItError::DescTooLong);

        let bridge = &mut ctx.accounts.gov_bridge;
        let proposal_id = bridge.proposal_count;
        bridge.proposal_count = bridge.proposal_count.checked_add(1).ok_or(SendItError::MathOverflow)?;

        let proposal = &mut ctx.accounts.proposal;
        proposal.proposal_id = proposal_id;
        proposal.mint = bridge.mint;
        proposal.proposer = ctx.accounts.proposer.key();
        proposal.title = title.clone();
        proposal.description = description;
        proposal.votes_for = 0;
        proposal.votes_against = 0;
        proposal.created_at = Clock::get()?.unix_timestamp;
        proposal.finalized = false;
        proposal.bump = ctx.bumps.proposal;

        emit!(ProposalCreated { proposal_id, mint: bridge.mint, proposer: ctx.accounts.proposer.key(), title });
        Ok(())
    }

    /// Cast a vote on a governance proposal
    pub fn cast_realms_vote(ctx: Context<CastRealmsVote>, approve: bool, weight: u64) -> Result<()> {
        require!(weight > 0, SendItError::ZeroAmount);
        let proposal = &mut ctx.accounts.proposal;
        require!(!proposal.finalized, SendItError::ProposalFinalized);

        if approve {
            proposal.votes_for = proposal.votes_for.checked_add(weight).ok_or(SendItError::MathOverflow)?;
        } else {
            proposal.votes_against = proposal.votes_against.checked_add(weight).ok_or(SendItError::MathOverflow)?;
        }

        let vote_record = &mut ctx.accounts.vote_record;
        vote_record.voter = ctx.accounts.voter.key();
        vote_record.proposal_id = proposal.proposal_id;
        vote_record.approve = approve;
        vote_record.weight = weight;
        vote_record.timestamp = Clock::get()?.unix_timestamp;
        vote_record.bump = ctx.bumps.vote_record;

        emit!(VoteCast { proposal_id: proposal.proposal_id, voter: ctx.accounts.voter.key(), approve, weight });
        Ok(())
    }

    // ══════════════════════════════════════════════════════════════
    // TAPESTRY — Social Layer
    // ══════════════════════════════════════════════════════════════

    /// Create a social profile linked to the user's wallet
    pub fn create_social_profile(ctx: Context<CreateSocialProfile>, display_name: String, bio: String) -> Result<()> {
        require!(display_name.len() <= MAX_DISPLAY_NAME_LEN, SendItError::NameTooLong);
        require!(bio.len() <= MAX_BIO_LEN, SendItError::BioTooLong);

        let profile = &mut ctx.accounts.social_profile;
        profile.owner = ctx.accounts.owner.key();
        profile.display_name = display_name.clone();
        profile.bio = bio;
        profile.post_count = 0;
        profile.follower_count = 0;
        profile.following_count = 0;
        profile.created_at = Clock::get()?.unix_timestamp;
        profile.bump = ctx.bumps.social_profile;

        emit!(SocialProfileCreated { owner: profile.owner, display_name });
        Ok(())
    }

    /// Update an existing social profile
    pub fn update_social_profile(ctx: Context<UpdateSocialProfile>, display_name: Option<String>, bio: Option<String>) -> Result<()> {
        let profile = &mut ctx.accounts.social_profile;
        if let Some(name) = display_name.clone() {
            require!(name.len() <= MAX_DISPLAY_NAME_LEN, SendItError::NameTooLong);
            profile.display_name = name;
        }
        if let Some(b) = bio {
            require!(b.len() <= MAX_BIO_LEN, SendItError::BioTooLong);
            profile.bio = b;
        }
        emit!(SocialProfileUpdated { owner: profile.owner, display_name: profile.display_name.clone() });
        Ok(())
    }

    /// Follow another user
    pub fn follow_user(ctx: Context<FollowUser>) -> Result<()> {
        let follow = &mut ctx.accounts.follow_account;
        follow.follower = ctx.accounts.follower.key();
        follow.followee = ctx.accounts.followee_profile.owner;
        follow.created_at = Clock::get()?.unix_timestamp;
        follow.bump = ctx.bumps.follow_account;

        let follower_profile = &mut ctx.accounts.follower_profile;
        follower_profile.following_count = follower_profile.following_count.checked_add(1).ok_or(SendItError::MathOverflow)?;

        let followee_profile = &mut ctx.accounts.followee_profile;
        followee_profile.follower_count = followee_profile.follower_count.checked_add(1).ok_or(SendItError::MathOverflow)?;

        emit!(UserFollowed { follower: follow.follower, followee: follow.followee });
        Ok(())
    }

    /// Create a social post
    pub fn create_social_post(ctx: Context<CreateSocialPost>, content: String) -> Result<()> {
        require!(content.len() <= MAX_POST_LEN, SendItError::PostTooLong);

        let profile = &mut ctx.accounts.author_profile;
        let post_id = profile.post_count;
        profile.post_count = profile.post_count.checked_add(1).ok_or(SendItError::MathOverflow)?;

        let post = &mut ctx.accounts.social_post;
        post.author = ctx.accounts.author.key();
        post.post_id = post_id;
        post.content = content.clone();
        post.likes = 0;
        post.created_at = Clock::get()?.unix_timestamp;
        post.bump = ctx.bumps.social_post;

        emit!(SocialPostCreated { author: post.author, post_id, content });
        Ok(())
    }

    /// Like a social post
    pub fn like_post(ctx: Context<LikePost>) -> Result<()> {
        let post = &mut ctx.accounts.social_post;
        post.likes = post.likes.checked_add(1).ok_or(SendItError::MathOverflow)?;

        let like = &mut ctx.accounts.like_account;
        like.liker = ctx.accounts.liker.key();
        like.post_author = post.author;
        like.post_id = post.post_id;
        like.timestamp = Clock::get()?.unix_timestamp;
        like.bump = ctx.bumps.like_account;

        emit!(PostLiked { liker: like.liker, post_author: like.post_author, post_id: like.post_id });
        Ok(())
    }

    // ══════════════════════════════════════════════════════════════
    // TORQUE — Loyalty Points
    // ══════════════════════════════════════════════════════════════

    /// Initialize loyalty configuration for a token
    pub fn init_loyalty_config(ctx: Context<InitLoyaltyConfig>, points_per_sol: u64) -> Result<()> {
        require!(points_per_sol > 0, SendItError::ZeroAmount);
        let config = &mut ctx.accounts.loyalty_config;
        config.mint = ctx.accounts.token_mint.key();
        config.authority = ctx.accounts.authority.key();
        config.points_per_sol = points_per_sol;
        config.total_points_distributed = 0;
        config.created_at = Clock::get()?.unix_timestamp;
        config.bump = ctx.bumps.loyalty_config;

        emit!(LoyaltyConfigInitialized { mint: config.mint, authority: config.authority, points_per_sol });
        Ok(())
    }

    /// Award loyalty points to a user
    pub fn award_loyalty_points(ctx: Context<AwardLoyaltyPoints>, points: u64) -> Result<()> {
        require!(points > 0, SendItError::ZeroAmount);

        let loyalty = &mut ctx.accounts.loyalty_account;
        if loyalty.bump == 0 {
            loyalty.owner = ctx.accounts.user.key();
            loyalty.mint = ctx.accounts.loyalty_config.mint;
            loyalty.bump = ctx.bumps.loyalty_account;
        }
        loyalty.points = loyalty.points.checked_add(points).ok_or(SendItError::MathOverflow)?;
        loyalty.last_updated = Clock::get()?.unix_timestamp;

        let config = &mut ctx.accounts.loyalty_config;
        config.total_points_distributed = config.total_points_distributed.checked_add(points).ok_or(SendItError::MathOverflow)?;

        emit!(LoyaltyPointsAwarded { user: loyalty.owner, mint: loyalty.mint, points, total_points: loyalty.points });
        Ok(())
    }
}

/// Integer square root (Babylonian method)
fn isqrt(n: u128) -> u128 {
    if n == 0 { return 0; }
    let mut x = n;
    let mut y = (x + 1) / 2;
    while y < x { x = y; y = (x + n / x) / 2; }
    x
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

#[account]
pub struct AmmPool {
    pub mint: Pubkey,
    pub token_reserve: u64,
    pub sol_reserve: u64,
    pub lp_mint: Pubkey,
    pub lp_supply: u64,
    pub total_fees_sol: u64,
    pub total_fees_token: u64,
    pub created_at: i64,
    pub bump: u8,
    pub pool_sol_vault_bump: u8,
}
impl AmmPool { pub const SIZE: usize = 8+32+8+8+32+8+8+8+8+1+1; }

// ── Governance Accounts ──

#[account]
pub struct GovernanceBridge {
    pub mint: Pubkey,
    pub realm: Pubkey,
    pub authority: Pubkey,
    pub proposal_count: u64,
    pub created_at: i64,
    pub bump: u8,
}
impl GovernanceBridge { pub const SIZE: usize = 8+32+32+32+8+8+1; }

#[account]
pub struct Proposal {
    pub proposal_id: u64,
    pub mint: Pubkey,
    pub proposer: Pubkey,
    pub title: String,
    pub description: String,
    pub votes_for: u64,
    pub votes_against: u64,
    pub created_at: i64,
    pub finalized: bool,
    pub bump: u8,
}
impl Proposal { pub const SIZE: usize = 8+8+32+32+(4+64)+(4+256)+8+8+8+1+1; }

#[account]
pub struct VoteRecord {
    pub voter: Pubkey,
    pub proposal_id: u64,
    pub approve: bool,
    pub weight: u64,
    pub timestamp: i64,
    pub bump: u8,
}
impl VoteRecord { pub const SIZE: usize = 8+32+8+1+8+8+1; }

// ── Social (Tapestry) Accounts ──

#[account]
pub struct SocialProfile {
    pub owner: Pubkey,
    pub display_name: String,
    pub bio: String,
    pub post_count: u64,
    pub follower_count: u64,
    pub following_count: u64,
    pub created_at: i64,
    pub bump: u8,
}
impl SocialProfile { pub const SIZE: usize = 8+32+(4+32)+(4+160)+8+8+8+8+1; }

#[account]
pub struct SocialPost {
    pub author: Pubkey,
    pub post_id: u64,
    pub content: String,
    pub likes: u64,
    pub created_at: i64,
    pub bump: u8,
}
impl SocialPost { pub const SIZE: usize = 8+32+8+(4+280)+8+8+1; }

#[account]
pub struct FollowAccount {
    pub follower: Pubkey,
    pub followee: Pubkey,
    pub created_at: i64,
    pub bump: u8,
}
impl FollowAccount { pub const SIZE: usize = 8+32+32+8+1; }

#[account]
pub struct LikeAccount {
    pub liker: Pubkey,
    pub post_author: Pubkey,
    pub post_id: u64,
    pub timestamp: i64,
    pub bump: u8,
}
impl LikeAccount { pub const SIZE: usize = 8+32+32+8+8+1; }

// ── Loyalty (Torque) Accounts ──

#[account]
pub struct LoyaltyConfig {
    pub mint: Pubkey,
    pub authority: Pubkey,
    pub points_per_sol: u64,
    pub total_points_distributed: u64,
    pub created_at: i64,
    pub bump: u8,
}
impl LoyaltyConfig { pub const SIZE: usize = 8+32+32+8+8+8+1; }

#[account]
pub struct LoyaltyAccount {
    pub owner: Pubkey,
    pub mint: Pubkey,
    pub points: u64,
    pub last_updated: i64,
    pub bump: u8,
}
impl LoyaltyAccount { pub const SIZE: usize = 8+32+32+8+8+1; }

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

// ── AMM Contexts ──

#[derive(Accounts)]
pub struct CreatePool<'info> {
    #[account(init, payer=creator, space=AmmPool::SIZE, seeds=[POOL_SEED, token_mint.key().as_ref()], bump)]
    pub amm_pool: Account<'info, AmmPool>,
    #[account(mut, seeds=[TOKEN_LAUNCH_SEED, token_mint.key().as_ref()], bump=token_launch.bump, has_one=creator)]
    pub token_launch: Account<'info, TokenLaunch>,
    pub token_mint: Account<'info, Mint>,
    #[account(mut, associated_token::mint=token_mint, associated_token::authority=token_launch)]
    pub launch_token_vault: Account<'info, TokenAccount>,
    /// CHECK: Launch SOL vault
    #[account(mut, seeds=[SOL_VAULT_SEED, token_mint.key().as_ref()], bump)]
    pub launch_sol_vault: AccountInfo<'info>,
    #[account(init, payer=creator, associated_token::mint=token_mint, associated_token::authority=amm_pool)]
    pub pool_token_vault: Account<'info, TokenAccount>,
    /// CHECK: Pool SOL vault PDA
    #[account(mut, seeds=[POOL_SOL_VAULT_SEED, token_mint.key().as_ref()], bump)]
    pub pool_sol_vault: AccountInfo<'info>,
    #[account(init, payer=creator, mint::decimals=TOKEN_DECIMALS, mint::authority=amm_pool)]
    pub lp_mint: Account<'info, Mint>,
    #[account(init, payer=creator, associated_token::mint=lp_mint, associated_token::authority=creator)]
    pub creator_lp_account: Account<'info, TokenAccount>,
    #[account(seeds=[PLATFORM_CONFIG_SEED], bump=platform_config.bump)]
    pub platform_config: Account<'info, PlatformConfig>,
    #[account(mut)] pub creator: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct Swap<'info> {
    #[account(mut, seeds=[POOL_SEED, amm_pool.mint.as_ref()], bump=amm_pool.bump)]
    pub amm_pool: Account<'info, AmmPool>,
    #[account(mut, associated_token::mint=amm_pool.mint, associated_token::authority=amm_pool)]
    pub pool_token_vault: Account<'info, TokenAccount>,
    /// CHECK: Pool SOL vault
    #[account(mut, seeds=[POOL_SOL_VAULT_SEED, amm_pool.mint.as_ref()], bump=amm_pool.pool_sol_vault_bump)]
    pub pool_sol_vault: AccountInfo<'info>,
    #[account(init_if_needed, payer=user, associated_token::mint=amm_pool.mint, associated_token::authority=user)]
    pub user_token_account: Account<'info, TokenAccount>,
    /// CHECK: Platform vault
    #[account(mut, seeds=[PLATFORM_VAULT_SEED], bump)]
    pub platform_vault: AccountInfo<'info>,
    #[account(mut)] pub user: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct AddLiquidity<'info> {
    #[account(mut, seeds=[POOL_SEED, amm_pool.mint.as_ref()], bump=amm_pool.bump)]
    pub amm_pool: Account<'info, AmmPool>,
    #[account(mut, associated_token::mint=amm_pool.mint, associated_token::authority=amm_pool)]
    pub pool_token_vault: Account<'info, TokenAccount>,
    /// CHECK: Pool SOL vault
    #[account(mut, seeds=[POOL_SOL_VAULT_SEED, amm_pool.mint.as_ref()], bump=amm_pool.pool_sol_vault_bump)]
    pub pool_sol_vault: AccountInfo<'info>,
    #[account(mut, constraint=lp_mint.key()==amm_pool.lp_mint)]
    pub lp_mint: Account<'info, Mint>,
    #[account(mut, associated_token::mint=amm_pool.mint, associated_token::authority=user)]
    pub user_token_account: Account<'info, TokenAccount>,
    #[account(init_if_needed, payer=user, associated_token::mint=lp_mint, associated_token::authority=user)]
    pub user_lp_account: Account<'info, TokenAccount>,
    #[account(mut)] pub user: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct RemoveLiquidity<'info> {
    #[account(mut, seeds=[POOL_SEED, amm_pool.mint.as_ref()], bump=amm_pool.bump)]
    pub amm_pool: Account<'info, AmmPool>,
    #[account(mut, associated_token::mint=amm_pool.mint, associated_token::authority=amm_pool)]
    pub pool_token_vault: Account<'info, TokenAccount>,
    /// CHECK: Pool SOL vault
    #[account(mut, seeds=[POOL_SOL_VAULT_SEED, amm_pool.mint.as_ref()], bump=amm_pool.pool_sol_vault_bump)]
    pub pool_sol_vault: AccountInfo<'info>,
    #[account(mut, constraint=lp_mint.key()==amm_pool.lp_mint)]
    pub lp_mint: Account<'info, Mint>,
    #[account(mut, associated_token::mint=amm_pool.mint, associated_token::authority=user)]
    pub user_token_account: Account<'info, TokenAccount>,
    #[account(mut, associated_token::mint=lp_mint, associated_token::authority=user)]
    pub user_lp_account: Account<'info, TokenAccount>,
    #[account(mut)] pub user: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

// ── Governance Contexts ──

#[derive(Accounts)]
pub struct InitGovernanceBridge<'info> {
    #[account(init, payer=authority, space=GovernanceBridge::SIZE, seeds=[GOV_BRIDGE_SEED, token_mint.key().as_ref()], bump)]
    pub gov_bridge: Account<'info, GovernanceBridge>,
    pub token_mint: Account<'info, Mint>,
    #[account(mut)] pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct CreateRealmsProposal<'info> {
    #[account(mut, seeds=[GOV_BRIDGE_SEED, gov_bridge.mint.as_ref()], bump=gov_bridge.bump)]
    pub gov_bridge: Account<'info, GovernanceBridge>,
    #[account(init, payer=proposer, space=Proposal::SIZE, seeds=[PROPOSAL_SEED, gov_bridge.mint.as_ref(), &gov_bridge.proposal_count.to_le_bytes()], bump)]
    pub proposal: Account<'info, Proposal>,
    #[account(mut)] pub proposer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct CastRealmsVote<'info> {
    #[account(mut, seeds=[PROPOSAL_SEED, proposal.mint.as_ref(), &proposal.proposal_id.to_le_bytes()], bump=proposal.bump)]
    pub proposal: Account<'info, Proposal>,
    #[account(init, payer=voter, space=VoteRecord::SIZE, seeds=[VOTE_SEED, proposal.mint.as_ref(), &proposal.proposal_id.to_le_bytes(), voter.key().as_ref()], bump)]
    pub vote_record: Account<'info, VoteRecord>,
    #[account(mut)] pub voter: Signer<'info>,
    pub system_program: Program<'info, System>,
}

// ── Social (Tapestry) Contexts ──

#[derive(Accounts)]
pub struct CreateSocialProfile<'info> {
    #[account(init, payer=owner, space=SocialProfile::SIZE, seeds=[SOCIAL_PROFILE_SEED, owner.key().as_ref()], bump)]
    pub social_profile: Account<'info, SocialProfile>,
    #[account(mut)] pub owner: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UpdateSocialProfile<'info> {
    #[account(mut, seeds=[SOCIAL_PROFILE_SEED, owner.key().as_ref()], bump=social_profile.bump, has_one=owner)]
    pub social_profile: Account<'info, SocialProfile>,
    pub owner: Signer<'info>,
}

#[derive(Accounts)]
pub struct FollowUser<'info> {
    #[account(init, payer=follower, space=FollowAccount::SIZE, seeds=[FOLLOW_SEED, follower.key().as_ref(), followee_profile.owner.as_ref()], bump)]
    pub follow_account: Account<'info, FollowAccount>,
    #[account(mut, seeds=[SOCIAL_PROFILE_SEED, follower.key().as_ref()], bump=follower_profile.bump)]
    pub follower_profile: Account<'info, SocialProfile>,
    #[account(mut)]
    pub followee_profile: Account<'info, SocialProfile>,
    #[account(mut)] pub follower: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct CreateSocialPost<'info> {
    #[account(init, payer=author, space=SocialPost::SIZE, seeds=[SOCIAL_POST_SEED, author.key().as_ref(), &author_profile.post_count.to_le_bytes()], bump)]
    pub social_post: Account<'info, SocialPost>,
    #[account(mut, seeds=[SOCIAL_PROFILE_SEED, author.key().as_ref()], bump=author_profile.bump)]
    pub author_profile: Account<'info, SocialProfile>,
    #[account(mut)] pub author: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct LikePost<'info> {
    #[account(mut, seeds=[SOCIAL_POST_SEED, social_post.author.as_ref(), &social_post.post_id.to_le_bytes()], bump=social_post.bump)]
    pub social_post: Account<'info, SocialPost>,
    #[account(init, payer=liker, space=LikeAccount::SIZE, seeds=[LIKE_SEED, liker.key().as_ref(), social_post.author.as_ref(), &social_post.post_id.to_le_bytes()], bump)]
    pub like_account: Account<'info, LikeAccount>,
    #[account(mut)] pub liker: Signer<'info>,
    pub system_program: Program<'info, System>,
}

// ── Loyalty (Torque) Contexts ──

#[derive(Accounts)]
pub struct InitLoyaltyConfig<'info> {
    #[account(init, payer=authority, space=LoyaltyConfig::SIZE, seeds=[LOYALTY_CONFIG_SEED, token_mint.key().as_ref()], bump)]
    pub loyalty_config: Account<'info, LoyaltyConfig>,
    pub token_mint: Account<'info, Mint>,
    #[account(mut)] pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct AwardLoyaltyPoints<'info> {
    #[account(mut, seeds=[LOYALTY_CONFIG_SEED, loyalty_config.mint.as_ref()], bump=loyalty_config.bump, has_one=authority)]
    pub loyalty_config: Account<'info, LoyaltyConfig>,
    #[account(init_if_needed, payer=authority, space=LoyaltyAccount::SIZE, seeds=[LOYALTY_ACCOUNT_SEED, loyalty_config.mint.as_ref(), user.key().as_ref()], bump)]
    pub loyalty_account: Account<'info, LoyaltyAccount>,
    /// CHECK: User receiving points
    pub user: AccountInfo<'info>,
    #[account(mut)] pub authority: Signer<'info>,
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
    #[msg("Token has not graduated")] NotGraduated,
    #[msg("Invalid swap — set exactly one of sol_amount or token_amount")] InvalidSwap,
    #[msg("Title too long")] TitleTooLong,
    #[msg("Description too long")] DescTooLong,
    #[msg("Proposal already finalized")] ProposalFinalized,
    #[msg("Bio too long")] BioTooLong,
    #[msg("Post too long")] PostTooLong,
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
