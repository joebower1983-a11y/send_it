use anchor_lang::prelude::*;
use anchor_lang::system_program;

// ── Seeds ──────────────────────────────────────────────────────────────────────
pub const CHAT_ROOM_SEED: &[u8] = b"chat_room";
pub const LIVE_MESSAGE_SEED: &[u8] = b"live_message";
pub const USER_CHAT_STATE_SEED: &[u8] = b"user_chat_state";

// ── Errors ─────────────────────────────────────────────────────────────────────
#[error_code]
pub enum LiveChatError {
    #[msg("Chat room is closed")]
    ChatClosed,
    #[msg("Message text exceeds 200 characters")]
    MessageTooLong,
    #[msg("Slowmode: wait before sending another message")]
    SlowmodeActive,
    #[msg("Unauthorized: not the creator or authority")]
    Unauthorized,
    #[msg("Slowmode seconds must be 0-300")]
    InvalidSlowmode,
}

// ── Account Structs ────────────────────────────────────────────────────────────
#[account]
pub struct ChatRoom {
    pub token_mint: Pubkey,      // the launch token this room belongs to
    pub creator: Pubkey,         // launch creator (can moderate)
    pub authority: Pubkey,       // program authority (can also moderate)
    pub message_count: u64,
    pub is_active: bool,
    pub slowmode_seconds: u16,
    pub bump: u8,
}

impl ChatRoom {
    pub const SIZE: usize = 8 + 32 + 32 + 32 + 8 + 1 + 2 + 1; // 116
}

#[account]
pub struct LiveMessage {
    pub chat_room: Pubkey,
    pub index: u64,
    pub author: Pubkey,
    pub text: String,            // max 200 chars (4 + 200 bytes)
    pub timestamp: i64,
    pub tips_received: u64,      // lamports tipped to creator
    pub bump: u8,
}

impl LiveMessage {
    pub const SIZE: usize = 8 + 32 + 8 + 32 + (4 + 200) + 8 + 8 + 1; // 301
}

/// Per-user per-room rate-limit tracker.
#[account]
pub struct UserChatState {
    pub chat_room: Pubkey,
    pub user: Pubkey,
    pub last_message_ts: i64,
    pub bump: u8,
}

impl UserChatState {
    pub const SIZE: usize = 8 + 32 + 32 + 8 + 1; // 81
}

// ── Events ─────────────────────────────────────────────────────────────────────
#[event]
pub struct MessageSent {
    pub chat_room: Pubkey,
    pub index: u64,
    pub author: Pubkey,
    pub text: String,
    pub tip_lamports: u64,
}

#[event]
pub struct SlowmodeToggled {
    pub chat_room: Pubkey,
    pub slowmode_seconds: u16,
}

#[event]
pub struct ChatClosed {
    pub chat_room: Pubkey,
}

// ── Instructions ───────────────────────────────────────────────────────────────

/// Initialize a chat room for a token launch.
pub fn initialize_chat_room(
    ctx: Context<InitializeChatRoom>,
    slowmode_seconds: u16,
) -> Result<()> {
    require!(slowmode_seconds <= 300, LiveChatError::InvalidSlowmode);

    let room = &mut ctx.accounts.chat_room;
    room.token_mint = ctx.accounts.token_mint.key();
    room.creator = ctx.accounts.creator.key();
    room.authority = ctx.accounts.authority.key();
    room.message_count = 0;
    room.is_active = true;
    room.slowmode_seconds = slowmode_seconds;
    room.bump = ctx.bumps.chat_room;
    Ok(())
}

