use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer, MintTo, Burn};
use anchor_spl::associated_token::AssociatedToken;

pub mod achievements;
pub mod airdrops;
pub mod analytics;
pub mod bridge;
pub mod copy_trading;
pub mod creator_dashboard;
pub mod custom_pages;
pub mod daily_rewards;
pub mod holder_rewards;
pub mod lending;
pub mod limit_orders;
pub mod live_chat;
pub mod perps;
pub mod prediction_market;
pub mod premium;
pub mod price_alerts;
pub mod raffle;
pub mod referral;
pub mod reputation;
pub mod seasons;
pub mod share_cards;
pub mod staking;
pub mod token_chat;
pub mod token_videos;
pub mod voting;

pub mod fee_splitting;
pub mod content_claims;
pub mod embeddable_widgets;
pub mod stable_pairs;
pub mod social_launch;
pub mod points_system;
pub mod fund_tokens;

declare_id!("SendiTLaunchPad111111111111111111111111111");

// ============================================================================
// CONSTANTS
// ============================================================================

pub const PLATFORM_CONFIG_SEED: &[u8] = b"platform_config";
pub const TOKEN_LAUNCH_SEED: &[u8] = b"token_launch";
pub const USER_POSITION_SEED: &[u8] = b"user_position";
pub const PLATFORM_VAULT_SEED: &[u8] = b"platform_vault";
pub const LEADERBOARD_SEED: &[u8] = b"leaderboard";
pub const BLOCKLIST_SEED: &[u8] = b"blocklist";
pub const CREATOR_VESTING_SEED: &[u8] = b"creator_vesting";

pub const MAX_NAME_LEN: usize = 32;
pub const MAX_SYMBOL_LEN: usize = 10;
pub const MAX_URI_LEN: usize = 200;
pub const MAX_BLOCKLIST_SIZE: usize = 50;
pub const MAX_LEADERBOARD_SIZE: usize = 20;
pub const LAMPORTS_PER_SOL: u64 = 1_000_000_000;

pub const DEFAULT_PLATFORM_FEE_BPS: u16 = 100; // 1%
pub const DEFAULT_CREATOR_FEE_BPS: u16 = 100;  // 1%
pub const DEFAULT_MIGRATION_THRESHOLD: u64 = 85 * 1_000_000_000; // 85 SOL
pub const DEFAULT_TOTAL_SUPPLY: u64 = 1_000_000_000_000_000; // 1B tokens (6 decimals)
pub const TOKEN_DECIMALS: u8 = 6;

// Curve precision constant (fixed-point math with 1e12 scaling)
pub const PRECISION: u128 = 1_000_000_000_000;

// ============================================================================
// PROGRAM
// ============================================================================

#[program]
pub mod send_it {
    use super::*;

    // ------------------------------------------------------------------------
    // Admin: Initialize Platform Config
    // ------------------------------------------------------------------------
    pub fn initialize_platform(
        ctx: Context<InitializePlatform>,
        platform_fee_bps: u16,
        migration_threshold: u64,
    ) -> Result<()> {
        require!(platform_fee_bps <= 1000, SendItError::FeeTooHigh); // max 10%

        let config = &mut ctx.accounts.platform_config;
        config.authority = ctx.accounts.authority.key();
        config.platform_fee_bps = platform_fee_bps;
        config.migration_threshold = if migration_threshold == 0 {
            DEFAULT_MIGRATION_THRESHOLD
        } else {
            migration_threshold
        };
        config.total_launches = 0;
        config.total_volume_sol = 0;
        config.paused = false;
        config.bump = ctx.bumps.platform_config;

        emit!(PlatformInitialized {
            authority: config.authority,
            platform_fee_bps,
            migration_threshold: config.migration_threshold,
        });

        Ok(())
    }

    // ------------------------------------------------------------------------
    // Admin: Update Platform Config
    // ------------------------------------------------------------------------
    pub fn update_platform_config(
        ctx: Context<UpdatePlatformConfig>,
        new_fee_bps: Option<u16>,
        new_migration_threshold: Option<u64>,
        new_authority: Option<Pubkey>,
    ) -> Result<()> {
        let config = &mut ctx.accounts.platform_config;

        if let Some(fee) = new_fee_bps {
            require!(fee <= 1000, SendItError::FeeTooHigh);
            config.platform_fee_bps = fee;
        }
        if let Some(threshold) = new_migration_threshold {
            config.migration_threshold = threshold;
        }
        if let Some(auth) = new_authority {
            config.authority = auth;
        }

        Ok(())
    }

    // ------------------------------------------------------------------------
    // Admin: Emergency Pause / Unpause
    // ------------------------------------------------------------------------
    pub fn set_paused(ctx: Context<UpdatePlatformConfig>, paused: bool) -> Result<()> {
        ctx.accounts.platform_config.paused = paused;
        emit!(PlatformPauseToggled { paused });
        Ok(())
    }

    // ------------------------------------------------------------------------
    // Admin: Initialize Leaderboard
    // ------------------------------------------------------------------------
    pub fn initialize_leaderboard(ctx: Context<InitializeLeaderboard>) -> Result<()> {
        let lb = &mut ctx.accounts.leaderboard;
        lb.top_tokens_by_volume = Vec::new();
        lb.top_creators_by_launches = Vec::new();
        lb.top_creators_by_volume = Vec::new();
        lb.bump = ctx.bumps.leaderboard;
        Ok(())
    }

    // ------------------------------------------------------------------------
    // Admin: Manage Blocklist
    // ------------------------------------------------------------------------
    pub fn initialize_blocklist(ctx: Context<InitializeBlocklist>) -> Result<()> {
        let bl = &mut ctx.accounts.blocklist;
        bl.blocked_wallets = Vec::new();
        bl.authority = ctx.accounts.authority.key();
        bl.bump = ctx.bumps.blocklist;
        Ok(())
    }

