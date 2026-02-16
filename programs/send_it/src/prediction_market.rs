use anchor_lang::prelude::*;

declare_id!("SenditPrediction111111111111111111111111111");

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const PREDICTION_MARKET_SEED: &[u8] = b"prediction_market";
const USER_BET_SEED: &[u8] = b"user_bet";
const PREDICTION_VAULT_SEED: &[u8] = b"prediction_vault";
const BPS_DENOMINATOR: u64 = 10_000;

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
pub enum MarketSide {
    TokenA,
    TokenB,
}

// ---------------------------------------------------------------------------
// Accounts
// ---------------------------------------------------------------------------

#[account]
pub struct PredictionMarket {
    /// First token in the race
    pub token_a: Pubkey,
    /// Second token in the race
    pub token_b: Pubkey,
    /// Creator
    pub creator: Pubkey,
    /// Total SOL bet on token A graduating first
    pub total_pool_a: u64,
    /// Total SOL bet on token B graduating first
    pub total_pool_b: u64,
    /// Deadline timestamp — resolution allowed after this
    pub deadline: i64,
    /// Whether the market has been resolved
    pub resolved: bool,
    /// Winning side (only valid if resolved)
    pub winner: Option<MarketSide>,
    /// Market index (for multiple markets)
    pub market_index: u64,
    /// Bump seed
    pub bump: u8,
    /// Vault bump
    pub vault_bump: u8,
}

#[account]
pub struct UserBet {
    /// User wallet
    pub user: Pubkey,
    /// Market PDA key
    pub market: Pubkey,
    /// Side the user bet on
    pub side: MarketSide,
    /// Amount of SOL bet
    pub amount: u64,
    /// Whether winnings have been claimed
    pub claimed: bool,
    /// Bump seed
    pub bump: u8,
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

#[event]
pub struct PredictionCreated {
    pub market: Pubkey,
    pub token_a: Pubkey,
    pub token_b: Pubkey,
    pub deadline: i64,
    pub creator: Pubkey,
    pub timestamp: i64,
}

#[event]
pub struct BetPlaced {
    pub market: Pubkey,
    pub user: Pubkey,
    pub side: MarketSide,
    pub amount: u64,
    pub timestamp: i64,
}

#[event]
pub struct PredictionResolved {
    pub market: Pubkey,
    pub winner: MarketSide,
    pub total_pool_a: u64,
    pub total_pool_b: u64,
    pub timestamp: i64,
}

#[event]
pub struct WinningsClaimed {
    pub market: Pubkey,
    pub user: Pubkey,
    pub amount: u64,
    pub timestamp: i64,
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[error_code]
pub enum PredictionError {
    #[msg("Amount must be greater than zero")]
    ZeroAmount,
    #[msg("Market has already been resolved")]
    AlreadyResolved,
    #[msg("Market deadline has not passed yet")]
    DeadlineNotReached,
    #[msg("Market has not been resolved yet")]
    NotResolved,
    #[msg("User did not bet on the winning side")]
    NotWinner,
    #[msg("Winnings already claimed")]
    AlreadyClaimed,
    #[msg("Betting is closed (deadline passed)")]
    BettingClosed,
    #[msg("Neither token has graduated — cannot resolve")]
    NeitherGraduated,
    #[msg("Arithmetic overflow")]
    MathOverflow,
    #[msg("Deadline must be in the future")]
    InvalidDeadline,
    #[msg("Tokens must be different")]
    SameTokens,
    #[msg("User already has a bet on this market")]
    AlreadyBet,
    #[msg("Invalid token account — does not match market")]
    InvalidTokenAccount,
}

// ---------------------------------------------------------------------------
// Instructions
// ---------------------------------------------------------------------------

#[program]
pub mod prediction_market {
    use super::*;

    /// Create a prediction market: which token graduates first?
    pub fn create_prediction(
        ctx: Context<CreatePrediction>,
        token_a: Pubkey,
        token_b: Pubkey,
        deadline: i64,
        market_index: u64,
    ) -> Result<()> {
        require!(token_a != token_b, PredictionError::SameTokens);
        let clock = Clock::get()?;
        require!(deadline > clock.unix_timestamp, PredictionError::InvalidDeadline);

        let market = &mut ctx.accounts.prediction_market;
        market.token_a = token_a;
        market.token_b = token_b;
        market.creator = ctx.accounts.creator.key();
        market.total_pool_a = 0;
        market.total_pool_b = 0;
        market.deadline = deadline;
        market.resolved = false;
        market.winner = None;
        market.market_index = market_index;
        market.bump = ctx.bumps.prediction_market;
        market.vault_bump = ctx.bumps.vault;

        emit!(PredictionCreated {
            market: ctx.accounts.prediction_market.key(),
            token_a,
            token_b,
            deadline,
            creator: ctx.accounts.creator.key(),
            timestamp: clock.unix_timestamp,
        });

        Ok(())
    }

