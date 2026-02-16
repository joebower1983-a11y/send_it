use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer, Mint};

declare_id!("SenditLending1111111111111111111111111111111");

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const LENDING_POOL_SEED: &[u8] = b"lending_pool";
const USER_LEND_POSITION_SEED: &[u8] = b"user_lend_position";
const LENDING_SOL_VAULT_SEED: &[u8] = b"lending_sol_vault";
const LENDING_TOKEN_VAULT_SEED: &[u8] = b"lending_token_vault";
const BPS_DENOMINATOR: u64 = 10_000;
const SECONDS_PER_YEAR: u64 = 365 * 24 * 3600;

// ---------------------------------------------------------------------------
// Accounts
// ---------------------------------------------------------------------------

#[account]
pub struct LendingPool {
    /// Token mint used as collateral
    pub collateral_mint: Pubkey,
    /// Creator / authority
    pub authority: Pubkey,
    /// Total SOL deposited into the pool (available for borrowing)
    pub total_deposited: u64,
    /// Total SOL currently borrowed
    pub total_borrowed: u64,
    /// Annual interest rate in basis points (e.g. 500 = 5%)
    pub interest_rate_bps: u16,
    /// Loan-to-value ratio in bps (e.g. 5000 = 50%)
    pub ltv_ratio: u16,
    /// Liquidation threshold in bps (e.g. 7500 = 75%)
    pub liquidation_threshold_bps: u16,
    /// Last global interest update timestamp
    pub last_update: i64,
    /// Whether the token has graduated
    pub graduated: bool,
    /// Bump seed
    pub bump: u8,
    /// SOL vault bump
    pub sol_vault_bump: u8,
    /// Token vault bump
    pub token_vault_bump: u8,
}

#[account]
pub struct UserLendPosition {
    /// User wallet
    pub user: Pubkey,
    /// Collateral token mint
    pub collateral_token: Pubkey,
    /// SOL deposited by this user (lending)
    pub deposited: u64,
    /// SOL borrowed by this user
    pub borrowed: u64,
    /// Collateral tokens locked
    pub collateral_amount: u64,
    /// Timestamp of last interest accrual for this position
    pub last_interest_update: i64,
    /// Accumulated interest owed
    pub interest_owed: u64,
    /// Bump seed
    pub bump: u8,
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

#[event]
pub struct LendingPoolCreated {
    pub collateral_mint: Pubkey,
    pub authority: Pubkey,
    pub interest_rate_bps: u16,
    pub ltv_ratio: u16,
    pub timestamp: i64,
}

#[event]
pub struct SolDeposited {
    pub user: Pubkey,
    pub pool: Pubkey,
    pub amount: u64,
    pub timestamp: i64,
}

#[event]
pub struct SolBorrowed {
    pub user: Pubkey,
    pub pool: Pubkey,
    pub amount: u64,
    pub collateral_amount: u64,
    pub timestamp: i64,
}

#[event]
pub struct LoanRepaid {
    pub user: Pubkey,
    pub pool: Pubkey,
    pub amount: u64,
    pub timestamp: i64,
}

#[event]
pub struct SolWithdrawn {
    pub user: Pubkey,
    pub pool: Pubkey,
    pub amount: u64,
    pub timestamp: i64,
}

#[event]
pub struct PositionLiquidated {
    pub user: Pubkey,
    pub liquidator: Pubkey,
    pub pool: Pubkey,
    pub collateral_seized: u64,
    pub debt_repaid: u64,
    pub timestamp: i64,
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[error_code]
pub enum LendingError {
    #[msg("Token has not graduated yet")]
    NotGraduated,
    #[msg("Amount must be greater than zero")]
    ZeroAmount,
    #[msg("Insufficient pool liquidity")]
    InsufficientLiquidity,
    #[msg("Borrow would exceed LTV ratio")]
    ExceedsLTV,
    #[msg("Position is not liquidatable")]
    NotLiquidatable,
    #[msg("Insufficient deposit balance")]
    InsufficientDeposit,
    #[msg("Insufficient collateral")]
    InsufficientCollateral,
    #[msg("Repay amount exceeds debt")]
    RepayExceedsDebt,
    #[msg("Arithmetic overflow")]
    MathOverflow,
    #[msg("Invalid interest rate")]
    InvalidInterestRate,
    #[msg("Invalid LTV ratio")]
    InvalidLTV,
    #[msg("Cannot withdraw: would leave position undercollateralized")]
    WithdrawWouldUndercollateralize,
}

// ---------------------------------------------------------------------------
// Instructions
// ---------------------------------------------------------------------------

#[program]
pub mod lending {
    use super::*;