    pub fn add_to_blocklist(ctx: Context<ManageBlocklist>, wallet: Pubkey) -> Result<()> {
        let bl = &mut ctx.accounts.blocklist;
        require!(bl.blocked_wallets.len() < MAX_BLOCKLIST_SIZE, SendItError::BlocklistFull);
        if !bl.blocked_wallets.contains(&wallet) {
            bl.blocked_wallets.push(wallet);
        }
        Ok(())
    }

    pub fn remove_from_blocklist(ctx: Context<ManageBlocklist>, wallet: Pubkey) -> Result<()> {
        let bl = &mut ctx.accounts.blocklist;
        bl.blocked_wallets.retain(|w| w != &wallet);
        Ok(())
    }

    // ------------------------------------------------------------------------
    // Create Token Launch
    // ------------------------------------------------------------------------
    pub fn create_token(
        ctx: Context<CreateToken>,
        name: String,
        symbol: String,
        uri: String,
        curve_type: CurveType,
        creator_fee_bps: u16,
        launch_delay_seconds: i64,
        snipe_window_seconds: i64,
        max_buy_during_snipe: u64,
        lock_period_seconds: i64,
        creator_vesting_duration: i64,
        creator_token_allocation_bps: u16, // basis points of total supply to creator
    ) -> Result<()> {
        require!(name.len() <= MAX_NAME_LEN, SendItError::NameTooLong);
        require!(symbol.len() <= MAX_SYMBOL_LEN, SendItError::SymbolTooLong);
        require!(uri.len() <= MAX_URI_LEN, SendItError::UriTooLong);
        require!(creator_fee_bps <= 500, SendItError::FeeTooHigh); // max 5%
        require!(creator_token_allocation_bps <= 1000, SendItError::AllocationTooHigh); // max 10%

        let config = &ctx.accounts.platform_config;
        require!(!config.paused, SendItError::PlatformPaused);

        let clock = Clock::get()?;

        let launch = &mut ctx.accounts.token_launch;
        launch.creator = ctx.accounts.creator.key();
        launch.mint = ctx.accounts.token_mint.key();
        launch.name = name.clone();
        launch.symbol = symbol.clone();
        launch.uri = uri.clone();
        launch.curve_type = curve_type;
        launch.creator_fee_bps = creator_fee_bps;
        launch.total_supply = DEFAULT_TOTAL_SUPPLY;
        launch.tokens_sold = 0;
        launch.reserve_sol = 0;
        launch.created_at = clock.unix_timestamp;
        launch.trading_starts_at = clock.unix_timestamp + launch_delay_seconds;
        launch.snipe_window_end = clock.unix_timestamp + launch_delay_seconds + snipe_window_seconds;
        launch.max_buy_during_snipe = if max_buy_during_snipe == 0 {
            DEFAULT_TOTAL_SUPPLY / 100 // 1% max during snipe window
        } else {
            max_buy_during_snipe
        };
        launch.lock_period_end = clock.unix_timestamp + lock_period_seconds;
        launch.migrated = false;
        launch.paused = false;
        launch.total_volume_sol = 0;
        launch.bump = ctx.bumps.token_launch;

        // Mint total supply to the launch's token vault
        let creator_allocation = (DEFAULT_TOTAL_SUPPLY as u128)
            .checked_mul(creator_token_allocation_bps as u128)
            .unwrap()
            .checked_div(10_000)
            .unwrap() as u64;
        let curve_supply = DEFAULT_TOTAL_SUPPLY - creator_allocation;

        // Mint curve supply to the launch vault
        let cpi_accounts = MintTo {
            mint: ctx.accounts.token_mint.to_account_info(),
            to: ctx.accounts.launch_token_vault.to_account_info(),
            authority: ctx.accounts.token_mint.to_account_info(),
        };
        let mint_key = ctx.accounts.token_mint.key();
        let seeds: &[&[u8]] = &[
            TOKEN_LAUNCH_SEED,
            mint_key.as_ref(),
            &[launch.bump],
        ];
        // The mint authority is the token_launch PDA
        // Actually we need the mint authority to be the launch PDA
        // For simplicity, mint authority = launch PDA
        let launch_key = launch.key();
        let launch_seeds: &[&[u8]] = &[
            TOKEN_LAUNCH_SEED,
            ctx.accounts.token_mint.to_account_info().key.as_ref(),
            &[launch.bump],
        ];
        let signer_seeds = &[launch_seeds];

        token::mint_to(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                cpi_accounts,
                signer_seeds,
            ),
            curve_supply,
        )?;

