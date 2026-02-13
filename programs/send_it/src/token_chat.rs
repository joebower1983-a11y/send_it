use anchor_lang::prelude::*;

// ============================================================================
// SEEDS & CONSTANTS
// ============================================================================

pub const CHAT_MESSAGE_SEED: &[u8] = b"chat_message";
pub const CHAT_STATE_SEED: &[u8] = b"chat_state";
pub const MAX_MESSAGE_LEN: usize = 280;

// ============================================================================
// ACCOUNTS
// ============================================================================

/// Tracks the next message index for a given token mint.
#[account]
#[derive(Default)]
pub struct ChatState {
    /// The token mint this chat belongs to.
    pub token_mint: Pubkey,
    /// Next message index (auto-increments).
    pub next_index: u64,
    /// Bump seed.
    pub bump: u8,
}

impl ChatState {
    pub const SIZE: usize = 8 + 32 + 8 + 1;
}

/// A single chat message, PDA derived from [CHAT_MESSAGE_SEED, token_mint, index].
#[account]
pub struct ChatMessage {
    /// The token mint this message is associated with.
    pub token_mint: Pubkey,
    /// Sequential index within this token's chat.
    pub index: u64,
    /// Author of the message.
    pub author: Pubkey,
    /// Message text (max 280 UTF-8 chars stored as bytes).
    pub text: String,
    /// Unix timestamp of when the message was posted.
    pub timestamp: i64,
    /// Number of likes.
    pub likes: u64,
    /// Whether the message has been deleted (soft delete).
    pub deleted: bool,
    /// Bump seed.
    pub bump: u8,
}

impl ChatMessage {
    // 8 discriminator + 32 mint + 8 index + 32 author + (4+280) text + 8 ts + 8 likes + 1 deleted + 1 bump
    pub const SIZE: usize = 8 + 32 + 8 + 32 + (4 + MAX_MESSAGE_LEN) + 8 + 8 + 1 + 1;
}

// ============================================================================
// INSTRUCTION CONTEXTS
// ============================================================================

#[derive(Accounts)]
pub struct InitializeChatState<'info> {
    #[account(
        init,
        payer = payer,
        space = ChatState::SIZE,
        seeds = [CHAT_STATE_SEED, token_mint.key().as_ref()],
        bump,
    )]
    pub chat_state: Account<'info, ChatState>,
    /// CHECK: The token mint for this chat. Validated by seed derivation.
    pub token_mint: AccountInfo<'info>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(text: String)]
pub struct PostMessage<'info> {
    #[account(
        mut,
        seeds = [CHAT_STATE_SEED, token_mint.key().as_ref()],
        bump = chat_state.bump,
    )]
    pub chat_state: Account<'info, ChatState>,
    #[account(
        init,
        payer = author,
        space = ChatMessage::SIZE,
        seeds = [CHAT_MESSAGE_SEED, token_mint.key().as_ref(), &chat_state.next_index.to_le_bytes()],
        bump,
    )]
    pub chat_message: Account<'info, ChatMessage>,
    /// CHECK: The token mint. Validated by chat_state seed.
    pub token_mint: AccountInfo<'info>,
    #[account(mut)]
    pub author: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct LikeMessage<'info> {
    #[account(
        mut,
        seeds = [CHAT_MESSAGE_SEED, chat_message.token_mint.as_ref(), &chat_message.index.to_le_bytes()],
        bump = chat_message.bump,
    )]
    pub chat_message: Account<'info, ChatMessage>,
    pub liker: Signer<'info>,
}

#[derive(Accounts)]
pub struct DeleteMessage<'info> {
    #[account(
        mut,
        seeds = [CHAT_MESSAGE_SEED, chat_message.token_mint.as_ref(), &chat_message.index.to_le_bytes()],
        bump = chat_message.bump,
        constraint = chat_message.author == author.key() @ ChatError::NotAuthor,
    )]
    pub chat_message: Account<'info, ChatMessage>,
    pub author: Signer<'info>,
}

// ============================================================================
// INSTRUCTIONS
// ============================================================================

pub fn handle_initialize_chat_state(ctx: Context<InitializeChatState>) -> Result<()> {
    let state = &mut ctx.accounts.chat_state;
    state.token_mint = ctx.accounts.token_mint.key();
    state.next_index = 0;
    state.bump = ctx.bumps.chat_state;
    Ok(())
}

pub fn handle_post_message(ctx: Context<PostMessage>, text: String) -> Result<()> {
    require!(text.len() <= MAX_MESSAGE_LEN, ChatError::MessageTooLong);
    require!(!text.is_empty(), ChatError::EmptyMessage);

    let clock = Clock::get()?;
    let state = &mut ctx.accounts.chat_state;
    let msg = &mut ctx.accounts.chat_message;

    msg.token_mint = ctx.accounts.token_mint.key();
    msg.index = state.next_index;
    msg.author = ctx.accounts.author.key();
    msg.text = text.clone();
    msg.timestamp = clock.unix_timestamp;
    msg.likes = 0;
    msg.deleted = false;
    msg.bump = ctx.bumps.chat_message;

    state.next_index = state.next_index.checked_add(1).unwrap();

    emit!(NewMessageEvent {
        token_mint: msg.token_mint,
        index: msg.index,
        author: msg.author,
        text,
        timestamp: msg.timestamp,
    });

    Ok(())
}

pub fn handle_like_message(ctx: Context<LikeMessage>) -> Result<()> {
    let msg = &mut ctx.accounts.chat_message;
    require!(!msg.deleted, ChatError::MessageDeleted);
    msg.likes = msg.likes.checked_add(1).unwrap();
    Ok(())
}

pub fn handle_delete_message(ctx: Context<DeleteMessage>) -> Result<()> {
    let msg = &mut ctx.accounts.chat_message;
    require!(!msg.deleted, ChatError::MessageDeleted);
    msg.deleted = true;
    msg.text = String::new();

    emit!(MessageDeletedEvent {
        token_mint: msg.token_mint,
        index: msg.index,
        author: msg.author,
    });

    Ok(())
}

// ============================================================================
// EVENTS
// ============================================================================

#[event]
pub struct NewMessageEvent {
    pub token_mint: Pubkey,
    pub index: u64,
    pub author: Pubkey,
    pub text: String,
    pub timestamp: i64,
}

#[event]
pub struct MessageDeletedEvent {
    pub token_mint: Pubkey,
    pub index: u64,
    pub author: Pubkey,
}

// ============================================================================
// ERRORS
// ============================================================================

#[error_code]
pub enum ChatError {
    #[msg("Message exceeds 280 characters")]
    MessageTooLong,
    #[msg("Message cannot be empty")]
    EmptyMessage,
    #[msg("Only the author can delete this message")]
    NotAuthor,
    #[msg("Message has already been deleted")]
    MessageDeleted,
}
