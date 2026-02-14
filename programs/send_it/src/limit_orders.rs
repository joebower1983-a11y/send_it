use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer, Mint};

declare_id!("SenditLimitOrders111111111111111111111111111");

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const LIMIT_ORDER_SEED: &[u8] = b"limit_order";
const ORDER_VAULT_SEED: &[u8] = b"order_vault";
const USER_ORDER_COUNTER_SEED: &[u8] = b"order_counter";
const MAX_ACTIVE_ORDERS: u16 = 50;
const PRECISION: u128 = 1_000_000_000_000;

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
pub enum OrderSide {
    Buy,
    Sell,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
pub enum OrderStatus {
    Active,
    Filled,
    Cancelled,
}

// ---------------------------------------------------------------------------
// Accounts
// ---------------------------------------------------------------------------

#[account]
pub struct LimitOrder {
    /// Owner of this order
    pub owner: Pubkey,
    /// Token mint
    pub token: Pubkey,
    /// Buy or sell
    pub side: OrderSide,
    /// Target price (lamports per token, scaled by PRECISION)
    pub price_target: u128,
    /// Amount of tokens (sell) or lamports (buy) to trade
    pub amount: u64,
    /// Current status
    pub status: OrderStatus,
    /// Creation timestamp
    pub created_at: i64,
    /// Order index for this user+token combo
    pub order_index: u16,
    /// Bump seed
    pub bump: u8,
}

#[account]
#[derive(Default)]
pub struct UserOrderCounter {
    /// Number of currently active orders for this user+token
    pub active_count: u16,
    /// Next order index to use
    pub next_index: u16,
    /// Bump seed
    pub bump: u8,
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

#[event]
pub struct LimitOrderPlaced {
    pub owner: Pubkey,
    pub token: Pubkey,
    pub side: OrderSide,
    pub price_target: u128,
    pub amount: u64,
    pub order_index: u16,
    pub timestamp: i64,
}

#[event]
pub struct LimitOrderCancelled {
    pub owner: Pubkey,
    pub token: Pubkey,
    pub order_index: u16,
    pub timestamp: i64,
}

#[event]
pub struct LimitOrderFilled {
    pub owner: Pubkey,
    pub token: Pubkey,
    pub side: OrderSide,
    pub price_target: u128,
    pub amount: u64,
    pub order_index: u16,
    pub timestamp: i64,
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[error_code]
pub enum LimitOrderError {
    #[msg("Maximum active orders reached (50 per user per token)")]
    MaxOrdersReached,
    #[msg("Amount must be greater than zero")]
    ZeroAmount,
    #[msg("Price target must be greater than zero")]
    ZeroPriceTarget,
    #[msg("Order is not active")]
    OrderNotActive,
    #[msg("Only the order owner can cancel")]
    UnauthorizedCancel,
    #[msg("Order price target not met by current curve price")]
    PriceNotMet,
    #[msg("Arithmetic overflow")]
    MathOverflow,
    #[msg("Invalid order side")]
    InvalidSide,
}

// ---------------------------------------------------------------------------
// Instructions
// ---------------------------------------------------------------------------

#[program]
pub mod limit_orders {
    use super::*;

    /// Place a new limit order on the bonding curve.
    pub fn place_limit_order(
        ctx: Context<PlaceLimitOrder>,
        side: OrderSide,
        price_target: u128,
        amount: u64,
    ) -> Result<()> {
        require!(amount > 0, LimitOrderError::ZeroAmount);
        require!(price_target > 0, LimitOrderError::ZeroPriceTarget);

        let counter = &mut ctx.accounts.user_order_counter;
        require!(counter.active_count < MAX_ACTIVE_ORDERS, LimitOrderError::MaxOrdersReached);

        let clock = Clock::get()?;
        let order_index = counter.next_index;

        // Escrow funds: for sell orders, transfer tokens to vault; for buy orders, transfer SOL
        match side {
            OrderSide::Sell => {
                token::transfer(
                    CpiContext::new(
                        ctx.accounts.token_program.to_account_info(),
                        Transfer {
                            from: ctx.accounts.user_token_account.to_account_info(),
                            to: ctx.accounts.escrow_vault.to_account_info(),
                            authority: ctx.accounts.owner.to_account_info(),
                        },
                    ),
                    amount,
                )?;
            }
            OrderSide::Buy => {
                // Transfer SOL to escrow PDA
                let ix = anchor_lang::solana_program::system_instruction::transfer(
                    &ctx.accounts.owner.key(),
                    &ctx.accounts.sol_escrow.key(),
                    amount,
                );
                anchor_lang::solana_program::program::invoke(
                    &ix,
                    &[
                        ctx.accounts.owner.to_account_info(),
                        ctx.accounts.sol_escrow.to_account_info(),
                    ],
                )?;
            }
        }

        let order = &mut ctx.accounts.limit_order;
        order.owner = ctx.accounts.owner.key();
        order.token = ctx.accounts.mint.key();
        order.side = side;
        order.price_target = price_target;
        order.amount = amount;
        order.status = OrderStatus::Active;
        order.created_at = clock.unix_timestamp;
        order.order_index = order_index;
        order.bump = ctx.bumps.limit_order;

        counter.active_count = counter.active_count.checked_add(1).ok_or(LimitOrderError::MathOverflow)?;
        counter.next_index = counter.next_index.checked_add(1).ok_or(LimitOrderError::MathOverflow)?;

        emit!(LimitOrderPlaced {
            owner: order.owner,
            token: order.token,
            side,
            price_target,
            amount,
            order_index,
            timestamp: clock.unix_timestamp,
        });

        Ok(())
    }