        // Setup creator vesting if allocation > 0
        if creator_allocation > 0 {
            let vesting = &mut ctx.accounts.creator_vesting;
            vesting.creator = ctx.accounts.creator.key();
            vesting.mint = ctx.accounts.token_mint.key();
            vesting.total_amount = creator_allocation;
            vesting.claimed_amount = 0;
            vesting.vesting_start = clock.unix_timestamp;
            vesting.vesting_duration = creator_vesting_duration;
            vesting.bump = ctx.bumps.creator_vesting;

            // Mint creator allocation to vesting vault (held by launch PDA)
            let vest_cpi = MintTo {
                mint: ctx.accounts.token_mint.to_account_info(),
                to: ctx.accounts.launch_token_vault.to_account_info(),
                authority: ctx.accounts.token_mint.to_account_info(),
            };
            // Mint authority is the launch PDA
            token::mint_to(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    vest_cpi,
                    signer_seeds,
                ),
                creator_allocation,
            )?;
        }

        // Update platform stats
        let config = &mut ctx.accounts.platform_config;
        config.total_launches += 1;

        emit!(TokenCreated {
            mint: launch.mint,
            creator: launch.creator,
            name,
            symbol,
            curve_type,
            trading_starts_at: launch.trading_starts_at,
        });

        Ok(())
    }

    // ------------------------------------------------------------------------
    // Buy Tokens
    // ------------------------------------------------------------------------
    pub fn buy(ctx: Context<BuyTokens>, sol_amount: u64) -> Result<()> {
        require!(sol_amount > 0, SendItError::ZeroAmount);

        let config = &ctx.accounts.platform_config;
        require!(!config.paused, SendItError::PlatformPaused);

        let launch = &ctx.accounts.token_launch;
        require!(!launch.paused, SendItError::TokenPaused);
        require!(!launch.migrated, SendItError::AlreadyMigrated);

        let clock = Clock::get()?;
        require!(clock.unix_timestamp >= launch.trading_starts_at, SendItError::TradingNotStarted);

        // Check blocklist
        let blocklist = &ctx.accounts.blocklist;
        require!(
            !blocklist.blocked_wallets.contains(&ctx.accounts.buyer.key()),
            SendItError::WalletBlocked
        );

        // Anti-snipe check
        if clock.unix_timestamp < launch.snipe_window_end {
            let position = &ctx.accounts.user_position;
            let would_buy = position.tokens_bought.checked_add(sol_amount).unwrap_or(u64::MAX);
            require!(would_buy <= launch.max_buy_during_snipe, SendItError::SnipeLimitExceeded);
        }

        // Calculate tokens out from bonding curve
        let launch_data = &ctx.accounts.token_launch;
        let tokens_out = calculate_tokens_for_sol(
            launch_data.curve_type,
            launch_data.tokens_sold,
            launch_data.total_supply,
            sol_amount,
        )?;
        require!(tokens_out > 0, SendItError::InsufficientOutput);

        // Calculate fees
        let platform_fee = (sol_amount as u128)
            .checked_mul(config.platform_fee_bps as u128)
            .unwrap()
            .checked_div(10_000)
            .unwrap() as u64;

        let creator_fee = (sol_amount as u128)
            .checked_mul(launch_data.creator_fee_bps as u128)
            .unwrap()
            .checked_div(10_000)
            .unwrap() as u64;

        let net_sol = sol_amount
            .checked_sub(platform_fee)
            .unwrap()
            .checked_sub(creator_fee)
            .unwrap();

        // Transfer SOL from buyer to launch vault (reserve)
        anchor_lang::system_program::transfer(
            CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.buyer.to_account_info(),
                    to: ctx.accounts.launch_sol_vault.to_account_info(),
                },
            ),
            net_sol,
        )?;

        // Transfer platform fee to platform vault
        if platform_fee > 0 {
            anchor_lang::system_program::transfer(
                CpiContext::new(
                    ctx.accounts.system_program.to_account_info(),
                    anchor_lang::system_program::Transfer {
                        from: ctx.accounts.buyer.to_account_info(),
                        to: ctx.accounts.platform_vault.to_account_info(),
                    },
                ),
                platform_fee,
            )?;
        }

        // Transfer creator fee
        if creator_fee > 0 {
            anchor_lang::system_program::transfer(
                CpiContext::new(
                    ctx.accounts.system_program.to_account_info(),
                    anchor_lang::system_program::Transfer {
                        from: ctx.accounts.buyer.to_account_info(),
                        to: ctx.accounts.creator_wallet.to_account_info(),
                    },
                ),
                creator_fee,
            )?;
        }

        // Transfer tokens from launch vault to buyer
        let mint_key = ctx.accounts.token_mint.key();
        let launch_seeds: &[&[u8]] = &[
            TOKEN_LAUNCH_SEED,
            mint_key.as_ref(),
            &[ctx.accounts.token_launch.bump],
        ];
        let signer_seeds = &[launch_seeds];

        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.launch_token_vault.to_account_info(),
                    to: ctx.accounts.buyer_token_account.to_account_info(),
                    authority: ctx.accounts.token_launch.to_account_info(),
                },
                signer_seeds,
            ),
            tokens_out,
        )?;

        // Update launch state
        let launch = &mut ctx.accounts.token_launch;
        launch.tokens_sold = launch.tokens_sold.checked_add(tokens_out).unwrap();
        launch.reserve_sol = launch.reserve_sol.checked_add(net_sol).unwrap();
        launch.total_volume_sol = launch.total_volume_sol.checked_add(sol_amount).unwrap();

        // Update user position
        let position = &mut ctx.accounts.user_position;
        position.owner = ctx.accounts.buyer.key();
        position.mint = ctx.accounts.token_mint.key();
        position.tokens_bought = position.tokens_bought.checked_add(tokens_out).unwrap();
        position.sol_spent = position.sol_spent.checked_add(sol_amount).unwrap();
        if position.bump == 0 {
            position.bump = ctx.bumps.user_position;
        }

        // Update platform volume
        let config = &mut ctx.accounts.platform_config;
        config.total_volume_sol = config.total_volume_sol.checked_add(sol_amount).unwrap();

        let current_price = get_current_price(
            launch.curve_type,
            launch.tokens_sold,
            launch.total_supply,
        );

        emit!(TokenBought {
            mint: launch.mint,
            buyer: ctx.accounts.buyer.key(),
            sol_amount,
            tokens_received: tokens_out,
            new_price: current_price,
            platform_fee,
            creator_fee,
        });

        // Check migration threshold
        if launch.reserve_sol >= ctx.accounts.platform_config.migration_threshold && !launch.migrated {
            emit!(MigrationReady {
                mint: launch.mint,
                reserve_sol: launch.reserve_sol,
            });
        }

        Ok(())
    }

    // ------------------------------------------------------------------------
    // Sell Tokens
    // ------------------------------------------------------------------------
    pub fn sell(ctx: Context<SellTokens>, token_amount: u64) -> Result<()> {
        require!(token_amount > 0, SendItError::ZeroAmount);

        let config = &ctx.accounts.platform_config;
        require!(!config.paused, SendItError::PlatformPaused);

        let launch = &ctx.accounts.token_launch;
        require!(!launch.paused, SendItError::TokenPaused);
        require!(!launch.migrated, SendItError::AlreadyMigrated);

        // Calculate SOL out from bonding curve (reverse)
        let sol_out = calculate_sol_for_tokens(
            launch.curve_type,
            launch.tokens_sold,
            launch.total_supply,
            token_amount,
        )?;
        require!(sol_out > 0, SendItError::InsufficientOutput);

        // Calculate fees
        let platform_fee = (sol_out as u128)
            .checked_mul(config.platform_fee_bps as u128)
            .unwrap()
            .checked_div(10_000)
            .unwrap() as u64;

        let creator_fee = (sol_out as u128)
            .checked_mul(launch.creator_fee_bps as u128)
            .unwrap()
            .checked_div(10_000)
            .unwrap() as u64;

        let net_sol_out = sol_out
            .checked_sub(platform_fee)
            .unwrap()
            .checked_sub(creator_fee)
            .unwrap();

        require!(net_sol_out <= launch.reserve_sol, SendItError::InsufficientReserve);

        // Transfer tokens from seller to launch vault (burn back into curve)
        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.seller_token_account.to_account_info(),
                    to: ctx.accounts.launch_token_vault.to_account_info(),
                    authority: ctx.accounts.seller.to_account_info(),
                },
            ),
            token_amount,
        )?;

        // Transfer SOL from launch vault to seller
        let mint_key = ctx.accounts.token_mint.key();
        let launch_seeds: &[&[u8]] = &[
            TOKEN_LAUNCH_SEED,
            mint_key.as_ref(),
            &[ctx.accounts.token_launch.bump],
        ];

        // SOL vault is a PDA owned by the launch, transfer via lamports
        **ctx.accounts.launch_sol_vault.to_account_info().try_borrow_mut_lamports()? -= net_sol_out;
        **ctx.accounts.seller.to_account_info().try_borrow_mut_lamports()? += net_sol_out;

        // Platform fee
        if platform_fee > 0 {
            **ctx.accounts.launch_sol_vault.to_account_info().try_borrow_mut_lamports()? -= platform_fee;
            **ctx.accounts.platform_vault.to_account_info().try_borrow_mut_lamports()? += platform_fee;
        }

        // Creator fee
        if creator_fee > 0 {
            **ctx.accounts.launch_sol_vault.to_account_info().try_borrow_mut_lamports()? -= creator_fee;
            **ctx.accounts.creator_wallet.to_account_info().try_borrow_mut_lamports()? += creator_fee;
        }

        // Update state
        let launch = &mut ctx.accounts.token_launch;
        launch.tokens_sold = launch.tokens_sold.checked_sub(token_amount).unwrap();
        launch.reserve_sol = launch.reserve_sol.checked_sub(sol_out).unwrap();
        launch.total_volume_sol = launch.total_volume_sol.checked_add(sol_out).unwrap();

        // Update user position
        let position = &mut ctx.accounts.user_position;
        position.tokens_bought = position.tokens_bought.saturating_sub(token_amount);

        // Update platform volume
        let config = &mut ctx.accounts.platform_config;
        config.total_volume_sol = config.total_volume_sol.checked_add(sol_out).unwrap();

        let current_price = get_current_price(
            launch.curve_type,
            launch.tokens_sold,
            launch.total_supply,
        );

        emit!(TokenSold {
            mint: launch.mint,
            seller: ctx.accounts.seller.key(),
            token_amount,
            sol_received: net_sol_out,
            new_price: current_price,
            platform_fee,
            creator_fee,
        });

        Ok(())
    }

    // ------------------------------------------------------------------------
    // Claim Vested Creator Tokens
    // ------------------------------------------------------------------------
    pub fn claim_vested_tokens(ctx: Context<ClaimVestedTokens>) -> Result<()> {
        let vesting = &ctx.accounts.creator_vesting;
        let clock = Clock::get()?;

        let elapsed = clock.unix_timestamp.checked_sub(vesting.vesting_start).unwrap_or(0);
        let vested_amount = if elapsed >= vesting.vesting_duration {
            vesting.total_amount
        } else {
            (vesting.total_amount as u128)
                .checked_mul(elapsed as u128)
                .unwrap()
                .checked_div(vesting.vesting_duration as u128)
                .unwrap() as u64
        };

        let claimable = vested_amount.checked_sub(vesting.claimed_amount).unwrap_or(0);
        require!(claimable > 0, SendItError::NothingToClaim);

        // Transfer from launch vault to creator
        let mint_key = ctx.accounts.token_mint.key();
        let launch_seeds: &[&[u8]] = &[
            TOKEN_LAUNCH_SEED,
            mint_key.as_ref(),
            &[ctx.accounts.token_launch.bump],
        ];
        let signer_seeds = &[launch_seeds];

        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.launch_token_vault.to_account_info(),
                    to: ctx.accounts.creator_token_account.to_account_info(),
                    authority: ctx.accounts.token_launch.to_account_info(),
                },
                signer_seeds,
            ),
            claimable,
        )?;

        let vesting = &mut ctx.accounts.creator_vesting;
        vesting.claimed_amount = vesting.claimed_amount.checked_add(claimable).unwrap();

        emit!(VestedTokensClaimed {
            creator: ctx.accounts.creator.key(),
            mint: ctx.accounts.token_mint.key(),
            amount: claimable,
        });

        Ok(())
    }

    // ------------------------------------------------------------------------
    // Migrate to Raydium (triggered when threshold met)
    // ------------------------------------------------------------------------
    pub fn migrate_to_raydium(ctx: Context<MigrateToRaydium>) -> Result<()> {
        let launch = &ctx.accounts.token_launch;
        let config = &ctx.accounts.platform_config;

        require!(!launch.migrated, SendItError::AlreadyMigrated);
        require!(
            launch.reserve_sol >= config.migration_threshold,
            SendItError::MigrationThresholdNotMet
        );

        let clock = Clock::get()?;
        require!(
            clock.unix_timestamp >= launch.lock_period_end,
            SendItError::LockPeriodActive
        );

        // In production, this would CPI into Raydium AMM to:
        // 1. Create a Raydium liquidity pool
        // 2. Deposit SOL reserve + remaining tokens as liquidity
        // 3. Lock the LP tokens in a PDA
        //
        // For now, we mark as migrated and emit event.
        // The actual Raydium CPI requires their specific program accounts
        // which would be passed as remaining_accounts.

        let launch = &mut ctx.accounts.token_launch;
        launch.migrated = true;

        emit!(MigratedToRaydium {
            mint: launch.mint,
            reserve_sol: launch.reserve_sol,
            tokens_remaining: launch.total_supply.checked_sub(launch.tokens_sold).unwrap_or(0),
            timestamp: clock.unix_timestamp,
        });

        Ok(())
    }

    // ------------------------------------------------------------------------
    // Update Leaderboard (permissionless crank)
    // ------------------------------------------------------------------------
    pub fn update_leaderboard(ctx: Context<UpdateLeaderboard>) -> Result<()> {
        let launch = &ctx.accounts.token_launch;
        let lb = &mut ctx.accounts.leaderboard;

        // Update top tokens by volume
        update_top_list(
            &mut lb.top_tokens_by_volume,
            LeaderboardEntry {
                key: launch.mint,
                value: launch.total_volume_sol,
                secondary_key: launch.creator,
            },
            MAX_LEADERBOARD_SIZE,
        );

        // Update top creators by volume
        update_top_list(
            &mut lb.top_creators_by_volume,
            LeaderboardEntry {
                key: launch.creator,
                value: launch.total_volume_sol,
                secondary_key: launch.mint,
            },
            MAX_LEADERBOARD_SIZE,
        );

        Ok(())
    }
}

