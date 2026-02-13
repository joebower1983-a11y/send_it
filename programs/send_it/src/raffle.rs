use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer};
use anchor_spl::associated_token::AssociatedToken;

use crate::{
    TokenLaunch, PlatformConfig, SendItError,
    TOKEN_LAUNCH_SEED, PLATFORM_CONFIG_SEED,
};

// ============================================================================
// CONSTANTS
// ============================================================================

pub const RAFFLE_SEED: &[u8] = b"raffle";
pub const RAFFLE_TICKET_SEED: &[u8] = b"raffle_ticket";
pub const MAX_WINNER_COUNT: u16 = 100;
pub const MAX_TICKETS: u32 = 10_000;

// ============================================================================
// ACCOUNTS
// ============================================================================

#[account]
pub struct Raffle {
    pub token_launch: Pubkey,       // associated TokenLaunch PDA
    pub mint: Pubkey,               // token mint
    pub creator: Pubkey,            // launch creator
    pub ticket_price: u64,          // price per ticket in lamports
    pub max_tickets: u32,           // maximum tickets available
    pub sold_tickets: u32,          // tickets sold so far
    pub winner_count: u16,          // how many winners to draw
    pub draw_time: i64,             // unix timestamp when draw can occur
    pub randomness_seed: u64,       // slot hash based seed after draw
    pub token_allocation: u64,      // total tokens allocated for raffle prizes
    pub tokens_per_winner: u64,     // tokens each winner receives
    pub drawn: bool,                // whether winners have been drawn
    pub bump: u8,
}

impl Raffle {
    pub const SIZE: usize = 8      // discriminator
        + 32    // token_launch
        + 32    // mint
        + 32    // creator
        + 8     // ticket_price
        + 4     // max_tickets
        + 4     // sold_tickets
        + 2     // winner_count
        + 8     // draw_time
        + 8     // randomness_seed
        + 8     // token_allocation
        + 8     // tokens_per_winner
        + 1     // drawn
        + 1;    // bump
}

#[account]
pub struct RaffleTicket {
    pub raffle: Pubkey,             // parent raffle PDA
    pub owner: Pubkey,              // ticket holder
    pub ticket_index: u32,          // sequential ticket number (0-based)
    pub is_winner: bool,            // set to true after draw if this ticket wins
    pub claimed: bool,              // whether prize has been claimed
    pub bump: u8,
}

impl RaffleTicket {
    pub const SIZE: usize = 8      // discriminator
        + 32    // raffle
        + 32    // owner
        + 4     // ticket_index
        + 1     // is_winner
        + 1     // claimed
        + 1;    // bump
}

// ============================================================================
// INSTRUCTIONS
// ============================================================================

/// Create a raffle tied to a token launch.
/// Called by the token creator at launch time.
pub fn create_raffle(
    ctx: Context<CreateRaffle>,
    ticket_price: u64,
    max_tickets: u32,
    winner_count: u16,
    draw_delay_seconds: i64,
    token_allocation: u64,
) -> Result<()> {
    require!(ticket_price > 0, SendItError::ZeroAmount);
    require!(max_tickets > 0 && max_tickets <= MAX_TICKETS, RaffleError::InvalidTicketCount);
    require!(winner_count > 0 && winner_count <= MAX_WINNER_COUNT, RaffleError::InvalidWinnerCount);
    require!(winner_count <= max_tickets as u16, RaffleError::WinnerCountExceedsTickets);
    require!(token_allocation > 0, SendItError::ZeroAmount);

    let config = &ctx.accounts.platform_config;
    require!(!config.paused, SendItError::PlatformPaused);

    let launch = &ctx.accounts.token_launch;
    require!(launch.creator == ctx.accounts.creator.key(), SendItError::InvalidCreator);
    require!(!launch.migrated, SendItError::AlreadyMigrated);

    let clock = Clock::get()?;

    let raffle = &mut ctx.accounts.raffle;
    raffle.token_launch = ctx.accounts.token_launch.key();
    raffle.mint = launch.mint;
    raffle.creator = ctx.accounts.creator.key();
    raffle.ticket_price = ticket_price;
    raffle.max_tickets = max_tickets;
    raffle.sold_tickets = 0;
    raffle.winner_count = winner_count;
    raffle.draw_time = clock.unix_timestamp + draw_delay_seconds;
    raffle.randomness_seed = 0;
    raffle.token_allocation = token_allocation;
    raffle.tokens_per_winner = token_allocation / winner_count as u64;
    raffle.drawn = false;
    raffle.bump = ctx.bumps.raffle;

    // Transfer raffle token allocation from launch vault to raffle vault
    let mint_key = launch.mint;
    let launch_seeds: &[&[u8]] = &[
        TOKEN_LAUNCH_SEED,
        mint_key.as_ref(),
        &[launch.bump],
    ];
    let signer_seeds = &[launch_seeds];

    token::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.launch_token_vault.to_account_info(),
                to: ctx.accounts.raffle_token_vault.to_account_info(),
                authority: ctx.accounts.token_launch.to_account_info(),
            },
            signer_seeds,
        ),
        token_allocation,
    )?;

    emit!(RaffleCreated {
        raffle: raffle.key(),
        mint: raffle.mint,
        ticket_price,
        max_tickets,
        winner_count,
        draw_time: raffle.draw_time,
        token_allocation,
    });

    Ok(())
}