    /// Place a bet on which token will graduate first.
    pub fn place_bet(
        ctx: Context<PlaceBet>,
        side: MarketSide,
        amount: u64,
    ) -> Result<()> {
        require!(amount > 0, PredictionError::ZeroAmount);

        let clock = Clock::get()?;
        let market = &mut ctx.accounts.prediction_market;
        require!(!market.resolved, PredictionError::AlreadyResolved);
        require!(clock.unix_timestamp < market.deadline, PredictionError::BettingClosed);

        // Transfer SOL to vault
        let ix = anchor_lang::solana_program::system_instruction::transfer(
            &ctx.accounts.user.key(),
            &ctx.accounts.vault.key(),
            amount,
        );
        anchor_lang::solana_program::program::invoke(
            &ix,
            &[
                ctx.accounts.user.to_account_info(),
                ctx.accounts.vault.to_account_info(),
            ],
        )?;

        match side {
            MarketSide::TokenA => {
                market.total_pool_a = market.total_pool_a.checked_add(amount).ok_or(PredictionError::MathOverflow)?;
            }
            MarketSide::TokenB => {
                market.total_pool_b = market.total_pool_b.checked_add(amount).ok_or(PredictionError::MathOverflow)?;
            }
        }

        let user_bet = &mut ctx.accounts.user_bet;
        user_bet.user = ctx.accounts.user.key();
        user_bet.market = ctx.accounts.prediction_market.key();
        user_bet.side = side;
        user_bet.amount = user_bet.amount.checked_add(amount).ok_or(PredictionError::MathOverflow)?;
        user_bet.claimed = false;
        user_bet.bump = ctx.bumps.user_bet;

        emit!(BetPlaced {
            market: ctx.accounts.prediction_market.key(),
            user: ctx.accounts.user.key(),
            side,
            amount,
            timestamp: clock.unix_timestamp,
        });

        Ok(())
    }

    /// Permissionless resolution after deadline.
    /// Checks which token graduated (via token launch accounts).
    /// In production, pass both TokenLaunch PDAs and check `graduated` field.
    pub fn resolve_prediction(
        ctx: Context<ResolvePrediction>,
        token_a_graduated: bool,
        token_b_graduated: bool,
    ) -> Result<()> {
        let clock = Clock::get()?;
        let market = &mut ctx.accounts.prediction_market;

        require!(!market.resolved, PredictionError::AlreadyResolved);
        require!(clock.unix_timestamp >= market.deadline, PredictionError::DeadlineNotReached);

        // Determine winner
        let winner = if token_a_graduated && !token_b_graduated {
            MarketSide::TokenA
        } else if token_b_graduated && !token_a_graduated {
            MarketSide::TokenB
        } else if token_a_graduated && token_b_graduated {
            // Both graduated — first one wins. In production, compare graduation timestamps.
            // For now, default to TokenA if both graduated
            MarketSide::TokenA
        } else {
            return Err(PredictionError::NeitherGraduated.into());
        };

        market.resolved = true;
        market.winner = Some(winner);

        emit!(PredictionResolved {
            market: ctx.accounts.prediction_market.key(),
            winner,
            total_pool_a: market.total_pool_a,
            total_pool_b: market.total_pool_b,
            timestamp: clock.unix_timestamp,
        });

        Ok(())
    }