// ============================================================================
// BONDING CURVE MATH
// ============================================================================

/// Calculate how many tokens you get for a given SOL input
pub fn calculate_tokens_for_sol(
    curve: CurveType,
    tokens_sold: u64,
    total_supply: u64,
    sol_amount: u64,
) -> Result<u64> {
    match curve {
        CurveType::Linear => {
            // Price = base_price + slope * tokens_sold
            // base_price = 0.000001 SOL, slope scales to reach ~0.001 SOL at full supply
            // Integral: cost = base_price * tokens + slope * tokens^2 / 2
            let base = 1_000u128; // 0.000001 SOL in lamports
            let slope = PRECISION / (total_supply as u128); // normalized slope

            let s0 = tokens_sold as u128;
            let sol = sol_amount as u128;

            // Solve: sol = base * (s1 - s0) + slope * (s1^2 - s0^2) / (2 * PRECISION)
            // Using quadratic approximation for efficiency
            let current_price = base + slope.checked_mul(s0).unwrap() / PRECISION;
            let tokens = if current_price > 0 {
                sol.checked_mul(LAMPORTS_PER_SOL as u128)
                    .unwrap()
                    .checked_div(current_price)
                    .unwrap_or(0)
                    .min((total_supply - tokens_sold) as u128)
            } else {
                0
            };
            Ok(tokens as u64)
        }
        CurveType::Exponential => {
            // Price = base_price * e^(k * tokens_sold / total_supply)
            // Approximation using: price = base * (1 + ratio)^3 where ratio = sold/supply
            let base = 1_000u128;
            let ratio = (tokens_sold as u128)
                .checked_mul(PRECISION)
                .unwrap()
                .checked_div(total_supply as u128)
                .unwrap();

            // Cubic growth approximation
            let multiplier = PRECISION
                .checked_add(ratio.checked_mul(3).unwrap())
                .unwrap();
            let current_price = base.checked_mul(multiplier).unwrap() / PRECISION;

            let tokens = if current_price > 0 {
                (sol_amount as u128)
                    .checked_mul(LAMPORTS_PER_SOL as u128)
                    .unwrap()
                    .checked_div(current_price)
                    .unwrap_or(0)
                    .min((total_supply - tokens_sold) as u128)
            } else {
                0
            };
            Ok(tokens as u64)
        }
        CurveType::Sigmoid => {
            // S-curve: slow start, fast middle, slow end
            // price = base * (1 + sigmoid_scale * sigmoid(sold/supply))
            let base = 1_000u128;
            let ratio = (tokens_sold as u128)
                .checked_mul(100)
                .unwrap()
                .checked_div(total_supply as u128)
                .unwrap(); // 0-100

            // Piecewise sigmoid approximation
            let price_multiplier = if ratio < 20 {
                PRECISION + ratio * PRECISION / 100
            } else if ratio < 80 {
                PRECISION + (ratio - 10) * PRECISION / 25
            } else {
                PRECISION * 4
            };

            let current_price = base.checked_mul(price_multiplier).unwrap() / PRECISION;

            let tokens = if current_price > 0 {
                (sol_amount as u128)
                    .checked_mul(LAMPORTS_PER_SOL as u128)
                    .unwrap()
                    .checked_div(current_price)
                    .unwrap_or(0)
                    .min((total_supply - tokens_sold) as u128)
            } else {
                0
            };
            Ok(tokens as u64)
        }
    }
}