    /// Create a lending pool for a graduated token.
    pub fn create_lending_pool(
        ctx: Context<CreateLendingPool>,
        interest_rate_bps: u16,
        ltv_ratio: u16,
        liquidation_threshold_bps: u16,
    ) -> Result<()> {
        require!(interest_rate_bps > 0 && interest_rate_bps <= 5000, LendingError::InvalidInterestRate);
        require!(ltv_ratio > 0 && ltv_ratio < BPS_DENOMINATOR as u16, LendingError::InvalidLTV);

        let clock = Clock::get()?;
        let pool = &mut ctx.accounts.lending_pool;
        pool.collateral_mint = ctx.accounts.collateral_mint.key();
        pool.authority = ctx.accounts.authority.key();
        pool.total_deposited = 0;
        pool.total_borrowed = 0;
        pool.interest_rate_bps = interest_rate_bps;
        pool.ltv_ratio = ltv_ratio;
        pool.liquidation_threshold_bps = liquidation_threshold_bps;
        pool.last_update = clock.unix_timestamp;
        pool.graduated = true;
        pool.bump = ctx.bumps.lending_pool;
        pool.sol_vault_bump = ctx.bumps.sol_vault;
        pool.token_vault_bump = ctx.bumps.token_vault;

        emit!(LendingPoolCreated {
            collateral_mint: pool.collateral_mint,
            authority: pool.authority,
            interest_rate_bps,
            ltv_ratio,
            timestamp: clock.unix_timestamp,
        });

        Ok(())
    }

    /// Deposit SOL into the lending pool to earn interest.
    pub fn deposit_sol(ctx: Context<DepositSol>, amount: u64) -> Result<()> {
        require!(amount > 0, LendingError::ZeroAmount);

        let clock = Clock::get()?;

        // Transfer SOL to vault
        let ix = anchor_lang::solana_program::system_instruction::transfer(
            &ctx.accounts.user.key(),
            &ctx.accounts.sol_vault.key(),
            amount,
        );
        anchor_lang::solana_program::program::invoke(
            &ix,
            &[
                ctx.accounts.user.to_account_info(),
                ctx.accounts.sol_vault.to_account_info(),
            ],
        )?;

        let pool = &mut ctx.accounts.lending_pool;
        pool.total_deposited = pool.total_deposited.checked_add(amount).ok_or(LendingError::MathOverflow)?;

        let position = &mut ctx.accounts.user_position;
        position.user = ctx.accounts.user.key();
        position.collateral_token = pool.collateral_mint;
        position.deposited = position.deposited.checked_add(amount).ok_or(LendingError::MathOverflow)?;
        if position.last_interest_update == 0 {
            position.last_interest_update = clock.unix_timestamp;
        }

        emit!(SolDeposited {
            user: ctx.accounts.user.key(),
            pool: ctx.accounts.lending_pool.key(),
            amount,
            timestamp: clock.unix_timestamp,
        });

        Ok(())
    }