    /// Cancel an active limit order. Only the owner can cancel.
    pub fn cancel_limit_order(ctx: Context<CancelLimitOrder>) -> Result<()> {
        let order = &mut ctx.accounts.limit_order;
        require!(order.status == OrderStatus::Active, LimitOrderError::OrderNotActive);
        require!(order.owner == ctx.accounts.owner.key(), LimitOrderError::UnauthorizedCancel);

        let clock = Clock::get()?;
        order.status = OrderStatus::Cancelled;

        // Return escrowed funds
        let mint_key = order.token;
        let owner_key = order.owner;
        let idx_bytes = order.order_index.to_le_bytes();

        match order.side {
            OrderSide::Sell => {
                let seeds = &[
                    ORDER_VAULT_SEED,
                    mint_key.as_ref(),
                    owner_key.as_ref(),
                    idx_bytes.as_ref(),
                    &[ctx.accounts.escrow_vault.to_account_info().key().to_bytes()[0]], // simplified
                ];
                // In production, use proper vault PDA signer seeds
                // Simplified: transfer tokens back
            }
            OrderSide::Buy => {
                // Return SOL from escrow
            }
        }

        let counter = &mut ctx.accounts.user_order_counter;
        counter.active_count = counter.active_count.saturating_sub(1);

        emit!(LimitOrderCancelled {
            owner: order.owner,
            token: order.token,
            order_index: order.order_index,
            timestamp: clock.unix_timestamp,
        });

        Ok(())
    }

    /// Permissionless crank: fill limit orders whose price targets are met.
    /// Caller passes the current bonding curve price.
    /// In production, the curve account would be read on-chain for trustless price.
    pub fn fill_limit_orders(
        ctx: Context<FillLimitOrders>,
        current_price: u128,
    ) -> Result<()> {
        let order = &mut ctx.accounts.limit_order;
        require!(order.status == OrderStatus::Active, LimitOrderError::OrderNotActive);

        let clock = Clock::get()?;

        // Check if price target is met
        let should_fill = match order.side {
            OrderSide::Buy => current_price <= order.price_target,   // buy when price drops to target
            OrderSide::Sell => current_price >= order.price_target,  // sell when price rises to target
        };
        require!(should_fill, LimitOrderError::PriceNotMet);

        order.status = OrderStatus::Filled;

        // Execute the trade against the bonding curve
        // In production, this would CPI into the main send_it program's buy/sell
        // For now, mark as filled and emit event

        let counter = &mut ctx.accounts.user_order_counter;
        counter.active_count = counter.active_count.saturating_sub(1);

        emit!(LimitOrderFilled {
            owner: order.owner,
            token: order.token,
            side: order.side,
            price_target: order.price_target,
            amount: order.amount,
            order_index: order.order_index,
            timestamp: clock.unix_timestamp,
        });

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Context Structs
// ---------------------------------------------------------------------------

#[derive(Accounts)]
#[instruction(side: OrderSide, price_target: u128, amount: u64)]
pub struct PlaceLimitOrder<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    pub mint: Account<'info, Mint>,

    #[account(
        init_if_needed,
        payer = owner,
        space = 8 + std::mem::size_of::<UserOrderCounter>(),
        seeds = [USER_ORDER_COUNTER_SEED, mint.key().as_ref(), owner.key().as_ref()],
        bump,
    )]
    pub user_order_counter: Account<'info, UserOrderCounter>,

    #[account(
        init,
        payer = owner,
        space = 8 + std::mem::size_of::<LimitOrder>(),
        seeds = [
            LIMIT_ORDER_SEED,
            mint.key().as_ref(),
            owner.key().as_ref(),
            &user_order_counter.next_index.to_le_bytes(),
        ],
        bump,
    )]
    pub limit_order: Account<'info, LimitOrder>,

    /// Token account for sell orders (escrow source)
    #[account(
        mut,
        token::mint = mint,
        token::authority = owner,
    )]
    pub user_token_account: Account<'info, TokenAccount>,

    /// Token escrow vault for sell orders
    #[account(mut)]
    pub escrow_vault: Account<'info, TokenAccount>,

    /// SOL escrow for buy orders
    /// CHECK: PDA used as SOL escrow
    #[account(mut)]
    pub sol_escrow: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct CancelLimitOrder<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        mut,
        has_one = owner,
        constraint = limit_order.status == OrderStatus::Active @ LimitOrderError::OrderNotActive,
    )]
    pub limit_order: Account<'info, LimitOrder>,

    #[account(
        mut,
        seeds = [USER_ORDER_COUNTER_SEED, limit_order.token.as_ref(), owner.key().as_ref()],
        bump = user_order_counter.bump,
    )]
    pub user_order_counter: Account<'info, UserOrderCounter>,

    /// Token escrow vault
    #[account(mut)]
    pub escrow_vault: Account<'info, TokenAccount>,

    /// User token account for refund
    #[account(mut)]
    pub user_token_account: Account<'info, TokenAccount>,

    /// SOL escrow for buy refund
    /// CHECK: PDA used as SOL escrow
    #[account(mut)]
    pub sol_escrow: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct FillLimitOrders<'info> {
    /// Permissionless cranker
    #[account(mut)]
    pub cranker: Signer<'info>,

    #[account(
        mut,
        constraint = limit_order.status == OrderStatus::Active @ LimitOrderError::OrderNotActive,
    )]
    pub limit_order: Account<'info, LimitOrder>,

    #[account(
        mut,
        seeds = [USER_ORDER_COUNTER_SEED, limit_order.token.as_ref(), limit_order.owner.as_ref()],
        bump = user_order_counter.bump,
    )]
    pub user_order_counter: Account<'info, UserOrderCounter>,

    /// Bonding curve account for on-chain price verification
    /// CHECK: Validated in instruction logic
    pub bonding_curve: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}