/// Calculate SOL returned for selling tokens
pub fn calculate_sol_for_tokens(
    curve: CurveType,
    tokens_sold: u64,
    total_supply: u64,
    token_amount: u64,
) -> Result<u64> {
    require!(token_amount <= tokens_sold, SendItError::InsufficientTokensSold);

    let new_sold = tokens_sold - token_amount;

    // Calculate what it would cost to buy from new_sold to tokens_sold
    // That's the SOL returned (area under curve between those points)
    match curve {
        CurveType::Linear => {
            let base = 1_000u128;
            let slope = PRECISION / (total_supply as u128);
            let s0 = new_sold as u128;
            let s1 = tokens_sold as u128;

            let avg_price = base + slope.checked_mul(s0 + s1).unwrap() / (2 * PRECISION);
            let sol = avg_price
                .checked_mul(s1 - s0)
                .unwrap()
                .checked_div(LAMPORTS_PER_SOL as u128)
                .unwrap_or(0);
            Ok(sol as u64)
        }
        CurveType::Exponential => {
            let base = 1_000u128;
            let ratio = (tokens_sold as u128)
                .checked_mul(PRECISION)
                .unwrap()
                .checked_div(total_supply as u128)
                .unwrap();
            let multiplier = PRECISION.checked_add(ratio.checked_mul(3).unwrap()).unwrap();
            let current_price = base.checked_mul(multiplier).unwrap() / PRECISION;

            let sol = current_price
                .checked_mul(token_amount as u128)
                .unwrap()
                .checked_div(LAMPORTS_PER_SOL as u128)
                .unwrap_or(0);
            Ok(sol as u64)
        }
        CurveType::Sigmoid => {
            let base = 1_000u128;
            let ratio = (tokens_sold as u128)
                .checked_mul(100)
                .unwrap()
                .checked_div(total_supply as u128)
                .unwrap();

            let price_multiplier = if ratio < 20 {
                PRECISION + ratio * PRECISION / 100
            } else if ratio < 80 {
                PRECISION + (ratio - 10) * PRECISION / 25
            } else {
                PRECISION * 4
            };

            let current_price = base.checked_mul(price_multiplier).unwrap() / PRECISION;

            let sol = current_price
                .checked_mul(token_amount as u128)
                .unwrap()
                .checked_div(LAMPORTS_PER_SOL as u128)
                .unwrap_or(0);
            Ok(sol as u64)
        }
    }
}