/// Send a live message with an optional SOL tip to the creator.
pub fn send_live_message(
    ctx: Context<SendLiveMessage>,
    text: String,
    tip_lamports: u64,
) -> Result<()> {
    let room = &ctx.accounts.chat_room;
    require!(room.is_active, LiveChatError::ChatClosed);
    require!(text.len() <= 200, LiveChatError::MessageTooLong);

    // Rate limit check
    let clock = Clock::get()?;
    let user_state = &mut ctx.accounts.user_chat_state;
    if user_state.last_message_ts > 0 && room.slowmode_seconds > 0 {
        let elapsed = clock.unix_timestamp - user_state.last_message_ts;
        require!(
            elapsed >= room.slowmode_seconds as i64,
            LiveChatError::SlowmodeActive
        );
    }
    user_state.chat_room = ctx.accounts.chat_room.key();
    user_state.user = ctx.accounts.author.key();
    user_state.last_message_ts = clock.unix_timestamp;
    user_state.bump = ctx.bumps.user_chat_state;

    // Optional tip transfer to creator
    if tip_lamports > 0 {
        system_program::transfer(
            CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                system_program::Transfer {
                    from: ctx.accounts.author.to_account_info(),
                    to: ctx.accounts.creator.to_account_info(),
                },
            ),
            tip_lamports,
        )?;
    }

    // Write message
    let msg = &mut ctx.accounts.live_message;
    let index = ctx.accounts.chat_room.message_count;
    msg.chat_room = ctx.accounts.chat_room.key();
    msg.index = index;
    msg.author = ctx.accounts.author.key();
    msg.text = text.clone();
    msg.timestamp = clock.unix_timestamp;
    msg.tips_received = tip_lamports;
    msg.bump = ctx.bumps.live_message;

    // Increment room counter
    let room = &mut ctx.accounts.chat_room;
    room.message_count = room.message_count.checked_add(1).unwrap();

    emit!(MessageSent {
        chat_room: room.key(),
        index,
        author: ctx.accounts.author.key(),
        text,
        tip_lamports,
    });

    Ok(())
}

/// Toggle slowmode duration (creator only).
pub fn toggle_slowmode(ctx: Context<ModerateChat>, slowmode_seconds: u16) -> Result<()> {
    require!(slowmode_seconds <= 300, LiveChatError::InvalidSlowmode);
    let room = &mut ctx.accounts.chat_room;
    require!(
        ctx.accounts.signer.key() == room.creator || ctx.accounts.signer.key() == room.authority,
        LiveChatError::Unauthorized
    );
    room.slowmode_seconds = slowmode_seconds;

    emit!(SlowmodeToggled {
        chat_room: room.key(),
        slowmode_seconds,
    });
    Ok(())
}

/// Close the chat room (creator or authority).
pub fn close_chat(ctx: Context<ModerateChat>) -> Result<()> {
    let room = &mut ctx.accounts.chat_room;
    require!(
        ctx.accounts.signer.key() == room.creator || ctx.accounts.signer.key() == room.authority,
        LiveChatError::Unauthorized
    );
    room.is_active = false;

    emit!(ChatClosed {
        chat_room: room.key(),
    });
    Ok(())
}

// ── Contexts ───────────────────────────────────────────────────────────────────

#[derive(Accounts)]
pub struct InitializeChatRoom<'info> {
    #[account(
        init,
        payer = creator,
        space = ChatRoom::SIZE,
        seeds = [CHAT_ROOM_SEED, token_mint.key().as_ref()],
        bump,
    )]
    pub chat_room: Account<'info, ChatRoom>,
    /// CHECK: token mint address used as seed
    pub token_mint: UncheckedAccount<'info>,
    #[account(mut)]
    pub creator: Signer<'info>,
    /// CHECK: program authority
    pub authority: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct SendLiveMessage<'info> {
    #[account(
        mut,
        seeds = [CHAT_ROOM_SEED, chat_room.token_mint.as_ref()],
        bump = chat_room.bump,
    )]
    pub chat_room: Account<'info, ChatRoom>,

    #[account(
        init,
        payer = author,
        space = LiveMessage::SIZE,
        seeds = [LIVE_MESSAGE_SEED, chat_room.key().as_ref(), &chat_room.message_count.to_le_bytes()],
        bump,
    )]
    pub live_message: Account<'info, LiveMessage>,

    #[account(
        init_if_needed,
        payer = author,
        space = UserChatState::SIZE,
        seeds = [USER_CHAT_STATE_SEED, chat_room.key().as_ref(), author.key().as_ref()],
        bump,
    )]
    pub user_chat_state: Account<'info, UserChatState>,

    #[account(mut)]
    pub author: Signer<'info>,

    /// CHECK: creator receives tips; validated against chat_room.creator
    #[account(mut, constraint = creator.key() == chat_room.creator)]
    pub creator: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ModerateChat<'info> {
    #[account(
        mut,
        seeds = [CHAT_ROOM_SEED, chat_room.token_mint.as_ref()],
        bump = chat_room.bump,
    )]
    pub chat_room: Account<'info, ChatRoom>,
    pub signer: Signer<'info>,
}