/// Buy a raffle ticket. User pays ticket_price in SOL.
pub fn buy_ticket(ctx: Context<BuyTicket>) -> Result<()> {
    let raffle = &ctx.accounts.raffle;
    require!(!raffle.drawn, RaffleError::RaffleAlreadyDrawn);
    require!(raffle.sold_tickets < raffle.max_tickets, RaffleError::SoldOut);

    let clock = Clock::get()?;
    require!(clock.unix_timestamp < raffle.draw_time, RaffleError::DrawTimePassed);

    // Transfer SOL from buyer to raffle creator (proceeds go to creator)
    anchor_lang::system_program::transfer(
        CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            anchor_lang::system_program::Transfer {
                from: ctx.accounts.buyer.to_account_info(),
                to: ctx.accounts.creator_wallet.to_account_info(),
            },
        ),
        raffle.ticket_price,
    )?;

    let ticket_index = raffle.sold_tickets;

    // Initialize ticket
    let ticket = &mut ctx.accounts.raffle_ticket;
    ticket.raffle = ctx.accounts.raffle.key();
    ticket.owner = ctx.accounts.buyer.key();
    ticket.ticket_index = ticket_index;
    ticket.is_winner = false;
    ticket.claimed = false;
    ticket.bump = ctx.bumps.raffle_ticket;

    // Increment sold tickets
    let raffle = &mut ctx.accounts.raffle;
    raffle.sold_tickets += 1;

    emit!(TicketPurchased {
        raffle: raffle.key(),
        buyer: ctx.accounts.buyer.key(),
        ticket_index,
        total_sold: raffle.sold_tickets,
    });

    Ok(())
}

/// Draw winners using slot hash as randomness source.
/// Can be called by anyone after draw_time (permissionless crank).
pub fn draw_winners(ctx: Context<DrawWinners>) -> Result<()> {
    let raffle = &ctx.accounts.raffle;
    require!(!raffle.drawn, RaffleError::RaffleAlreadyDrawn);
    require!(raffle.sold_tickets > 0, RaffleError::NoTicketsSold);

    let clock = Clock::get()?;
    require!(clock.unix_timestamp >= raffle.draw_time, RaffleError::DrawTimeNotReached);

    // Use slot hashes for randomness
    // The SlotHashes sysvar contains recent slot hashes
    let slot_hashes = &ctx.accounts.slot_hashes;
    let data = slot_hashes.try_borrow_data()?;

    // Take first 8 bytes of slot hash data as seed (after the count prefix)
    let seed_bytes: [u8; 8] = if data.len() >= 16 {
        let mut arr = [0u8; 8];
        arr.copy_from_slice(&data[8..16]);
        arr
    } else {
        // Fallback: use clock slot
        clock.slot.to_le_bytes()
    };

    let randomness_seed = u64::from_le_bytes(seed_bytes);

    let raffle = &mut ctx.accounts.raffle;
    raffle.randomness_seed = randomness_seed;
    raffle.drawn = true;

    // Winners are determined deterministically from the seed:
    // winner_ticket_index[i] = (seed + i * 7919) % sold_tickets
    // This is checked in claim_raffle_prize to verify if a ticket is a winner.
    // Using a large prime multiplier for distribution.

    emit!(RaffleDrawn {
        raffle: raffle.key(),
        randomness_seed,
        sold_tickets: raffle.sold_tickets,
        winner_count: raffle.winner_count,
    });

    Ok(())
}