pub fn get_current_price(curve: CurveType, tokens_sold: u64, total_supply: u64) -> u64 {
    let base = 1_000u128;
    match curve {
        CurveType::Linear => {
            let slope = PRECISION / (total_supply as u128);
            let price = base + slope * (tokens_sold as u128) / PRECISION;
            price as u64
        }
        CurveType::Exponential => {
            let ratio = (tokens_sold as u128) * PRECISION / (total_supply as u128);
            let multiplier = PRECISION + ratio * 3;
            (base * multiplier / PRECISION) as u64
        }
        CurveType::Sigmoid => {
            let ratio = (tokens_sold as u128) * 100 / (total_supply as u128);
            let m = if ratio < 20 {
                PRECISION + ratio * PRECISION / 100
            } else if ratio < 80 {
                PRECISION + (ratio - 10) * PRECISION / 25
            } else {
                PRECISION * 4
            };
            (base * m / PRECISION) as u64
        }
    }
}

fn update_top_list(list: &mut Vec<LeaderboardEntry>, entry: LeaderboardEntry, max_size: usize) {
    // Check if entry already exists, update if so
    if let Some(existing) = list.iter_mut().find(|e| e.key == entry.key) {
        existing.value = entry.value;
    } else {
        list.push(entry);
    }
    // Sort descending by value
    list.sort_by(|a, b| b.value.cmp(&a.value));
    // Truncate
    list.truncate(max_size);
}

// ============================================================================
// ACCOUNT STRUCTS
// ============================================================================

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

impl PlatformConfig {
    pub const SIZE: usize = 8 + 32 + 2 + 8 + 8 + 8 + 1 + 1;
}

#[account]
pub struct TokenLaunch {
    pub creator: Pubkey,
    pub mint: Pubkey,
    pub name: String,
    pub symbol: String,
    pub uri: String,
    pub curve_type: CurveType,
    pub creator_fee_bps: u16,
    pub total_supply: u64,
    pub tokens_sold: u64,
    pub reserve_sol: u64,
    pub created_at: i64,
    pub trading_starts_at: i64,
    pub snipe_window_end: i64,
    pub max_buy_during_snipe: u64,
    pub lock_period_end: i64,
    pub migrated: bool,
    pub paused: bool,
    pub total_volume_sol: u64,
    pub bump: u8,
}

impl TokenLaunch {
    pub const SIZE: usize = 8  // discriminator
        + 32   // creator
        + 32   // mint
        + (4 + MAX_NAME_LEN)    // name
        + (4 + MAX_SYMBOL_LEN)  // symbol
        + (4 + MAX_URI_LEN)     // uri
        + 1    // curve_type
        + 2    // creator_fee_bps
        + 8    // total_supply
        + 8    // tokens_sold
        + 8    // reserve_sol
        + 8    // created_at
        + 8    // trading_starts_at
        + 8    // snipe_window_end
        + 8    // max_buy_during_snipe
        + 8    // lock_period_end
        + 1    // migrated
        + 1    // paused
        + 8    // total_volume_sol
        + 1;   // bump
}

#[account]
pub struct UserPosition {
    pub owner: Pubkey,
    pub mint: Pubkey,
    pub tokens_bought: u64,
    pub sol_spent: u64,
    pub bump: u8,
}

impl UserPosition {
    pub const SIZE: usize = 8 + 32 + 32 + 8 + 8 + 1;
}

#[account]
pub struct CreatorVesting {
    pub creator: Pubkey,
    pub mint: Pubkey,
    pub total_amount: u64,
    pub claimed_amount: u64,
    pub vesting_start: i64,
    pub vesting_duration: i64,
    pub bump: u8,
}