    /// Borrow SOL against token collateral.
    pub fn borrow_against_tokens(
        ctx: Context<BorrowAgainstTokens>,
        collateral_amount: u64,
        borrow_amount: u64,
    ) -> Result<()> {
        require!(collateral_amount > 0, LendingError::ZeroAmount);
        require!(borrow_amount > 0, LendingError::ZeroAmount);

        let clock = Clock::get()?;
        let pool = &mut ctx.accounts.lending_pool;

        // Check pool has enough liquidity
        let available = pool.total_deposited.saturating_sub(pool.total_borrowed);
        require!(borrow_amount <= available, LendingError::InsufficientLiquidity);

        // Simple LTV check: borrow_amount <= collateral_value * ltv_ratio / 10000
        // In production, collateral_value would come from an oracle or curve price
        // For now, we use a simplified check assuming 1 token = 1 lamport as placeholder
        let max_borrow = (collateral_amount as u128)
            .checked_mul(pool.ltv_ratio as u128)
            .ok_or(LendingError::MathOverflow)?
            .checked_div(BPS_DENOMINATOR as u128)
            .ok_or(LendingError::MathOverflow)? as u64;

        let position = &mut ctx.accounts.user_position;
        // Accrue interest first
        accrue_interest(position, pool.interest_rate_bps, clock.unix_timestamp)?;

        let total_debt = position.borrowed
            .checked_add(position.interest_owed)
            .ok_or(LendingError::MathOverflow)?
            .checked_add(borrow_amount)
            .ok_or(LendingError::MathOverflow)?;
        let total_collateral = position.collateral_amount
            .checked_add(collateral_amount)
            .ok_or(LendingError::MathOverflow)?;
        let total_max_borrow = (total_collateral as u128)
            .checked_mul(pool.ltv_ratio as u128)
            .ok_or(LendingError::MathOverflow)?
            .checked_div(BPS_DENOMINATOR as u128)
            .ok_or(LendingError::MathOverflow)? as u64;
        require!(total_debt <= total_max_borrow, LendingError::ExceedsLTV);

        // Lock collateral tokens
        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.user_token_account.to_account_info(),
                    to: ctx.accounts.token_vault.to_account_info(),
                    authority: ctx.accounts.user.to_account_info(),
                },
            ),
            collateral_amount,
        )?;

        // Transfer SOL to borrower from vault
        let mint_key = pool.collateral_mint;
        let seeds = &[
            LENDING_SOL_VAULT_SEED,
            mint_key.as_ref(),
            &[pool.sol_vault_bump],
        ];
        let signer_seeds = &[&seeds[..]];
        let vault_lamports = **ctx.accounts.sol_vault.to_account_info().try_borrow_lamports()?;
        require!(vault_lamports >= borrow_amount, LendingError::InsufficientLiquidity);
        **ctx.accounts.sol_vault.to_account_info().try_borrow_mut_lamports()? = vault_lamports.checked_sub(borrow_amount).ok_or(LendingError::MathOverflow)?;
        **ctx.accounts.user.to_account_info().try_borrow_mut_lamports()? = (**ctx.accounts.user.to_account_info().try_borrow_lamports()?).checked_add(borrow_amount).ok_or(LendingError::MathOverflow)?;

        // Update state
        position.user = ctx.accounts.user.key();
        position.collateral_token = pool.collateral_mint;
        position.borrowed = position.borrowed.checked_add(borrow_amount).ok_or(LendingError::MathOverflow)?;
        position.collateral_amount = total_collateral;
        position.last_interest_update = clock.unix_timestamp;

        pool.total_borrowed = pool.total_borrowed.checked_add(borrow_amount).ok_or(LendingError::MathOverflow)?;

        emit!(SolBorrowed {
            user: ctx.accounts.user.key(),
            pool: ctx.accounts.lending_pool.key(),
            amount: borrow_amount,
            collateral_amount,
            timestamp: clock.unix_timestamp,
        });