/// Claim raffle prize. Ticket holder checks if their ticket is a winner.
pub fn claim_raffle_prize(ctx: Context<ClaimRafflePrize>) -> Result<()> {
    let raffle = &ctx.accounts.raffle;
    require!(raffle.drawn, RaffleError::RaffleNotDrawn);

    let ticket = &ctx.accounts.raffle_ticket;
    require!(ticket.owner == ctx.accounts.claimer.key(), SendItError::InvalidCreator);
    require!(!ticket.claimed, RaffleError::AlreadyClaimed);

    // Determine if this ticket is a winner
    let seed = raffle.randomness_seed;
    let sold = raffle.sold_tickets;
    let winners = raffle.winner_count;
    let ticket_idx = ticket.ticket_index;

    let is_winner = check_winner(seed, sold, winners, ticket_idx);
    require!(is_winner, RaffleError::NotAWinner);

    // Transfer tokens from raffle vault to claimer
    let mint_key = raffle.mint;
    let raffle_seeds: &[&[u8]] = &[
        RAFFLE_SEED,
        mint_key.as_ref(),
        &[raffle.bump],
    ];
    let signer_seeds = &[raffle_seeds];

    token::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.raffle_token_vault.to_account_info(),
                to: ctx.accounts.claimer_token_account.to_account_info(),
                authority: ctx.accounts.raffle.to_account_info(),
            },
            signer_seeds,
        ),
        raffle.tokens_per_winner,
    )?;

    // Mark ticket as claimed
    let ticket = &mut ctx.accounts.raffle_ticket;
    ticket.is_winner = true;
    ticket.claimed = true;

    emit!(RafflePrizeClaimed {
        raffle: ctx.accounts.raffle.key(),
        winner: ctx.accounts.claimer.key(),
        ticket_index: ticket.ticket_index,
        tokens_received: raffle.tokens_per_winner,
    });

    Ok(())
}

// ============================================================================
// WINNER DETERMINATION
// ============================================================================

/// Deterministic winner check using the randomness seed.
/// Generates `winner_count` unique winning indices from the seed.
/// Returns true if `ticket_index` is among them.
pub fn check_winner(seed: u64, sold_tickets: u32, winner_count: u16, ticket_index: u32) -> bool {
    let effective_winners = (winner_count as u32).min(sold_tickets);
    let mut winning_indices = Vec::with_capacity(effective_winners as usize);

    for i in 0..effective_winners {
        // Hash-like mixing: seed * prime + index * another_prime
        let mixed = (seed as u128)
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add((i as u128).wrapping_mul(1_442_695_040_888_963_407));
        let idx = (mixed % sold_tickets as u128) as u32;

        // If collision, walk forward to find unused slot
        let mut final_idx = idx;
        loop {
            if !winning_indices.contains(&final_idx) {
                break;
            }
            final_idx = (final_idx + 1) % sold_tickets;
        }
        winning_indices.push(final_idx);
    }

    winning_indices.contains(&ticket_index)
}

// ============================================================================
// CONTEXT STRUCTS
// ============================================================================