impl CreatorVesting {
    pub const SIZE: usize = 8 + 32 + 32 + 8 + 8 + 8 + 8 + 1;
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct LeaderboardEntry {
    pub key: Pubkey,
    pub value: u64,
    pub secondary_key: Pubkey,
}

impl LeaderboardEntry {
    pub const SIZE: usize = 32 + 8 + 32;
}

#[account]
pub struct Leaderboard {
    pub top_tokens_by_volume: Vec<LeaderboardEntry>,
    pub top_creators_by_launches: Vec<LeaderboardEntry>,
    pub top_creators_by_volume: Vec<LeaderboardEntry>,
    pub bump: u8,
}

impl Leaderboard {
    pub const SIZE: usize = 8
        + (4 + MAX_LEADERBOARD_SIZE * LeaderboardEntry::SIZE)
        + (4 + MAX_LEADERBOARD_SIZE * LeaderboardEntry::SIZE)
        + (4 + MAX_LEADERBOARD_SIZE * LeaderboardEntry::SIZE)
        + 1;
}

#[account]
pub struct Blocklist {
    pub blocked_wallets: Vec<Pubkey>,
    pub authority: Pubkey,
    pub bump: u8,
}

impl Blocklist {
    pub const SIZE: usize = 8 + (4 + MAX_BLOCKLIST_SIZE * 32) + 32 + 1;
}

// ============================================================================
// ENUMS
// ============================================================================

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
pub enum CurveType {
    Linear,
    Exponential,
    Sigmoid,
}

// ============================================================================
// CONTEXT STRUCTS (ACCOUNTS)
// ============================================================================

#[derive(Accounts)]
pub struct InitializePlatform<'info> {
    #[account(
        init,
        payer = authority,
        space = PlatformConfig::SIZE,
        seeds = [PLATFORM_CONFIG_SEED],
        bump,
    )]
    pub platform_config: Account<'info, PlatformConfig>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UpdatePlatformConfig<'info> {
    #[account(
        mut,
        seeds = [PLATFORM_CONFIG_SEED],
        bump = platform_config.bump,
        has_one = authority,
    )]
    pub platform_config: Account<'info, PlatformConfig>,

    pub authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct InitializeLeaderboard<'info> {
    #[account(
        init,
        payer = authority,
        space = Leaderboard::SIZE,
        seeds = [LEADERBOARD_SEED],
        bump,
    )]
    pub leaderboard: Account<'info, Leaderboard>,

    #[account(
        seeds = [PLATFORM_CONFIG_SEED],
        bump = platform_config.bump,
        has_one = authority,
    )]
    pub platform_config: Account<'info, PlatformConfig>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct InitializeBlocklist<'info> {
    #[account(
        init,
        payer = authority,
        space = Blocklist::SIZE,
        seeds = [BLOCKLIST_SEED],
        bump,
    )]
    pub blocklist: Account<'info, Blocklist>,

    #[account(
        seeds = [PLATFORM_CONFIG_SEED],
        bump = platform_config.bump,
        has_one = authority,
    )]
    pub platform_config: Account<'info, PlatformConfig>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ManageBlocklist<'info> {
    #[account(
        mut,
        seeds = [BLOCKLIST_SEED],
        bump = blocklist.bump,
        has_one = authority,
    )]
    pub blocklist: Account<'info, Blocklist>,

    pub authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct CreateToken<'info> {
    #[account(
        init,
        payer = creator,
        space = TokenLaunch::SIZE,
        seeds = [TOKEN_LAUNCH_SEED, token_mint.key().as_ref()],
        bump,
    )]
    pub token_launch: Account<'info, TokenLaunch>,

    #[account(
        init,
        payer = creator,
        mint::decimals = TOKEN_DECIMALS,
        mint::authority = token_launch,
    )]
    pub token_mint: Account<'info, Mint>,

    #[account(
        init,
        payer = creator,
        associated_token::mint = token_mint,
        associated_token::authority = token_launch,
    )]
    pub launch_token_vault: Account<'info, TokenAccount>,

    #[account(
        init_if_needed,
        payer = creator,
        space = CreatorVesting::SIZE,
        seeds = [CREATOR_VESTING_SEED, token_mint.key().as_ref()],
        bump,
    )]
    pub creator_vesting: Account<'info, CreatorVesting>,

    #[account(
        mut,
        seeds = [PLATFORM_CONFIG_SEED],
        bump = platform_config.bump,
    )]
    pub platform_config: Account<'info, PlatformConfig>,

    #[account(mut)]
    pub creator: Signer<'info>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct BuyTokens<'info> {
    #[account(
        mut,
        seeds = [TOKEN_LAUNCH_SEED, token_mint.key().as_ref()],
        bump = token_launch.bump,
    )]
    pub token_launch: Account<'info, TokenLaunch>,

    pub token_mint: Account<'info, Mint>,

    #[account(
        mut,
        associated_token::mint = token_mint,
        associated_token::authority = token_launch,
    )]
    pub launch_token_vault: Account<'info, TokenAccount>,

    /// CHECK: SOL vault PDA for the launch
    #[account(
        mut,
        seeds = [b"sol_vault", token_mint.key().as_ref()],
        bump,
    )]
    pub launch_sol_vault: AccountInfo<'info>,

    #[account(
        init_if_needed,
        payer = buyer,
        associated_token::mint = token_mint,
        associated_token::authority = buyer,
    )]
    pub buyer_token_account: Account<'info, TokenAccount>,

    #[account(
        init_if_needed,
        payer = buyer,
        space = UserPosition::SIZE,
        seeds = [USER_POSITION_SEED, buyer.key().as_ref(), token_mint.key().as_ref()],
        bump,
    )]
    pub user_position: Account<'info, UserPosition>,

    /// CHECK: Platform vault for fees
    #[account(
        mut,
        seeds = [PLATFORM_VAULT_SEED],
        bump,
    )]
    pub platform_vault: AccountInfo<'info>,

    /// CHECK: Creator wallet for fee share
    #[account(
        mut,
        constraint = creator_wallet.key() == token_launch.creator @ SendItError::InvalidCreator,
    )]
    pub creator_wallet: AccountInfo<'info>,

    #[account(
        mut,
        seeds = [PLATFORM_CONFIG_SEED],
        bump = platform_config.bump,
    )]
    pub platform_config: Account<'info, PlatformConfig>,

    #[account(
        seeds = [BLOCKLIST_SEED],
        bump = blocklist.bump,
    )]
    pub blocklist: Account<'info, Blocklist>,

    #[account(mut)]
    pub buyer: Signer<'info>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct SellTokens<'info> {
    #[account(
        mut,
        seeds = [TOKEN_LAUNCH_SEED, token_mint.key().as_ref()],
        bump = token_launch.bump,
    )]
    pub token_launch: Account<'info, TokenLaunch>,

    pub token_mint: Account<'info, Mint>,

    #[account(
        mut,
        associated_token::mint = token_mint,
        associated_token::authority = token_launch,
    )]
    pub launch_token_vault: Account<'info, TokenAccount>,

    /// CHECK: SOL vault
    #[account(
        mut,
        seeds = [b"sol_vault", token_mint.key().as_ref()],
        bump,
    )]
    pub launch_sol_vault: AccountInfo<'info>,

    #[account(
        mut,
        associated_token::mint = token_mint,
        associated_token::authority = seller,
    )]
    pub seller_token_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        seeds = [USER_POSITION_SEED, seller.key().as_ref(), token_mint.key().as_ref()],
        bump = user_position.bump,
    )]
    pub user_position: Account<'info, UserPosition>,

    /// CHECK: Platform vault
    #[account(
        mut,
        seeds = [PLATFORM_VAULT_SEED],
        bump,
    )]
    pub platform_vault: AccountInfo<'info>,

    /// CHECK: Creator wallet
    #[account(
        mut,
        constraint = creator_wallet.key() == token_launch.creator @ SendItError::InvalidCreator,
    )]
    pub creator_wallet: AccountInfo<'info>,

    #[account(
        mut,
        seeds = [PLATFORM_CONFIG_SEED],
        bump = platform_config.bump,
    )]
    pub platform_config: Account<'info, PlatformConfig>,

    #[account(mut)]
    pub seller: Signer<'info>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ClaimVestedTokens<'info> {
    #[account(
        seeds = [TOKEN_LAUNCH_SEED, token_mint.key().as_ref()],
        bump = token_launch.bump,
    )]
    pub token_launch: Account<'info, TokenLaunch>,

    pub token_mint: Account<'info, Mint>,

    #[account(
        mut,
        associated_token::mint = token_mint,
        associated_token::authority = token_launch,
    )]
    pub launch_token_vault: Account<'info, TokenAccount>,

    #[account(
        mut,
        seeds = [CREATOR_VESTING_SEED, token_mint.key().as_ref()],
        bump = creator_vesting.bump,
        constraint = creator_vesting.creator == creator.key() @ SendItError::InvalidCreator,
    )]
    pub creator_vesting: Account<'info, CreatorVesting>,

    #[account(
        init_if_needed,
        payer = creator,
        associated_token::mint = token_mint,
        associated_token::authority = creator,
    )]
    pub creator_token_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub creator: Signer<'info>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct MigrateToRaydium<'info> {
    #[account(
        mut,
        seeds = [TOKEN_LAUNCH_SEED, token_mint.key().as_ref()],
        bump = token_launch.bump,
    )]
    pub token_launch: Account<'info, TokenLaunch>,

    pub token_mint: Account<'info, Mint>,

    #[account(
        seeds = [PLATFORM_CONFIG_SEED],
        bump = platform_config.bump,
    )]
    pub platform_config: Account<'info, PlatformConfig>,

    /// CHECK: Can be called by anyone (permissionless crank)
    #[account(mut)]
    pub payer: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UpdateLeaderboard<'info> {
    #[account(
        seeds = [TOKEN_LAUNCH_SEED, token_mint.key().as_ref()],
        bump = token_launch.bump,
    )]
    pub token_launch: Account<'info, TokenLaunch>,

    pub token_mint: Account<'info, Mint>,

    #[account(
        mut,
        seeds = [LEADERBOARD_SEED],
        bump = leaderboard.bump,
    )]
    pub leaderboard: Account<'info, Leaderboard>,
}