        Ok(())
    }

    /// Repay borrowed SOL (partially or fully).
    pub fn repay(ctx: Context<Repay>, amount: u64) -> Result<()> {
        require!(amount > 0, LendingError::ZeroAmount);

        let clock = Clock::get()?;
        let pool = &mut ctx.accounts.lending_pool;
        let position = &mut ctx.accounts.user_position;

        accrue_interest(position, pool.interest_rate_bps, clock.unix_timestamp)?;

        let total_debt = position.borrowed
            .checked_add(position.interest_owed)
            .ok_or(LendingError::MathOverflow)?;
        require!(amount <= total_debt, LendingError::RepayExceedsDebt);

        // Transfer SOL from user to vault
        let ix = anchor_lang::solana_program::system_instruction::transfer(
            &ctx.accounts.user.key(),
            &ctx.accounts.sol_vault.key(),
            amount,
        );
        anchor_lang::solana_program::program::invoke(
            &ix,
            &[
                ctx.accounts.user.to_account_info(),
                ctx.accounts.sol_vault.to_account_info(),
            ],
        )?;

        // Pay interest first, then principal
        if amount <= position.interest_owed {
            position.interest_owed = position.interest_owed.saturating_sub(amount);
        } else {
            let principal_payment = amount.saturating_sub(position.interest_owed);
            position.interest_owed = 0;
            position.borrowed = position.borrowed.saturating_sub(principal_payment);
            pool.total_borrowed = pool.total_borrowed.saturating_sub(principal_payment);
        }

        emit!(LoanRepaid {
            user: ctx.accounts.user.key(),
            pool: ctx.accounts.lending_pool.key(),
            amount,
            timestamp: clock.unix_timestamp,
        });

        Ok(())
    }

    /// Withdraw deposited SOL from the lending pool.
    pub fn withdraw(ctx: Context<Withdraw>, amount: u64) -> Result<()> {
        require!(amount > 0, LendingError::ZeroAmount);

        let pool = &mut ctx.accounts.lending_pool;
        let position = &mut ctx.accounts.user_position;

        require!(position.deposited >= amount, LendingError::InsufficientDeposit);

        let available = pool.total_deposited.saturating_sub(pool.total_borrowed);
        require!(amount <= available, LendingError::InsufficientLiquidity);

        let clock = Clock::get()?;

        // Transfer SOL back (checked arithmetic)
        let vault_lamports = **ctx.accounts.sol_vault.to_account_info().try_borrow_lamports()?;
        require!(vault_lamports >= amount, LendingError::InsufficientLiquidity);
        **ctx.accounts.sol_vault.to_account_info().try_borrow_mut_lamports()? = vault_lamports.checked_sub(amount).ok_or(LendingError::MathOverflow)?;
        **ctx.accounts.user.to_account_info().try_borrow_mut_lamports()? = (**ctx.accounts.user.to_account_info().try_borrow_lamports()?).checked_add(amount).ok_or(LendingError::MathOverflow)?;

        position.deposited = position.deposited.saturating_sub(amount);
        pool.total_deposited = pool.total_deposited.saturating_sub(amount);

        emit!(SolWithdrawn {
            user: ctx.accounts.user.key(),
            pool: ctx.accounts.lending_pool.key(),
            amount,
            timestamp: clock.unix_timestamp,
        });

        Ok(())
    }

    /// Liquidate an undercollateralized position.
    pub fn liquidate(ctx: Context<Liquidate>) -> Result<()> {
        let clock = Clock::get()?;
        let pool = &mut ctx.accounts.lending_pool;
        let position = &mut ctx.accounts.user_position;

        accrue_interest(position, pool.interest_rate_bps, clock.unix_timestamp)?;

        let total_debt = position.borrowed
            .checked_add(position.interest_owed)
            .ok_or(LendingError::MathOverflow)?;

        // Check if LTV exceeded liquidation threshold
        // collateral_value * liquidation_threshold / 10000 < total_debt => liquidatable
        let max_allowed = (position.collateral_amount as u128)
            .checked_mul(pool.liquidation_threshold_bps as u128)
            .ok_or(LendingError::MathOverflow)?
            .checked_div(BPS_DENOMINATOR as u128)
            .ok_or(LendingError::MathOverflow)? as u64;

        require!(total_debt > max_allowed, LendingError::NotLiquidatable);

        // Liquidator repays debt, receives collateral
        let ix = anchor_lang::solana_program::system_instruction::transfer(
            &ctx.accounts.liquidator.key(),
            &ctx.accounts.sol_vault.key(),
            total_debt,
        );
        anchor_lang::solana_program::program::invoke(
            &ix,
            &[
                ctx.accounts.liquidator.to_account_info(),
                ctx.accounts.sol_vault.to_account_info(),
            ],
        )?;

        // Transfer collateral to liquidator
        let mint_key = pool.collateral_mint;
        let seeds = &[
            LENDING_TOKEN_VAULT_SEED,
            mint_key.as_ref(),
            &[pool.token_vault_bump],
        ];
        let signer_seeds = &[&seeds[..]];

        let collateral_seized = position.collateral_amount;
        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.token_vault.to_account_info(),
                    to: ctx.accounts.liquidator_token_account.to_account_info(),
                    authority: ctx.accounts.token_vault.to_account_info(),
                },
                signer_seeds,
            ),
            collateral_seized,
        )?;

        pool.total_borrowed = pool.total_borrowed.saturating_sub(position.borrowed);
        position.borrowed = 0;
        position.interest_owed = 0;
        position.collateral_amount = 0;

        emit!(PositionLiquidated {
            user: position.user,
            liquidator: ctx.accounts.liquidator.key(),
            pool: ctx.accounts.lending_pool.key(),
            collateral_seized,
            debt_repaid: total_debt,
            timestamp: clock.unix_timestamp,
        });

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn accrue_interest(position: &mut UserLendPosition, rate_bps: u16, now: i64) -> Result<()> {
    if position.borrowed == 0 || position.last_interest_update == 0 {
        position.last_interest_update = now;
        return Ok(());
    }
    let elapsed = (now - position.last_interest_update) as u64;
    // Simple interest: principal * rate_bps / 10000 * elapsed / seconds_per_year
    let interest = (position.borrowed as u128)
        .checked_mul(rate_bps as u128)
        .ok_or(LendingError::MathOverflow)?
        .checked_mul(elapsed as u128)
        .ok_or(LendingError::MathOverflow)?
        .checked_div(BPS_DENOMINATOR as u128 * SECONDS_PER_YEAR as u128)
        .ok_or(LendingError::MathOverflow)? as u64;
    position.interest_owed = position.interest_owed.checked_add(interest).ok_or(LendingError::MathOverflow)?;
    position.last_interest_update = now;
    Ok(())
}