#[derive(Accounts)]
pub struct CreateRaffle<'info> {
    #[account(
        init,
        payer = creator,
        space = Raffle::SIZE,
        seeds = [RAFFLE_SEED, token_launch.mint.as_ref()],
        bump,
    )]
    pub raffle: Account<'info, Raffle>,

    #[account(
        mut,
        seeds = [TOKEN_LAUNCH_SEED, token_launch.mint.as_ref()],
        bump = token_launch.bump,
        has_one = creator,
    )]
    pub token_launch: Account<'info, TokenLaunch>,

    #[account(
        mut,
        associated_token::mint = token_launch.mint,
        associated_token::authority = token_launch,
    )]
    pub launch_token_vault: Account<'info, TokenAccount>,

    #[account(
        init,
        payer = creator,
        associated_token::mint = token_launch.mint,
        associated_token::authority = raffle,
    )]
    pub raffle_token_vault: Account<'info, TokenAccount>,

    #[account(
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
pub struct BuyTicket<'info> {
    #[account(
        mut,
        seeds = [RAFFLE_SEED, raffle.mint.as_ref()],
        bump = raffle.bump,
    )]
    pub raffle: Account<'info, Raffle>,

    #[account(
        init,
        payer = buyer,
        space = RaffleTicket::SIZE,
        seeds = [
            RAFFLE_TICKET_SEED,
            raffle.key().as_ref(),
            buyer.key().as_ref(),
            &raffle.sold_tickets.to_le_bytes(),
        ],
        bump,
    )]
    pub raffle_ticket: Account<'info, RaffleTicket>,

    /// CHECK: Creator wallet receives ticket proceeds
    #[account(
        mut,
        constraint = creator_wallet.key() == raffle.creator @ SendItError::InvalidCreator,
    )]
    pub creator_wallet: AccountInfo<'info>,

    #[account(mut)]
    pub buyer: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct DrawWinners<'info> {
    #[account(
        mut,
        seeds = [RAFFLE_SEED, raffle.mint.as_ref()],
        bump = raffle.bump,
    )]
    pub raffle: Account<'info, Raffle>,

    /// CHECK: SlotHashes sysvar for randomness
    #[account(address = anchor_lang::solana_program::sysvar::slot_hashes::id())]
    pub slot_hashes: AccountInfo<'info>,

    #[account(mut)]
    pub payer: Signer<'info>,
}

#[derive(Accounts)]
pub struct ClaimRafflePrize<'info> {
    #[account(
        seeds = [RAFFLE_SEED, raffle.mint.as_ref()],
        bump = raffle.bump,
    )]
    pub raffle: Account<'info, Raffle>,

    #[account(
        mut,
        has_one = raffle,
        constraint = raffle_ticket.owner == claimer.key() @ SendItError::InvalidCreator,
    )]
    pub raffle_ticket: Account<'info, RaffleTicket>,

    #[account(
        mut,
        associated_token::mint = raffle.mint,
        associated_token::authority = raffle,
    )]
    pub raffle_token_vault: Account<'info, TokenAccount>,

    #[account(
        init_if_needed,
        payer = claimer,
        associated_token::mint = raffle.mint,
        associated_token::authority = claimer,
    )]
    pub claimer_token_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub claimer: Signer<'info>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

// ============================================================================
// EVENTS
// ============================================================================

#[event]
pub struct RaffleCreated {
    pub raffle: Pubkey,
    pub mint: Pubkey,
    pub ticket_price: u64,
    pub max_tickets: u32,
    pub winner_count: u16,
    pub draw_time: i64,
    pub token_allocation: u64,
}

#[event]
pub struct TicketPurchased {
    pub raffle: Pubkey,
    pub buyer: Pubkey,
    pub ticket_index: u32,
    pub total_sold: u32,
}

#[event]
pub struct RaffleDrawn {
    pub raffle: Pubkey,
    pub randomness_seed: u64,
    pub sold_tickets: u32,
    pub winner_count: u16,
}

#[event]
pub struct RafflePrizeClaimed {
    pub raffle: Pubkey,
    pub winner: Pubkey,
    pub ticket_index: u32,
    pub tokens_received: u64,
}

// ============================================================================
// ERRORS
// ============================================================================

#[error_code]
pub enum RaffleError {
    #[msg("Invalid ticket count")]
    InvalidTicketCount,
    #[msg("Invalid winner count")]
    InvalidWinnerCount,
    #[msg("Winner count exceeds max tickets")]
    WinnerCountExceedsTickets,
    #[msg("Raffle is sold out")]
    SoldOut,
    #[msg("Draw time has not been reached")]
    DrawTimeNotReached,
    #[msg("Draw time has passed, cannot buy tickets")]
    DrawTimePassed,
    #[msg("Raffle has already been drawn")]
    RaffleAlreadyDrawn,
    #[msg("Raffle has not been drawn yet")]
    RaffleNotDrawn,
    #[msg("No tickets sold")]
    NoTicketsSold,
    #[msg("Not a winning ticket")]
    NotAWinner,
    #[msg("Prize already claimed")]
    AlreadyClaimed,
}