// ============================================================================
// EVENTS
// ============================================================================

#[event]
pub struct PlatformInitialized {
    pub authority: Pubkey,
    pub platform_fee_bps: u16,
    pub migration_threshold: u64,
}

#[event]
pub struct PlatformPauseToggled {
    pub paused: bool,
}

#[event]
pub struct TokenCreated {
    pub mint: Pubkey,
    pub creator: Pubkey,
    pub name: String,
    pub symbol: String,
    pub curve_type: CurveType,
    pub trading_starts_at: i64,
}

#[event]
pub struct TokenBought {
    pub mint: Pubkey,
    pub buyer: Pubkey,
    pub sol_amount: u64,
    pub tokens_received: u64,
    pub new_price: u64,
    pub platform_fee: u64,
    pub creator_fee: u64,
}

#[event]
pub struct TokenSold {
    pub mint: Pubkey,
    pub seller: Pubkey,
    pub token_amount: u64,
    pub sol_received: u64,
    pub new_price: u64,
    pub platform_fee: u64,
    pub creator_fee: u64,
}

#[event]
pub struct MigrationReady {
    pub mint: Pubkey,
    pub reserve_sol: u64,
}

#[event]
pub struct MigratedToRaydium {
    pub mint: Pubkey,
    pub reserve_sol: u64,
    pub tokens_remaining: u64,
    pub timestamp: i64,
}

#[event]
pub struct VestedTokensClaimed {
    pub creator: Pubkey,
    pub mint: Pubkey,
    pub amount: u64,
}

// ============================================================================
// ERRORS
// ============================================================================

#[error_code]
pub enum SendItError {
    #[msg("Platform fee too high (max 10%)")]
    FeeTooHigh,
    #[msg("Token name too long")]
    NameTooLong,
    #[msg("Token symbol too long")]
    SymbolTooLong,
    #[msg("Token URI too long")]
    UriTooLong,
    #[msg("Creator allocation too high (max 10%)")]
    AllocationTooHigh,
    #[msg("Platform is paused")]
    PlatformPaused,
    #[msg("Token is paused")]
    TokenPaused,
    #[msg("Token already migrated to Raydium")]
    AlreadyMigrated,
    #[msg("Trading has not started yet")]
    TradingNotStarted,
    #[msg("Wallet is blocked")]
    WalletBlocked,
    #[msg("Snipe protection: buy limit exceeded during launch window")]
    SnipeLimitExceeded,
    #[msg("Amount must be greater than zero")]
    ZeroAmount,
    #[msg("Insufficient output amount")]
    InsufficientOutput,
    #[msg("Insufficient reserve SOL")]
    InsufficientReserve,
    #[msg("Insufficient tokens sold to sell this amount")]
    InsufficientTokensSold,
    #[msg("Invalid creator")]
    InvalidCreator,
    #[msg("Migration threshold not met")]
    MigrationThresholdNotMet,
    #[msg("Lock period still active")]
    LockPeriodActive,
    #[msg("Nothing to claim")]
    NothingToClaim,
    #[msg("Blocklist is full")]
    BlocklistFull,
    #[msg("Math overflow")]
    MathOverflow,
}