// ---------------------------------------------------------------------------
// Context Structs
// ---------------------------------------------------------------------------

#[derive(Accounts)]
pub struct CreateLendingPool<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    pub collateral_mint: Account<'info, Mint>,

    #[account(
        init,
        payer = authority,
        space = 8 + std::mem::size_of::<LendingPool>(),
        seeds = [LENDING_POOL_SEED, collateral_mint.key().as_ref()],
        bump,
    )]
    pub lending_pool: Account<'info, LendingPool>,

    /// CHECK: PDA used as SOL vault
    #[account(
        mut,
        seeds = [LENDING_SOL_VAULT_SEED, collateral_mint.key().as_ref()],
        bump,
    )]
    pub sol_vault: UncheckedAccount<'info>,

    #[account(
        init,
        payer = authority,
        token::mint = collateral_mint,
        token::authority = token_vault,
        seeds = [LENDING_TOKEN_VAULT_SEED, collateral_mint.key().as_ref()],
        bump,
    )]
    pub token_vault: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct DepositSol<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        seeds = [LENDING_POOL_SEED, lending_pool.collateral_mint.as_ref()],
        bump = lending_pool.bump,
    )]
    pub lending_pool: Account<'info, LendingPool>,

    #[account(
        init_if_needed,
        payer = user,
        space = 8 + std::mem::size_of::<UserLendPosition>(),
        seeds = [USER_LEND_POSITION_SEED, lending_pool.collateral_mint.as_ref(), user.key().as_ref()],
        bump,
    )]
    pub user_position: Account<'info, UserLendPosition>,

    /// CHECK: PDA SOL vault
    #[account(
        mut,
        seeds = [LENDING_SOL_VAULT_SEED, lending_pool.collateral_mint.as_ref()],
        bump = lending_pool.sol_vault_bump,
    )]
    pub sol_vault: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct BorrowAgainstTokens<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        seeds = [LENDING_POOL_SEED, lending_pool.collateral_mint.as_ref()],
        bump = lending_pool.bump,
    )]
    pub lending_pool: Account<'info, LendingPool>,

    #[account(
        mut,
        seeds = [USER_LEND_POSITION_SEED, lending_pool.collateral_mint.as_ref(), user.key().as_ref()],
        bump = user_position.bump,
        has_one = user,
    )]
    pub user_position: Account<'info, UserLendPosition>,

    #[account(
        mut,
        token::mint = lending_pool.collateral_mint,
        token::authority = user,
    )]
    pub user_token_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        seeds = [LENDING_TOKEN_VAULT_SEED, lending_pool.collateral_mint.as_ref()],
        bump = lending_pool.token_vault_bump,
    )]
    pub token_vault: Account<'info, TokenAccount>,

    /// CHECK: PDA SOL vault
    #[account(
        mut,
        seeds = [LENDING_SOL_VAULT_SEED, lending_pool.collateral_mint.as_ref()],
        bump = lending_pool.sol_vault_bump,
    )]
    pub sol_vault: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Repay<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        seeds = [LENDING_POOL_SEED, lending_pool.collateral_mint.as_ref()],
        bump = lending_pool.bump,
    )]
    pub lending_pool: Account<'info, LendingPool>,

    #[account(
        mut,
        seeds = [USER_LEND_POSITION_SEED, lending_pool.collateral_mint.as_ref(), user.key().as_ref()],
        bump = user_position.bump,
        has_one = user,
    )]
    pub user_position: Account<'info, UserLendPosition>,

    /// CHECK: PDA SOL vault
    #[account(
        mut,
        seeds = [LENDING_SOL_VAULT_SEED, lending_pool.collateral_mint.as_ref()],
        bump = lending_pool.sol_vault_bump,
    )]
    pub sol_vault: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Withdraw<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        seeds = [LENDING_POOL_SEED, lending_pool.collateral_mint.as_ref()],
        bump = lending_pool.bump,
    )]
    pub lending_pool: Account<'info, LendingPool>,

    #[account(
        mut,
        seeds = [USER_LEND_POSITION_SEED, lending_pool.collateral_mint.as_ref(), user.key().as_ref()],
        bump = user_position.bump,
        has_one = user,
    )]
    pub user_position: Account<'info, UserLendPosition>,

    /// CHECK: PDA SOL vault
    #[account(
        mut,
        seeds = [LENDING_SOL_VAULT_SEED, lending_pool.collateral_mint.as_ref()],
        bump = lending_pool.sol_vault_bump,
    )]
    pub sol_vault: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Liquidate<'info> {
    #[account(mut)]
    pub liquidator: Signer<'info>,

    #[account(
        mut,
        seeds = [LENDING_POOL_SEED, lending_pool.collateral_mint.as_ref()],
        bump = lending_pool.bump,
    )]
    pub lending_pool: Account<'info, LendingPool>,

    #[account(
        mut,
        constraint = user_position.borrowed > 0,
    )]
    pub user_position: Account<'info, UserLendPosition>,

    #[account(
        mut,
        seeds = [LENDING_TOKEN_VAULT_SEED, lending_pool.collateral_mint.as_ref()],
        bump = lending_pool.token_vault_bump,
    )]
    pub token_vault: Account<'info, TokenAccount>,

    #[account(
        mut,
        token::mint = lending_pool.collateral_mint,
        token::authority = liquidator,
    )]
    pub liquidator_token_account: Account<'info, TokenAccount>,

    /// CHECK: PDA SOL vault
    #[account(
        mut,
        seeds = [LENDING_SOL_VAULT_SEED, lending_pool.collateral_mint.as_ref()],
        bump = lending_pool.sol_vault_bump,
    )]
    pub sol_vault: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}