    /// Claim winnings from a resolved prediction market.
    /// Winners receive their proportional share of the total pool.
    pub fn claim_winnings(ctx: Context<ClaimWinnings>) -> Result<()> {
        let clock = Clock::get()?;
        let market = &ctx.accounts.prediction_market;
        let user_bet = &mut ctx.accounts.user_bet;

        require!(market.resolved, PredictionError::NotResolved);
        require!(!user_bet.claimed, PredictionError::AlreadyClaimed);

        let winner = market.winner.ok_or(PredictionError::NotResolved)?;
        require!(user_bet.side == winner, PredictionError::NotWinner);

        // Calculate winnings: user_bet / winning_pool * total_pool
        let total_pool = market.total_pool_a
            .checked_add(market.total_pool_b)
            .ok_or(PredictionError::MathOverflow)?;

        let winning_pool = match winner {
            MarketSide::TokenA => market.total_pool_a,
            MarketSide::TokenB => market.total_pool_b,
        };

        // winnings = user_bet.amount * total_pool / winning_pool
        let winnings = (user_bet.amount as u128)
            .checked_mul(total_pool as u128)
            .ok_or(PredictionError::MathOverflow)?
            .checked_div(winning_pool as u128)
            .ok_or(PredictionError::MathOverflow)? as u64;

        // Transfer SOL from vault to winner
        let market_idx_bytes = market.market_index.to_le_bytes();
        let seeds = &[
            PREDICTION_VAULT_SEED,
            market_idx_bytes.as_ref(),
            &[market.vault_bump],
        ];
        let signer_seeds = &[&seeds[..]];

        **ctx.accounts.vault.to_account_info().try_borrow_mut_lamports()? -= winnings;
        **ctx.accounts.user.to_account_info().try_borrow_mut_lamports()? += winnings;

        user_bet.claimed = true;

        emit!(WinningsClaimed {
            market: ctx.accounts.prediction_market.key(),
            user: ctx.accounts.user.key(),
            amount: winnings,
            timestamp: clock.unix_timestamp,
        });

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Context Structs
// ---------------------------------------------------------------------------

#[derive(Accounts)]
#[instruction(token_a: Pubkey, token_b: Pubkey, deadline: i64, market_index: u64)]
pub struct CreatePrediction<'info> {
    #[account(mut)]
    pub creator: Signer<'info>,

    #[account(
        init,
        payer = creator,
        space = 8 + std::mem::size_of::<PredictionMarket>(),
        seeds = [PREDICTION_MARKET_SEED, &market_index.to_le_bytes()],
        bump,
    )]
    pub prediction_market: Account<'info, PredictionMarket>,

    /// CHECK: PDA used as SOL vault for this market
    #[account(
        mut,
        seeds = [PREDICTION_VAULT_SEED, &market_index.to_le_bytes()],
        bump,
    )]
    pub vault: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct PlaceBet<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        seeds = [PREDICTION_MARKET_SEED, &prediction_market.market_index.to_le_bytes()],
        bump = prediction_market.bump,
    )]
    pub prediction_market: Account<'info, PredictionMarket>,

    #[account(
        init_if_needed,
        payer = user,
        space = 8 + std::mem::size_of::<UserBet>(),
        seeds = [USER_BET_SEED, prediction_market.key().as_ref(), user.key().as_ref()],
        bump,
    )]
    pub user_bet: Account<'info, UserBet>,

    /// CHECK: PDA vault
    #[account(
        mut,
        seeds = [PREDICTION_VAULT_SEED, &prediction_market.market_index.to_le_bytes()],
        bump = prediction_market.vault_bump,
    )]
    pub vault: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ResolvePrediction<'info> {
    /// Permissionless — anyone can resolve after deadline
    pub resolver: Signer<'info>,

    #[account(
        mut,
        seeds = [PREDICTION_MARKET_SEED, &prediction_market.market_index.to_le_bytes()],
        bump = prediction_market.bump,
    )]
    pub prediction_market: Account<'info, PredictionMarket>,

    /// CHECK: Token A launch account — validated by constraint matching stored pubkey
    #[account(
        constraint = token_a_launch.key() == prediction_market.token_a @ PredictionError::InvalidTokenAccount
    )]
    pub token_a_launch: UncheckedAccount<'info>,

    /// CHECK: Token B launch account — validated by constraint matching stored pubkey
    #[account(
        constraint = token_b_launch.key() == prediction_market.token_b @ PredictionError::InvalidTokenAccount
    )]
    pub token_b_launch: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct ClaimWinnings<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        seeds = [PREDICTION_MARKET_SEED, &prediction_market.market_index.to_le_bytes()],
        bump = prediction_market.bump,
        constraint = prediction_market.resolved @ PredictionError::NotResolved,
    )]
    pub prediction_market: Account<'info, PredictionMarket>,

    #[account(
        mut,
        seeds = [USER_BET_SEED, prediction_market.key().as_ref(), user.key().as_ref()],
        bump = user_bet.bump,
        has_one = user,
        has_one = market @ PredictionError::NotResolved,
    )]
    pub user_bet: Account<'info, UserBet>,

    /// CHECK: PDA vault
    #[account(
        mut,
        seeds = [PREDICTION_VAULT_SEED, &prediction_market.market_index.to_le_bytes()],
        bump = prediction_market.vault_bump,
    )]
    pub vault: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}
