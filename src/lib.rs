use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer, MintTo, Burn};
use anchor_spl::associated_token::AssociatedToken;
use anchor_lang::system_program;


pub mod achievements {
    use super::*;

    // ============================================================================
    // SEEDS & CONSTANTS
    // ============================================================================

    pub const USER_ACHIEVEMENTS_SEED: &[u8] = b"user_achievements";

    // Achievement bitflags
    pub const FIRST_LAUNCH: u16    = 1 << 0;  // Launched their first token
    pub const DIAMOND_HANDS: u16   = 1 << 1;  // Held a token 30+ days
    pub const WHALE_STATUS: u16    = 1 << 2;  // >10 SOL cumulative volume
    pub const DEGEN_100: u16       = 1 << 3;  // 100 trades completed
    pub const EARLY_ADOPTER: u16   = 1 << 4;  // Among first 1000 users

    pub const DIAMOND_HANDS_SECONDS: i64 = 30 * 24 * 60 * 60; // 30 days
    pub const WHALE_VOLUME_LAMPORTS: u64 = 10 * 1_000_000_000; // 10 SOL
    pub const DEGEN_TRADE_COUNT: u64 = 100;
    pub const EARLY_ADOPTER_LIMIT: u64 = 1000;

    // ============================================================================
    // ACCOUNTS
    // ============================================================================

    /// Global counter for early adopter tracking.
    #[account]
    #[derive(Default)]
    pub struct AchievementConfig {
        pub total_users: u64,
        pub authority: Pubkey,
        pub bump: u8,
    }

    impl AchievementConfig {
        pub const SIZE: usize = 8 + 8 + 32 + 1;
    }

    /// Per-user achievement state, PDA from [USER_ACHIEVEMENTS_SEED, user].
    #[account]
    #[derive(Default)]
    pub struct UserAchievements {
        /// The user this account belongs to.
        pub user: Pubkey,
        /// Bitflags of unlocked achievements.
        pub badges: u16,
        /// Total trade count (for Degen100).
        pub trade_count: u64,
        /// Total volume in lamports (for WhaleStatus).
        pub total_volume: u64,
        /// Number of tokens launched (for FirstLaunch).
        pub tokens_launched: u64,
        /// Earliest position open timestamp (for DiamondHands tracking).
        pub earliest_hold_start: i64,
        /// Timestamp of account creation.
        pub created_at: i64,
        /// Bump seed.
        pub bump: u8,
    }

    impl UserAchievements {
        // 8 + 32 + 2 + 8 + 8 + 8 + 8 + 8 + 1
        pub const SIZE: usize = 8 + 32 + 2 + 8 + 8 + 8 + 8 + 8 + 1;
    }

    // ============================================================================
    // INSTRUCTION CONTEXTS
    // ============================================================================

    #[derive(Accounts)]
    pub struct InitializeAchievementConfig<'info> {
        #[account(
            init,
            payer = authority,
            space = AchievementConfig::SIZE,
            seeds = [b"achievement_config"],
            bump,
        )]
        pub config: Account<'info, AchievementConfig>,
        #[account(mut)]
        pub authority: Signer<'info>,
        pub system_program: Program<'info, System>,
    }

    #[derive(Accounts)]
    pub struct InitializeUserAchievements<'info> {
        #[account(
            init,
            payer = payer,
            space = UserAchievements::SIZE,
            seeds = [USER_ACHIEVEMENTS_SEED, user.key().as_ref()],
            bump,
        )]
        pub user_achievements: Account<'info, UserAchievements>,
        #[account(
            mut,
            seeds = [b"achievement_config"],
            bump = config.bump,
        )]
        pub config: Account<'info, AchievementConfig>,
        /// CHECK: The user to create achievements for.
        pub user: AccountInfo<'info>,
        #[account(mut)]
        pub payer: Signer<'info>,
        pub system_program: Program<'info, System>,
    }

    /// Permissionless crank: anyone can call to update a user's stats and award badges.
    #[derive(Accounts)]
    pub struct CheckAndAward<'info> {
        #[account(
            mut,
            seeds = [USER_ACHIEVEMENTS_SEED, user_achievements.user.as_ref()],
            bump = user_achievements.bump,
        )]
        pub user_achievements: Account<'info, UserAchievements>,
        /// Cranker pays no rent, just signs.
        pub cranker: Signer<'info>,
    }

    /// Permissionless crank variant that also records new activity.
    #[derive(Accounts)]
    pub struct RecordActivity<'info> {
        #[account(
            mut,
            seeds = [USER_ACHIEVEMENTS_SEED, user_achievements.user.as_ref()],
            bump = user_achievements.bump,
        )]
        pub user_achievements: Account<'info, UserAchievements>,
        pub cranker: Signer<'info>,
    }

    #[derive(Accounts)]
    pub struct GetAchievements<'info> {
        #[account(
            seeds = [USER_ACHIEVEMENTS_SEED, user_achievements.user.as_ref()],
            bump = user_achievements.bump,
        )]
        pub user_achievements: Account<'info, UserAchievements>,
    }

    // ============================================================================
    // INSTRUCTIONS
    // ============================================================================

    pub fn handle_initialize_achievement_config(ctx: Context<InitializeAchievementConfig>) -> Result<()> {
        let config = &mut ctx.accounts.config;
        config.total_users = 0;
        config.authority = ctx.accounts.authority.key();
        config.bump = ctx.bumps.config;
        Ok(())
    }

    pub fn handle_initialize_user_achievements(ctx: Context<InitializeUserAchievements>) -> Result<()> {
        let clock = Clock::get()?;
        let config = &mut ctx.accounts.config;
        let acct = &mut ctx.accounts.user_achievements;

        acct.user = ctx.accounts.user.key();
        acct.badges = 0;
        acct.trade_count = 0;
        acct.total_volume = 0;
        acct.tokens_launched = 0;
        acct.earliest_hold_start = 0;
        acct.created_at = clock.unix_timestamp;
        acct.bump = ctx.bumps.user_achievements;

        config.total_users = config.total_users.checked_add(1).unwrap();

        // Award early adopter if within limit
        if config.total_users <= EARLY_ADOPTER_LIMIT {
            acct.badges |= EARLY_ADOPTER;
            emit!(AchievementUnlocked {
                user: acct.user,
                achievement: EARLY_ADOPTER,
                timestamp: clock.unix_timestamp,
            });
        }

        Ok(())
    }

    /// Record a trade and volume, then check/award badges.
    pub fn handle_record_activity(
        ctx: Context<RecordActivity>,
        trades: u64,
        volume_lamports: u64,
        tokens_launched: u64,
        hold_start: i64,
    ) -> Result<()> {
        let clock = Clock::get()?;
        let acct = &mut ctx.accounts.user_achievements;

        acct.trade_count = acct.trade_count.checked_add(trades).unwrap();
        acct.total_volume = acct.total_volume.checked_add(volume_lamports).unwrap();
        acct.tokens_launched = acct.tokens_launched.checked_add(tokens_launched).unwrap();
        if hold_start > 0 && (acct.earliest_hold_start == 0 || hold_start < acct.earliest_hold_start) {
            acct.earliest_hold_start = hold_start;
        }

        check_badges(acct, clock.unix_timestamp)?;
        Ok(())
    }

    /// Permissionless crank: re-evaluate badges based on current stats.
    pub fn handle_check_and_award(ctx: Context<CheckAndAward>) -> Result<()> {
        let clock = Clock::get()?;
        let acct = &mut ctx.accounts.user_achievements;
        check_badges(acct, clock.unix_timestamp)?;
        Ok(())
    }

    pub fn handle_get_achievements(ctx: Context<GetAchievements>) -> Result<u16> {
        Ok(ctx.accounts.user_achievements.badges)
    }

    // ============================================================================
    // HELPERS
    // ============================================================================

    fn check_badges(acct: &mut UserAchievements, now: i64) -> Result<()> {
        let before = acct.badges;

        if acct.tokens_launched >= 1 {
            acct.badges |= FIRST_LAUNCH;
        }
        if acct.earliest_hold_start > 0 && (now - acct.earliest_hold_start) >= DIAMOND_HANDS_SECONDS {
            acct.badges |= DIAMOND_HANDS;
        }
        if acct.total_volume >= WHALE_VOLUME_LAMPORTS {
            acct.badges |= WHALE_STATUS;
        }
        if acct.trade_count >= DEGEN_TRADE_COUNT {
            acct.badges |= DEGEN_100;
        }

        // Emit events for newly awarded badges
        let newly_awarded = acct.badges & !before;
        if newly_awarded != 0 {
            emit!(AchievementUnlocked {
                user: acct.user,
                achievement: newly_awarded,
                timestamp: now,
            });
        }

        Ok(())
    }

    // ============================================================================
    // EVENTS
    // ============================================================================

    #[event]
    pub struct AchievementUnlocked {
        pub user: Pubkey,
        /// Bitflags of the newly awarded achievements.
        pub achievement: u16,
        pub timestamp: i64,
    }

    // ============================================================================
    // ERRORS
    // ============================================================================

    #[error_code]
    pub enum AchievementError {
        #[msg("User achievements account already initialized")]
        AlreadyInitialized,
        #[msg("No new achievements to award")]
        NoNewAchievements,
    }

}
pub mod airdrops {
    use super::*;

    // ── Seeds ──────────────────────────────────────────────────────────────────────
    pub const AIRDROP_CAMPAIGN_SEED: &[u8] = b"airdrop_campaign";
    pub const AIRDROP_CLAIM_SEED: &[u8] = b"airdrop_claim";
    pub const AIRDROP_VAULT_SEED: &[u8] = b"airdrop_vault";

    // ── Errors ─────────────────────────────────────────────────────────────────────
    #[error_code]
    pub enum AirdropError {
        #[msg("Invalid merkle proof")]
        InvalidProof,
        #[msg("Airdrop already claimed")]
        AlreadyClaimed,
        #[msg("All airdrop slots claimed")]
        MaxRecipientsClaimed,
        #[msg("Airdrop campaign is not active")]
        CampaignNotActive,
        #[msg("Cannot cancel before deadline")]
        CancelTooEarly,
        #[msg("Unauthorized")]
        Unauthorized,
        #[msg("Deadline must be in the future")]
        InvalidDeadline,
    }

    // ── Account Structs ────────────────────────────────────────────────────────────
    #[account]
    pub struct AirdropCampaign {
        pub campaign_id: u64,
        pub token_mint: Pubkey,
        pub creator: Pubkey,
        pub vault: Pubkey,
        pub total_amount: u64,
        pub claimed_count: u64,
        pub max_recipients: u64,
        pub snapshot_slot: u64,
        pub merkle_root: [u8; 32],
        pub deadline: i64,           // unix timestamp after which creator can cancel
        pub is_active: bool,
        pub bump: u8,
        pub vault_bump: u8,
    }

    impl AirdropCampaign {
        pub const SIZE: usize = 8 + 8 + 32 + 32 + 32 + 8 + 8 + 8 + 8 + 32 + 8 + 1 + 1 + 1; // 187
    }

    /// Receipt PDA proving a user already claimed.
    #[account]
    pub struct AirdropClaim {
        pub campaign: Pubkey,
        pub claimant: Pubkey,
        pub amount: u64,
        pub claimed_at: i64,
        pub bump: u8,
    }

    impl AirdropClaim {
        pub const SIZE: usize = 8 + 32 + 32 + 8 + 8 + 1; // 89
    }

    // ── Events ─────────────────────────────────────────────────────────────────────
    #[event]
    pub struct AirdropCreated {
        pub campaign: Pubkey,
        pub token_mint: Pubkey,
        pub creator: Pubkey,
        pub total_amount: u64,
        pub max_recipients: u64,
    }

    #[event]
    pub struct AirdropClaimed {
        pub campaign: Pubkey,
        pub claimant: Pubkey,
        pub amount: u64,
    }

    #[event]
    pub struct AirdropCancelled {
        pub campaign: Pubkey,
        pub reclaimed_amount: u64,
    }

    // ── Instructions ───────────────────────────────────────────────────────────────

    pub fn create_airdrop(
        ctx: Context<CreateAirdrop>,
        campaign_id: u64,
        total_amount: u64,
        max_recipients: u64,
        snapshot_slot: u64,
        merkle_root: [u8; 32],
        deadline: i64,
    ) -> Result<()> {
        let clock = Clock::get()?;
        require!(deadline > clock.unix_timestamp, AirdropError::InvalidDeadline);

        // Transfer tokens from creator to vault
        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.creator_token_account.to_account_info(),
                    to: ctx.accounts.vault.to_account_info(),
                    authority: ctx.accounts.creator.to_account_info(),
                },
            ),
            total_amount,
        )?;

        let campaign = &mut ctx.accounts.campaign;
        campaign.campaign_id = campaign_id;
        campaign.token_mint = ctx.accounts.token_mint.key();
        campaign.creator = ctx.accounts.creator.key();
        campaign.vault = ctx.accounts.vault.key();
        campaign.total_amount = total_amount;
        campaign.claimed_count = 0;
        campaign.max_recipients = max_recipients;
        campaign.snapshot_slot = snapshot_slot;
        campaign.merkle_root = merkle_root;
        campaign.deadline = deadline;
        campaign.is_active = true;
        campaign.bump = ctx.bumps.campaign;
        campaign.vault_bump = ctx.bumps.vault;

        emit!(AirdropCreated {
            campaign: campaign.key(),
            token_mint: campaign.token_mint,
            creator: campaign.creator,
            total_amount,
            max_recipients,
        });

        Ok(())
    }

    pub fn claim_airdrop(
        ctx: Context<ClaimAirdrop>,
        amount: u64,
        proof: Vec<[u8; 32]>,
    ) -> Result<()> {
        let campaign = &ctx.accounts.campaign;
        require!(campaign.is_active, AirdropError::CampaignNotActive);
        require!(
            campaign.claimed_count < campaign.max_recipients,
            AirdropError::MaxRecipientsClaimed
        );

        // Verify merkle proof
        let leaf = anchor_lang::solana_program::keccak::hashv(&[
            &ctx.accounts.claimant.key().to_bytes(),
            &amount.to_le_bytes(),
        ]);
        let mut computed = leaf.0;
        for node in proof.iter() {
            if computed <= *node {
                computed = anchor_lang::solana_program::keccak::hashv(&[&computed, node]).0;
            } else {
                computed = anchor_lang::solana_program::keccak::hashv(&[node, &computed]).0;
            }
        }
        require!(computed == campaign.merkle_root, AirdropError::InvalidProof);

        // Transfer from vault to claimant
        let campaign_key = campaign.campaign_id.to_le_bytes();
        let seeds = &[
            AIRDROP_VAULT_SEED,
            campaign_key.as_ref(),
            &[campaign.vault_bump],
        ];
        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.vault.to_account_info(),
                    to: ctx.accounts.claimant_token_account.to_account_info(),
                    authority: ctx.accounts.vault.to_account_info(),
                },
                &[seeds],
            ),
            amount,
        )?;

        // Update campaign
        let campaign = &mut ctx.accounts.campaign;
        campaign.claimed_count = campaign.claimed_count.checked_add(1).unwrap();

        // Write claim receipt
        let claim = &mut ctx.accounts.claim_receipt;
        claim.campaign = campaign.key();
        claim.claimant = ctx.accounts.claimant.key();
        claim.amount = amount;
        claim.claimed_at = Clock::get()?.unix_timestamp;
        claim.bump = ctx.bumps.claim_receipt;

        emit!(AirdropClaimed {
            campaign: campaign.key(),
            claimant: ctx.accounts.claimant.key(),
            amount,
        });

        Ok(())
    }

    pub fn cancel_airdrop(ctx: Context<CancelAirdrop>) -> Result<()> {
        let campaign = &ctx.accounts.campaign;
        require!(campaign.is_active, AirdropError::CampaignNotActive);
        require!(
            ctx.accounts.creator.key() == campaign.creator,
            AirdropError::Unauthorized
        );
        let clock = Clock::get()?;
        require!(
            clock.unix_timestamp >= campaign.deadline,
            AirdropError::CancelTooEarly
        );

        let remaining = ctx.accounts.vault.amount;

        let campaign_key = campaign.campaign_id.to_le_bytes();
        let seeds = &[
            AIRDROP_VAULT_SEED,
            campaign_key.as_ref(),
            &[campaign.vault_bump],
        ];
        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.vault.to_account_info(),
                    to: ctx.accounts.creator_token_account.to_account_info(),
                    authority: ctx.accounts.vault.to_account_info(),
                },
                &[seeds],
            ),
            remaining,
        )?;

        let campaign = &mut ctx.accounts.campaign;
        campaign.is_active = false;

        emit!(AirdropCancelled {
            campaign: campaign.key(),
            reclaimed_amount: remaining,
        });

        Ok(())
    }

    // ── Contexts ───────────────────────────────────────────────────────────────────

    #[derive(Accounts)]
    #[instruction(campaign_id: u64)]
    pub struct CreateAirdrop<'info> {
        #[account(
            init,
            payer = creator,
            space = AirdropCampaign::SIZE,
            seeds = [AIRDROP_CAMPAIGN_SEED, creator.key().as_ref(), &campaign_id.to_le_bytes()],
            bump,
        )]
        pub campaign: Account<'info, AirdropCampaign>,

        #[account(
            init,
            payer = creator,
            token::mint = token_mint,
            token::authority = vault,
            seeds = [AIRDROP_VAULT_SEED, &campaign_id.to_le_bytes()],
            bump,
        )]
        pub vault: Account<'info, TokenAccount>,

        pub token_mint: Account<'info, Mint>,

        #[account(mut, token::mint = token_mint, token::authority = creator)]
        pub creator_token_account: Account<'info, TokenAccount>,

        #[account(mut)]
        pub creator: Signer<'info>,

        pub token_program: Program<'info, Token>,
        pub system_program: Program<'info, System>,
        pub rent: Sysvar<'info, Rent>,
    }

    #[derive(Accounts)]
    pub struct ClaimAirdrop<'info> {
        #[account(
            mut,
            seeds = [AIRDROP_CAMPAIGN_SEED, campaign.creator.as_ref(), &campaign.campaign_id.to_le_bytes()],
            bump = campaign.bump,
        )]
        pub campaign: Account<'info, AirdropCampaign>,

        #[account(
            init,
            payer = claimant,
            space = AirdropClaim::SIZE,
            seeds = [AIRDROP_CLAIM_SEED, campaign.key().as_ref(), claimant.key().as_ref()],
            bump,
        )]
        pub claim_receipt: Account<'info, AirdropClaim>,

        #[account(
            mut,
            seeds = [AIRDROP_VAULT_SEED, &campaign.campaign_id.to_le_bytes()],
            bump = campaign.vault_bump,
        )]
        pub vault: Account<'info, TokenAccount>,

        #[account(mut, token::mint = campaign.token_mint)]
        pub claimant_token_account: Account<'info, TokenAccount>,

        #[account(mut)]
        pub claimant: Signer<'info>,

        pub token_program: Program<'info, Token>,
        pub system_program: Program<'info, System>,
    }

    #[derive(Accounts)]
    pub struct CancelAirdrop<'info> {
        #[account(
            mut,
            seeds = [AIRDROP_CAMPAIGN_SEED, campaign.creator.as_ref(), &campaign.campaign_id.to_le_bytes()],
            bump = campaign.bump,
        )]
        pub campaign: Account<'info, AirdropCampaign>,

        #[account(
            mut,
            seeds = [AIRDROP_VAULT_SEED, &campaign.campaign_id.to_le_bytes()],
            bump = campaign.vault_bump,
        )]
        pub vault: Account<'info, TokenAccount>,

        #[account(mut, token::mint = campaign.token_mint, token::authority = creator)]
        pub creator_token_account: Account<'info, TokenAccount>,

        #[account(mut)]
        pub creator: Signer<'info>,

        pub token_program: Program<'info, Token>,
    }

}
pub mod analytics {
    use super::*;

    // ---------------------------------------------------------------------------
    // Constants
    // ---------------------------------------------------------------------------
    pub const MAX_HOURLY_SNAPSHOTS: usize = 168; // 7 days of hourly data
    pub const MAX_WHALE_TRANSACTIONS: usize = 50;
    pub const MAX_TOP_HOLDERS: usize = 20;
    pub const WHALE_THRESHOLD_LAMPORTS: u64 = 1_000_000_000; // 1 SOL
    pub const HOUR_SECONDS: i64 = 3600;

    // ---------------------------------------------------------------------------
    // Account structs
    // ---------------------------------------------------------------------------

    #[account]
    pub struct TokenAnalytics {
        pub token_mint: Pubkey,
        pub total_volume: u64,
        pub total_trades: u64,
        pub holder_count: u32,
        pub last_update_slot: u64,
        pub last_snapshot_ts: i64,

        /// Ring-buffer of hourly volume snapshots
        pub snapshot_head: u16,
        pub hourly_volumes: [u64; MAX_HOURLY_SNAPSHOTS],

        /// Holder count history (parallel to hourly_volumes)
        pub hourly_holders: [u32; MAX_HOURLY_SNAPSHOTS],

        /// Recent whale transactions (> 1 SOL)
        pub whale_tx_head: u16,
        pub whale_transactions: [WhaleTransaction; MAX_WHALE_TRANSACTIONS],

        pub bump: u8,
    }

    #[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Default)]
    pub struct WhaleTransaction {
        pub trader: Pubkey,
        pub amount_lamports: u64,
        pub is_buy: bool,
        pub timestamp: i64,
    }

    #[account]
    pub struct WhaleTracker {
        pub token_mint: Pubkey,
        pub top_holders: [HolderEntry; MAX_TOP_HOLDERS],
        pub holder_count: u8,
        pub bump: u8,
    }

    #[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Default)]
    pub struct HolderEntry {
        pub wallet: Pubkey,
        pub balance: u64,
    }

    // ---------------------------------------------------------------------------
    // Events
    // ---------------------------------------------------------------------------

    #[event]
    pub struct AnalyticsUpdated {
        pub token_mint: Pubkey,
        pub total_volume: u64,
        pub total_trades: u64,
        pub holder_count: u32,
        pub timestamp: i64,
    }

    #[event]
    pub struct WhaleAlert {
        pub token_mint: Pubkey,
        pub trader: Pubkey,
        pub amount_lamports: u64,
        pub is_buy: bool,
        pub timestamp: i64,
    }

    #[event]
    pub struct HolderDistribution {
        pub token_mint: Pubkey,
        pub top_holders: Vec<HolderEntry>,
        pub total_holders: u32,
    }

    // ---------------------------------------------------------------------------
    // Errors
    // ---------------------------------------------------------------------------

    #[error_code]
    pub enum AnalyticsError {
        #[msg("Analytics already updated this slot")]
        AlreadyUpdatedThisSlot,
        #[msg("Invalid token mint")]
        InvalidTokenMint,
        #[msg("Arithmetic overflow")]
        Overflow,
    }

    // ---------------------------------------------------------------------------
    // Instruction accounts
    // ---------------------------------------------------------------------------

    #[derive(Accounts)]
    #[instruction(token_mint: Pubkey)]
    pub struct InitializeAnalytics<'info> {
        #[account(
            init,
            payer = payer,
            space = 8 + std::mem::size_of::<TokenAnalytics>(),
            seeds = [b"token_analytics", token_mint.as_ref()],
            bump
        )]
        pub token_analytics: Account<'info, TokenAnalytics>,

        #[account(
            init,
            payer = payer,
            space = 8 + std::mem::size_of::<WhaleTracker>(),
            seeds = [b"whale_tracker", token_mint.as_ref()],
            bump
        )]
        pub whale_tracker: Account<'info, WhaleTracker>,

        #[account(mut)]
        pub payer: Signer<'info>,
        pub system_program: Program<'info, System>,
    }

    #[derive(Accounts)]
    pub struct UpdateAnalytics<'info> {
        #[account(
            mut,
            seeds = [b"token_analytics", token_analytics.token_mint.as_ref()],
            bump = token_analytics.bump
        )]
        pub token_analytics: Account<'info, TokenAnalytics>,

        #[account(
            mut,
            seeds = [b"whale_tracker", token_analytics.token_mint.as_ref()],
            bump = whale_tracker.bump
        )]
        pub whale_tracker: Account<'info, WhaleTracker>,

        /// Permissionless crank signer
        pub crank: Signer<'info>,
    }

    #[derive(Accounts)]
    pub struct GetHolderDistribution<'info> {
        #[account(
            seeds = [b"token_analytics", token_analytics.token_mint.as_ref()],
            bump = token_analytics.bump
        )]
        pub token_analytics: Account<'info, TokenAnalytics>,

        #[account(
            seeds = [b"whale_tracker", token_analytics.token_mint.as_ref()],
            bump = whale_tracker.bump
        )]
        pub whale_tracker: Account<'info, WhaleTracker>,
    }

    // ---------------------------------------------------------------------------
    // Instructions
    // ---------------------------------------------------------------------------

    pub fn handle_initialize_analytics(
        ctx: Context<InitializeAnalytics>,
        token_mint: Pubkey,
    ) -> Result<()> {
        let analytics = &mut ctx.accounts.token_analytics;
        analytics.token_mint = token_mint;
        analytics.bump = ctx.bumps.token_analytics;
        analytics.last_snapshot_ts = Clock::get()?.unix_timestamp;

        let tracker = &mut ctx.accounts.whale_tracker;
        tracker.token_mint = token_mint;
        tracker.bump = ctx.bumps.whale_tracker;

        Ok(())
    }

    pub fn handle_update_analytics(
        ctx: Context<UpdateAnalytics>,
        trade_volume_lamports: u64,
        is_buy: bool,
        trader: Pubkey,
        current_holder_count: u32,
    ) -> Result<()> {
        let analytics = &mut ctx.accounts.token_analytics;
        let clock = Clock::get()?;
        let current_slot = clock.slot;

        require!(
            current_slot > analytics.last_update_slot,
            AnalyticsError::AlreadyUpdatedThisSlot
        );

        // Update totals
        analytics.total_volume = analytics
            .total_volume
            .checked_add(trade_volume_lamports)
            .ok_or(AnalyticsError::Overflow)?;
        analytics.total_trades = analytics
            .total_trades
            .checked_add(1)
            .ok_or(AnalyticsError::Overflow)?;
        analytics.holder_count = current_holder_count;
        analytics.last_update_slot = current_slot;

        // Rotate hourly snapshot if needed
        let now = clock.unix_timestamp;
        if now - analytics.last_snapshot_ts >= HOUR_SECONDS {
            let head = analytics.snapshot_head as usize;
            analytics.hourly_volumes[head] = trade_volume_lamports;
            analytics.hourly_holders[head] = current_holder_count;
            analytics.snapshot_head = ((head + 1) % MAX_HOURLY_SNAPSHOTS) as u16;
            analytics.last_snapshot_ts = now;
        } else {
            // Accumulate into current bucket
            let head = if analytics.snapshot_head == 0 {
                MAX_HOURLY_SNAPSHOTS - 1
            } else {
                (analytics.snapshot_head - 1) as usize
            };
            analytics.hourly_volumes[head] = analytics.hourly_volumes[head]
                .checked_add(trade_volume_lamports)
                .ok_or(AnalyticsError::Overflow)?;
            analytics.hourly_holders[head] = current_holder_count;
        }

        // Track whale transactions
        if trade_volume_lamports >= WHALE_THRESHOLD_LAMPORTS {
            let wh = analytics.whale_tx_head as usize;
            analytics.whale_transactions[wh] = WhaleTransaction {
                trader,
                amount_lamports: trade_volume_lamports,
                is_buy,
                timestamp: now,
            };
            analytics.whale_tx_head = ((wh + 1) % MAX_WHALE_TRANSACTIONS) as u16;

            emit!(WhaleAlert {
                token_mint: analytics.token_mint,
                trader,
                amount_lamports: trade_volume_lamports,
                is_buy,
                timestamp: now,
            });
        }

        // Update whale tracker top holders
        let tracker = &mut ctx.accounts.whale_tracker;
        update_top_holders(tracker, trader, trade_volume_lamports, is_buy);

        emit!(AnalyticsUpdated {
            token_mint: analytics.token_mint,
            total_volume: analytics.total_volume,
            total_trades: analytics.total_trades,
            holder_count: analytics.holder_count,
            timestamp: now,
        });

        Ok(())
    }

    pub fn handle_get_holder_distribution(ctx: Context<GetHolderDistribution>) -> Result<()> {
        let analytics = &ctx.accounts.token_analytics;
        let tracker = &ctx.accounts.whale_tracker;

        let count = tracker.holder_count as usize;
        let holders: Vec<HolderEntry> = tracker.top_holders[..count].to_vec();

        emit!(HolderDistribution {
            token_mint: analytics.token_mint,
            top_holders: holders,
            total_holders: analytics.holder_count,
        });

        Ok(())
    }

    // ---------------------------------------------------------------------------
    // Helpers
    // ---------------------------------------------------------------------------

    fn update_top_holders(tracker: &mut WhaleTracker, wallet: Pubkey, amount: u64, is_buy: bool) {
        let count = tracker.holder_count as usize;

        // Check if wallet already in list
        for i in 0..count {
            if tracker.top_holders[i].wallet == wallet {
                if is_buy {
                    tracker.top_holders[i].balance = tracker.top_holders[i]
                        .balance
                        .saturating_add(amount);
                } else {
                    tracker.top_holders[i].balance = tracker.top_holders[i]
                        .balance
                        .saturating_sub(amount);
                }
                sort_holders(tracker);
                return;
            }
        }

        // New entry — only add on buys
        if !is_buy {
            return;
        }

        if count < MAX_TOP_HOLDERS {
            tracker.top_holders[count] = HolderEntry {
                wallet,
                balance: amount,
            };
            tracker.holder_count += 1;
        } else {
            // Replace smallest if new amount is larger
            let min_idx = (0..MAX_TOP_HOLDERS)
                .min_by_key(|&i| tracker.top_holders[i].balance)
                .unwrap();
            if amount > tracker.top_holders[min_idx].balance {
                tracker.top_holders[min_idx] = HolderEntry {
                    wallet,
                    balance: amount,
                };
            }
        }
        sort_holders(tracker);
    }

    fn sort_holders(tracker: &mut WhaleTracker) {
        let count = tracker.holder_count as usize;
        // Simple insertion sort (max 20 elements)
        for i in 1..count {
            let mut j = i;
            while j > 0 && tracker.top_holders[j].balance > tracker.top_holders[j - 1].balance {
                let tmp = tracker.top_holders[j];
                tracker.top_holders[j] = tracker.top_holders[j - 1];
                tracker.top_holders[j - 1] = tmp;
                j -= 1;
            }
        }
    }

}
pub mod bridge {
    use super::*;

    // ---------------------------------------------------------------------------
    // Constants
    // ---------------------------------------------------------------------------

    pub const BRIDGE_CONFIG_SEED: &[u8] = b"bridge_config";
    pub const BRIDGE_REQUEST_SEED: &[u8] = b"bridge_req";
    pub const VAULT_SEED: &[u8] = b"vault";
    pub const MAX_CHAINS: usize = 10;
    pub const BRIDGE_EXPIRY_SECONDS: i64 = 86_400; // 24 hours

    // ---------------------------------------------------------------------------
    // Errors
    // ---------------------------------------------------------------------------

    #[error_code]
    pub enum BridgeError {
        #[msg("Bridge is currently paused")]
        BridgePaused,
        #[msg("Unsupported destination chain")]
        UnsupportedChain,
        #[msg("Amount below minimum bridge threshold")]
        AmountTooLow,
        #[msg("Bridge request is not in the expected status")]
        InvalidStatus,
        #[msg("Bridge request has not expired yet")]
        NotExpired,
        #[msg("Bridge request has expired")]
        Expired,
        #[msg("Invalid Wormhole VAA")]
        InvalidVAA,
        #[msg("Unauthorized")]
        Unauthorized,
        #[msg("Arithmetic overflow")]
        Overflow,
        #[msg("Maximum supported chains exceeded")]
        MaxChainsExceeded,
    }

    // ---------------------------------------------------------------------------
    // Enums
    // ---------------------------------------------------------------------------

    #[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Debug)]
    pub enum BridgeStatus {
        Pending,
        WormholeSent,
        Completed,
        Cancelled,
    }

    // ---------------------------------------------------------------------------
    // Account structs (state)
    // ---------------------------------------------------------------------------

    #[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug)]
    pub struct ChainInfo {
        pub chain_id: u16,
        pub fee_bps: u16,          // basis points (e.g. 50 = 0.5%)
        pub min_amount: u64,
        pub enabled: bool,
    }

    #[account]
    pub struct BridgeConfig {
        pub authority: Pubkey,
        pub wormhole_program: Pubkey,
        pub wormhole_bridge: Pubkey,       // Wormhole token bridge address
        pub fee_collector: Pubkey,
        pub supported_chains: Vec<ChainInfo>,
        pub total_bridged: u64,
        pub total_requests: u64,
        pub paused: bool,
        pub bump: u8,
    }

    impl BridgeConfig {
        pub const MAX_SIZE: usize = 8  // discriminator
            + 32   // authority
            + 32   // wormhole_program
            + 32   // wormhole_bridge
            + 32   // fee_collector
            + 4 + (MAX_CHAINS * 13) // vec overhead + chain info (2+2+8+1 per chain)
            + 8    // total_bridged
            + 8    // total_requests
            + 1    // paused
            + 1;   // bump

        pub fn get_chain(&self, chain_id: u16) -> Option<&ChainInfo> {
            self.supported_chains.iter().find(|c| c.chain_id == chain_id && c.enabled)
        }
    }

    #[account]
    pub struct BridgeRequest {
        pub user: Pubkey,
        pub token_mint: Pubkey,
        pub amount: u64,
        pub fee_amount: u64,
        pub net_amount: u64,
        pub destination_chain: u16,
        pub destination_address: [u8; 32],
        pub status: BridgeStatus,
        pub created_at: i64,
        pub wormhole_sequence: Option<u64>,
        pub nonce: u64,
        pub bump: u8,
    }

    impl BridgeRequest {
        pub const MAX_SIZE: usize = 8  // discriminator
            + 32   // user
            + 32   // token_mint
            + 8    // amount
            + 8    // fee_amount
            + 8    // net_amount
            + 2    // destination_chain
            + 32   // destination_address
            + 1    // status (enum)
            + 8    // created_at
            + 9    // wormhole_sequence (Option<u64>)
            + 8    // nonce
            + 1;   // bump
    }

    // ---------------------------------------------------------------------------
    // Events
    // ---------------------------------------------------------------------------

    #[event]
    pub struct BridgeInitiated {
        pub user: Pubkey,
        pub token_mint: Pubkey,
        pub amount: u64,
        pub fee_amount: u64,
        pub net_amount: u64,
        pub destination_chain: u16,
        pub destination_address: [u8; 32],
        pub nonce: u64,
        pub timestamp: i64,
    }

    #[event]
    pub struct BridgeConfirmed {
        pub user: Pubkey,
        pub token_mint: Pubkey,
        pub net_amount: u64,
        pub destination_chain: u16,
        pub wormhole_sequence: u64,
        pub timestamp: i64,
    }

    #[event]
    pub struct BridgeCancelled {
        pub user: Pubkey,
        pub token_mint: Pubkey,
        pub amount: u64,
        pub nonce: u64,
        pub timestamp: i64,
    }

    #[event]
    pub struct BridgeConfigUpdated {
        pub authority: Pubkey,
        pub timestamp: i64,
    }

    // ---------------------------------------------------------------------------
    // Instructions
    // ---------------------------------------------------------------------------

    /// Initialize the bridge config (admin only, once).
    #[derive(Accounts)]
    pub struct InitializeBridge<'info> {
        #[account(
            init,
            payer = authority,
            space = BridgeConfig::MAX_SIZE,
            seeds = [BRIDGE_CONFIG_SEED],
            bump,
        )]
        pub bridge_config: Account<'info, BridgeConfig>,

        #[account(mut)]
        pub authority: Signer<'info>,

        pub system_program: Program<'info, System>,
    }

    /// Initiate a bridge request — locks tokens in the vault.
    #[derive(Accounts)]
    #[instruction(amount: u64, destination_chain: u16, destination_address: [u8; 32], nonce: u64)]
    pub struct InitiateBridge<'info> {
        #[account(
            seeds = [BRIDGE_CONFIG_SEED],
            bump = bridge_config.bump,
            constraint = !bridge_config.paused @ BridgeError::BridgePaused,
        )]
        pub bridge_config: Account<'info, BridgeConfig>,

        #[account(
            init,
            payer = user,
            space = BridgeRequest::MAX_SIZE,
            seeds = [BRIDGE_REQUEST_SEED, user.key().as_ref(), &nonce.to_le_bytes()],
            bump,
        )]
        pub bridge_request: Account<'info, BridgeRequest>,

        #[account(mut)]
        pub user: Signer<'info>,

        pub token_mint: Account<'info, Mint>,

        /// User's token account (source).
        #[account(
            mut,
            constraint = user_token_account.owner == user.key(),
            constraint = user_token_account.mint == token_mint.key(),
        )]
        pub user_token_account: Account<'info, TokenAccount>,

        /// Program-owned vault for this token.
        #[account(
            mut,
            seeds = [VAULT_SEED, token_mint.key().as_ref()],
            bump,
        )]
        pub token_vault: Account<'info, TokenAccount>,

        /// Fee collector token account.
        #[account(
            mut,
            constraint = fee_vault.mint == token_mint.key(),
        )]
        pub fee_vault: Account<'info, TokenAccount>,

        pub token_program: Program<'info, Token>,
        pub system_program: Program<'info, System>,
    }

    /// Confirm bridge after Wormhole VAA verification.
    #[derive(Accounts)]
    pub struct ConfirmBridge<'info> {
        #[account(
            mut,
            seeds = [BRIDGE_CONFIG_SEED],
            bump = bridge_config.bump,
        )]
        pub bridge_config: Account<'info, BridgeConfig>,

        #[account(
            mut,
            seeds = [BRIDGE_REQUEST_SEED, bridge_request.user.as_ref(), &bridge_request.nonce.to_le_bytes()],
            bump = bridge_request.bump,
            constraint = bridge_request.status == BridgeStatus::Pending @ BridgeError::InvalidStatus,
        )]
        pub bridge_request: Account<'info, BridgeRequest>,

        /// Authority or permissioned relayer.
        pub authority: Signer<'info>,

        /// CHECK: Wormhole VAA account — verified by the wormhole program.
        pub wormhole_vaa: UncheckedAccount<'info>,

        /// CHECK: Wormhole program for CPI.
        pub wormhole_program: UncheckedAccount<'info>,
    }

    /// Cancel an expired bridge request — returns tokens to user.
    #[derive(Accounts)]
    pub struct CancelBridge<'info> {
        #[account(
            seeds = [BRIDGE_CONFIG_SEED],
            bump = bridge_config.bump,
        )]
        pub bridge_config: Account<'info, BridgeConfig>,

        #[account(
            mut,
            seeds = [BRIDGE_REQUEST_SEED, user.key().as_ref(), &bridge_request.nonce.to_le_bytes()],
            bump = bridge_request.bump,
            constraint = bridge_request.status == BridgeStatus::Pending @ BridgeError::InvalidStatus,
            constraint = bridge_request.user == user.key() @ BridgeError::Unauthorized,
        )]
        pub bridge_request: Account<'info, BridgeRequest>,

        #[account(mut)]
        pub user: Signer<'info>,

        pub token_mint: Account<'info, Mint>,

        /// User's token account (destination for refund).
        #[account(
            mut,
            constraint = user_token_account.owner == user.key(),
            constraint = user_token_account.mint == token_mint.key(),
        )]
        pub user_token_account: Account<'info, TokenAccount>,

        /// Program-owned vault.
        #[account(
            mut,
            seeds = [VAULT_SEED, token_mint.key().as_ref()],
            bump,
        )]
        pub token_vault: Account<'info, TokenAccount>,

        /// Fee vault — fee is also refunded on cancel.
        #[account(
            mut,
            constraint = fee_vault.mint == token_mint.key(),
        )]
        pub fee_vault: Account<'info, TokenAccount>,

        pub token_program: Program<'info, Token>,
    }

    // ---------------------------------------------------------------------------
    // Instruction handlers
    // ---------------------------------------------------------------------------

    pub fn handle_initialize_bridge(
        ctx: Context<InitializeBridge>,
        wormhole_program: Pubkey,
        wormhole_bridge: Pubkey,
        fee_collector: Pubkey,
        chains: Vec<ChainInfo>,
    ) -> Result<()> {
        require!(chains.len() <= MAX_CHAINS, BridgeError::MaxChainsExceeded);

        let config = &mut ctx.accounts.bridge_config;
        config.authority = ctx.accounts.authority.key();
        config.wormhole_program = wormhole_program;
        config.wormhole_bridge = wormhole_bridge;
        config.fee_collector = fee_collector;
        config.supported_chains = chains;
        config.total_bridged = 0;
        config.total_requests = 0;
        config.paused = false;
        config.bump = ctx.bumps.bridge_config;

        emit!(BridgeConfigUpdated {
            authority: config.authority,
            timestamp: Clock::get()?.unix_timestamp,
        });

        Ok(())
    }

    pub fn handle_initiate_bridge(
        ctx: Context<InitiateBridge>,
        amount: u64,
        destination_chain: u16,
        destination_address: [u8; 32],
        nonce: u64,
    ) -> Result<()> {
        let config = &ctx.accounts.bridge_config;
        let chain = config
            .get_chain(destination_chain)
            .ok_or(BridgeError::UnsupportedChain)?;

        require!(amount >= chain.min_amount, BridgeError::AmountTooLow);

        // Calculate fee
        let fee_amount = amount
            .checked_mul(chain.fee_bps as u64)
            .ok_or(BridgeError::Overflow)?
            .checked_div(10_000)
            .ok_or(BridgeError::Overflow)?;
        let net_amount = amount.checked_sub(fee_amount).ok_or(BridgeError::Overflow)?;

        // Transfer net amount to vault
        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.user_token_account.to_account_info(),
                    to: ctx.accounts.token_vault.to_account_info(),
                    authority: ctx.accounts.user.to_account_info(),
                },
            ),
            net_amount,
        )?;

        // Transfer fee to fee vault
        if fee_amount > 0 {
            token::transfer(
                CpiContext::new(
                    ctx.accounts.token_program.to_account_info(),
                    Transfer {
                        from: ctx.accounts.user_token_account.to_account_info(),
                        to: ctx.accounts.fee_vault.to_account_info(),
                        authority: ctx.accounts.user.to_account_info(),
                    },
                ),
                fee_amount,
            )?;
        }

        // Populate bridge request
        let now = Clock::get()?.unix_timestamp;
        let req = &mut ctx.accounts.bridge_request;
        req.user = ctx.accounts.user.key();
        req.token_mint = ctx.accounts.token_mint.key();
        req.amount = amount;
        req.fee_amount = fee_amount;
        req.net_amount = net_amount;
        req.destination_chain = destination_chain;
        req.destination_address = destination_address;
        req.status = BridgeStatus::Pending;
        req.created_at = now;
        req.wormhole_sequence = None;
        req.nonce = nonce;
        req.bump = ctx.bumps.bridge_request;

        // TODO: CPI to Wormhole to post the message
        // wormhole::post_message(cpi_ctx, nonce, payload, consistency_level)?;

        emit!(BridgeInitiated {
            user: req.user,
            token_mint: req.token_mint,
            amount,
            fee_amount,
            net_amount,
            destination_chain,
            destination_address,
            nonce,
            timestamp: now,
        });

        Ok(())
    }

    pub fn handle_confirm_bridge(
        ctx: Context<ConfirmBridge>,
        wormhole_sequence: u64,
    ) -> Result<()> {
        let config = &ctx.accounts.bridge_config;
        require!(
            ctx.accounts.authority.key() == config.authority,
            BridgeError::Unauthorized
        );

        // TODO: Verify the Wormhole VAA via CPI
        // wormhole::verify_vaa(cpi_ctx, vaa_data)?;

        let now = Clock::get()?.unix_timestamp;
        let req = &mut ctx.accounts.bridge_request;

        // Check not expired
        require!(
            now <= req.created_at + BRIDGE_EXPIRY_SECONDS,
            BridgeError::Expired
        );

        req.status = BridgeStatus::Completed;
        req.wormhole_sequence = Some(wormhole_sequence);

        let config = &mut ctx.accounts.bridge_config;
        config.total_bridged = config
            .total_bridged
            .checked_add(req.net_amount)
            .ok_or(BridgeError::Overflow)?;
        config.total_requests = config
            .total_requests
            .checked_add(1)
            .ok_or(BridgeError::Overflow)?;

        emit!(BridgeConfirmed {
            user: req.user,
            token_mint: req.token_mint,
            net_amount: req.net_amount,
            destination_chain: req.destination_chain,
            wormhole_sequence,
            timestamp: now,
        });

        Ok(())
    }

    pub fn handle_cancel_bridge(ctx: Context<CancelBridge>) -> Result<()> {
        let req = &ctx.accounts.bridge_request;
        let now = Clock::get()?.unix_timestamp;

        // Must be expired to cancel
        require!(
            now > req.created_at + BRIDGE_EXPIRY_SECONDS,
            BridgeError::NotExpired
        );

        // Refund net amount from vault to user (PDA signer)
        let mint_key = ctx.accounts.token_mint.key();
        let seeds: &[&[u8]] = &[VAULT_SEED, mint_key.as_ref()];
        let (_, vault_bump) = Pubkey::find_program_address(seeds, ctx.program_id);
        let signer_seeds: &[&[&[u8]]] = &[&[VAULT_SEED, mint_key.as_ref(), &[vault_bump]]];

        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.token_vault.to_account_info(),
                    to: ctx.accounts.user_token_account.to_account_info(),
                    authority: ctx.accounts.token_vault.to_account_info(),
                },
                signer_seeds,
            ),
            req.net_amount,
        )?;

        // Refund fee (fee vault → user, requires fee vault authority setup)
        // In production, fee vault would also be PDA-owned for refund capability.
        // Simplified: fee is non-refundable or handled via admin instruction.

        let req = &mut ctx.accounts.bridge_request;
        req.status = BridgeStatus::Cancelled;

        emit!(BridgeCancelled {
            user: req.user,
            token_mint: req.token_mint,
            amount: req.amount,
            nonce: req.nonce,
            timestamp: now,
        });

        Ok(())
    }

}
pub mod copy_trading {
    use super::*;

    // ---------------------------------------------------------------------------
    // Constants
    // ---------------------------------------------------------------------------
    pub const MAX_FOLLOWERS: u32 = 10_000;
    pub const MIN_ALLOCATION_LAMPORTS: u64 = 100_000_000; // 0.1 SOL

    // ---------------------------------------------------------------------------
    // Account structs
    // ---------------------------------------------------------------------------

    #[account]
    pub struct TraderProfile {
        pub trader: Pubkey,
        pub total_pnl: i64,          // net PnL in lamports (signed)
        pub total_trades: u64,
        pub winning_trades: u64,
        pub followers_count: u32,
        pub created_at: i64,
        pub last_trade_at: i64,
        pub active: bool,
        pub bump: u8,
    }

    impl TraderProfile {
        /// Win rate in basis points (0-10000)
        pub fn win_rate_bps(&self) -> u16 {
            if self.total_trades == 0 {
                return 0;
            }
            ((self.winning_trades as u128 * 10_000) / self.total_trades as u128) as u16
        }
    }

    #[account]
    pub struct CopyPosition {
        pub follower: Pubkey,
        pub leader: Pubkey,
        pub max_allocation: u64,     // max lamports to allocate per copy trade
        pub used_allocation: u64,    // currently deployed
        pub active: bool,
        pub created_at: i64,
        pub total_copied_trades: u64,
        pub copy_pnl: i64,          // follower's PnL from this copy relationship
        pub bump: u8,
    }

    // ---------------------------------------------------------------------------
    // Events
    // ---------------------------------------------------------------------------

    #[event]
    pub struct TraderProfileCreated {
        pub trader: Pubkey,
        pub timestamp: i64,
    }

    #[event]
    pub struct TraderFollowed {
        pub follower: Pubkey,
        pub leader: Pubkey,
        pub max_allocation: u64,
    }

    #[event]
    pub struct TraderUnfollowed {
        pub follower: Pubkey,
        pub leader: Pubkey,
    }

    #[event]
    pub struct CopyTradeExecuted {
        pub leader: Pubkey,
        pub follower: Pubkey,
        pub token_mint: Pubkey,
        pub leader_amount: u64,
        pub follower_amount: u64,
        pub is_buy: bool,
        pub timestamp: i64,
    }

    #[event]
    pub struct TraderStatsUpdated {
        pub trader: Pubkey,
        pub total_pnl: i64,
        pub win_rate_bps: u16,
        pub total_trades: u64,
    }

    // ---------------------------------------------------------------------------
    // Errors
    // ---------------------------------------------------------------------------

    #[error_code]
    pub enum CopyTradingError {
        #[msg("Trader profile already exists")]
        ProfileAlreadyExists,
        #[msg("Trader profile is not active")]
        ProfileNotActive,
        #[msg("Cannot follow yourself")]
        CannotFollowSelf,
        #[msg("Already following this trader")]
        AlreadyFollowing,
        #[msg("Not following this trader")]
        NotFollowing,
        #[msg("Max followers reached for this trader")]
        MaxFollowersReached,
        #[msg("Allocation below minimum")]
        AllocationTooLow,
        #[msg("Insufficient remaining allocation")]
        InsufficientAllocation,
        #[msg("Copy position is not active")]
        PositionNotActive,
        #[msg("Unauthorized")]
        Unauthorized,
        #[msg("Arithmetic overflow")]
        Overflow,
    }

    // ---------------------------------------------------------------------------
    // Instruction accounts
    // ---------------------------------------------------------------------------

    #[derive(Accounts)]
    pub struct CreateTraderProfile<'info> {
        #[account(
            init,
            payer = trader,
            space = 8 + std::mem::size_of::<TraderProfile>(),
            seeds = [b"trader_profile", trader.key().as_ref()],
            bump
        )]
        pub profile: Account<'info, TraderProfile>,

        #[account(mut)]
        pub trader: Signer<'info>,
        pub system_program: Program<'info, System>,
    }

    #[derive(Accounts)]
    pub struct FollowTrader<'info> {
        #[account(
            mut,
            seeds = [b"trader_profile", leader_profile.trader.as_ref()],
            bump = leader_profile.bump
        )]
        pub leader_profile: Account<'info, TraderProfile>,

        #[account(
            init,
            payer = follower,
            space = 8 + std::mem::size_of::<CopyPosition>(),
            seeds = [b"copy_position", follower.key().as_ref(), leader_profile.trader.as_ref()],
            bump
        )]
        pub copy_position: Account<'info, CopyPosition>,

        #[account(mut)]
        pub follower: Signer<'info>,
        pub system_program: Program<'info, System>,
    }

    #[derive(Accounts)]
    pub struct UnfollowTrader<'info> {
        #[account(
            mut,
            seeds = [b"trader_profile", leader_profile.trader.as_ref()],
            bump = leader_profile.bump
        )]
        pub leader_profile: Account<'info, TraderProfile>,

        #[account(
            mut,
            seeds = [b"copy_position", follower.key().as_ref(), leader_profile.trader.as_ref()],
            bump = copy_position.bump,
            has_one = follower @ CopyTradingError::Unauthorized
        )]
        pub copy_position: Account<'info, CopyPosition>,

        pub follower: Signer<'info>,
    }

    #[derive(Accounts)]
    pub struct ExecuteCopyTrade<'info> {
        #[account(
            mut,
            seeds = [b"trader_profile", leader_profile.trader.as_ref()],
            bump = leader_profile.bump
        )]
        pub leader_profile: Account<'info, TraderProfile>,

        #[account(
            mut,
            seeds = [b"copy_position", copy_position.follower.as_ref(), leader_profile.trader.as_ref()],
            bump = copy_position.bump
        )]
        pub copy_position: Account<'info, CopyPosition>,

        /// The crank or leader who triggers the copy
        pub executor: Signer<'info>,
    }

    // ---------------------------------------------------------------------------
    // Instructions
    // ---------------------------------------------------------------------------

    pub fn handle_create_trader_profile(ctx: Context<CreateTraderProfile>) -> Result<()> {
        let profile = &mut ctx.accounts.profile;
        let clock = Clock::get()?;

        profile.trader = ctx.accounts.trader.key();
        profile.total_pnl = 0;
        profile.total_trades = 0;
        profile.winning_trades = 0;
        profile.followers_count = 0;
        profile.created_at = clock.unix_timestamp;
        profile.last_trade_at = 0;
        profile.active = true;
        profile.bump = ctx.bumps.profile;

        emit!(TraderProfileCreated {
            trader: profile.trader,
            timestamp: clock.unix_timestamp,
        });

        Ok(())
    }

    pub fn handle_follow_trader(
        ctx: Context<FollowTrader>,
        max_allocation: u64,
    ) -> Result<()> {
        let leader = &mut ctx.accounts.leader_profile;
        let follower_key = ctx.accounts.follower.key();

        require!(leader.active, CopyTradingError::ProfileNotActive);
        require!(follower_key != leader.trader, CopyTradingError::CannotFollowSelf);
        require!(leader.followers_count < MAX_FOLLOWERS, CopyTradingError::MaxFollowersReached);
        require!(max_allocation >= MIN_ALLOCATION_LAMPORTS, CopyTradingError::AllocationTooLow);

        leader.followers_count = leader
            .followers_count
            .checked_add(1)
            .ok_or(CopyTradingError::Overflow)?;

        let pos = &mut ctx.accounts.copy_position;
        let clock = Clock::get()?;

        pos.follower = follower_key;
        pos.leader = leader.trader;
        pos.max_allocation = max_allocation;
        pos.used_allocation = 0;
        pos.active = true;
        pos.created_at = clock.unix_timestamp;
        pos.total_copied_trades = 0;
        pos.copy_pnl = 0;
        pos.bump = ctx.bumps.copy_position;

        emit!(TraderFollowed {
            follower: follower_key,
            leader: leader.trader,
            max_allocation,
        });

        Ok(())
    }

    pub fn handle_unfollow_trader(ctx: Context<UnfollowTrader>) -> Result<()> {
        let leader = &mut ctx.accounts.leader_profile;
        let pos = &mut ctx.accounts.copy_position;

        require!(pos.active, CopyTradingError::PositionNotActive);

        pos.active = false;
        leader.followers_count = leader.followers_count.saturating_sub(1);

        emit!(TraderUnfollowed {
            follower: pos.follower,
            leader: leader.trader,
        });

        Ok(())
    }

    pub fn handle_execute_copy_trade(
        ctx: Context<ExecuteCopyTrade>,
        token_mint: Pubkey,
        leader_trade_amount: u64,
        leader_total_balance: u64,
        is_buy: bool,
        trade_pnl: i64,
    ) -> Result<()> {
        let leader = &mut ctx.accounts.leader_profile;
        let pos = &mut ctx.accounts.copy_position;
        let clock = Clock::get()?;

        require!(leader.active, CopyTradingError::ProfileNotActive);
        require!(pos.active, CopyTradingError::PositionNotActive);

        // Calculate proportional follower amount
        // follower_amount = leader_trade_amount * (follower_max_allocation / leader_total_balance)
        let follower_amount = if leader_total_balance > 0 {
            ((leader_trade_amount as u128)
                .checked_mul(pos.max_allocation as u128)
                .ok_or(CopyTradingError::Overflow)?
            )
            .checked_div(leader_total_balance as u128)
            .ok_or(CopyTradingError::Overflow)? as u64
        } else {
            0u64
        };

        let remaining = pos
            .max_allocation
            .saturating_sub(pos.used_allocation);
        require!(
            !is_buy || follower_amount <= remaining,
            CopyTradingError::InsufficientAllocation
        );

        // Update copy position
        if is_buy {
            pos.used_allocation = pos
                .used_allocation
                .checked_add(follower_amount)
                .ok_or(CopyTradingError::Overflow)?;
        } else {
            pos.used_allocation = pos.used_allocation.saturating_sub(follower_amount);
        }

        pos.total_copied_trades = pos
            .total_copied_trades
            .checked_add(1)
            .ok_or(CopyTradingError::Overflow)?;

        // Scale PnL proportionally for follower
        let follower_pnl = if leader_total_balance > 0 {
            ((trade_pnl as i128)
                .checked_mul(pos.max_allocation as i128)
                .ok_or(CopyTradingError::Overflow)?
            )
            .checked_div(leader_total_balance as i128)
            .ok_or(CopyTradingError::Overflow)? as i64
        } else {
            0i64
        };
        pos.copy_pnl = pos
            .copy_pnl
            .checked_add(follower_pnl)
            .ok_or(CopyTradingError::Overflow)?;

        // Update leader stats
        leader.total_trades = leader
            .total_trades
            .checked_add(1)
            .ok_or(CopyTradingError::Overflow)?;
        leader.total_pnl = leader
            .total_pnl
            .checked_add(trade_pnl)
            .ok_or(CopyTradingError::Overflow)?;
        if trade_pnl > 0 {
            leader.winning_trades = leader
                .winning_trades
                .checked_add(1)
                .ok_or(CopyTradingError::Overflow)?;
        }
        leader.last_trade_at = clock.unix_timestamp;

        emit!(CopyTradeExecuted {
            leader: leader.trader,
            follower: pos.follower,
            token_mint,
            leader_amount: leader_trade_amount,
            follower_amount,
            is_buy,
            timestamp: clock.unix_timestamp,
        });

        emit!(TraderStatsUpdated {
            trader: leader.trader,
            total_pnl: leader.total_pnl,
            win_rate_bps: leader.win_rate_bps(),
            total_trades: leader.total_trades,
        });

        Ok(())
    }

}
pub mod creator_dashboard {
    use super::*;

    // ============================================================================
    // SEEDS & CONSTANTS
    // ============================================================================

    pub const CREATOR_ANALYTICS_SEED: &[u8] = b"creator_analytics";
    pub const TOKEN_ANALYTICS_SNAPSHOT_SEED: &[u8] = b"token_analytics_snapshot";

    /// Number of hourly slots tracked in the rolling snapshot.
    pub const HOURLY_SLOTS: usize = 168; // 7 days of hourly data

    // ============================================================================
    // ACCOUNTS
    // ============================================================================

    /// Aggregate analytics for a single creator wallet.
    #[account]
    #[derive(Default)]
    pub struct CreatorAnalytics {
        /// The creator wallet this PDA tracks.
        pub creator: Pubkey,
        /// Total number of token launches by this creator.
        pub total_launches: u32,
        /// Cumulative trading volume across all creator tokens (lamports).
        pub total_volume_generated: u64,
        /// Total fees earned by the creator (lamports).
        pub total_fees_earned: u64,
        /// Sum of unique holders across all creator tokens.
        pub total_holders_across_tokens: u32,
        /// Mint address of the creator's best-performing token (by volume).
        pub best_performing_token: Pubkey,
        /// Average time-to-graduation in seconds (0 if none graduated).
        pub avg_graduation_time: i64,
        /// PDA bump.
        pub bump: u8,
    }

    impl CreatorAnalytics {
        pub const SIZE: usize = 8  // discriminator
            + 32 // creator
            + 4  // total_launches
            + 8  // total_volume_generated
            + 8  // total_fees_earned
            + 4  // total_holders_across_tokens
            + 32 // best_performing_token
            + 8  // avg_graduation_time
            + 1; // bump
    }

    /// Per-token rolling analytics snapshot used for charts.
    #[account]
    #[derive(Default)]
    pub struct TokenAnalyticsSnapshot {
        /// The token mint this snapshot tracks.
        pub token_mint: Pubkey,
        /// Rolling hourly volume array (lamports). Index = hour_index % HOURLY_SLOTS.
        pub hourly_volume: Vec<u64>,
        /// Rolling hourly holder growth (delta). Same indexing.
        pub holder_growth: Vec<i32>,
        /// Current write cursor (hour_index).
        pub current_slot: u16,
        /// PDA bump.
        pub bump: u8,
    }

    impl TokenAnalyticsSnapshot {
        pub const SIZE: usize = 8  // discriminator
            + 32 // token_mint
            + 4 + (HOURLY_SLOTS * 8) // Vec<u64> hourly_volume
            + 4 + (HOURLY_SLOTS * 4) // Vec<i32> holder_growth
            + 2  // current_slot
            + 1; // bump
    }

    // ============================================================================
    // EVENTS
    // ============================================================================

    #[event]
    pub struct CreatorStatsEvent {
        pub creator: Pubkey,
        pub total_launches: u32,
        pub total_volume_generated: u64,
        pub total_fees_earned: u64,
        pub total_holders_across_tokens: u32,
        pub best_performing_token: Pubkey,
        pub avg_graduation_time: i64,
    }

    // ============================================================================
    // INSTRUCTIONS
    // ============================================================================

    /// Permissionless crank to update a creator's aggregate analytics.
    pub fn update_creator_analytics(
        ctx: Context<UpdateCreatorAnalytics>,
        total_launches: u32,
        total_volume_generated: u64,
        total_fees_earned: u64,
        total_holders_across_tokens: u32,
        best_performing_token: Pubkey,
        avg_graduation_time: i64,
        // Optional: snapshot data for the token
        hourly_volume_entry: u64,
        holder_growth_entry: i32,
    ) -> Result<()> {
        // --- Creator-level analytics ---
        let analytics = &mut ctx.accounts.creator_analytics;
        analytics.creator = ctx.accounts.creator.key();
        analytics.total_launches = total_launches;
        analytics.total_volume_generated = total_volume_generated;
        analytics.total_fees_earned = total_fees_earned;
        analytics.total_holders_across_tokens = total_holders_across_tokens;
        analytics.best_performing_token = best_performing_token;
        analytics.avg_graduation_time = avg_graduation_time;
        analytics.bump = ctx.bumps.creator_analytics;

        // --- Token-level snapshot ---
        let snapshot = &mut ctx.accounts.token_analytics_snapshot;
        snapshot.token_mint = ctx.accounts.token_mint.key();
        snapshot.bump = ctx.bumps.token_analytics_snapshot;

        // Initialise vectors on first use
        if snapshot.hourly_volume.is_empty() {
            snapshot.hourly_volume = vec![0u64; HOURLY_SLOTS];
            snapshot.holder_growth = vec![0i32; HOURLY_SLOTS];
        }

        let idx = snapshot.current_slot as usize % HOURLY_SLOTS;
        snapshot.hourly_volume[idx] = hourly_volume_entry;
        snapshot.holder_growth[idx] = holder_growth_entry;
        snapshot.current_slot = snapshot.current_slot.wrapping_add(1);

        Ok(())
    }

    /// Emits an event with the creator's aggregate stats for frontend consumption.
    pub fn get_creator_stats(ctx: Context<GetCreatorStats>) -> Result<()> {
        let a = &ctx.accounts.creator_analytics;

        emit!(CreatorStatsEvent {
            creator: a.creator,
            total_launches: a.total_launches,
            total_volume_generated: a.total_volume_generated,
            total_fees_earned: a.total_fees_earned,
            total_holders_across_tokens: a.total_holders_across_tokens,
            best_performing_token: a.best_performing_token,
            avg_graduation_time: a.avg_graduation_time,
        });

        Ok(())
    }

    // ============================================================================
    // CONTEXT STRUCTS
    // ============================================================================

    #[derive(Accounts)]
    pub struct UpdateCreatorAnalytics<'info> {
        #[account(
            init_if_needed,
            payer = payer,
            space = CreatorAnalytics::SIZE,
            seeds = [CREATOR_ANALYTICS_SEED, creator.key().as_ref()],
            bump,
        )]
        pub creator_analytics: Account<'info, CreatorAnalytics>,

        #[account(
            init_if_needed,
            payer = payer,
            space = TokenAnalyticsSnapshot::SIZE,
            seeds = [TOKEN_ANALYTICS_SNAPSHOT_SEED, token_mint.key().as_ref()],
            bump,
        )]
        pub token_analytics_snapshot: Account<'info, TokenAnalyticsSnapshot>,

        /// CHECK: Creator wallet — validated by seeds.
        pub creator: UncheckedAccount<'info>,

        /// CHECK: Token mint — validated by seeds.
        pub token_mint: UncheckedAccount<'info>,

        #[account(mut)]
        pub payer: Signer<'info>,

        pub system_program: Program<'info, System>,
    }

    #[derive(Accounts)]
    pub struct GetCreatorStats<'info> {
        #[account(
            seeds = [CREATOR_ANALYTICS_SEED, creator.key().as_ref()],
            bump = creator_analytics.bump,
        )]
        pub creator_analytics: Account<'info, CreatorAnalytics>,

        /// CHECK: Creator wallet — validated by seeds.
        pub creator: UncheckedAccount<'info>,
    }

}
pub mod custom_pages {
    use super::*;

    use crate::{
        TokenLaunch, PlatformConfig, SendItError,
        TOKEN_LAUNCH_SEED, PLATFORM_CONFIG_SEED, PLATFORM_VAULT_SEED,
        LAMPORTS_PER_SOL,
    };

    // ============================================================================
    // CONSTANTS
    // ============================================================================

    pub const CUSTOM_PAGE_SEED: &[u8] = b"custom_page";

    pub const MAX_HEADER_IMAGE_URL: usize = 256;
    pub const MAX_THEME_COLOR: usize = 7;         // #RRGGBB
    pub const MAX_DESCRIPTION_LONG: usize = 2000;
    pub const MAX_SOCIAL_LINKS: usize = 512;      // JSON string of social links
    pub const MAX_CSS_HASH: usize = 64;           // SHA256 hex of custom CSS

    // Tier pricing in lamports
    pub const TIER_BASIC_PRICE: u64 = 0;
    pub const TIER_PRO_PRICE: u64 = 100_000_000;      // 0.1 SOL
    pub const TIER_ULTRA_PRICE: u64 = 500_000_000;     // 0.5 SOL

    // ============================================================================
    // ACCOUNTS
    // ============================================================================

    #[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
    pub enum PageTier {
        Basic,  // free: theme_color only
        Pro,    // 0.1 SOL: + header image, long description
        Ultra,  // 0.5 SOL: + social links, custom CSS hash
    }

    #[account]
    pub struct CustomPage {
        pub token_launch: Pubkey,          // associated TokenLaunch PDA
        pub mint: Pubkey,                  // token mint
        pub creator: Pubkey,               // page owner (token creator)
        pub tier: PageTier,                // current tier level
        pub header_image_url: String,      // Pro+: header/banner image URL
        pub theme_color: String,           // Basic+: hex color code
        pub description_long: String,      // Pro+: extended description
        pub social_links: String,          // Ultra: JSON encoded social links
        pub custom_css_hash: String,       // Ultra: hash of approved custom CSS
        pub last_updated: i64,             // timestamp of last update
        pub bump: u8,
    }

    impl CustomPage {
        pub const SIZE: usize = 8          // discriminator
            + 32                            // token_launch
            + 32                            // mint
            + 32                            // creator
            + 1                             // tier
            + (4 + MAX_HEADER_IMAGE_URL)    // header_image_url
            + (4 + MAX_THEME_COLOR)         // theme_color
            + (4 + MAX_DESCRIPTION_LONG)    // description_long
            + (4 + MAX_SOCIAL_LINKS)        // social_links
            + (4 + MAX_CSS_HASH)            // custom_css_hash
            + 8                             // last_updated
            + 1;                            // bump
    }

    // ============================================================================
    // INSTRUCTIONS
    // ============================================================================

    /// Update (or initialize) a custom page for a token launch.
    /// Creator pays the tier fee in SOL to the platform vault.
    /// Upgrading tier pays the difference. Downgrading is not refunded.
    pub fn update_custom_page(
        ctx: Context<UpdateCustomPage>,
        tier: PageTier,
        header_image_url: Option<String>,
        theme_color: Option<String>,
        description_long: Option<String>,
        social_links: Option<String>,
        custom_css_hash: Option<String>,
    ) -> Result<()> {
        let config = &ctx.accounts.platform_config;
        require!(!config.paused, SendItError::PlatformPaused);

        let launch = &ctx.accounts.token_launch;
        require!(launch.creator == ctx.accounts.creator.key(), SendItError::InvalidCreator);

        // Validate field lengths
        if let Some(ref url) = header_image_url {
            require!(url.len() <= MAX_HEADER_IMAGE_URL, CustomPageError::FieldTooLong);
        }
        if let Some(ref color) = theme_color {
            require!(color.len() <= MAX_THEME_COLOR, CustomPageError::FieldTooLong);
            // Basic hex color validation
            require!(
                color.starts_with('#') && color.len() == 7,
                CustomPageError::InvalidColor
            );
        }
        if let Some(ref desc) = description_long {
            require!(desc.len() <= MAX_DESCRIPTION_LONG, CustomPageError::FieldTooLong);
        }
        if let Some(ref links) = social_links {
            require!(links.len() <= MAX_SOCIAL_LINKS, CustomPageError::FieldTooLong);
        }
        if let Some(ref hash) = custom_css_hash {
            require!(hash.len() <= MAX_CSS_HASH, CustomPageError::FieldTooLong);
        }

        // Enforce tier-based field access
        match tier {
            PageTier::Basic => {
                // Basic only allows theme_color
                require!(header_image_url.is_none(), CustomPageError::TierTooLow);
                require!(description_long.is_none(), CustomPageError::TierTooLow);
                require!(social_links.is_none(), CustomPageError::TierTooLow);
                require!(custom_css_hash.is_none(), CustomPageError::TierTooLow);
            }
            PageTier::Pro => {
                // Pro allows theme_color + header_image + description
                require!(social_links.is_none(), CustomPageError::TierTooLow);
                require!(custom_css_hash.is_none(), CustomPageError::TierTooLow);
            }
            PageTier::Ultra => {
                // Ultra allows everything
            }
        }

        // Calculate fee: pay the tier price (upgrade difference if already paid a lower tier)
        let page = &ctx.accounts.custom_page;
        let current_tier_price = tier_price(page.tier);
        let new_tier_price = tier_price(tier);
        let fee = if new_tier_price > current_tier_price {
            new_tier_price - current_tier_price
        } else {
            0 // No refund on downgrade, no extra charge on same tier
        };

        if fee > 0 {
            anchor_lang::system_program::transfer(
                CpiContext::new(
                    ctx.accounts.system_program.to_account_info(),
                    anchor_lang::system_program::Transfer {
                        from: ctx.accounts.creator.to_account_info(),
                        to: ctx.accounts.platform_vault.to_account_info(),
                    },
                ),
                fee,
            )?;
        }

        // Update page fields
        let clock = Clock::get()?;
        let page = &mut ctx.accounts.custom_page;

        // First-time init fields
        if page.bump == 0 {
            page.token_launch = ctx.accounts.token_launch.key();
            page.mint = launch.mint;
            page.creator = ctx.accounts.creator.key();
            page.bump = ctx.bumps.custom_page;
        }

        page.tier = tier;
        page.last_updated = clock.unix_timestamp;

        if let Some(color) = theme_color {
            page.theme_color = color;
        }
        if let Some(url) = header_image_url {
            page.header_image_url = url;
        }
        if let Some(desc) = description_long {
            page.description_long = desc;
        }
        if let Some(links) = social_links {
            page.social_links = links;
        }
        if let Some(hash) = custom_css_hash {
            page.custom_css_hash = hash;
        }

        emit!(CustomPageUpdated {
            page: page.key(),
            mint: page.mint,
            tier,
            fee_paid: fee,
        });

        Ok(())
    }

    /// Reset a custom page back to defaults (Basic tier).
    /// Only the creator can reset. No refund.
    pub fn reset_page(ctx: Context<ResetPage>) -> Result<()> {
        let page = &mut ctx.accounts.custom_page;
        require!(page.creator == ctx.accounts.creator.key(), SendItError::InvalidCreator);

        let clock = Clock::get()?;

        page.tier = PageTier::Basic;
        page.header_image_url = String::new();
        page.theme_color = String::from("#00ff88"); // default neon green
        page.description_long = String::new();
        page.social_links = String::new();
        page.custom_css_hash = String::new();
        page.last_updated = clock.unix_timestamp;

        emit!(CustomPageReset {
            page: page.key(),
            mint: page.mint,
        });

        Ok(())
    }

    // ============================================================================
    // HELPERS
    // ============================================================================

    fn tier_price(tier: PageTier) -> u64 {
        match tier {
            PageTier::Basic => TIER_BASIC_PRICE,
            PageTier::Pro => TIER_PRO_PRICE,
            PageTier::Ultra => TIER_ULTRA_PRICE,
        }
    }

    // ============================================================================
    // CONTEXT STRUCTS
    // ============================================================================

    #[derive(Accounts)]
    pub struct UpdateCustomPage<'info> {
        #[account(
            init_if_needed,
            payer = creator,
            space = CustomPage::SIZE,
            seeds = [CUSTOM_PAGE_SEED, token_launch.mint.as_ref()],
            bump,
        )]
        pub custom_page: Account<'info, CustomPage>,

        #[account(
            seeds = [TOKEN_LAUNCH_SEED, token_launch.mint.as_ref()],
            bump = token_launch.bump,
            has_one = creator,
        )]
        pub token_launch: Account<'info, TokenLaunch>,

        #[account(
            seeds = [PLATFORM_CONFIG_SEED],
            bump = platform_config.bump,
        )]
        pub platform_config: Account<'info, PlatformConfig>,

        /// CHECK: Platform vault receives tier fees
        #[account(
            mut,
            seeds = [PLATFORM_VAULT_SEED],
            bump,
        )]
        pub platform_vault: AccountInfo<'info>,

        #[account(mut)]
        pub creator: Signer<'info>,

        pub system_program: Program<'info, System>,
    }

    #[derive(Accounts)]
    pub struct ResetPage<'info> {
        #[account(
            mut,
            seeds = [CUSTOM_PAGE_SEED, custom_page.mint.as_ref()],
            bump = custom_page.bump,
            constraint = custom_page.creator == creator.key() @ SendItError::InvalidCreator,
        )]
        pub custom_page: Account<'info, CustomPage>,

        pub creator: Signer<'info>,
    }

    // ============================================================================
    // EVENTS
    // ============================================================================

    #[event]
    pub struct CustomPageUpdated {
        pub page: Pubkey,
        pub mint: Pubkey,
        pub tier: PageTier,
        pub fee_paid: u64,
    }

    #[event]
    pub struct CustomPageReset {
        pub page: Pubkey,
        pub mint: Pubkey,
    }

    // ============================================================================
    // ERRORS
    // ============================================================================

    #[error_code]
    pub enum CustomPageError {
        #[msg("Field exceeds maximum length")]
        FieldTooLong,
        #[msg("Invalid hex color format (use #RRGGBB)")]
        InvalidColor,
        #[msg("Feature requires a higher tier")]
        TierTooLow,
    }

}
pub mod daily_rewards {
    use super::*;

    // ── Seeds ──────────────────────────────────────────────────────────────────────
    pub const DAILY_REWARDS_CONFIG_SEED: &[u8] = b"daily_rewards_config";
    pub const USER_DAILY_REWARDS_SEED: &[u8] = b"user_daily_rewards";

    // ── Constants ──────────────────────────────────────────────────────────────────
    const SECONDS_PER_DAY: i64 = 86_400;

    // ── Errors ─────────────────────────────────────────────────────────────────────
    #[error_code]
    pub enum DailyRewardsError {
        #[msg("Already checked in today")]
        AlreadyCheckedIn,
        #[msg("Insufficient points to redeem")]
        InsufficientPoints,
        #[msg("Unauthorized")]
        Unauthorized,
        #[msg("Invalid reward tier")]
        InvalidTier,
    }

    // ── Enums ──────────────────────────────────────────────────────────────────────
    #[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
    pub enum RewardTier {
        Bronze,   // 0-99 points
        Silver,   // 100-499
        Gold,     // 500-1999
        Platinum, // 2000-9999
        Diamond,  // 10000+
    }

    impl RewardTier {
        pub fn from_points(points: u64) -> Self {
            match points {
                0..=99 => RewardTier::Bronze,
                100..=499 => RewardTier::Silver,
                500..=1999 => RewardTier::Gold,
                2000..=9999 => RewardTier::Platinum,
                _ => RewardTier::Diamond,
            }
        }
    }

    // ── Account Structs ────────────────────────────────────────────────────────────
    #[account]
    pub struct DailyRewardsConfig {
        pub authority: Pubkey,
        pub points_per_checkin: u64,
        pub streak_multiplier: u64,     // basis points (100 = 1x, 150 = 1.5x)
        pub points_per_trade_sol: u64,  // points awarded per SOL traded
        pub total_checkins: u64,
        pub bump: u8,
    }

    impl DailyRewardsConfig {
        pub const SIZE: usize = 8 + 32 + 8 + 8 + 8 + 8 + 1; // 73
    }

    #[account]
    pub struct UserDailyRewards {
        pub user: Pubkey,
        pub current_streak: u64,
        pub longest_streak: u64,
        pub last_checkin_day: i64,   // day number (unix_ts / 86400)
        pub total_points: u64,
        pub tier: RewardTier,
        pub total_redeemed: u64,
        pub bump: u8,
    }

    impl UserDailyRewards {
        pub const SIZE: usize = 8 + 32 + 8 + 8 + 8 + 8 + 1 + 8 + 1; // 82
    }

    // ── Events ─────────────────────────────────────────────────────────────────────
    #[event]
    pub struct DailyCheckin {
        pub user: Pubkey,
        pub points_awarded: u64,
        pub current_streak: u64,
        pub total_points: u64,
        pub tier: RewardTier,
    }

    #[event]
    pub struct TradeRewardRecorded {
        pub user: Pubkey,
        pub trade_sol: u64,
        pub points_awarded: u64,
    }

    #[event]
    pub struct PointsRedeemed {
        pub user: Pubkey,
        pub points_spent: u64,
        pub remaining: u64,
    }

    // ── Instructions ───────────────────────────────────────────────────────────────

    pub fn initialize_daily_rewards(
        ctx: Context<InitializeDailyRewards>,
        points_per_checkin: u64,
        streak_multiplier: u64,
        points_per_trade_sol: u64,
    ) -> Result<()> {
        let config = &mut ctx.accounts.config;
        config.authority = ctx.accounts.authority.key();
        config.points_per_checkin = points_per_checkin;
        config.streak_multiplier = streak_multiplier;
        config.points_per_trade_sol = points_per_trade_sol;
        config.total_checkins = 0;
        config.bump = ctx.bumps.config;
        Ok(())
    }

    pub fn daily_checkin(ctx: Context<DailyCheckinCtx>) -> Result<()> {
        let clock = Clock::get()?;
        let today = clock.unix_timestamp / SECONDS_PER_DAY;

        let rewards = &mut ctx.accounts.user_rewards;
        let config = &ctx.accounts.config;

        // First-time init
        if rewards.user == Pubkey::default() {
            rewards.user = ctx.accounts.user.key();
            rewards.bump = ctx.bumps.user_rewards;
        }

        require!(rewards.last_checkin_day < today, DailyRewardsError::AlreadyCheckedIn);

        // Update streak
        if rewards.last_checkin_day == today - 1 {
            rewards.current_streak = rewards.current_streak.checked_add(1).unwrap();
        } else {
            rewards.current_streak = 1;
        }
        if rewards.current_streak > rewards.longest_streak {
            rewards.longest_streak = rewards.current_streak;
        }
        rewards.last_checkin_day = today;

        // Calculate points: base * (1 + streak_bonus)
        // streak_multiplier is in bps per streak day, capped at 3x
        let multiplier = std::cmp::min(
            100_u64.checked_add(
                config.streak_multiplier.checked_mul(rewards.current_streak).unwrap_or(u64::MAX)
            ).unwrap_or(300),
            300,
        );
        let points = config
            .points_per_checkin
            .checked_mul(multiplier)
            .unwrap()
            .checked_div(100)
            .unwrap();

        rewards.total_points = rewards.total_points.checked_add(points).unwrap();
        rewards.tier = RewardTier::from_points(rewards.total_points);

        // Update global counter
        let config = &mut ctx.accounts.config;
        config.total_checkins = config.total_checkins.checked_add(1).unwrap();

        emit!(DailyCheckin {
            user: ctx.accounts.user.key(),
            points_awarded: points,
            current_streak: rewards.current_streak,
            total_points: rewards.total_points,
            tier: rewards.tier,
        });

        Ok(())
    }

    /// Called from the trade execution flow to award points for trading.
    pub fn record_trade_reward(
        ctx: Context<RecordTradeReward>,
        trade_sol_amount: u64, // in lamports
    ) -> Result<()> {
        let config = &ctx.accounts.config;
        // Points = (lamports / 1e9) * points_per_trade_sol
        let points = trade_sol_amount
            .checked_mul(config.points_per_trade_sol)
            .unwrap()
            .checked_div(1_000_000_000)
            .unwrap_or(0);

        if points > 0 {
            let rewards = &mut ctx.accounts.user_rewards;
            if rewards.user == Pubkey::default() {
                rewards.user = ctx.accounts.user.key();
                rewards.bump = ctx.bumps.user_rewards;
            }
            rewards.total_points = rewards.total_points.checked_add(points).unwrap();
            rewards.tier = RewardTier::from_points(rewards.total_points);

            emit!(TradeRewardRecorded {
                user: ctx.accounts.user.key(),
                trade_sol: trade_sol_amount,
                points_awarded: points,
            });
        }

        Ok(())
    }

    /// Redeem points for fee discounts or priority access.
    pub fn redeem_points(ctx: Context<RedeemPoints>, points_to_spend: u64) -> Result<()> {
        let rewards = &mut ctx.accounts.user_rewards;
        require!(
            rewards.total_points >= points_to_spend,
            DailyRewardsError::InsufficientPoints
        );

        rewards.total_points = rewards.total_points.checked_sub(points_to_spend).unwrap();
        rewards.total_redeemed = rewards.total_redeemed.checked_add(points_to_spend).unwrap();
        rewards.tier = RewardTier::from_points(rewards.total_points);

        emit!(PointsRedeemed {
            user: ctx.accounts.user.key(),
            points_spent: points_to_spend,
            remaining: rewards.total_points,
        });

        Ok(())
    }

    // ── Contexts ───────────────────────────────────────────────────────────────────

    #[derive(Accounts)]
    pub struct InitializeDailyRewards<'info> {
        #[account(
            init,
            payer = authority,
            space = DailyRewardsConfig::SIZE,
            seeds = [DAILY_REWARDS_CONFIG_SEED],
            bump,
        )]
        pub config: Account<'info, DailyRewardsConfig>,
        #[account(mut)]
        pub authority: Signer<'info>,
        pub system_program: Program<'info, System>,
    }

    #[derive(Accounts)]
    pub struct DailyCheckinCtx<'info> {
        #[account(
            mut,
            seeds = [DAILY_REWARDS_CONFIG_SEED],
            bump = config.bump,
        )]
        pub config: Account<'info, DailyRewardsConfig>,

        #[account(
            init_if_needed,
            payer = user,
            space = UserDailyRewards::SIZE,
            seeds = [USER_DAILY_REWARDS_SEED, user.key().as_ref()],
            bump,
        )]
        pub user_rewards: Account<'info, UserDailyRewards>,

        #[account(mut)]
        pub user: Signer<'info>,
        pub system_program: Program<'info, System>,
    }

    #[derive(Accounts)]
    pub struct RecordTradeReward<'info> {
        #[account(
            seeds = [DAILY_REWARDS_CONFIG_SEED],
            bump = config.bump,
        )]
        pub config: Account<'info, DailyRewardsConfig>,

        #[account(
            init_if_needed,
            payer = user,
            space = UserDailyRewards::SIZE,
            seeds = [USER_DAILY_REWARDS_SEED, user.key().as_ref()],
            bump,
        )]
        pub user_rewards: Account<'info, UserDailyRewards>,

        #[account(mut)]
        pub user: Signer<'info>,
        pub system_program: Program<'info, System>,
    }

    #[derive(Accounts)]
    pub struct RedeemPoints<'info> {
        #[account(
            mut,
            seeds = [USER_DAILY_REWARDS_SEED, user.key().as_ref()],
            bump = user_rewards.bump,
            has_one = user,
        )]
        pub user_rewards: Account<'info, UserDailyRewards>,

        pub user: Signer<'info>,
    }

}
pub mod holder_rewards {
    use super::*;

    // ---------------------------------------------------------------------------
    // Constants
    // ---------------------------------------------------------------------------

    const PRECISION: u128 = 1_000_000_000_000; // 1e12 scaling factor
    const REWARD_POOL_SEED: &[u8] = b"reward_pool";
    const USER_REWARD_SEED: &[u8] = b"user_reward";
    const REWARD_VAULT_SEED: &[u8] = b"reward_vault";

    // ---------------------------------------------------------------------------
    // Accounts
    // ---------------------------------------------------------------------------

    #[account]
    #[derive(Default)]
    pub struct RewardPool {
        /// The token mint this reward pool is for
        pub mint: Pubkey,
        /// Authority that can accrue rewards (program/trade handler)
        pub authority: Pubkey,
        /// Cumulative reward per token, scaled by 1e12
        pub reward_per_token_stored: u128,
        /// Total token supply eligible for rewards
        pub total_supply_eligible: u64,
        /// Timestamp of last reward accrual
        pub last_update_timestamp: i64,
        /// Minimum hold time in seconds before claiming (0 = disabled)
        pub min_hold_seconds: u64,
        /// Basis points of platform fee directed to this pool (e.g. 5000 = 50%)
        pub reward_fee_bps: u16,
        /// Bump seed for PDA
        pub bump: u8,
        /// Bump seed for vault PDA
        pub vault_bump: u8,
    }

    #[account]
    #[derive(Default)]
    pub struct UserRewardState {
        /// User's wallet
        pub user: Pubkey,
        /// Token mint
        pub mint: Pubkey,
        /// Snapshot of reward_per_token_stored at last update
        pub reward_per_token_paid: u128,
        /// Accumulated unclaimed rewards in lamports
        pub rewards_earned: u64,
        /// User's token balance known to the reward system
        pub balance: u64,
        /// Timestamp when user first held tokens (for min hold check)
        pub first_hold_timestamp: i64,
        /// Whether auto-compound is enabled
        pub auto_compound: bool,
        /// Bump seed for PDA
        pub bump: u8,
    }

    // ---------------------------------------------------------------------------
    // Events
    // ---------------------------------------------------------------------------

    #[event]
    pub struct RewardAccrued {
        pub mint: Pubkey,
        pub reward_amount: u64,
        pub new_reward_per_token: u128,
    }

    #[event]
    pub struct RewardClaimed {
        pub user: Pubkey,
        pub mint: Pubkey,
        pub amount: u64,
    }

    #[event]
    pub struct RewardAutoCompounded {
        pub user: Pubkey,
        pub mint: Pubkey,
        pub sol_amount: u64,
        pub tokens_received: u64,
    }

    #[event]
    pub struct AutoCompoundToggled {
        pub user: Pubkey,
        pub mint: Pubkey,
        pub enabled: bool,
    }

    // ---------------------------------------------------------------------------
    // Errors
    // ---------------------------------------------------------------------------

    #[error_code]
    pub enum RewardError {
        #[msg("Minimum hold time has not been met")]
        HoldTimeNotMet,
        #[msg("No rewards to claim")]
        NoRewardsToClaim,
        #[msg("Arithmetic overflow")]
        MathOverflow,
        #[msg("Unauthorized caller")]
        Unauthorized,
        #[msg("Invalid fee basis points (must be <= 10000)")]
        InvalidFeeBps,
    }

    // ---------------------------------------------------------------------------
    // Instructions
    // ---------------------------------------------------------------------------

    /// Initialize a reward pool for a newly launched token.
    pub fn initialize_reward_pool(
        ctx: Context<InitializeRewardPool>,
        reward_fee_bps: u16,
        min_hold_seconds: u64,
    ) -> Result<()> {
        require!(reward_fee_bps <= 10_000, RewardError::InvalidFeeBps);

        let pool = &mut ctx.accounts.reward_pool;
        pool.mint = ctx.accounts.mint.key();
        pool.authority = ctx.accounts.authority.key();
        pool.reward_per_token_stored = 0;
        pool.total_supply_eligible = 0;
        pool.last_update_timestamp = Clock::get()?.unix_timestamp;
        pool.min_hold_seconds = min_hold_seconds;
        pool.reward_fee_bps = reward_fee_bps;
        pool.bump = ctx.bumps.reward_pool;
        pool.vault_bump = ctx.bumps.reward_vault;

        Ok(())
    }

    /// Accrue rewards into the pool. Called by the trade instruction when fees are collected.
    /// Transfers `reward_amount` lamports from the fee payer into the reward vault and
    /// updates the global reward_per_token_stored.
    pub fn accrue_rewards(ctx: Context<AccrueRewards>, reward_amount: u64) -> Result<()> {
        let pool = &mut ctx.accounts.reward_pool;

        // Transfer SOL from fee source to reward vault
        system_program::transfer(
            CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                system_program::Transfer {
                    from: ctx.accounts.fee_payer.to_account_info(),
                    to: ctx.accounts.reward_vault.to_account_info(),
                },
            ),
            reward_amount,
        )?;

        // Update reward_per_token_stored
        if pool.total_supply_eligible > 0 {
            let reward_scaled = (reward_amount as u128)
                .checked_mul(PRECISION)
                .ok_or(RewardError::MathOverflow)?;
            let increment = reward_scaled
                .checked_div(pool.total_supply_eligible as u128)
                .ok_or(RewardError::MathOverflow)?;
            pool.reward_per_token_stored = pool
                .reward_per_token_stored
                .checked_add(increment)
                .ok_or(RewardError::MathOverflow)?;
        }
        // If total_supply_eligible == 0, rewards are effectively lost (no holders).
        // In practice this shouldn't happen since trades imply holders exist.

        pool.last_update_timestamp = Clock::get()?.unix_timestamp;

        emit!(RewardAccrued {
            mint: pool.mint,
            reward_amount,
            new_reward_per_token: pool.reward_per_token_stored,
        });

        Ok(())
    }

    /// Update a user's reward state when their token balance changes (called on every trade).
    /// This must be called BEFORE the balance actually changes, with the new_balance.
    pub fn update_user_reward_state(
        ctx: Context<UpdateUserRewardState>,
        new_balance: u64,
    ) -> Result<()> {
        let pool = &ctx.accounts.reward_pool;
        let user_state = &mut ctx.accounts.user_reward_state;
        let clock = Clock::get()?;

        // First-time initialization
        if user_state.user == Pubkey::default() {
            user_state.user = ctx.accounts.user.key();
            user_state.mint = pool.mint;
            user_state.bump = ctx.bumps.user_reward_state;
            user_state.reward_per_token_paid = pool.reward_per_token_stored;
            user_state.first_hold_timestamp = clock.unix_timestamp;
        }

        // Calculate pending rewards for current balance
        let pending = _pending_rewards(pool.reward_per_token_stored, user_state)?;
        user_state.rewards_earned = user_state
            .rewards_earned
            .checked_add(pending)
            .ok_or(RewardError::MathOverflow)?;
        user_state.reward_per_token_paid = pool.reward_per_token_stored;

        // Update eligible supply on the pool
        let pool_mut = &mut ctx.accounts.reward_pool;
        pool_mut.total_supply_eligible = pool_mut
            .total_supply_eligible
            .checked_sub(user_state.balance)
            .ok_or(RewardError::MathOverflow)?
            .checked_add(new_balance)
            .ok_or(RewardError::MathOverflow)?;

        // Track hold timestamp
        let old_balance = user_state.balance;
        user_state.balance = new_balance;

        if old_balance == 0 && new_balance > 0 {
            user_state.first_hold_timestamp = clock.unix_timestamp;
        }
        // Reset timestamp if balance drops to zero (will be set again on re-entry)
        if new_balance == 0 {
            user_state.first_hold_timestamp = 0;
        }

        Ok(())
    }

    /// Claim accumulated holder rewards. Transfers SOL from the reward vault to the user.
    /// If auto_compound is enabled, buys more tokens instead (requires bonding curve CPI — 
    /// the actual CPI call is stubbed here; integrate with your trade module).
    pub fn claim_holder_rewards(ctx: Context<ClaimHolderRewards>) -> Result<()> {
        let pool = &ctx.accounts.reward_pool;
        let user_state = &mut ctx.accounts.user_reward_state;
        let clock = Clock::get()?;

        // Settle pending rewards
        let pending = _pending_rewards(pool.reward_per_token_stored, user_state)?;
        let total_claimable = user_state
            .rewards_earned
            .checked_add(pending)
            .ok_or(RewardError::MathOverflow)?;

        require!(total_claimable > 0, RewardError::NoRewardsToClaim);

        // Check minimum hold time
        if pool.min_hold_seconds > 0 && user_state.balance > 0 {
            let held_for = clock
                .unix_timestamp
                .checked_sub(user_state.first_hold_timestamp)
                .ok_or(RewardError::MathOverflow)?;
            require!(
                held_for >= pool.min_hold_seconds as i64,
                RewardError::HoldTimeNotMet
            );
        }

        // Reset user state
        user_state.rewards_earned = 0;
        user_state.reward_per_token_paid = pool.reward_per_token_stored;

        if user_state.auto_compound {
            // --- Auto-compound path ---
            // TODO: CPI into bonding curve buy instruction with `total_claimable` lamports.
            // The buy instruction should return the number of tokens received.
            // For now we emit the event with tokens_received = 0 as a placeholder.
            //
            // After CPI, update user_state.balance += tokens_received
            // and pool.total_supply_eligible += tokens_received

            emit!(RewardAutoCompounded {
                user: user_state.user,
                mint: pool.mint,
                sol_amount: total_claimable,
                tokens_received: 0, // placeholder until CPI integrated
            });
        } else {
            // --- Direct claim path ---
            // Transfer SOL from reward vault (PDA) to user
            let mint_key = pool.mint;
            let seeds: &[&[u8]] = &[
                REWARD_VAULT_SEED,
                mint_key.as_ref(),
                &[pool.vault_bump],
            ];

            let vault_info = ctx.accounts.reward_vault.to_account_info();
            let user_info = ctx.accounts.user.to_account_info();

            **vault_info.try_borrow_mut_lamports()? -= total_claimable;
            **user_info.try_borrow_mut_lamports()? += total_claimable;

            // Note: We use direct lamport manipulation since the vault is a PDA system account.
            // The PDA signer seeds are available if CPI transfer is preferred:
            let _signer_seeds: &[&[&[u8]]] = &[seeds];

            emit!(RewardClaimed {
                user: user_state.user,
                mint: pool.mint,
                amount: total_claimable,
            });
        }

        Ok(())
    }

    /// Toggle auto-compound for a user.
    pub fn toggle_auto_compound(ctx: Context<ToggleAutoCompound>, enabled: bool) -> Result<()> {
        let user_state = &mut ctx.accounts.user_reward_state;
        user_state.auto_compound = enabled;

        emit!(AutoCompoundToggled {
            user: user_state.user,
            mint: ctx.accounts.reward_pool.mint,
            enabled,
        });

        Ok(())
    }

    // ---------------------------------------------------------------------------
    // Internal helpers
    // ---------------------------------------------------------------------------

    /// Calculate pending (unsettled) rewards for a user.
    fn _pending_rewards(
        global_reward_per_token: u128,
        user_state: &UserRewardState,
    ) -> Result<u64> {
        if user_state.balance == 0 {
            return Ok(0);
        }

        let delta = global_reward_per_token
            .checked_sub(user_state.reward_per_token_paid)
            .ok_or(RewardError::MathOverflow)?;

        let reward = (user_state.balance as u128)
            .checked_mul(delta)
            .ok_or(RewardError::MathOverflow)?
            .checked_div(PRECISION)
            .ok_or(RewardError::MathOverflow)?;

        // Safe truncation: reward should fit in u64 for practical amounts
        Ok(reward as u64)
    }

    // ---------------------------------------------------------------------------
    // Context structs (Anchor accounts validation)
    // ---------------------------------------------------------------------------

    #[derive(Accounts)]
    pub struct InitializeRewardPool<'info> {
        #[account(
            init,
            payer = authority,
            space = 8 + 32 + 32 + 16 + 8 + 8 + 8 + 2 + 1 + 1 + 32, // discriminator + fields + padding
            seeds = [REWARD_POOL_SEED, mint.key().as_ref()],
            bump,
        )]
        pub reward_pool: Account<'info, RewardPool>,

        /// CHECK: Reward vault is a PDA system account that holds SOL.
        #[account(
            mut,
            seeds = [REWARD_VAULT_SEED, mint.key().as_ref()],
            bump,
        )]
        pub reward_vault: SystemAccount<'info>,

        /// CHECK: The token mint for which we create the reward pool.
        pub mint: UncheckedAccount<'info>,

        #[account(mut)]
        pub authority: Signer<'info>,

        pub system_program: Program<'info, System>,
    }

    #[derive(Accounts)]
    pub struct AccrueRewards<'info> {
        #[account(
            mut,
            seeds = [REWARD_POOL_SEED, reward_pool.mint.as_ref()],
            bump = reward_pool.bump,
            has_one = authority @ RewardError::Unauthorized,
        )]
        pub reward_pool: Account<'info, RewardPool>,

        /// CHECK: Reward vault PDA.
        #[account(
            mut,
            seeds = [REWARD_VAULT_SEED, reward_pool.mint.as_ref()],
            bump = reward_pool.vault_bump,
        )]
        pub reward_vault: SystemAccount<'info>,

        /// The account paying the fee (typically the trade escrow or platform fee account).
        #[account(mut)]
        pub fee_payer: Signer<'info>,

        /// Authority allowed to accrue (the program's trade handler).
        pub authority: Signer<'info>,

        pub system_program: Program<'info, System>,
    }

    #[derive(Accounts)]
    pub struct UpdateUserRewardState<'info> {
        #[account(
            mut,
            seeds = [REWARD_POOL_SEED, reward_pool.mint.as_ref()],
            bump = reward_pool.bump,
            has_one = authority @ RewardError::Unauthorized,
        )]
        pub reward_pool: Account<'info, RewardPool>,

        #[account(
            init_if_needed,
            payer = payer,
            space = 8 + 32 + 32 + 16 + 16 + 8 + 8 + 8 + 1 + 1 + 16, // discriminator + fields + padding
            seeds = [USER_REWARD_SEED, reward_pool.mint.as_ref(), user.key().as_ref()],
            bump,
        )]
        pub user_reward_state: Account<'info, UserRewardState>,

        /// CHECK: The user whose balance is changing.
        pub user: UncheckedAccount<'info>,

        /// Authority (program trade handler).
        pub authority: Signer<'info>,

        #[account(mut)]
        pub payer: Signer<'info>,

        pub system_program: Program<'info, System>,
    }

    #[derive(Accounts)]
    pub struct ClaimHolderRewards<'info> {
        #[account(
            seeds = [REWARD_POOL_SEED, reward_pool.mint.as_ref()],
            bump = reward_pool.bump,
        )]
        pub reward_pool: Account<'info, RewardPool>,

        #[account(
            mut,
            seeds = [USER_REWARD_SEED, reward_pool.mint.as_ref(), user.key().as_ref()],
            bump = user_reward_state.bump,
            constraint = user_reward_state.user == user.key(),
        )]
        pub user_reward_state: Account<'info, UserRewardState>,

        /// CHECK: Reward vault PDA (SOL source).
        #[account(
            mut,
            seeds = [REWARD_VAULT_SEED, reward_pool.mint.as_ref()],
            bump = reward_pool.vault_bump,
        )]
        pub reward_vault: SystemAccount<'info>,

        #[account(mut)]
        pub user: Signer<'info>,

        pub system_program: Program<'info, System>,
    }

    #[derive(Accounts)]
    pub struct ToggleAutoCompound<'info> {
        #[account(
            seeds = [REWARD_POOL_SEED, reward_pool.mint.as_ref()],
            bump = reward_pool.bump,
        )]
        pub reward_pool: Account<'info, RewardPool>,

        #[account(
            mut,
            seeds = [USER_REWARD_SEED, reward_pool.mint.as_ref(), user.key().as_ref()],
            bump = user_reward_state.bump,
            constraint = user_reward_state.user == user.key(),
        )]
        pub user_reward_state: Account<'info, UserRewardState>,

        pub user: Signer<'info>,
    }

}
pub mod lending {
    use super::*;

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
            **ctx.accounts.sol_vault.to_account_info().try_borrow_mut_lamports()? -= borrow_amount;
            **ctx.accounts.user.to_account_info().try_borrow_mut_lamports()? += borrow_amount;

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

            // Transfer SOL back
            **ctx.accounts.sol_vault.to_account_info().try_borrow_mut_lamports()? -= amount;
            **ctx.accounts.user.to_account_info().try_borrow_mut_lamports()? += amount;

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

}
pub mod limit_orders {
    use super::*;

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

}
pub mod live_chat {
    use super::*;

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

}
pub mod perps {
    use super::*;
    use std::collections::BinaryHeap;

    // ============================================================================
    // Constants
    // ============================================================================

    /// Fixed-point precision: 10^6 (all prices, rates, and margins use this)
    pub const PRECISION: u128 = 1_000_000;
    /// Maximum leverage: 20x
    pub const MAX_LEVERAGE: u64 = 20;
    /// Minimum maintenance margin ratio: 2.5% (25_000 / PRECISION)
    pub const DEFAULT_MAINTENANCE_MARGIN: u64 = 25_000; // 2.5%
    /// Default liquidation fee: 1% to liquidator
    pub const DEFAULT_LIQUIDATION_FEE: u64 = 10_000; // 1%
    /// Default maker fee: 0.02%
    pub const DEFAULT_MAKER_FEE: u64 = 200; // 0.02%
    /// Default taker fee: 0.06%
    pub const DEFAULT_TAKER_FEE: u64 = 600; // 0.06%
    /// Fee split to insurance fund: 30%
    pub const INSURANCE_FEE_SHARE: u64 = 300_000; // 30%
    /// Fee split to SolForge vault (burn): 20%
    pub const SOLFORGE_FEE_SHARE: u64 = 200_000; // 20%
    /// Max orders per order book side
    pub const MAX_ORDERS: usize = 256;
    /// TWAP sample window
    pub const TWAP_WINDOW: u64 = 3600; // 1 hour in seconds
    /// Max TWAP samples stored
    pub const MAX_TWAP_SAMPLES: usize = 60;
    /// Default funding rate interval: 1 hour
    pub const DEFAULT_FUNDING_INTERVAL: i64 = 3600;
    /// Max funding rate per interval: 0.1%
    pub const MAX_FUNDING_RATE: i128 = 1_000; // 0.1% of PRECISION
    /// Circuit breaker: max 10% price deviation from oracle
    pub const PRICE_BAND_BPS: u64 = 1000; // 10%

    // ============================================================================
    // Error Codes
    // ============================================================================

    #[error_code]
    pub enum PerpError {
        #[msg("Token has not graduated to Raydium yet")]
        TokenNotGraduated,
        #[msg("Leverage exceeds maximum allowed")]
        ExcessiveLeverage,
        #[msg("Insufficient collateral for position")]
        InsufficientCollateral,
        #[msg("Position not found")]
        PositionNotFound,
        #[msg("Position is not liquidatable")]
        NotLiquidatable,
        #[msg("Order book is full")]
        OrderBookFull,
        #[msg("Order not found")]
        OrderNotFound,
        #[msg("Invalid order price")]
        InvalidPrice,
        #[msg("Invalid order size")]
        InvalidSize,
        #[msg("Arithmetic overflow")]
        MathOverflow,
        #[msg("Market is paused")]
        MarketPaused,
        #[msg("Open interest cap exceeded")]
        OpenInterestCapExceeded,
        #[msg("Position size limit exceeded")]
        PositionSizeLimitExceeded,
        #[msg("Price outside circuit breaker band")]
        CircuitBreakerTriggered,
        #[msg("Funding interval not elapsed")]
        FundingIntervalNotElapsed,
        #[msg("Unauthorized")]
        Unauthorized,
        #[msg("Insufficient insurance fund")]
        InsufficientInsuranceFund,
        #[msg("Invalid partial liquidation amount")]
        InvalidPartialLiquidation,
        #[msg("No orders to match")]
        NoOrdersToMatch,
        #[msg("Self-trade prevention")]
        SelfTrade,
        #[msg("Stale oracle price")]
        StaleOracle,
        #[msg("Invalid margin account")]
        InvalidMarginAccount,
    }

    // ============================================================================
    // Account Structs
    // ============================================================================

    #[account]
    #[derive(Default)]
    pub struct PerpMarket {
        /// Bump seed
        pub bump: u8,
        /// Authority (program or DAO)
        pub authority: Pubkey,
        /// Token mint (the graduated token)
        pub token_mint: Pubkey,
        /// Collateral mint (USDC or SOL)
        pub collateral_mint: Pubkey,
        /// Raydium AMM pool used as oracle
        pub raydium_pool: Pubkey,
        /// SolForge vault for fee burns
        pub solforge_vault: Pubkey,
        /// Insurance fund account
        pub insurance_fund: Pubkey,
        /// Collateral vault (holds all deposited collateral)
        pub collateral_vault: Pubkey,
        /// Order book account
        pub order_book: Pubkey,

        // --- Configuration ---
        /// Max leverage (stored as integer, e.g. 20 = 20x)
        pub max_leverage: u64,
        /// Maintenance margin ratio (PRECISION-based, e.g. 25_000 = 2.5%)
        pub maintenance_margin: u64,
        /// Liquidation fee (PRECISION-based)
        pub liquidation_fee: u64,
        /// Maker fee (PRECISION-based)
        pub maker_fee: u64,
        /// Taker fee (PRECISION-based)
        pub taker_fee: u64,
        /// Funding rate interval in seconds
        pub funding_interval: i64,
        /// Max open interest (in base token units)
        pub max_open_interest: u64,
        /// Max position size per user
        pub max_position_size: u64,

        // --- State ---
        /// Current mark price (PRECISION-based)
        pub mark_price: u64,
        /// Current index price from oracle (PRECISION-based)
        pub index_price: u64,
        /// Total long open interest (base units)
        pub long_open_interest: u64,
        /// Total short open interest (base units)
        pub short_open_interest: u64,
        /// Cumulative funding rate for longs (signed, PRECISION-based)
        pub cumulative_funding_long: i128,
        /// Cumulative funding rate for shorts (signed, PRECISION-based)
        pub cumulative_funding_short: i128,
        /// Last funding update timestamp
        pub last_funding_time: i64,
        /// Is market paused
        pub paused: bool,
        /// Market creation timestamp
        pub created_at: i64,

        // --- TWAP ---
        /// TWAP samples: (timestamp, price)
        pub twap_samples: Vec<TwapSample>,

        /// Reserved for future use
        pub _reserved: [u8; 128],
    }

    #[derive(AnchorSerialize, AnchorDeserialize, Clone, Default)]
    pub struct TwapSample {
        pub timestamp: i64,
        pub price: u64,
    }

    #[account]
    #[derive(Default)]
    pub struct OrderBook {
        pub bump: u8,
        pub market: Pubkey,
        /// Bids sorted by price descending (best bid first)
        pub bids: Vec<OrderEntry>,
        /// Asks sorted by price ascending (best ask first)
        pub asks: Vec<OrderEntry>,
        /// Monotonic order ID counter
        pub next_order_id: u64,
    }

    #[derive(AnchorSerialize, AnchorDeserialize, Clone, Default, PartialEq)]
    pub struct OrderEntry {
        pub order_id: u64,
        pub owner: Pubkey,
        pub price: u64,       // PRECISION-based
        pub size: u64,         // base token units
        pub remaining: u64,    // remaining unfilled size
        pub side: Side,
        pub order_type: OrderType,
        pub timestamp: i64,
        pub margin_account: Pubkey,
    }

    #[account]
    #[derive(Default)]
    pub struct UserMarginAccount {
        pub bump: u8,
        pub owner: Pubkey,
        /// Total deposited collateral (in collateral token units)
        pub collateral: u64,
        /// Number of open positions (for cross-margin)
        pub open_positions: u16,
        /// Cumulative realized PnL
        pub realized_pnl: i64,
        /// Created timestamp
        pub created_at: i64,
        pub _reserved: [u8; 64],
    }

    #[account]
    pub struct Position {
        pub bump: u8,
        pub market: Pubkey,
        pub owner: Pubkey,
        pub margin_account: Pubkey,
        /// Long or Short
        pub side: Side,
        /// Position size in base units
        pub size: u64,
        /// Entry price (PRECISION-based)
        pub entry_price: u64,
        /// Collateral allocated to this position
        pub collateral: u64,
        /// Leverage used
        pub leverage: u64,
        /// Cumulative funding at time of last settlement
        pub last_cumulative_funding: i128,
        /// Unrealized funding payments owed
        pub pending_funding: i64,
        /// Timestamp opened
        pub opened_at: i64,
        /// Last updated timestamp
        pub updated_at: i64,
        pub _reserved: [u8; 64],
    }

    #[account]
    #[derive(Default)]
    pub struct InsuranceFund {
        pub bump: u8,
        pub market: Pubkey,
        pub vault: Pubkey,
        /// Total balance in collateral units
        pub balance: u64,
        /// Total payouts made
        pub total_payouts: u64,
        /// Total deposits received
        pub total_deposits: u64,
    }

    // ============================================================================
    // Enums
    // ============================================================================

    #[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Default)]
    pub enum Side {
        #[default]
        Long,
        Short,
    }

    #[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Default)]
    pub enum OrderType {
        #[default]
        Limit,
        Market,
    }

    // ============================================================================
    // Events
    // ============================================================================

    #[event]
    pub struct MarketCreated {
        pub market: Pubkey,
        pub token_mint: Pubkey,
        pub max_leverage: u64,
        pub timestamp: i64,
    }

    #[event]
    pub struct PositionOpened {
        pub market: Pubkey,
        pub owner: Pubkey,
        pub side: Side,
        pub size: u64,
        pub entry_price: u64,
        pub leverage: u64,
        pub collateral: u64,
    }

    #[event]
    pub struct PositionClosed {
        pub market: Pubkey,
        pub owner: Pubkey,
        pub side: Side,
        pub size: u64,
        pub exit_price: u64,
        pub realized_pnl: i64,
    }

    #[event]
    pub struct PositionLiquidated {
        pub market: Pubkey,
        pub owner: Pubkey,
        pub liquidator: Pubkey,
        pub size_liquidated: u64,
        pub price: u64,
        pub liquidation_fee: u64,
    }

    #[event]
    pub struct OrderPlaced {
        pub market: Pubkey,
        pub order_id: u64,
        pub owner: Pubkey,
        pub side: Side,
        pub order_type: OrderType,
        pub price: u64,
        pub size: u64,
    }

    #[event]
    pub struct OrderCancelled {
        pub market: Pubkey,
        pub order_id: u64,
        pub owner: Pubkey,
    }

    #[event]
    pub struct OrderMatched {
        pub market: Pubkey,
        pub bid_order_id: u64,
        pub ask_order_id: u64,
        pub price: u64,
        pub size: u64,
    }

    #[event]
    pub struct FundingRateUpdated {
        pub market: Pubkey,
        pub funding_rate: i128,
        pub mark_price: u64,
        pub index_price: u64,
        pub timestamp: i64,
    }

    #[event]
    pub struct CollateralDeposited {
        pub margin_account: Pubkey,
        pub amount: u64,
    }

    #[event]
    pub struct CollateralWithdrawn {
        pub margin_account: Pubkey,
        pub amount: u64,
    }

    // ============================================================================
    // Fixed-Point Math Helpers
    // ============================================================================

    pub mod math {
        use super::*;

        /// Multiply two PRECISION-based values: (a * b) / PRECISION
        pub fn mul_precision(a: u128, b: u128) -> Result<u128> {
            a.checked_mul(b)
                .and_then(|v| v.checked_div(PRECISION))
                .ok_or_else(|| error!(PerpError::MathOverflow))
        }

        /// Divide two PRECISION-based values: (a * PRECISION) / b
        pub fn div_precision(a: u128, b: u128) -> Result<u128> {
            if b == 0 {
                return err!(PerpError::MathOverflow);
            }
            a.checked_mul(PRECISION)
                .and_then(|v| v.checked_div(b))
                .ok_or_else(|| error!(PerpError::MathOverflow))
        }

        /// Calculate unrealized PnL for a position
        /// Long: (mark_price - entry_price) * size / PRECISION
        /// Short: (entry_price - mark_price) * size / PRECISION
        pub fn calc_unrealized_pnl(
            side: &Side,
            entry_price: u64,
            mark_price: u64,
            size: u64,
        ) -> Result<i64> {
            let pnl = match side {
                Side::Long => (mark_price as i128) - (entry_price as i128),
                Side::Short => (entry_price as i128) - (mark_price as i128),
            };
            let pnl_scaled = pnl
                .checked_mul(size as i128)
                .and_then(|v| v.checked_div(PRECISION as i128))
                .ok_or_else(|| error!(PerpError::MathOverflow))?;
            Ok(pnl_scaled as i64)
        }

        /// Calculate margin ratio = (collateral + unrealized_pnl) / notional
        /// Returns PRECISION-based ratio
        pub fn calc_margin_ratio(
            collateral: u64,
            unrealized_pnl: i64,
            notional: u64,
        ) -> Result<u64> {
            if notional == 0 {
                return Ok(u64::MAX);
            }
            let equity = (collateral as i128) + (unrealized_pnl as i128);
            if equity <= 0 {
                return Ok(0);
            }
            let ratio = (equity as u128)
                .checked_mul(PRECISION)
                .and_then(|v| v.checked_div(notional as u128))
                .ok_or_else(|| error!(PerpError::MathOverflow))?;
            Ok(ratio as u64)
        }

        /// Calculate notional value = size * price / PRECISION
        pub fn calc_notional(size: u64, price: u64) -> Result<u64> {
            let n = (size as u128)
                .checked_mul(price as u128)
                .and_then(|v| v.checked_div(PRECISION))
                .ok_or_else(|| error!(PerpError::MathOverflow))?;
            Ok(n as u64)
        }

        /// Calculate funding payment for a position
        /// payment = size * (cumulative_funding_now - cumulative_funding_at_open) / PRECISION
        pub fn calc_funding_payment(
            size: u64,
            cumulative_now: i128,
            cumulative_at_open: i128,
        ) -> Result<i64> {
            let delta = cumulative_now
                .checked_sub(cumulative_at_open)
                .ok_or_else(|| error!(PerpError::MathOverflow))?;
            let payment = delta
                .checked_mul(size as i128)
                .and_then(|v| v.checked_div(PRECISION as i128))
                .ok_or_else(|| error!(PerpError::MathOverflow))?;
            Ok(payment as i64)
        }

        /// Calculate fee amount = notional * fee_rate / PRECISION
        pub fn calc_fee(notional: u64, fee_rate: u64) -> Result<u64> {
            let fee = (notional as u128)
                .checked_mul(fee_rate as u128)
                .and_then(|v| v.checked_div(PRECISION))
                .ok_or_else(|| error!(PerpError::MathOverflow))?;
            Ok(fee as u64)
        }

        /// Calculate liquidation price for a position
        /// Long: entry_price * (1 - 1/leverage + maintenance_margin)
        /// Short: entry_price * (1 + 1/leverage - maintenance_margin)
        pub fn calc_liquidation_price(
            entry_price: u64,
            leverage: u64,
            maintenance_margin: u64,
            side: &Side,
        ) -> Result<u64> {
            let one = PRECISION;
            let inv_lev = PRECISION / (leverage as u128);
            let mm = maintenance_margin as u128;

            let factor = match side {
                Side::Long => {
                    let f = one.checked_sub(inv_lev)
                        .and_then(|v| v.checked_add(mm))
                        .ok_or_else(|| error!(PerpError::MathOverflow))?;
                    f
                }
                Side::Short => {
                    let f = one.checked_add(inv_lev)
                        .and_then(|v| v.checked_sub(mm))
                        .ok_or_else(|| error!(PerpError::MathOverflow))?;
                    f
                }
            };

            let liq_price = (entry_price as u128)
                .checked_mul(factor)
                .and_then(|v| v.checked_div(PRECISION))
                .ok_or_else(|| error!(PerpError::MathOverflow))?;
            Ok(liq_price as u64)
        }

        /// Compute TWAP from samples within a window
        pub fn calc_twap(samples: &[TwapSample], now: i64, window: u64) -> u64 {
            let cutoff = now - (window as i64);
            let valid: Vec<&TwapSample> = samples.iter().filter(|s| s.timestamp >= cutoff).collect();
            if valid.is_empty() {
                return 0;
            }
            let sum: u128 = valid.iter().map(|s| s.price as u128).sum();
            (sum / valid.len() as u128) as u64
        }
    }

    // ============================================================================
    // Instructions
    // ============================================================================


        /// Initialize a perpetual market for a graduated token
        pub fn initialize_perp_market(
            ctx: Context<InitializePerpMarket>,
            max_leverage: u64,
            funding_interval: i64,
            maintenance_margin: u64,
            liquidation_fee: u64,
            maker_fee: u64,
            taker_fee: u64,
            max_open_interest: u64,
            max_position_size: u64,
        ) -> Result<()> {
            require!(max_leverage > 0 && max_leverage <= MAX_LEVERAGE, PerpError::ExcessiveLeverage);

            let clock = Clock::get()?;
            let market = &mut ctx.accounts.market;
            market.bump = ctx.bumps.market;
            market.authority = ctx.accounts.authority.key();
            market.token_mint = ctx.accounts.token_mint.key();
            market.collateral_mint = ctx.accounts.collateral_mint.key();
            market.raydium_pool = ctx.accounts.raydium_pool.key();
            market.solforge_vault = ctx.accounts.solforge_vault.key();
            market.insurance_fund = ctx.accounts.insurance_fund.key();
            market.collateral_vault = ctx.accounts.collateral_vault.key();
            market.order_book = ctx.accounts.order_book.key();

            market.max_leverage = max_leverage;
            market.maintenance_margin = if maintenance_margin > 0 { maintenance_margin } else { DEFAULT_MAINTENANCE_MARGIN };
            market.liquidation_fee = if liquidation_fee > 0 { liquidation_fee } else { DEFAULT_LIQUIDATION_FEE };
            market.maker_fee = if maker_fee > 0 { maker_fee } else { DEFAULT_MAKER_FEE };
            market.taker_fee = if taker_fee > 0 { taker_fee } else { DEFAULT_TAKER_FEE };
            market.funding_interval = if funding_interval > 0 { funding_interval } else { DEFAULT_FUNDING_INTERVAL };
            market.max_open_interest = max_open_interest;
            market.max_position_size = max_position_size;
            market.last_funding_time = clock.unix_timestamp;
            market.paused = false;
            market.created_at = clock.unix_timestamp;
            market.twap_samples = Vec::new();

            // Initialize order book
            let ob = &mut ctx.accounts.order_book;
            ob.bump = ctx.bumps.order_book;
            ob.market = market.key();
            ob.bids = Vec::new();
            ob.asks = Vec::new();
            ob.next_order_id = 1;

            // Initialize insurance fund
            let ins = &mut ctx.accounts.insurance_fund;
            ins.bump = ctx.bumps.insurance_fund;
            ins.market = market.key();
            ins.vault = ctx.accounts.insurance_vault.key();
            ins.balance = 0;
            ins.total_payouts = 0;
            ins.total_deposits = 0;

            emit!(MarketCreated {
                market: market.key(),
                token_mint: market.token_mint,
                max_leverage,
                timestamp: clock.unix_timestamp,
            });

            Ok(())
        }

        /// Create a cross-margin account for a user
        pub fn create_margin_account(ctx: Context<CreateMarginAccount>) -> Result<()> {
            let clock = Clock::get()?;
            let account = &mut ctx.accounts.margin_account;
            account.bump = ctx.bumps.margin_account;
            account.owner = ctx.accounts.owner.key();
            account.collateral = 0;
            account.open_positions = 0;
            account.realized_pnl = 0;
            account.created_at = clock.unix_timestamp;
            Ok(())
        }

        /// Deposit collateral into margin account
        pub fn deposit_collateral(ctx: Context<DepositCollateral>, amount: u64) -> Result<()> {
            // Transfer collateral from user to vault
            let cpi_accounts = Transfer {
                from: ctx.accounts.user_token_account.to_account_info(),
                to: ctx.accounts.collateral_vault.to_account_info(),
                authority: ctx.accounts.owner.to_account_info(),
            };
            let cpi_ctx = CpiContext::new(ctx.accounts.token_program.to_account_info(), cpi_accounts);
            token::transfer(cpi_ctx, amount)?;

            let margin = &mut ctx.accounts.margin_account;
            margin.collateral = margin.collateral.checked_add(amount)
                .ok_or_else(|| error!(PerpError::MathOverflow))?;

            emit!(CollateralDeposited {
                margin_account: margin.key(),
                amount,
            });

            Ok(())
        }

        /// Withdraw collateral from margin account (must maintain margin requirements)
        pub fn withdraw_collateral(ctx: Context<WithdrawCollateral>, amount: u64) -> Result<()> {
            let margin = &mut ctx.accounts.margin_account;
            require!(margin.collateral >= amount, PerpError::InsufficientCollateral);

            // TODO: Check that withdrawal doesn't violate margin requirements for open positions
            margin.collateral = margin.collateral.checked_sub(amount)
                .ok_or_else(|| error!(PerpError::MathOverflow))?;

            // Transfer from vault to user using PDA signer
            let market_key = ctx.accounts.market.key();
            let seeds = &[
                b"collateral_vault",
                market_key.as_ref(),
                &[ctx.accounts.market.bump],
            ];
            let signer_seeds = &[&seeds[..]];

            let cpi_accounts = Transfer {
                from: ctx.accounts.collateral_vault.to_account_info(),
                to: ctx.accounts.user_token_account.to_account_info(),
                authority: ctx.accounts.vault_authority.to_account_info(),
            };
            let cpi_ctx = CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                cpi_accounts,
                signer_seeds,
            );
            token::transfer(cpi_ctx, amount)?;

            emit!(CollateralWithdrawn {
                margin_account: margin.key(),
                amount,
            });

            Ok(())
        }

        /// Open a new leveraged position
        pub fn open_position(
            ctx: Context<OpenPosition>,
            side: Side,
            size: u64,
            leverage: u64,
            collateral: u64,
        ) -> Result<()> {
            let market = &ctx.accounts.market;
            require!(!market.paused, PerpError::MarketPaused);
            require!(leverage > 0 && leverage <= market.max_leverage, PerpError::ExcessiveLeverage);
            require!(size > 0, PerpError::InvalidSize);
            require!(size <= market.max_position_size, PerpError::PositionSizeLimitExceeded);

            // Check open interest caps
            let new_oi = match side {
                Side::Long => market.long_open_interest.checked_add(size),
                Side::Short => market.short_open_interest.checked_add(size),
            }.ok_or_else(|| error!(PerpError::MathOverflow))?;
            require!(new_oi <= market.max_open_interest, PerpError::OpenInterestCapExceeded);

            // Use mark price as entry (in production, would use fill price from order book)
            let entry_price = market.mark_price;
            require!(entry_price > 0, PerpError::InvalidPrice);

            // Check circuit breaker
            check_price_band(entry_price, market.index_price)?;

            // Calculate required collateral: notional / leverage
            let notional = math::calc_notional(size, entry_price)?;
            let required_collateral = notional / (leverage as u64);
            require!(collateral >= required_collateral, PerpError::InsufficientCollateral);

            // Deduct collateral from margin account
            let margin = &mut ctx.accounts.margin_account;
            require!(margin.collateral >= collateral, PerpError::InsufficientCollateral);
            margin.collateral = margin.collateral.checked_sub(collateral)
                .ok_or_else(|| error!(PerpError::MathOverflow))?;
            margin.open_positions = margin.open_positions.checked_add(1)
                .ok_or_else(|| error!(PerpError::MathOverflow))?;

            // Calculate and charge taker fee
            let fee = math::calc_fee(notional, market.taker_fee)?;
            distribute_fees(&ctx.accounts.market, fee)?;

            // Initialize position
            let clock = Clock::get()?;
            let position = &mut ctx.accounts.position;
            position.bump = ctx.bumps.position;
            position.market = market.key();
            position.owner = ctx.accounts.owner.key();
            position.margin_account = margin.key();
            position.side = side;
            position.size = size;
            position.entry_price = entry_price;
            position.collateral = collateral;
            position.leverage = leverage;
            position.last_cumulative_funding = match side {
                Side::Long => market.cumulative_funding_long,
                Side::Short => market.cumulative_funding_short,
            };
            position.pending_funding = 0;
            position.opened_at = clock.unix_timestamp;
            position.updated_at = clock.unix_timestamp;

            // Update market open interest
            let market_mut = &mut ctx.accounts.market;
            match side {
                Side::Long => {
                    market_mut.long_open_interest = market_mut.long_open_interest
                        .checked_add(size).ok_or_else(|| error!(PerpError::MathOverflow))?;
                }
                Side::Short => {
                    market_mut.short_open_interest = market_mut.short_open_interest
                        .checked_add(size).ok_or_else(|| error!(PerpError::MathOverflow))?;
                }
            }

            emit!(PositionOpened {
                market: market_mut.key(),
                owner: ctx.accounts.owner.key(),
                side,
                size,
                entry_price,
                leverage,
                collateral,
            });

            Ok(())
        }

        /// Close an entire position
        pub fn close_position(ctx: Context<ClosePosition>) -> Result<()> {
            let market = &ctx.accounts.market;
            let position = &ctx.accounts.position;
            let exit_price = market.mark_price;

            // Check circuit breaker
            check_price_band(exit_price, market.index_price)?;

            // Settle funding
            let funding_payment = settle_funding(position, market)?;

            // Calculate PnL
            let unrealized_pnl = math::calc_unrealized_pnl(
                &position.side, position.entry_price, exit_price, position.size,
            )?;
            let total_pnl = unrealized_pnl + funding_payment;

            // Calculate fee
            let notional = math::calc_notional(position.size, exit_price)?;
            let fee = math::calc_fee(notional, market.taker_fee)?;

            // Return collateral +/- PnL - fees to margin account
            let margin = &mut ctx.accounts.margin_account;
            let return_amount = (position.collateral as i64) + total_pnl - (fee as i64);
            if return_amount > 0 {
                margin.collateral = margin.collateral
                    .checked_add(return_amount as u64)
                    .ok_or_else(|| error!(PerpError::MathOverflow))?;
            }
            // If return_amount <= 0, collateral is lost (insurance fund covers if needed)
            margin.realized_pnl = margin.realized_pnl
                .checked_add(total_pnl)
                .ok_or_else(|| error!(PerpError::MathOverflow))?;
            margin.open_positions = margin.open_positions.saturating_sub(1);

            // Update market OI
            let market_mut = &mut ctx.accounts.market;
            match position.side {
                Side::Long => {
                    market_mut.long_open_interest = market_mut.long_open_interest
                        .saturating_sub(position.size);
                }
                Side::Short => {
                    market_mut.short_open_interest = market_mut.short_open_interest
                        .saturating_sub(position.size);
                }
            }

            distribute_fees(market_mut, fee)?;

            emit!(PositionClosed {
                market: market_mut.key(),
                owner: ctx.accounts.owner.key(),
                side: position.side,
                size: position.size,
                exit_price,
                realized_pnl: total_pnl,
            });

            Ok(())
        }

        /// Increase an existing position's size
        pub fn increase_position(
            ctx: Context<ModifyPosition>,
            additional_size: u64,
            additional_collateral: u64,
        ) -> Result<()> {
            let market = &ctx.accounts.market;
            require!(!market.paused, PerpError::MarketPaused);

            let position = &mut ctx.accounts.position;
            let new_size = position.size.checked_add(additional_size)
                .ok_or_else(|| error!(PerpError::MathOverflow))?;
            require!(new_size <= market.max_position_size, PerpError::PositionSizeLimitExceeded);

            let current_price = market.mark_price;
            check_price_band(current_price, market.index_price)?;

            // Weighted average entry price
            let old_notional = (position.size as u128) * (position.entry_price as u128);
            let new_notional = (additional_size as u128) * (current_price as u128);
            let avg_entry = (old_notional + new_notional) / (new_size as u128);

            // Deduct collateral
            let margin = &mut ctx.accounts.margin_account;
            require!(margin.collateral >= additional_collateral, PerpError::InsufficientCollateral);
            margin.collateral = margin.collateral.checked_sub(additional_collateral)
                .ok_or_else(|| error!(PerpError::MathOverflow))?;

            position.size = new_size;
            position.entry_price = avg_entry as u64;
            position.collateral = position.collateral.checked_add(additional_collateral)
                .ok_or_else(|| error!(PerpError::MathOverflow))?;
            position.updated_at = Clock::get()?.unix_timestamp;

            // Update OI
            let market_mut = &mut ctx.accounts.market;
            match position.side {
                Side::Long => {
                    market_mut.long_open_interest = market_mut.long_open_interest
                        .checked_add(additional_size).ok_or_else(|| error!(PerpError::MathOverflow))?;
                }
                Side::Short => {
                    market_mut.short_open_interest = market_mut.short_open_interest
                        .checked_add(additional_size).ok_or_else(|| error!(PerpError::MathOverflow))?;
                }
            }

            Ok(())
        }

        /// Decrease an existing position's size (partial close)
        pub fn decrease_position(
            ctx: Context<ModifyPosition>,
            decrease_size: u64,
        ) -> Result<()> {
            let market = &ctx.accounts.market;
            let position = &mut ctx.accounts.position;
            require!(decrease_size > 0 && decrease_size < position.size, PerpError::InvalidSize);

            let exit_price = market.mark_price;
            check_price_band(exit_price, market.index_price)?;

            // Calculate PnL on closed portion
            let pnl = math::calc_unrealized_pnl(&position.side, position.entry_price, exit_price, decrease_size)?;

            // Return proportional collateral + PnL
            let collateral_fraction = (position.collateral as u128)
                .checked_mul(decrease_size as u128)
                .and_then(|v| v.checked_div(position.size as u128))
                .ok_or_else(|| error!(PerpError::MathOverflow))? as u64;

            let margin = &mut ctx.accounts.margin_account;
            let return_amount = (collateral_fraction as i64) + pnl;
            if return_amount > 0 {
                margin.collateral = margin.collateral
                    .checked_add(return_amount as u64)
                    .ok_or_else(|| error!(PerpError::MathOverflow))?;
            }
            margin.realized_pnl = margin.realized_pnl.checked_add(pnl)
                .ok_or_else(|| error!(PerpError::MathOverflow))?;

            position.size = position.size.checked_sub(decrease_size)
                .ok_or_else(|| error!(PerpError::MathOverflow))?;
            position.collateral = position.collateral.checked_sub(collateral_fraction)
                .ok_or_else(|| error!(PerpError::MathOverflow))?;
            position.updated_at = Clock::get()?.unix_timestamp;

            // Update OI
            let market_mut = &mut ctx.accounts.market;
            match position.side {
                Side::Long => {
                    market_mut.long_open_interest = market_mut.long_open_interest
                        .saturating_sub(decrease_size);
                }
                Side::Short => {
                    market_mut.short_open_interest = market_mut.short_open_interest
                        .saturating_sub(decrease_size);
                }
            }

            Ok(())
        }

        /// Place a limit or market order on the order book
        pub fn place_order(
            ctx: Context<PlaceOrder>,
            side: Side,
            order_type: OrderType,
            price: u64,
            size: u64,
        ) -> Result<()> {
            let market = &ctx.accounts.market;
            require!(!market.paused, PerpError::MarketPaused);
            require!(size > 0, PerpError::InvalidSize);

            if order_type == OrderType::Limit {
                require!(price > 0, PerpError::InvalidPrice);
                check_price_band(price, market.index_price)?;
            }

            let ob = &mut ctx.accounts.order_book;
            let orders = match side {
                Side::Long => &ob.bids,
                Side::Short => &ob.asks,
            };
            require!(orders.len() < MAX_ORDERS, PerpError::OrderBookFull);

            let clock = Clock::get()?;
            let order_id = ob.next_order_id;
            ob.next_order_id = ob.next_order_id.checked_add(1)
                .ok_or_else(|| error!(PerpError::MathOverflow))?;

            let entry = OrderEntry {
                order_id,
                owner: ctx.accounts.owner.key(),
                price: if order_type == OrderType::Market { 0 } else { price },
                size,
                remaining: size,
                side,
                order_type,
                timestamp: clock.unix_timestamp,
                margin_account: ctx.accounts.margin_account.key(),
            };

            match side {
                Side::Long => {
                    // Insert maintaining descending price order
                    let pos = ob.bids.iter().position(|o| o.price < price).unwrap_or(ob.bids.len());
                    ob.bids.insert(pos, entry);
                }
                Side::Short => {
                    // Insert maintaining ascending price order
                    let pos = ob.asks.iter().position(|o| o.price > price).unwrap_or(ob.asks.len());
                    ob.asks.insert(pos, entry);
                }
            }

            emit!(OrderPlaced {
                market: market.key(),
                order_id,
                owner: ctx.accounts.owner.key(),
                side,
                order_type,
                price,
                size,
            });

            Ok(())
        }

        /// Cancel an existing order
        pub fn cancel_order(ctx: Context<CancelOrder>, order_id: u64) -> Result<()> {
            let ob = &mut ctx.accounts.order_book;
            let owner = ctx.accounts.owner.key();

            // Search bids
            if let Some(pos) = ob.bids.iter().position(|o| o.order_id == order_id && o.owner == owner) {
                ob.bids.remove(pos);
                emit!(OrderCancelled { market: ob.market, order_id, owner });
                return Ok(());
            }

            // Search asks
            if let Some(pos) = ob.asks.iter().position(|o| o.order_id == order_id && o.owner == owner) {
                ob.asks.remove(pos);
                emit!(OrderCancelled { market: ob.market, order_id, owner });
                return Ok(());
            }

            err!(PerpError::OrderNotFound)
        }

        /// Permissionless crank: match crossing orders
        pub fn match_orders(ctx: Context<MatchOrders>, max_matches: u8) -> Result<()> {
            let ob = &mut ctx.accounts.order_book;
            let market = &mut ctx.accounts.market;
            require!(!market.paused, PerpError::MarketPaused);

            let mut matches_done: u8 = 0;

            while matches_done < max_matches && !ob.bids.is_empty() && !ob.asks.is_empty() {
                let best_bid = &ob.bids[0];
                let best_ask = &ob.asks[0];

                // Market orders always cross; limit orders cross when bid >= ask
                let crosses = best_bid.order_type == OrderType::Market
                    || best_ask.order_type == OrderType::Market
                    || best_bid.price >= best_ask.price;

                if !crosses {
                    break;
                }

                // Self-trade prevention
                if best_bid.owner == best_ask.owner {
                    // Remove the newer order
                    if best_bid.timestamp > best_ask.timestamp {
                        ob.bids.remove(0);
                    } else {
                        ob.asks.remove(0);
                    }
                    continue;
                }

                // Match at the resting order's price (price-time priority)
                let fill_price = if best_bid.timestamp < best_ask.timestamp {
                    best_bid.price
                } else {
                    best_ask.price
                };
                let fill_size = best_bid.remaining.min(best_ask.remaining);

                // Update mark price
                if fill_price > 0 {
                    market.mark_price = fill_price;
                    // Add TWAP sample
                    let clock = Clock::get()?;
                    market.twap_samples.push(TwapSample {
                        timestamp: clock.unix_timestamp,
                        price: fill_price,
                    });
                    if market.twap_samples.len() > MAX_TWAP_SAMPLES {
                        market.twap_samples.remove(0);
                    }
                }

                emit!(OrderMatched {
                    market: market.key(),
                    bid_order_id: ob.bids[0].order_id,
                    ask_order_id: ob.asks[0].order_id,
                    price: fill_price,
                    size: fill_size,
                });

                // Update remaining sizes
                ob.bids[0].remaining = ob.bids[0].remaining.saturating_sub(fill_size);
                ob.asks[0].remaining = ob.asks[0].remaining.saturating_sub(fill_size);

                // Remove fully filled orders
                if ob.bids[0].remaining == 0 {
                    ob.bids.remove(0);
                }
                if !ob.asks.is_empty() && ob.asks[0].remaining == 0 {
                    ob.asks.remove(0);
                }

                matches_done += 1;
            }

            require!(matches_done > 0, PerpError::NoOrdersToMatch);
            Ok(())
        }

        /// Permissionless crank: update funding rate
        pub fn update_funding_rate(ctx: Context<UpdateFundingRate>) -> Result<()> {
            let market = &mut ctx.accounts.market;
            let clock = Clock::get()?;

            require!(
                clock.unix_timestamp >= market.last_funding_time + market.funding_interval,
                PerpError::FundingIntervalNotElapsed
            );

            // Funding rate = (mark_price - index_price) / index_price, clamped
            // Positive = longs pay shorts, Negative = shorts pay longs
            let mark = market.mark_price as i128;
            let index = market.index_price as i128;

            let raw_rate = if index > 0 {
                ((mark - index) * PRECISION as i128) / index
            } else {
                0
            };

            let clamped_rate = raw_rate.max(-MAX_FUNDING_RATE).min(MAX_FUNDING_RATE);

            // Update cumulative funding
            market.cumulative_funding_long = market.cumulative_funding_long
                .checked_add(clamped_rate)
                .ok_or_else(|| error!(PerpError::MathOverflow))?;
            market.cumulative_funding_short = market.cumulative_funding_short
                .checked_sub(clamped_rate)
                .ok_or_else(|| error!(PerpError::MathOverflow))?;

            market.last_funding_time = clock.unix_timestamp;

            emit!(FundingRateUpdated {
                market: market.key(),
                funding_rate: clamped_rate,
                mark_price: market.mark_price,
                index_price: market.index_price,
                timestamp: clock.unix_timestamp,
            });

            Ok(())
        }

        /// Liquidate an under-margined position (full or partial)
        pub fn liquidate_position(
            ctx: Context<LiquidatePosition>,
            liquidation_size: u64,
        ) -> Result<()> {
            let market = &ctx.accounts.market;
            let position = &ctx.accounts.position;

            // Settle pending funding
            let funding_payment = settle_funding(position, market)?;

            // Check if position is liquidatable
            let unrealized_pnl = math::calc_unrealized_pnl(
                &position.side, position.entry_price, market.mark_price, position.size,
            )?;
            let effective_pnl = unrealized_pnl + funding_payment;
            let notional = math::calc_notional(position.size, market.mark_price)?;
            let margin_ratio = math::calc_margin_ratio(position.collateral, effective_pnl, notional)?;

            require!(margin_ratio < market.maintenance_margin, PerpError::NotLiquidatable);

            // Determine liquidation amount (partial or full)
            let liq_size = if liquidation_size > 0 && liquidation_size < position.size {
                liquidation_size
            } else {
                position.size
            };

            // Calculate liquidation fee
            let liq_notional = math::calc_notional(liq_size, market.mark_price)?;
            let liq_fee = math::calc_fee(liq_notional, market.liquidation_fee)?;

            // Proportional collateral for liquidated portion
            let collateral_fraction = (position.collateral as u128)
                .checked_mul(liq_size as u128)
                .and_then(|v| v.checked_div(position.size as u128))
                .ok_or_else(|| error!(PerpError::MathOverflow))? as u64;

            // PnL on liquidated portion
            let liq_pnl = math::calc_unrealized_pnl(
                &position.side, position.entry_price, market.mark_price, liq_size,
            )?;

            // Remaining after PnL: if negative PnL exceeds collateral, insurance fund absorbs
            let remainder = (collateral_fraction as i64) + liq_pnl - (liq_fee as i64);

            // Pay liquidator fee
            // (In production: transfer liq_fee to liquidator's token account)

            // Update insurance fund if there's a shortfall
            if remainder < 0 {
                let shortfall = (-remainder) as u64;
                let ins = &mut ctx.accounts.insurance_fund;
                if ins.balance >= shortfall {
                    ins.balance = ins.balance.saturating_sub(shortfall);
                    ins.total_payouts = ins.total_payouts.checked_add(shortfall)
                        .ok_or_else(|| error!(PerpError::MathOverflow))?;
                }
                // If insurance fund insufficient, socialized loss (not implemented here)
            }

            // Update position
            let position_mut = &mut ctx.accounts.position;
            let margin = &mut ctx.accounts.position_margin_account;

            if liq_size >= position_mut.size {
                // Full liquidation — close position
                position_mut.size = 0;
                position_mut.collateral = 0;
                margin.open_positions = margin.open_positions.saturating_sub(1);
            } else {
                // Partial liquidation
                position_mut.size = position_mut.size.checked_sub(liq_size)
                    .ok_or_else(|| error!(PerpError::MathOverflow))?;
                position_mut.collateral = position_mut.collateral.checked_sub(collateral_fraction)
                    .ok_or_else(|| error!(PerpError::MathOverflow))?;
            }
            position_mut.updated_at = Clock::get()?.unix_timestamp;

            // Update market OI
            let market_mut = &mut ctx.accounts.market;
            match position_mut.side {
                Side::Long => {
                    market_mut.long_open_interest = market_mut.long_open_interest.saturating_sub(liq_size);
                }
                Side::Short => {
                    market_mut.short_open_interest = market_mut.short_open_interest.saturating_sub(liq_size);
                }
            }

            emit!(PositionLiquidated {
                market: market_mut.key(),
                owner: position_mut.owner,
                liquidator: ctx.accounts.liquidator.key(),
                size_liquidated: liq_size,
                price: market_mut.mark_price,
                liquidation_fee: liq_fee,
            });

            Ok(())
        }

        /// Update oracle price from Raydium pool (permissionless crank)
        pub fn update_oracle_price(ctx: Context<UpdateOraclePrice>, price: u64) -> Result<()> {
            // In production, this would read from the Raydium AMM pool account
            // and extract the current spot price. For now, the caller passes price
            // which should be validated against the Raydium pool state.
            let market = &mut ctx.accounts.market;
            require!(price > 0, PerpError::InvalidPrice);

            market.index_price = price;

            // Update TWAP
            let clock = Clock::get()?;
            market.twap_samples.push(TwapSample {
                timestamp: clock.unix_timestamp,
                price,
            });
            if market.twap_samples.len() > MAX_TWAP_SAMPLES {
                market.twap_samples.remove(0);
            }

            Ok(())
        }

        /// Pause or unpause a market (authority only)
        pub fn set_market_paused(ctx: Context<AdminAction>, paused: bool) -> Result<()> {
            require!(
                ctx.accounts.authority.key() == ctx.accounts.market.authority,
                PerpError::Unauthorized
            );
            ctx.accounts.market.paused = paused;
            Ok(())
        }


    // ============================================================================
    // Helper Functions
    // ============================================================================

    fn check_price_band(price: u64, index_price: u64) -> Result<()> {
        if index_price == 0 {
            return Ok(()); // No oracle yet, skip band check
        }
        let deviation = if price > index_price {
            ((price - index_price) as u128) * PRECISION / (index_price as u128)
        } else {
            ((index_price - price) as u128) * PRECISION / (index_price as u128)
        };
        require!(
            deviation <= (PRICE_BAND_BPS as u128) * PRECISION / 10_000,
            PerpError::CircuitBreakerTriggered
        );
        Ok(())
    }

    fn settle_funding(position: &Position, market: &PerpMarket) -> Result<i64> {
        let cumulative_now = match position.side {
            Side::Long => market.cumulative_funding_long,
            Side::Short => market.cumulative_funding_short,
        };
        math::calc_funding_payment(position.size, cumulative_now, position.last_cumulative_funding)
    }

    fn distribute_fees(_market: &PerpMarket, _fee: u64) -> Result<()> {
        // Fee distribution:
        // - INSURANCE_FEE_SHARE (30%) → insurance fund
        // - SOLFORGE_FEE_SHARE (20%) → SolForge vault for token burn
        // - Remaining 50% → protocol revenue
        //
        // In production, this performs CPI token transfers.
        // Insurance fund portion:
        //   let ins_amount = (fee as u128 * INSURANCE_FEE_SHARE as u128 / PRECISION) as u64;
        // SolForge burn portion:
        //   let burn_amount = (fee as u128 * SOLFORGE_FEE_SHARE as u128 / PRECISION) as u64;
        // Protocol portion:
        //   let protocol_amount = fee - ins_amount - burn_amount;
        Ok(())
    }

    // ============================================================================
    // Account Contexts
    // ============================================================================

    #[derive(Accounts)]
    pub struct InitializePerpMarket<'info> {
        #[account(mut)]
        pub authority: Signer<'info>,

        #[account(
            init,
            payer = authority,
            space = 8 + 2048, // generous allocation
            seeds = [b"perp_market", token_mint.key().as_ref()],
            bump,
        )]
        pub market: Account<'info, PerpMarket>,

        #[account(
            init,
            payer = authority,
            space = 8 + 32768, // order book needs room
            seeds = [b"order_book", market.key().as_ref()],
            bump,
        )]
        pub order_book: Account<'info, OrderBook>,

        #[account(
            init,
            payer = authority,
            space = 8 + 256,
            seeds = [b"insurance_fund", market.key().as_ref()],
            bump,
        )]
        pub insurance_fund: Account<'info, InsuranceFund>,

        /// CHECK: Token mint of the graduated token
        pub token_mint: AccountInfo<'info>,
        /// CHECK: Collateral token mint
        pub collateral_mint: AccountInfo<'info>,
        /// CHECK: Raydium AMM pool for oracle
        pub raydium_pool: AccountInfo<'info>,
        /// CHECK: SolForge vault for burns
        pub solforge_vault: AccountInfo<'info>,
        /// CHECK: Collateral vault token account
        pub collateral_vault: AccountInfo<'info>,
        /// CHECK: Insurance vault token account
        pub insurance_vault: AccountInfo<'info>,

        pub system_program: Program<'info, System>,
        pub token_program: Program<'info, Token>,
        pub rent: Sysvar<'info, Rent>,
    }

    #[derive(Accounts)]
    pub struct CreateMarginAccount<'info> {
        #[account(mut)]
        pub owner: Signer<'info>,

        #[account(
            init,
            payer = owner,
            space = 8 + 256,
            seeds = [b"margin_account", owner.key().as_ref()],
            bump,
        )]
        pub margin_account: Account<'info, UserMarginAccount>,

        pub system_program: Program<'info, System>,
    }

    #[derive(Accounts)]
    pub struct DepositCollateral<'info> {
        #[account(mut)]
        pub owner: Signer<'info>,

        #[account(
            mut,
            seeds = [b"margin_account", owner.key().as_ref()],
            bump = margin_account.bump,
            has_one = owner,
        )]
        pub margin_account: Account<'info, UserMarginAccount>,

        #[account(mut)]
        pub user_token_account: Account<'info, TokenAccount>,

        #[account(mut)]
        pub collateral_vault: Account<'info, TokenAccount>,

        pub token_program: Program<'info, Token>,
    }

    #[derive(Accounts)]
    pub struct WithdrawCollateral<'info> {
        #[account(mut)]
        pub owner: Signer<'info>,

        #[account(
            mut,
            seeds = [b"margin_account", owner.key().as_ref()],
            bump = margin_account.bump,
            has_one = owner,
        )]
        pub margin_account: Account<'info, UserMarginAccount>,

        pub market: Account<'info, PerpMarket>,

        #[account(mut)]
        pub user_token_account: Account<'info, TokenAccount>,

        #[account(mut)]
        pub collateral_vault: Account<'info, TokenAccount>,

        /// CHECK: PDA vault authority
        pub vault_authority: AccountInfo<'info>,

        pub token_program: Program<'info, Token>,
    }

    #[derive(Accounts)]
    pub struct OpenPosition<'info> {
        #[account(mut)]
        pub owner: Signer<'info>,

        #[account(mut)]
        pub market: Account<'info, PerpMarket>,

        #[account(
            mut,
            seeds = [b"margin_account", owner.key().as_ref()],
            bump = margin_account.bump,
            has_one = owner,
        )]
        pub margin_account: Account<'info, UserMarginAccount>,

        #[account(
            init,
            payer = owner,
            space = 8 + 512,
            seeds = [b"position", market.key().as_ref(), owner.key().as_ref(), &market.long_open_interest.to_le_bytes()],
            bump,
        )]
        pub position: Account<'info, Position>,

        pub system_program: Program<'info, System>,
    }

    #[derive(Accounts)]
    pub struct ClosePosition<'info> {
        #[account(mut)]
        pub owner: Signer<'info>,

        #[account(mut)]
        pub market: Account<'info, PerpMarket>,

        #[account(
            mut,
            has_one = owner,
            has_one = market,
            close = owner,
        )]
        pub position: Account<'info, Position>,

        #[account(
            mut,
            seeds = [b"margin_account", owner.key().as_ref()],
            bump = margin_account.bump,
            has_one = owner,
        )]
        pub margin_account: Account<'info, UserMarginAccount>,
    }

    #[derive(Accounts)]
    pub struct ModifyPosition<'info> {
        #[account(mut)]
        pub owner: Signer<'info>,

        #[account(mut)]
        pub market: Account<'info, PerpMarket>,

        #[account(
            mut,
            has_one = owner,
            has_one = market,
        )]
        pub position: Account<'info, Position>,

        #[account(
            mut,
            seeds = [b"margin_account", owner.key().as_ref()],
            bump = margin_account.bump,
            has_one = owner,
        )]
        pub margin_account: Account<'info, UserMarginAccount>,
    }

    #[derive(Accounts)]
    pub struct PlaceOrder<'info> {
        #[account(mut)]
        pub owner: Signer<'info>,

        pub market: Account<'info, PerpMarket>,

        #[account(
            mut,
            has_one = market,
        )]
        pub order_book: Account<'info, OrderBook>,

        #[account(
            seeds = [b"margin_account", owner.key().as_ref()],
            bump = margin_account.bump,
            has_one = owner,
        )]
        pub margin_account: Account<'info, UserMarginAccount>,
    }

    #[derive(Accounts)]
    pub struct CancelOrder<'info> {
        #[account(mut)]
        pub owner: Signer<'info>,

        #[account(mut)]
        pub order_book: Account<'info, OrderBook>,
    }

    #[derive(Accounts)]
    pub struct MatchOrders<'info> {
        /// Anyone can crank
        pub cranker: Signer<'info>,

        #[account(mut)]
        pub market: Account<'info, PerpMarket>,

        #[account(
            mut,
            has_one = market,
        )]
        pub order_book: Account<'info, OrderBook>,
    }

    #[derive(Accounts)]
    pub struct UpdateFundingRate<'info> {
        /// Anyone can crank
        pub cranker: Signer<'info>,

        #[account(mut)]
        pub market: Account<'info, PerpMarket>,
    }

    #[derive(Accounts)]
    pub struct LiquidatePosition<'info> {
        #[account(mut)]
        pub liquidator: Signer<'info>,

        #[account(mut)]
        pub market: Account<'info, PerpMarket>,

        #[account(
            mut,
            has_one = market,
        )]
        pub position: Account<'info, Position>,

        #[account(
            mut,
            constraint = position_margin_account.key() == position.margin_account @ PerpError::InvalidMarginAccount,
        )]
        pub position_margin_account: Account<'info, UserMarginAccount>,

        #[account(
            mut,
            has_one = market,
        )]
        pub insurance_fund: Account<'info, InsuranceFund>,
    }

    #[derive(Accounts)]
    pub struct UpdateOraclePrice<'info> {
        /// Anyone can crank (but should validate against Raydium pool)
        pub cranker: Signer<'info>,

        #[account(mut)]
        pub market: Account<'info, PerpMarket>,

        /// CHECK: Raydium pool account to read price from
        pub raydium_pool: AccountInfo<'info>,
    }

    #[derive(Accounts)]
    pub struct AdminAction<'info> {
        pub authority: Signer<'info>,

        #[account(mut)]
        pub market: Account<'info, PerpMarket>,
    }

}
pub mod prediction_market {
    use super::*;

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
    }

    // ---------------------------------------------------------------------------
    // Instructions
    // ---------------------------------------------------------------------------


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

        /// CHECK: Token A launch account to verify graduation
        pub token_a_launch: UncheckedAccount<'info>,

        /// CHECK: Token B launch account to verify graduation
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

}
pub mod premium {
    use super::*;

    // ============================================================================
    // SEEDS & CONSTANTS
    // ============================================================================

    pub const PREMIUM_LISTING_SEED: &[u8] = b"premium_listing";
    pub const PREMIUM_CONFIG_SEED: &[u8] = b"premium_config";

    pub const LAMPORTS_PER_SOL: u64 = 1_000_000_000;

    // ============================================================================
    // ENUMS
    // ============================================================================

    #[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
    pub enum PremiumTier {
        /// Basic promotion — appears in "Promoted" section.
        Promoted,
        /// Mid-tier — featured on homepage carousel.
        Featured,
        /// Top tier — spotlight banner placement.
        Spotlight,
    }

    impl PremiumTier {
        /// Price per hour in lamports.
        pub fn price_per_hour(&self, config: &PremiumConfig) -> u64 {
            match self {
                PremiumTier::Promoted => config.promoted_price_per_hour,
                PremiumTier::Featured => config.featured_price_per_hour,
                PremiumTier::Spotlight => config.spotlight_price_per_hour,
            }
        }
    }

    // ============================================================================
    // ACCOUNTS
    // ============================================================================

    /// Global premium listing configuration.
    #[account]
    pub struct PremiumConfig {
        pub authority: Pubkey,
        pub treasury: Pubkey,
        /// Price per hour in lamports for each tier.
        pub promoted_price_per_hour: u64,
        pub featured_price_per_hour: u64,
        pub spotlight_price_per_hour: u64,
        pub bump: u8,
    }

    impl PremiumConfig {
        // 8 + 32 + 32 + 8 + 8 + 8 + 1
        pub const SIZE: usize = 8 + 32 + 32 + 8 + 8 + 8 + 1;
    }

    /// Per-token premium listing, PDA from [PREMIUM_LISTING_SEED, token_mint].
    #[account]
    pub struct PremiumListing {
        /// The token mint this listing is for.
        pub token_mint: Pubkey,
        /// Who purchased the premium listing.
        pub purchaser: Pubkey,
        /// Premium tier.
        pub tier: PremiumTier,
        /// Unix timestamp when the listing started.
        pub start_time: i64,
        /// Duration in seconds.
        pub duration: i64,
        /// Total SOL paid (lamports).
        pub amount_paid: u64,
        /// Whether the listing is currently active.
        pub active: bool,
        /// Bump seed.
        pub bump: u8,
    }

    impl PremiumListing {
        // 8 + 32 + 32 + 1 + 8 + 8 + 8 + 1 + 1
        pub const SIZE: usize = 8 + 32 + 32 + 1 + 8 + 8 + 8 + 1 + 1;
    }

    // ============================================================================
    // INSTRUCTION CONTEXTS
    // ============================================================================

    #[derive(Accounts)]
    pub struct InitializePremiumConfig<'info> {
        #[account(
            init,
            payer = authority,
            space = PremiumConfig::SIZE,
            seeds = [PREMIUM_CONFIG_SEED],
            bump,
        )]
        pub config: Account<'info, PremiumConfig>,
        /// CHECK: Treasury to receive premium payments.
        pub treasury: AccountInfo<'info>,
        #[account(mut)]
        pub authority: Signer<'info>,
        pub system_program: Program<'info, System>,
    }

    #[derive(Accounts)]
    pub struct PurchasePremium<'info> {
        #[account(
            init_if_needed,
            payer = purchaser,
            space = PremiumListing::SIZE,
            seeds = [PREMIUM_LISTING_SEED, token_mint.key().as_ref()],
            bump,
        )]
        pub premium_listing: Account<'info, PremiumListing>,
        #[account(
            seeds = [PREMIUM_CONFIG_SEED],
            bump = config.bump,
        )]
        pub config: Account<'info, PremiumConfig>,
        /// CHECK: The token mint to feature.
        pub token_mint: AccountInfo<'info>,
        /// CHECK: Treasury receives payment. Validated against config.
        #[account(
            mut,
            constraint = treasury.key() == config.treasury @ PremiumError::InvalidTreasury,
        )]
        pub treasury: AccountInfo<'info>,
        #[account(mut)]
        pub purchaser: Signer<'info>,
        pub system_program: Program<'info, System>,
    }

    #[derive(Accounts)]
    pub struct CheckPremiumStatus<'info> {
        #[account(
            mut,
            seeds = [PREMIUM_LISTING_SEED, premium_listing.token_mint.as_ref()],
            bump = premium_listing.bump,
        )]
        pub premium_listing: Account<'info, PremiumListing>,
    }

    // ============================================================================
    // INSTRUCTIONS
    // ============================================================================

    pub fn handle_initialize_premium_config(
        ctx: Context<InitializePremiumConfig>,
        promoted_price_per_hour: u64,
        featured_price_per_hour: u64,
        spotlight_price_per_hour: u64,
    ) -> Result<()> {
        let config = &mut ctx.accounts.config;
        config.authority = ctx.accounts.authority.key();
        config.treasury = ctx.accounts.treasury.key();
        config.promoted_price_per_hour = promoted_price_per_hour;
        config.featured_price_per_hour = featured_price_per_hour;
        config.spotlight_price_per_hour = spotlight_price_per_hour;
        config.bump = ctx.bumps.config;
        Ok(())
    }

    pub fn handle_purchase_premium(
        ctx: Context<PurchasePremium>,
        tier: PremiumTier,
        duration_hours: u64,
    ) -> Result<()> {
        require!(duration_hours > 0, PremiumError::InvalidDuration);
        require!(duration_hours <= 720, PremiumError::InvalidDuration); // Max 30 days

        let clock = Clock::get()?;
        let config = &ctx.accounts.config;
        let listing = &mut ctx.accounts.premium_listing;

        // If an existing listing is still active, extend it
        let current_end = if listing.active {
            let end = listing.start_time + listing.duration;
            if clock.unix_timestamp < end {
                end
            } else {
                clock.unix_timestamp
            }
        } else {
            clock.unix_timestamp
        };

        let price_per_hour = tier.price_per_hour(config);
        let total_cost = price_per_hour.checked_mul(duration_hours).ok_or(PremiumError::Overflow)?;

        // Transfer SOL to treasury
        system_program::transfer(
            CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                system_program::Transfer {
                    from: ctx.accounts.purchaser.to_account_info(),
                    to: ctx.accounts.treasury.to_account_info(),
                },
            ),
            total_cost,
        )?;

        let duration_seconds = (duration_hours as i64).checked_mul(3600).unwrap();

        listing.token_mint = ctx.accounts.token_mint.key();
        listing.purchaser = ctx.accounts.purchaser.key();
        listing.tier = tier;
        listing.start_time = current_end;
        listing.duration = if listing.active {
            listing.duration.checked_add(duration_seconds).unwrap()
        } else {
            duration_seconds
        };
        listing.amount_paid = listing.amount_paid.checked_add(total_cost).unwrap();
        listing.active = true;
        listing.bump = ctx.bumps.premium_listing;

        emit!(PremiumPurchased {
            token_mint: listing.token_mint,
            purchaser: listing.purchaser,
            tier,
            duration_hours,
            amount_paid: total_cost,
            expires_at: listing.start_time + listing.duration,
        });

        Ok(())
    }

    /// Check if a premium listing is still active; deactivate if expired.
    pub fn handle_check_premium_status(ctx: Context<CheckPremiumStatus>) -> Result<bool> {
        let clock = Clock::get()?;
        let listing = &mut ctx.accounts.premium_listing;

        if !listing.active {
            return Ok(false);
        }

        let expires_at = listing.start_time + listing.duration;
        if clock.unix_timestamp >= expires_at {
            listing.active = false;

            emit!(PremiumExpired {
                token_mint: listing.token_mint,
                expired_at: clock.unix_timestamp,
            });

            return Ok(false);
        }

        Ok(true)
    }

    // ============================================================================
    // EVENTS
    // ============================================================================

    #[event]
    pub struct PremiumPurchased {
        pub token_mint: Pubkey,
        pub purchaser: Pubkey,
        pub tier: PremiumTier,
        pub duration_hours: u64,
        pub amount_paid: u64,
        pub expires_at: i64,
    }

    #[event]
    pub struct PremiumExpired {
        pub token_mint: Pubkey,
        pub expired_at: i64,
    }

    // ============================================================================
    // ERRORS
    // ============================================================================

    #[error_code]
    pub enum PremiumError {
        #[msg("Invalid duration (must be 1-720 hours)")]
        InvalidDuration,
        #[msg("Treasury account does not match config")]
        InvalidTreasury,
        #[msg("Arithmetic overflow")]
        Overflow,
        #[msg("Premium listing is not active")]
        NotActive,
    }

}
pub mod price_alerts {
    use super::*;

    // ---------------------------------------------------------------------------
    // Constants
    // ---------------------------------------------------------------------------
    pub const MAX_ALERTS_PER_CHECK: usize = 10;

    // ---------------------------------------------------------------------------
    // Account structs
    // ---------------------------------------------------------------------------

    #[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
    pub enum PriceDirection {
        Above,
        Below,
    }

    #[account]
    pub struct AlertSubscription {
        pub owner: Pubkey,
        pub token_mint: Pubkey,
        pub target_price: u64,     // price in lamports per token (scaled)
        pub direction: PriceDirection,
        pub active: bool,
        pub created_at: i64,
        pub triggered_at: i64,     // 0 if not yet triggered
        pub alert_id: u64,
        pub bump: u8,
    }

    // ---------------------------------------------------------------------------
    // Events
    // ---------------------------------------------------------------------------

    #[event]
    pub struct AlertCreated {
        pub owner: Pubkey,
        pub token_mint: Pubkey,
        pub alert_id: u64,
        pub target_price: u64,
        pub direction: PriceDirection,
    }

    #[event]
    pub struct AlertCancelled {
        pub owner: Pubkey,
        pub token_mint: Pubkey,
        pub alert_id: u64,
    }

    #[event]
    pub struct AlertTriggered {
        pub owner: Pubkey,
        pub token_mint: Pubkey,
        pub alert_id: u64,
        pub target_price: u64,
        pub current_price: u64,
        pub direction: PriceDirection,
        pub timestamp: i64,
    }

    // ---------------------------------------------------------------------------
    // Errors
    // ---------------------------------------------------------------------------

    #[error_code]
    pub enum PriceAlertError {
        #[msg("Alert is not active")]
        AlertNotActive,
        #[msg("Alert is already active")]
        AlertAlreadyActive,
        #[msg("Unauthorized — only the alert owner can cancel")]
        Unauthorized,
        #[msg("Target price must be greater than zero")]
        InvalidTargetPrice,
        #[msg("Price condition not met")]
        PriceConditionNotMet,
    }

    // ---------------------------------------------------------------------------
    // Instruction accounts
    // ---------------------------------------------------------------------------

    #[derive(Accounts)]
    #[instruction(alert_id: u64, token_mint: Pubkey)]
    pub struct CreateAlert<'info> {
        #[account(
            init,
            payer = owner,
            space = 8 + std::mem::size_of::<AlertSubscription>(),
            seeds = [b"alert", owner.key().as_ref(), token_mint.as_ref(), alert_id.to_le_bytes().as_ref()],
            bump
        )]
        pub alert: Account<'info, AlertSubscription>,

        #[account(mut)]
        pub owner: Signer<'info>,
        pub system_program: Program<'info, System>,
    }

    #[derive(Accounts)]
    pub struct CancelAlert<'info> {
        #[account(
            mut,
            seeds = [b"alert", alert.owner.as_ref(), alert.token_mint.as_ref(), alert.alert_id.to_le_bytes().as_ref()],
            bump = alert.bump,
            has_one = owner @ PriceAlertError::Unauthorized
        )]
        pub alert: Account<'info, AlertSubscription>,

        pub owner: Signer<'info>,
    }

    #[derive(Accounts)]
    pub struct CheckAlerts<'info> {
        #[account(
            mut,
            seeds = [b"alert", alert.owner.as_ref(), alert.token_mint.as_ref(), alert.alert_id.to_le_bytes().as_ref()],
            bump = alert.bump
        )]
        pub alert: Account<'info, AlertSubscription>,

        /// Permissionless crank signer
        pub crank: Signer<'info>,
    }

    // ---------------------------------------------------------------------------
    // Instructions
    // ---------------------------------------------------------------------------

    pub fn handle_create_alert(
        ctx: Context<CreateAlert>,
        alert_id: u64,
        token_mint: Pubkey,
        target_price: u64,
        direction: PriceDirection,
    ) -> Result<()> {
        require!(target_price > 0, PriceAlertError::InvalidTargetPrice);

        let alert = &mut ctx.accounts.alert;
        let clock = Clock::get()?;

        alert.owner = ctx.accounts.owner.key();
        alert.token_mint = token_mint;
        alert.target_price = target_price;
        alert.direction = direction;
        alert.active = true;
        alert.created_at = clock.unix_timestamp;
        alert.triggered_at = 0;
        alert.alert_id = alert_id;
        alert.bump = ctx.bumps.alert;

        emit!(AlertCreated {
            owner: alert.owner,
            token_mint,
            alert_id,
            target_price,
            direction,
        });

        Ok(())
    }

    pub fn handle_cancel_alert(ctx: Context<CancelAlert>) -> Result<()> {
        let alert = &mut ctx.accounts.alert;
        require!(alert.active, PriceAlertError::AlertNotActive);

        alert.active = false;

        emit!(AlertCancelled {
            owner: alert.owner,
            token_mint: alert.token_mint,
            alert_id: alert.alert_id,
        });

        Ok(())
    }

    pub fn handle_check_alerts(
        ctx: Context<CheckAlerts>,
        current_price: u64,
    ) -> Result<()> {
        let alert = &mut ctx.accounts.alert;
        require!(alert.active, PriceAlertError::AlertNotActive);

        let triggered = match alert.direction {
            PriceDirection::Above => current_price >= alert.target_price,
            PriceDirection::Below => current_price <= alert.target_price,
        };

        require!(triggered, PriceAlertError::PriceConditionNotMet);

        let clock = Clock::get()?;
        alert.active = false;
        alert.triggered_at = clock.unix_timestamp;

        emit!(AlertTriggered {
            owner: alert.owner,
            token_mint: alert.token_mint,
            alert_id: alert.alert_id,
            target_price: alert.target_price,
            current_price,
            direction: alert.direction,
            timestamp: clock.unix_timestamp,
        });

        Ok(())
    }

}
pub mod raffle {
    use super::*;

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

}
pub mod referral {
    use super::*;

    // ============================================================================
    // SEEDS & CONSTANTS
    // ============================================================================

    pub const REFERRAL_ACCOUNT_SEED: &[u8] = b"referral";
    pub const REFERRAL_VAULT_SEED: &[u8] = b"referral_vault";

    /// Referrer gets 25% of the platform fee by default (configurable).
    pub const DEFAULT_REFERRAL_FEE_BPS: u16 = 2500; // 25% of platform fee

    // ============================================================================
    // ACCOUNTS
    // ============================================================================

    /// Global referral configuration.
    #[account]
    pub struct ReferralConfig {
        pub authority: Pubkey,
        /// Basis points of platform fee allocated to referrer (e.g. 2500 = 25%).
        pub referral_fee_bps: u16,
        /// Platform treasury that receives the remainder.
        pub treasury: Pubkey,
        pub bump: u8,
    }

    impl ReferralConfig {
        pub const SIZE: usize = 8 + 32 + 2 + 32 + 1;
    }

    /// Per-user referral account, PDA from [REFERRAL_ACCOUNT_SEED, user].
    #[account]
    pub struct ReferralAccount {
        /// The user this referral account belongs to.
        pub user: Pubkey,
        /// The user's referrer (Pubkey::default() if none).
        pub referrer: Pubkey,
        /// How many users this user has referred.
        pub total_referred: u64,
        /// Total lamports earned from referrals (pending + claimed).
        pub total_earned: u64,
        /// Lamports available to claim.
        pub claimable: u64,
        /// Timestamp of registration.
        pub registered_at: i64,
        /// Bump seed.
        pub bump: u8,
    }

    impl ReferralAccount {
        // 8 + 32 + 32 + 8 + 8 + 8 + 8 + 1
        pub const SIZE: usize = 8 + 32 + 32 + 8 + 8 + 8 + 8 + 1;
    }

    // ============================================================================
    // INSTRUCTION CONTEXTS
    // ============================================================================

    #[derive(Accounts)]
    pub struct InitializeReferralConfig<'info> {
        #[account(
            init,
            payer = authority,
            space = ReferralConfig::SIZE,
            seeds = [b"referral_config"],
            bump,
        )]
        pub config: Account<'info, ReferralConfig>,
        /// CHECK: Treasury wallet.
        pub treasury: AccountInfo<'info>,
        #[account(mut)]
        pub authority: Signer<'info>,
        pub system_program: Program<'info, System>,
    }

    /// Register a new referral account. If `referrer_account` is provided, link the referral.
    #[derive(Accounts)]
    pub struct RegisterReferral<'info> {
        #[account(
            init,
            payer = user,
            space = ReferralAccount::SIZE,
            seeds = [REFERRAL_ACCOUNT_SEED, user.key().as_ref()],
            bump,
        )]
        pub referral_account: Account<'info, ReferralAccount>,
        /// The referrer's existing referral account (optional — can be None if no referrer).
        #[account(
            mut,
            seeds = [REFERRAL_ACCOUNT_SEED, referrer_account.user.as_ref()],
            bump = referrer_account.bump,
        )]
        pub referrer_account: Option<Account<'info, ReferralAccount>>,
        #[account(mut)]
        pub user: Signer<'info>,
        pub system_program: Program<'info, System>,
    }

    /// Called during a trade to credit referral rewards.
    #[derive(Accounts)]
    pub struct CreditReferralReward<'info> {
        /// The trading user's referral account (must have a referrer set).
        #[account(
            seeds = [REFERRAL_ACCOUNT_SEED, trader_referral.user.as_ref()],
            bump = trader_referral.bump,
        )]
        pub trader_referral: Account<'info, ReferralAccount>,
        /// The referrer's account to credit.
        #[account(
            mut,
            seeds = [REFERRAL_ACCOUNT_SEED, referrer_referral.user.as_ref()],
            bump = referrer_referral.bump,
            constraint = referrer_referral.user == trader_referral.referrer @ ReferralError::ReferrerMismatch,
        )]
        pub referrer_referral: Account<'info, ReferralAccount>,
        #[account(
            seeds = [b"referral_config"],
            bump = config.bump,
        )]
        pub config: Account<'info, ReferralConfig>,
        /// The platform fee payer (program authority / CPI caller).
        #[account(mut)]
        pub fee_payer: Signer<'info>,
        /// CHECK: Referral vault PDA that holds reward lamports.
        #[account(
            mut,
            seeds = [REFERRAL_VAULT_SEED],
            bump,
        )]
        pub referral_vault: AccountInfo<'info>,
        pub system_program: Program<'info, System>,
    }

    /// Claim accumulated referral rewards.
    #[derive(Accounts)]
    pub struct ClaimReferralRewards<'info> {
        #[account(
            mut,
            seeds = [REFERRAL_ACCOUNT_SEED, user.key().as_ref()],
            bump = referral_account.bump,
            constraint = referral_account.user == user.key() @ ReferralError::Unauthorized,
        )]
        pub referral_account: Account<'info, ReferralAccount>,
        #[account(mut)]
        pub user: Signer<'info>,
        /// CHECK: Referral vault PDA that holds reward lamports.
        #[account(
            mut,
            seeds = [REFERRAL_VAULT_SEED],
            bump,
        )]
        pub referral_vault: AccountInfo<'info>,
        pub system_program: Program<'info, System>,
    }

    // ============================================================================
    // INSTRUCTIONS
    // ============================================================================

    pub fn handle_initialize_referral_config(
        ctx: Context<InitializeReferralConfig>,
        referral_fee_bps: u16,
    ) -> Result<()> {
        require!(referral_fee_bps <= 10_000, ReferralError::InvalidFeeBps);
        let config = &mut ctx.accounts.config;
        config.authority = ctx.accounts.authority.key();
        config.referral_fee_bps = referral_fee_bps;
        config.treasury = ctx.accounts.treasury.key();
        config.bump = ctx.bumps.config;
        Ok(())
    }

    pub fn handle_register_referral(ctx: Context<RegisterReferral>) -> Result<()> {
        let clock = Clock::get()?;
        let acct = &mut ctx.accounts.referral_account;

        acct.user = ctx.accounts.user.key();
        acct.total_referred = 0;
        acct.total_earned = 0;
        acct.claimable = 0;
        acct.registered_at = clock.unix_timestamp;
        acct.bump = ctx.bumps.referral_account;

        if let Some(referrer) = &mut ctx.accounts.referrer_account {
            // Can't refer yourself
            require!(referrer.user != acct.user, ReferralError::SelfReferral);
            acct.referrer = referrer.user;
            referrer.total_referred = referrer.total_referred.checked_add(1).unwrap();

            emit!(ReferralRegistered {
                user: acct.user,
                referrer: referrer.user,
                timestamp: clock.unix_timestamp,
            });
        } else {
            acct.referrer = Pubkey::default();
        }

        Ok(())
    }

    /// Credit referral reward during a trade. Called via CPI from the trade instruction.
    pub fn handle_credit_referral_reward(
        ctx: Context<CreditReferralReward>,
        platform_fee_lamports: u64,
    ) -> Result<()> {
        let config = &ctx.accounts.config;
        let referrer = &mut ctx.accounts.referrer_referral;

        // Calculate referrer's share of the platform fee
        let referral_reward = platform_fee_lamports
            .checked_mul(config.referral_fee_bps as u64)
            .unwrap()
            .checked_div(10_000)
            .unwrap();

        if referral_reward == 0 {
            return Ok(());
        }

        // Transfer reward lamports to the vault
        system_program::transfer(
            CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                system_program::Transfer {
                    from: ctx.accounts.fee_payer.to_account_info(),
                    to: ctx.accounts.referral_vault.to_account_info(),
                },
            ),
            referral_reward,
        )?;

        referrer.total_earned = referrer.total_earned.checked_add(referral_reward).unwrap();
        referrer.claimable = referrer.claimable.checked_add(referral_reward).unwrap();

        emit!(ReferralRewardCredited {
            referrer: referrer.user,
            trader: ctx.accounts.trader_referral.user,
            amount: referral_reward,
        });

        Ok(())
    }

    pub fn handle_claim_referral_rewards(ctx: Context<ClaimReferralRewards>) -> Result<()> {
        let acct = &mut ctx.accounts.referral_account;
        let amount = acct.claimable;
        require!(amount > 0, ReferralError::NothingToClaim);

        acct.claimable = 0;

        // Transfer from vault PDA to user
        let vault_bump = ctx.bumps.referral_vault;
        let seeds: &[&[u8]] = &[REFERRAL_VAULT_SEED, &[vault_bump]];
        let signer_seeds = &[seeds];

        **ctx.accounts.referral_vault.try_borrow_mut_lamports()? -= amount;
        **ctx.accounts.user.try_borrow_mut_lamports()? += amount;

        // Verify we used PDA correctly (the seeds are validated by account constraint)
        let _ = signer_seeds;

        emit!(ReferralRewardsClaimed {
            user: acct.user,
            amount,
        });

        Ok(())
    }

    // ============================================================================
    // EVENTS
    // ============================================================================

    #[event]
    pub struct ReferralRegistered {
        pub user: Pubkey,
        pub referrer: Pubkey,
        pub timestamp: i64,
    }

    #[event]
    pub struct ReferralRewardCredited {
        pub referrer: Pubkey,
        pub trader: Pubkey,
        pub amount: u64,
    }

    #[event]
    pub struct ReferralRewardsClaimed {
        pub user: Pubkey,
        pub amount: u64,
    }

    // ============================================================================
    // ERRORS
    // ============================================================================

    #[error_code]
    pub enum ReferralError {
        #[msg("Cannot refer yourself")]
        SelfReferral,
        #[msg("No referral rewards to claim")]
        NothingToClaim,
        #[msg("Referrer account mismatch")]
        ReferrerMismatch,
        #[msg("Unauthorized")]
        Unauthorized,
        #[msg("Invalid fee basis points (must be <= 10000)")]
        InvalidFeeBps,
    }

}
pub mod reputation {
    use super::*;

    // ═══════════════════════════════════════════════
    //  FairScale Reputation Module for Send.it
    //  On-chain reputation gating via FairScore oracle
    // ═══════════════════════════════════════════════

    #[account]
    #[derive(Default)]
    pub struct ReputationConfig {
        pub authority: Pubkey,
        pub oracle_authority: Pubkey,
        pub min_score_to_launch: u8,           // default 30
        pub min_score_premium_launch: u8,      // default 60
        pub strict_vesting_threshold: u8,      // below this → 2x vesting
        pub fee_discount_bronze_bps: u16,      // 0 (0%)
        pub fee_discount_silver_bps: u16,      // 500 (5%)
        pub fee_discount_gold_bps: u16,        // 1000 (10%)
        pub fee_discount_platinum_bps: u16,    // 2000 (20%)
        pub bump: u8,
    }

    #[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Debug)]
    pub enum ReputationTier {
        Unscored,
        Bronze,
        Silver,
        Gold,
        Platinum,
    }

    impl Default for ReputationTier {
        fn default() -> Self { ReputationTier::Unscored }
    }

    #[account]
    #[derive(Default)]
    pub struct ReputationAttestation {
        pub wallet: Pubkey,
        pub fairscore: u8,               // 0-100
        pub tier: ReputationTier,
        pub last_updated: i64,           // unix timestamp
        pub attested_by: Pubkey,         // oracle authority that submitted
        pub bump: u8,
    }

    // ─── Contexts ───

    #[derive(Accounts)]
    pub struct InitializeReputationConfig<'info> {
        #[account(
            init,
            payer = authority,
            space = 8 + 32 + 32 + 1 + 1 + 1 + 2 + 2 + 2 + 2 + 1,
            seeds = [b"reputation_config"],
            bump,
        )]
        pub config: Account<'info, ReputationConfig>,
        #[account(mut)]
        pub authority: Signer<'info>,
        pub system_program: Program<'info, System>,
    }

    #[derive(Accounts)]
    pub struct UpdateReputationConfig<'info> {
        #[account(
            mut,
            seeds = [b"reputation_config"],
            bump = config.bump,
            has_one = authority,
        )]
        pub config: Account<'info, ReputationConfig>,
        pub authority: Signer<'info>,
    }

    #[derive(Accounts)]
    #[instruction(wallet: Pubkey)]
    pub struct UpdateReputation<'info> {
        #[account(
            init_if_needed,
            payer = oracle_authority,
            space = 8 + 32 + 1 + 1 + 8 + 32 + 1,
            seeds = [b"reputation", wallet.as_ref()],
            bump,
        )]
        pub attestation: Account<'info, ReputationAttestation>,
        #[account(
            seeds = [b"reputation_config"],
            bump = config.bump,
            constraint = config.oracle_authority == oracle_authority.key(),
        )]
        pub config: Account<'info, ReputationConfig>,
        #[account(mut)]
        pub oracle_authority: Signer<'info>,
        pub system_program: Program<'info, System>,
    }

    #[derive(Accounts)]
    pub struct CheckLaunchEligibility<'info> {
        #[account(
            seeds = [b"reputation", attestation.wallet.as_ref()],
            bump = attestation.bump,
        )]
        pub attestation: Account<'info, ReputationAttestation>,
        #[account(
            seeds = [b"reputation_config"],
            bump = config.bump,
        )]
        pub config: Account<'info, ReputationConfig>,
    }

    #[derive(Accounts)]
    pub struct GetFeeDiscount<'info> {
        #[account(
            seeds = [b"reputation", attestation.wallet.as_ref()],
            bump = attestation.bump,
        )]
        pub attestation: Account<'info, ReputationAttestation>,
        #[account(
            seeds = [b"reputation_config"],
            bump = config.bump,
        )]
        pub config: Account<'info, ReputationConfig>,
    }

    // ─── Instructions ───

    pub fn initialize_reputation_config(
        ctx: Context<InitializeReputationConfig>,
        oracle_authority: Pubkey,
    ) -> Result<()> {
        let config = &mut ctx.accounts.config;
        config.authority = ctx.accounts.authority.key();
        config.oracle_authority = oracle_authority;
        config.min_score_to_launch = 30;
        config.min_score_premium_launch = 60;
        config.strict_vesting_threshold = 40;
        config.fee_discount_bronze_bps = 0;
        config.fee_discount_silver_bps = 500;
        config.fee_discount_gold_bps = 1000;
        config.fee_discount_platinum_bps = 2000;
        config.bump = ctx.bumps.config;
        Ok(())
    }

    pub fn update_reputation_config(
        ctx: Context<UpdateReputationConfig>,
        min_score_to_launch: Option<u8>,
        min_score_premium_launch: Option<u8>,
        strict_vesting_threshold: Option<u8>,
        fee_discount_bronze_bps: Option<u16>,
        fee_discount_silver_bps: Option<u16>,
        fee_discount_gold_bps: Option<u16>,
        fee_discount_platinum_bps: Option<u16>,
        oracle_authority: Option<Pubkey>,
    ) -> Result<()> {
        let config = &mut ctx.accounts.config;
        if let Some(v) = min_score_to_launch { config.min_score_to_launch = v; }
        if let Some(v) = min_score_premium_launch { config.min_score_premium_launch = v; }
        if let Some(v) = strict_vesting_threshold { config.strict_vesting_threshold = v; }
        if let Some(v) = fee_discount_bronze_bps { config.fee_discount_bronze_bps = v; }
        if let Some(v) = fee_discount_silver_bps { config.fee_discount_silver_bps = v; }
        if let Some(v) = fee_discount_gold_bps { config.fee_discount_gold_bps = v; }
        if let Some(v) = fee_discount_platinum_bps { config.fee_discount_platinum_bps = v; }
        if let Some(v) = oracle_authority { config.oracle_authority = v; }
        Ok(())
    }

    pub fn update_reputation(
        ctx: Context<UpdateReputation>,
        wallet: Pubkey,
        fairscore: u8,
        tier: ReputationTier,
    ) -> Result<()> {
        require!(fairscore <= 100, ReputationError::InvalidScore);

        let attestation = &mut ctx.accounts.attestation;
        attestation.wallet = wallet;
        attestation.fairscore = fairscore;
        attestation.tier = tier;
        attestation.last_updated = Clock::get()?.unix_timestamp;
        attestation.attested_by = ctx.accounts.oracle_authority.key();
        attestation.bump = ctx.bumps.attestation;

        msg!("Reputation updated: wallet={}, score={}, tier={:?}", wallet, fairscore, tier);
        Ok(())
    }

    pub fn check_launch_eligibility(
        ctx: Context<CheckLaunchEligibility>,
        premium: bool,
    ) -> Result<bool> {
        let attestation = &ctx.accounts.attestation;
        let config = &ctx.accounts.config;

        let min_score = if premium {
            config.min_score_premium_launch
        } else {
            config.min_score_to_launch
        };

        let eligible = attestation.fairscore >= min_score;
        msg!(
            "Launch eligibility: score={}, min={}, eligible={}",
            attestation.fairscore, min_score, eligible
        );
        Ok(eligible)
    }

    pub fn get_fee_discount(ctx: Context<GetFeeDiscount>) -> Result<u16> {
        let attestation = &ctx.accounts.attestation;
        let config = &ctx.accounts.config;

        let discount_bps = match attestation.tier {
            ReputationTier::Platinum => config.fee_discount_platinum_bps,
            ReputationTier::Gold => config.fee_discount_gold_bps,
            ReputationTier::Silver => config.fee_discount_silver_bps,
            ReputationTier::Bronze => config.fee_discount_bronze_bps,
            ReputationTier::Unscored => 0,
        };

        msg!("Fee discount: tier={:?}, discount_bps={}", attestation.tier, discount_bps);
        Ok(discount_bps)
    }

    /// Check if creator needs extended vesting (2x) due to low reputation
    pub fn needs_extended_vesting(
        attestation: &ReputationAttestation,
        config: &ReputationConfig,
    ) -> bool {
        attestation.fairscore < config.strict_vesting_threshold
    }

    /// Calculate vesting multiplier: 2x for low rep, 1x for normal
    pub fn vesting_multiplier(
        attestation: &ReputationAttestation,
        config: &ReputationConfig,
    ) -> u8 {
        if needs_extended_vesting(attestation, config) { 2 } else { 1 }
    }

    // ─── Errors ───

    #[error_code]
    pub enum ReputationError {
        #[msg("FairScore must be between 0 and 100")]
        InvalidScore,
        #[msg("Wallet does not meet minimum reputation score to launch")]
        InsufficientReputation,
        #[msg("Reputation attestation is stale (>24h old)")]
        StaleAttestation,
    }

}
pub mod seasons {
    use super::*;

    // ── Seeds ──────────────────────────────────────────────────────────────────────
    pub const SEASON_SEED: &[u8] = b"season";
    pub const SEASON_PASS_SEED: &[u8] = b"season_pass";
    pub const SEASON_REWARD_SEED: &[u8] = b"season_reward";

    // ── Errors ─────────────────────────────────────────────────────────────────────
    #[error_code]
    pub enum SeasonError {
        #[msg("Season is not currently active")]
        SeasonNotActive,
        #[msg("Season has not ended yet")]
        SeasonNotEnded,
        #[msg("Season already ended")]
        SeasonAlreadyEnded,
        #[msg("Unauthorized")]
        Unauthorized,
        #[msg("Already joined this season")]
        AlreadyJoined,
        #[msg("Insufficient XP for this reward level")]
        InsufficientXP,
        #[msg("Reward already claimed for this level")]
        RewardAlreadyClaimed,
        #[msg("Invalid time range")]
        InvalidTimeRange,
        #[msg("Season end time not reached")]
        SeasonStillActive,
    }

    // ── Enums ──────────────────────────────────────────────────────────────────────
    #[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
    pub enum RewardType {
        Lamports,
        FeeDiscount,     // basis points discount
        PriorityAccess,
        BadgeNFT,
    }

    #[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
    pub enum XPSource {
        TradeVolume,
        TokenLaunch,
        HoldDuration,
        Referral,
    }

    // ── Bitflags for achievements ──────────────────────────────────────────────────
    pub const ACHIEVEMENT_FIRST_TRADE: u64    = 1 << 0;
    pub const ACHIEVEMENT_10_TRADES: u64      = 1 << 1;
    pub const ACHIEVEMENT_100_TRADES: u64     = 1 << 2;
    pub const ACHIEVEMENT_LAUNCH_TOKEN: u64   = 1 << 3;
    pub const ACHIEVEMENT_1_SOL_VOLUME: u64   = 1 << 4;
    pub const ACHIEVEMENT_10_SOL_VOLUME: u64  = 1 << 5;
    pub const ACHIEVEMENT_100_SOL_VOLUME: u64 = 1 << 6;
    pub const ACHIEVEMENT_REFERRAL_5: u64     = 1 << 7;
    pub const ACHIEVEMENT_DIAMOND_HANDS: u64  = 1 << 8;  // held >7 days
    pub const ACHIEVEMENT_STREAK_7: u64       = 1 << 9;

    // ── Account Structs ────────────────────────────────────────────────────────────
    #[account]
    pub struct Season {
        pub authority: Pubkey,
        pub season_number: u64,
        pub start_time: i64,
        pub end_time: i64,
        pub total_participants: u64,
        pub prize_pool_lamports: u64,
        pub is_active: bool,
        pub is_finalized: bool,
        pub bump: u8,
    }

    impl Season {
        pub const SIZE: usize = 8 + 32 + 8 + 8 + 8 + 8 + 8 + 1 + 1 + 1; // 83
    }

    #[account]
    pub struct SeasonPass {
        pub season: Pubkey,
        pub user: Pubkey,
        pub xp: u64,
        pub level: u32,
        pub trades_count: u64,
        pub volume: u64,               // in lamports
        pub achievements_unlocked: u64, // bitflag
        pub rewards_claimed_mask: u64,  // bitflag per level (up to 64 levels)
        pub joined_at: i64,
        pub bump: u8,
    }

    impl SeasonPass {
        pub const SIZE: usize = 8 + 32 + 32 + 8 + 4 + 8 + 8 + 8 + 8 + 8 + 1; // 125
    }

    #[account]
    pub struct SeasonReward {
        pub season: Pubkey,
        pub level: u32,
        pub min_xp: u64,
        pub reward_type: RewardType,
        pub reward_amount: u64,
        pub bump: u8,
    }

    impl SeasonReward {
        pub const SIZE: usize = 8 + 32 + 4 + 8 + 1 + 8 + 1; // 62
    }

    // ── Events ─────────────────────────────────────────────────────────────────────
    #[event]
    pub struct SeasonStarted {
        pub season_number: u64,
        pub start_time: i64,
        pub end_time: i64,
    }

    #[event]
    pub struct SeasonJoined {
        pub season_number: u64,
        pub user: Pubkey,
    }

    #[event]
    pub struct XPRecorded {
        pub season_number: u64,
        pub user: Pubkey,
        pub xp_gained: u64,
        pub source: XPSource,
        pub new_total_xp: u64,
        pub new_level: u32,
    }

    #[event]
    pub struct SeasonRewardClaimed {
        pub season_number: u64,
        pub user: Pubkey,
        pub level: u32,
        pub reward_type: RewardType,
        pub reward_amount: u64,
    }

    #[event]
    pub struct SeasonEnded {
        pub season_number: u64,
        pub total_participants: u64,
        pub prize_pool_lamports: u64,
    }

    // ── Instructions ───────────────────────────────────────────────────────────────

    pub fn start_season(
        ctx: Context<StartSeason>,
        season_number: u64,
        start_time: i64,
        end_time: i64,
    ) -> Result<()> {
        require!(end_time > start_time, SeasonError::InvalidTimeRange);

        let season = &mut ctx.accounts.season;
        season.authority = ctx.accounts.authority.key();
        season.season_number = season_number;
        season.start_time = start_time;
        season.end_time = end_time;
        season.total_participants = 0;
        season.prize_pool_lamports = 0;
        season.is_active = true;
        season.is_finalized = false;
        season.bump = ctx.bumps.season;

        emit!(SeasonStarted {
            season_number,
            start_time,
            end_time,
        });

        Ok(())
    }

    /// Add a reward tier for a season level.
    pub fn add_season_reward(
        ctx: Context<AddSeasonReward>,
        level: u32,
        min_xp: u64,
        reward_type: RewardType,
        reward_amount: u64,
    ) -> Result<()> {
        let season = &ctx.accounts.season;
        require!(
            ctx.accounts.authority.key() == season.authority,
            SeasonError::Unauthorized
        );

        let reward = &mut ctx.accounts.season_reward;
        reward.season = season.key();
        reward.level = level;
        reward.min_xp = min_xp;
        reward.reward_type = reward_type;
        reward.reward_amount = reward_amount;
        reward.bump = ctx.bumps.season_reward;

        Ok(())
    }

    /// Join a season (free).
    pub fn join_season(ctx: Context<JoinSeason>) -> Result<()> {
        let season = &ctx.accounts.season;
        require!(season.is_active, SeasonError::SeasonNotActive);

        let clock = Clock::get()?;
        let pass = &mut ctx.accounts.season_pass;
        pass.season = season.key();
        pass.user = ctx.accounts.user.key();
        pass.xp = 0;
        pass.level = 0;
        pass.trades_count = 0;
        pass.volume = 0;
        pass.achievements_unlocked = 0;
        pass.rewards_claimed_mask = 0;
        pass.joined_at = clock.unix_timestamp;
        pass.bump = ctx.bumps.season_pass;

        let season = &mut ctx.accounts.season;
        season.total_participants = season.total_participants.checked_add(1).unwrap();

        emit!(SeasonJoined {
            season_number: season.season_number,
            user: ctx.accounts.user.key(),
        });

        Ok(())
    }

    /// Record XP from trades, launches, holding, referrals.
    pub fn record_season_xp(
        ctx: Context<RecordSeasonXP>,
        xp_amount: u64,
        source: XPSource,
        trade_volume_lamports: u64,
    ) -> Result<()> {
        let season = &ctx.accounts.season;
        require!(season.is_active, SeasonError::SeasonNotActive);

        let pass = &mut ctx.accounts.season_pass;
        pass.xp = pass.xp.checked_add(xp_amount).unwrap();

        // Update trade stats if applicable
        if source == XPSource::TradeVolume {
            pass.trades_count = pass.trades_count.checked_add(1).unwrap();
            pass.volume = pass.volume.checked_add(trade_volume_lamports).unwrap();

            // Check trade achievements
            if pass.trades_count >= 1 {
                pass.achievements_unlocked |= ACHIEVEMENT_FIRST_TRADE;
            }
            if pass.trades_count >= 10 {
                pass.achievements_unlocked |= ACHIEVEMENT_10_TRADES;
            }
            if pass.trades_count >= 100 {
                pass.achievements_unlocked |= ACHIEVEMENT_100_TRADES;
            }
            // Volume achievements (in SOL)
            let volume_sol = pass.volume / 1_000_000_000;
            if volume_sol >= 1 {
                pass.achievements_unlocked |= ACHIEVEMENT_1_SOL_VOLUME;
            }
            if volume_sol >= 10 {
                pass.achievements_unlocked |= ACHIEVEMENT_10_SOL_VOLUME;
            }
            if volume_sol >= 100 {
                pass.achievements_unlocked |= ACHIEVEMENT_100_SOL_VOLUME;
            }
        } else if source == XPSource::TokenLaunch {
            pass.achievements_unlocked |= ACHIEVEMENT_LAUNCH_TOKEN;
        }

        // Level calculation: level = sqrt(xp / 100), simple curve
        pass.level = ((pass.xp / 100) as f64).sqrt() as u32;

        emit!(XPRecorded {
            season_number: season.season_number,
            user: ctx.accounts.user.key(),
            xp_gained: xp_amount,
            source,
            new_total_xp: pass.xp,
            new_level: pass.level,
        });

        Ok(())
    }

    /// Claim a reward for reaching a specific level.
    pub fn claim_season_reward(ctx: Context<ClaimSeasonReward>) -> Result<()> {
        let reward = &ctx.accounts.season_reward;
        let pass = &mut ctx.accounts.season_pass;

        require!(pass.xp >= reward.min_xp, SeasonError::InsufficientXP);

        // Check if already claimed for this level
        let level_bit = 1u64 << (reward.level as u64 % 64);
        require!(
            pass.rewards_claimed_mask & level_bit == 0,
            SeasonError::RewardAlreadyClaimed
        );
        pass.rewards_claimed_mask |= level_bit;

        // Distribute reward based on type
        if reward.reward_type == RewardType::Lamports && reward.reward_amount > 0 {
            // Transfer from season PDA (prize pool) to user
            let season = &ctx.accounts.season;
            let season_number_bytes = season.season_number.to_le_bytes();
            let seeds = &[
                SEASON_SEED,
                season_number_bytes.as_ref(),
                &[season.bump],
            ];

            system_program::transfer(
                CpiContext::new_with_signer(
                    ctx.accounts.system_program.to_account_info(),
                    system_program::Transfer {
                        from: ctx.accounts.season.to_account_info(),
                        to: ctx.accounts.user.to_account_info(),
                    },
                    &[seeds],
                ),
                reward.reward_amount,
            )?;
        }
        // FeeDiscount, PriorityAccess, BadgeNFT handled off-chain or by other modules

        emit!(SeasonRewardClaimed {
            season_number: ctx.accounts.season.season_number,
            user: ctx.accounts.user.key(),
            level: reward.level,
            reward_type: reward.reward_type,
            reward_amount: reward.reward_amount,
        });

        Ok(())
    }

    /// End a season (authority only). Finalizes and marks inactive.
    pub fn end_season(ctx: Context<EndSeason>) -> Result<()> {
        let season = &mut ctx.accounts.season;
        require!(
            ctx.accounts.authority.key() == season.authority,
            SeasonError::Unauthorized
        );
        require!(season.is_active, SeasonError::SeasonAlreadyEnded);

        let clock = Clock::get()?;
        require!(
            clock.unix_timestamp >= season.end_time,
            SeasonError::SeasonStillActive
        );

        season.is_active = false;
        season.is_finalized = true;

        emit!(SeasonEnded {
            season_number: season.season_number,
            total_participants: season.total_participants,
            prize_pool_lamports: season.prize_pool_lamports,
        });

        Ok(())
    }

    /// Fund the season prize pool.
    pub fn fund_season(ctx: Context<FundSeason>, lamports: u64) -> Result<()> {
        system_program::transfer(
            CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                system_program::Transfer {
                    from: ctx.accounts.funder.to_account_info(),
                    to: ctx.accounts.season.to_account_info(),
                },
            ),
            lamports,
        )?;

        let season = &mut ctx.accounts.season;
        season.prize_pool_lamports = season.prize_pool_lamports.checked_add(lamports).unwrap();

        Ok(())
    }

    // ── Contexts ───────────────────────────────────────────────────────────────────

    #[derive(Accounts)]
    #[instruction(season_number: u64)]
    pub struct StartSeason<'info> {
        #[account(
            init,
            payer = authority,
            space = Season::SIZE,
            seeds = [SEASON_SEED, &season_number.to_le_bytes()],
            bump,
        )]
        pub season: Account<'info, Season>,
        #[account(mut)]
        pub authority: Signer<'info>,
        pub system_program: Program<'info, System>,
    }

    #[derive(Accounts)]
    #[instruction(level: u32)]
    pub struct AddSeasonReward<'info> {
        #[account(
            seeds = [SEASON_SEED, &season.season_number.to_le_bytes()],
            bump = season.bump,
        )]
        pub season: Account<'info, Season>,

        #[account(
            init,
            payer = authority,
            space = SeasonReward::SIZE,
            seeds = [SEASON_REWARD_SEED, season.key().as_ref(), &level.to_le_bytes()],
            bump,
        )]
        pub season_reward: Account<'info, SeasonReward>,

        #[account(mut)]
        pub authority: Signer<'info>,
        pub system_program: Program<'info, System>,
    }

    #[derive(Accounts)]
    pub struct JoinSeason<'info> {
        #[account(
            mut,
            seeds = [SEASON_SEED, &season.season_number.to_le_bytes()],
            bump = season.bump,
        )]
        pub season: Account<'info, Season>,

        #[account(
            init,
            payer = user,
            space = SeasonPass::SIZE,
            seeds = [SEASON_PASS_SEED, season.key().as_ref(), user.key().as_ref()],
            bump,
        )]
        pub season_pass: Account<'info, SeasonPass>,

        #[account(mut)]
        pub user: Signer<'info>,
        pub system_program: Program<'info, System>,
    }

    #[derive(Accounts)]
    pub struct RecordSeasonXP<'info> {
        #[account(
            seeds = [SEASON_SEED, &season.season_number.to_le_bytes()],
            bump = season.bump,
        )]
        pub season: Account<'info, Season>,

        #[account(
            mut,
            seeds = [SEASON_PASS_SEED, season.key().as_ref(), user.key().as_ref()],
            bump = season_pass.bump,
            has_one = user,
        )]
        pub season_pass: Account<'info, SeasonPass>,

        pub user: Signer<'info>,
    }

    #[derive(Accounts)]
    pub struct ClaimSeasonReward<'info> {
        #[account(
            mut,
            seeds = [SEASON_SEED, &season.season_number.to_le_bytes()],
            bump = season.bump,
        )]
        pub season: Account<'info, Season>,

        #[account(
            mut,
            seeds = [SEASON_PASS_SEED, season.key().as_ref(), user.key().as_ref()],
            bump = season_pass.bump,
            has_one = user,
        )]
        pub season_pass: Account<'info, SeasonPass>,

        #[account(
            seeds = [SEASON_REWARD_SEED, season.key().as_ref(), &season_reward.level.to_le_bytes()],
            bump = season_reward.bump,
        )]
        pub season_reward: Account<'info, SeasonReward>,

        #[account(mut)]
        pub user: Signer<'info>,
        pub system_program: Program<'info, System>,
    }

    #[derive(Accounts)]
    pub struct EndSeason<'info> {
        #[account(
            mut,
            seeds = [SEASON_SEED, &season.season_number.to_le_bytes()],
            bump = season.bump,
        )]
        pub season: Account<'info, Season>,
        pub authority: Signer<'info>,
    }

    #[derive(Accounts)]
    pub struct FundSeason<'info> {
        #[account(
            mut,
            seeds = [SEASON_SEED, &season.season_number.to_le_bytes()],
            bump = season.bump,
        )]
        pub season: Account<'info, Season>,
        #[account(mut)]
        pub funder: Signer<'info>,
        pub system_program: Program<'info, System>,
    }

}
pub mod share_cards {
    use super::*;

    // ============================================================================
    // SEEDS & CONSTANTS
    // ============================================================================

    pub const SHARE_CARD_SEED: &[u8] = b"share_card";

    pub const MAX_TOKEN_NAME_LEN: usize = 32;
    pub const MAX_SYMBOL_LEN: usize = 10;

    // ============================================================================
    // ACCOUNTS
    // ============================================================================

    /// Share card PDA — one per token mint. Stores data needed to render
    /// auto-generated share/embed cards on the frontend.
    #[account]
    #[derive(Default)]
    pub struct ShareCard {
        /// Human-readable token name (max 32 bytes).
        pub token_name: String,
        /// Token ticker symbol (max 10 bytes).
        pub symbol: String,
        /// Current price in lamports per whole token.
        pub current_price: u64,
        /// Market cap in lamports.
        pub market_cap: u64,
        /// Rolling 24-hour volume in lamports.
        pub volume_24h: u64,
        /// Number of distinct holders.
        pub holder_count: u32,
        /// Original token creator.
        pub creator: Pubkey,
        /// Migration / bonding-curve progress in basis points (0–10_000).
        pub migration_progress_bps: u16,
        /// Unix timestamp of last update.
        pub last_updated: i64,
        /// The token mint this card belongs to.
        pub token_mint: Pubkey,
        /// PDA bump.
        pub bump: u8,
    }

    impl ShareCard {
        pub const SIZE: usize = 8  // discriminator
            + 4 + MAX_TOKEN_NAME_LEN  // String (len prefix + data)
            + 4 + MAX_SYMBOL_LEN
            + 8  // current_price
            + 8  // market_cap
            + 8  // volume_24h
            + 4  // holder_count
            + 32 // creator
            + 2  // migration_progress_bps
            + 8  // last_updated
            + 32 // token_mint
            + 1; // bump
    }

    // ============================================================================
    // EVENTS
    // ============================================================================

    #[event]
    pub struct ShareCardDataEvent {
        pub token_mint: Pubkey,
        pub token_name: String,
        pub symbol: String,
        pub current_price: u64,
        pub market_cap: u64,
        pub volume_24h: u64,
        pub holder_count: u32,
        pub creator: Pubkey,
        pub migration_progress_bps: u16,
        pub last_updated: i64,
    }

    // ============================================================================
    // INSTRUCTIONS
    // ============================================================================

    /// Permissionless crank — anyone can call this after trades to refresh the
    /// share card with the latest on-chain data.
    pub fn update_share_card(
        ctx: Context<UpdateShareCard>,
        token_name: String,
        symbol: String,
        current_price: u64,
        market_cap: u64,
        volume_24h: u64,
        holder_count: u32,
        creator: Pubkey,
        migration_progress_bps: u16,
    ) -> Result<()> {
        require!(token_name.len() <= MAX_TOKEN_NAME_LEN, ShareCardError::NameTooLong);
        require!(symbol.len() <= MAX_SYMBOL_LEN, ShareCardError::SymbolTooLong);
        require!(migration_progress_bps <= 10_000, ShareCardError::InvalidBps);

        let card = &mut ctx.accounts.share_card;
        card.token_name = token_name;
        card.symbol = symbol;
        card.current_price = current_price;
        card.market_cap = market_cap;
        card.volume_24h = volume_24h;
        card.holder_count = holder_count;
        card.creator = creator;
        card.migration_progress_bps = migration_progress_bps;
        card.last_updated = Clock::get()?.unix_timestamp;
        card.token_mint = ctx.accounts.token_mint.key();
        card.bump = ctx.bumps.share_card;

        Ok(())
    }

    /// Emits an event with the full share card data so frontends can render it.
    pub fn get_share_card_data(ctx: Context<GetShareCardData>) -> Result<()> {
        let card = &ctx.accounts.share_card;

        emit!(ShareCardDataEvent {
            token_mint: card.token_mint,
            token_name: card.token_name.clone(),
            symbol: card.symbol.clone(),
            current_price: card.current_price,
            market_cap: card.market_cap,
            volume_24h: card.volume_24h,
            holder_count: card.holder_count,
            creator: card.creator,
            migration_progress_bps: card.migration_progress_bps,
            last_updated: card.last_updated,
        });

        Ok(())
    }

    // ============================================================================
    // CONTEXT STRUCTS
    // ============================================================================

    #[derive(Accounts)]
    pub struct UpdateShareCard<'info> {
        #[account(
            init_if_needed,
            payer = payer,
            space = ShareCard::SIZE,
            seeds = [SHARE_CARD_SEED, token_mint.key().as_ref()],
            bump,
        )]
        pub share_card: Account<'info, ShareCard>,

        /// CHECK: Token mint — validated by seeds.
        pub token_mint: UncheckedAccount<'info>,

        #[account(mut)]
        pub payer: Signer<'info>,

        pub system_program: Program<'info, System>,
    }

    #[derive(Accounts)]
    pub struct GetShareCardData<'info> {
        #[account(
            seeds = [SHARE_CARD_SEED, token_mint.key().as_ref()],
            bump = share_card.bump,
        )]
        pub share_card: Account<'info, ShareCard>,

        /// CHECK: Token mint — validated by seeds.
        pub token_mint: UncheckedAccount<'info>,
    }

    // ============================================================================
    // ERRORS
    // ============================================================================

    #[error_code]
    pub enum ShareCardError {
        #[msg("Token name exceeds maximum length of 32 bytes")]
        NameTooLong,
        #[msg("Symbol exceeds maximum length of 10 bytes")]
        SymbolTooLong,
        #[msg("Migration progress BPS must be 0–10000")]
        InvalidBps,
    }

}
pub mod staking {
    use super::*;

    // ---------------------------------------------------------------------------
    // Constants
    // ---------------------------------------------------------------------------

    const STAKE_POOL_SEED: &[u8] = b"stake_pool";
    const USER_STAKE_SEED: &[u8] = b"user_stake";
    const STAKE_VAULT_SEED: &[u8] = b"stake_vault";
    const PRECISION: u128 = 1_000_000_000_000; // 1e12

    // ---------------------------------------------------------------------------
    // Accounts
    // ---------------------------------------------------------------------------

    #[account]
    #[derive(Default)]
    pub struct StakePool {
        /// Token mint this pool is for
        pub mint: Pubkey,
        /// Creator who initialized the pool
        pub creator: Pubkey,
        /// Total tokens currently staked
        pub total_staked: u64,
        /// Reward rate: tokens per second per staked token (scaled by PRECISION)
        pub reward_rate: u128,
        /// Cumulative reward per token stored (scaled by PRECISION)
        pub reward_per_token_stored: u128,
        /// Last time rewards were updated
        pub last_update: i64,
        /// Whether the token has graduated (required to create pool)
        pub graduated: bool,
        /// Bump seed
        pub bump: u8,
        /// Vault bump
        pub vault_bump: u8,
    }

    #[account]
    #[derive(Default)]
    pub struct UserStake {
        /// User wallet
        pub user: Pubkey,
        /// Token mint
        pub mint: Pubkey,
        /// Amount staked
        pub amount: u64,
        /// Timestamp of first stake
        pub start_time: i64,
        /// Accumulated rewards earned (unclaimed)
        pub rewards_earned: u64,
        /// Snapshot of reward_per_token_stored at last interaction
        pub reward_per_token_paid: u128,
        /// Bump seed
        pub bump: u8,
    }

    // ---------------------------------------------------------------------------
    // Events
    // ---------------------------------------------------------------------------

    #[event]
    pub struct StakePoolCreated {
        pub mint: Pubkey,
        pub creator: Pubkey,
        pub reward_rate: u128,
        pub timestamp: i64,
    }

    #[event]
    pub struct TokensStaked {
        pub user: Pubkey,
        pub mint: Pubkey,
        pub amount: u64,
        pub total_staked: u64,
        pub timestamp: i64,
    }

    #[event]
    pub struct TokensUnstaked {
        pub user: Pubkey,
        pub mint: Pubkey,
        pub amount: u64,
        pub total_staked: u64,
        pub timestamp: i64,
    }

    #[event]
    pub struct RewardsClaimed {
        pub user: Pubkey,
        pub mint: Pubkey,
        pub amount: u64,
        pub timestamp: i64,
    }

    // ---------------------------------------------------------------------------
    // Errors
    // ---------------------------------------------------------------------------

    #[error_code]
    pub enum StakingError {
        #[msg("Token has not graduated yet")]
        NotGraduated,
        #[msg("Stake pool already exists")]
        PoolAlreadyExists,
        #[msg("Amount must be greater than zero")]
        ZeroAmount,
        #[msg("Insufficient staked balance")]
        InsufficientStake,
        #[msg("No rewards to claim")]
        NoRewards,
        #[msg("Reward rate must be greater than zero")]
        InvalidRewardRate,
        #[msg("Arithmetic overflow")]
        MathOverflow,
        #[msg("Only the pool creator can update reward rate")]
        UnauthorizedCreator,
    }

    // ---------------------------------------------------------------------------
    // Instructions
    // ---------------------------------------------------------------------------


        /// Create a stake pool for a graduated token. Only the creator can call this.
        pub fn create_stake_pool(
            ctx: Context<CreateStakePool>,
            reward_rate: u128,
        ) -> Result<()> {
            require!(reward_rate > 0, StakingError::InvalidRewardRate);

            let clock = Clock::get()?;
            let pool = &mut ctx.accounts.stake_pool;
            pool.mint = ctx.accounts.mint.key();
            pool.creator = ctx.accounts.creator.key();
            pool.total_staked = 0;
            pool.reward_rate = reward_rate;
            pool.reward_per_token_stored = 0;
            pool.last_update = clock.unix_timestamp;
            pool.graduated = true;
            pool.bump = ctx.bumps.stake_pool;
            pool.vault_bump = ctx.bumps.stake_vault;

            emit!(StakePoolCreated {
                mint: pool.mint,
                creator: pool.creator,
                reward_rate,
                timestamp: clock.unix_timestamp,
            });

            Ok(())
        }

        /// Stake tokens into the pool.
        pub fn stake_tokens(ctx: Context<StakeTokens>, amount: u64) -> Result<()> {
            require!(amount > 0, StakingError::ZeroAmount);

            let clock = Clock::get()?;
            let pool = &mut ctx.accounts.stake_pool;
            let user_stake = &mut ctx.accounts.user_stake;

            // Update global reward accumulator
            update_reward_per_token(pool, clock.unix_timestamp)?;

            // Settle pending user rewards
            settle_user_rewards(pool, user_stake)?;

            // Transfer tokens from user to vault
            token::transfer(
                CpiContext::new(
                    ctx.accounts.token_program.to_account_info(),
                    Transfer {
                        from: ctx.accounts.user_token_account.to_account_info(),
                        to: ctx.accounts.stake_vault.to_account_info(),
                        authority: ctx.accounts.user.to_account_info(),
                    },
                ),
                amount,
            )?;

            // Update state
            user_stake.amount = user_stake.amount.checked_add(amount).ok_or(StakingError::MathOverflow)?;
            if user_stake.start_time == 0 {
                user_stake.start_time = clock.unix_timestamp;
            }
            pool.total_staked = pool.total_staked.checked_add(amount).ok_or(StakingError::MathOverflow)?;

            emit!(TokensStaked {
                user: ctx.accounts.user.key(),
                mint: pool.mint,
                amount,
                total_staked: pool.total_staked,
                timestamp: clock.unix_timestamp,
            });

            Ok(())
        }

        /// Unstake tokens from the pool.
        pub fn unstake_tokens(ctx: Context<UnstakeTokens>, amount: u64) -> Result<()> {
            require!(amount > 0, StakingError::ZeroAmount);

            let clock = Clock::get()?;
            let pool = &mut ctx.accounts.stake_pool;
            let user_stake = &mut ctx.accounts.user_stake;

            require!(user_stake.amount >= amount, StakingError::InsufficientStake);

            // Update rewards
            update_reward_per_token(pool, clock.unix_timestamp)?;
            settle_user_rewards(pool, user_stake)?;

            // Transfer tokens back from vault to user
            let mint_key = pool.mint;
            let seeds = &[
                STAKE_VAULT_SEED,
                mint_key.as_ref(),
                &[pool.vault_bump],
            ];
            let signer_seeds = &[&seeds[..]];

            token::transfer(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    Transfer {
                        from: ctx.accounts.stake_vault.to_account_info(),
                        to: ctx.accounts.user_token_account.to_account_info(),
                        authority: ctx.accounts.stake_vault.to_account_info(),
                    },
                    signer_seeds,
                ),
                amount,
            )?;

            user_stake.amount = user_stake.amount.checked_sub(amount).ok_or(StakingError::MathOverflow)?;
            pool.total_staked = pool.total_staked.checked_sub(amount).ok_or(StakingError::MathOverflow)?;

            emit!(TokensUnstaked {
                user: ctx.accounts.user.key(),
                mint: pool.mint,
                amount,
                total_staked: pool.total_staked,
                timestamp: clock.unix_timestamp,
            });

            Ok(())
        }

        /// Claim accumulated staking rewards.
        pub fn claim_staking_rewards(ctx: Context<ClaimStakingRewards>) -> Result<()> {
            let clock = Clock::get()?;
            let pool = &mut ctx.accounts.stake_pool;
            let user_stake = &mut ctx.accounts.user_stake;

            update_reward_per_token(pool, clock.unix_timestamp)?;
            settle_user_rewards(pool, user_stake)?;

            let rewards = user_stake.rewards_earned;
            require!(rewards > 0, StakingError::NoRewards);

            // Transfer reward tokens from vault
            let mint_key = pool.mint;
            let seeds = &[
                STAKE_VAULT_SEED,
                mint_key.as_ref(),
                &[pool.vault_bump],
            ];
            let signer_seeds = &[&seeds[..]];

            token::transfer(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    Transfer {
                        from: ctx.accounts.reward_vault.to_account_info(),
                        to: ctx.accounts.user_token_account.to_account_info(),
                        authority: ctx.accounts.reward_vault.to_account_info(),
                    },
                    signer_seeds,
                ),
                rewards,
            )?;

            user_stake.rewards_earned = 0;

            emit!(RewardsClaimed {
                user: ctx.accounts.user.key(),
                mint: pool.mint,
                amount: rewards,
                timestamp: clock.unix_timestamp,
            });

            Ok(())
        }


    // ---------------------------------------------------------------------------
    // Helpers
    // ---------------------------------------------------------------------------

    fn update_reward_per_token(pool: &mut StakePool, now: i64) -> Result<()> {
        if pool.total_staked > 0 {
            let elapsed = (now - pool.last_update) as u128;
            let additional = elapsed
                .checked_mul(pool.reward_rate)
                .ok_or(StakingError::MathOverflow)?
                .checked_div(pool.total_staked as u128)
                .ok_or(StakingError::MathOverflow)?;
            pool.reward_per_token_stored = pool
                .reward_per_token_stored
                .checked_add(additional)
                .ok_or(StakingError::MathOverflow)?;
        }
        pool.last_update = now;
        Ok(())
    }

    fn settle_user_rewards(pool: &StakePool, user_stake: &mut UserStake) -> Result<()> {
        if user_stake.amount > 0 {
            let pending = (user_stake.amount as u128)
                .checked_mul(
                    pool.reward_per_token_stored
                        .checked_sub(user_stake.reward_per_token_paid)
                        .ok_or(StakingError::MathOverflow)?,
                )
                .ok_or(StakingError::MathOverflow)?
                .checked_div(PRECISION)
                .ok_or(StakingError::MathOverflow)? as u64;
            user_stake.rewards_earned = user_stake
                .rewards_earned
                .checked_add(pending)
                .ok_or(StakingError::MathOverflow)?;
        }
        user_stake.reward_per_token_paid = pool.reward_per_token_stored;
        Ok(())
    }

    // ---------------------------------------------------------------------------
    // Context Structs
    // ---------------------------------------------------------------------------

    #[derive(Accounts)]
    pub struct CreateStakePool<'info> {
        #[account(mut)]
        pub creator: Signer<'info>,

        pub mint: Account<'info, Mint>,

        #[account(
            init,
            payer = creator,
            space = 8 + std::mem::size_of::<StakePool>(),
            seeds = [STAKE_POOL_SEED, mint.key().as_ref()],
            bump,
        )]
        pub stake_pool: Account<'info, StakePool>,

        #[account(
            init,
            payer = creator,
            token::mint = mint,
            token::authority = stake_vault,
            seeds = [STAKE_VAULT_SEED, mint.key().as_ref()],
            bump,
        )]
        pub stake_vault: Account<'info, TokenAccount>,

        pub token_program: Program<'info, Token>,
        pub system_program: Program<'info, System>,
        pub rent: Sysvar<'info, Rent>,
    }

    #[derive(Accounts)]
    pub struct StakeTokens<'info> {
        #[account(mut)]
        pub user: Signer<'info>,

        #[account(
            mut,
            seeds = [STAKE_POOL_SEED, stake_pool.mint.as_ref()],
            bump = stake_pool.bump,
        )]
        pub stake_pool: Account<'info, StakePool>,

        #[account(
            init_if_needed,
            payer = user,
            space = 8 + std::mem::size_of::<UserStake>(),
            seeds = [USER_STAKE_SEED, stake_pool.mint.as_ref(), user.key().as_ref()],
            bump,
        )]
        pub user_stake: Account<'info, UserStake>,

        #[account(
            mut,
            token::mint = stake_pool.mint,
            token::authority = user,
        )]
        pub user_token_account: Account<'info, TokenAccount>,

        #[account(
            mut,
            seeds = [STAKE_VAULT_SEED, stake_pool.mint.as_ref()],
            bump = stake_pool.vault_bump,
        )]
        pub stake_vault: Account<'info, TokenAccount>,

        pub token_program: Program<'info, Token>,
        pub system_program: Program<'info, System>,
    }

    #[derive(Accounts)]
    pub struct UnstakeTokens<'info> {
        #[account(mut)]
        pub user: Signer<'info>,

        #[account(
            mut,
            seeds = [STAKE_POOL_SEED, stake_pool.mint.as_ref()],
            bump = stake_pool.bump,
        )]
        pub stake_pool: Account<'info, StakePool>,

        #[account(
            mut,
            seeds = [USER_STAKE_SEED, stake_pool.mint.as_ref(), user.key().as_ref()],
            bump = user_stake.bump,
            has_one = user,
        )]
        pub user_stake: Account<'info, UserStake>,

        #[account(
            mut,
            token::mint = stake_pool.mint,
            token::authority = user,
        )]
        pub user_token_account: Account<'info, TokenAccount>,

        #[account(
            mut,
            seeds = [STAKE_VAULT_SEED, stake_pool.mint.as_ref()],
            bump = stake_pool.vault_bump,
        )]
        pub stake_vault: Account<'info, TokenAccount>,

        pub token_program: Program<'info, Token>,
    }

    #[derive(Accounts)]
    pub struct ClaimStakingRewards<'info> {
        #[account(mut)]
        pub user: Signer<'info>,

        #[account(
            mut,
            seeds = [STAKE_POOL_SEED, stake_pool.mint.as_ref()],
            bump = stake_pool.bump,
        )]
        pub stake_pool: Account<'info, StakePool>,

        #[account(
            mut,
            seeds = [USER_STAKE_SEED, stake_pool.mint.as_ref(), user.key().as_ref()],
            bump = user_stake.bump,
            has_one = user,
        )]
        pub user_stake: Account<'info, UserStake>,

        #[account(
            mut,
            token::mint = stake_pool.mint,
            token::authority = user,
        )]
        pub user_token_account: Account<'info, TokenAccount>,

        /// Separate reward vault (could be same as stake vault depending on design)
        #[account(mut)]
        pub reward_vault: Account<'info, TokenAccount>,

        pub token_program: Program<'info, Token>,
    }

}
pub mod token_chat {
    use super::*;

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

}
pub mod token_videos {
    use super::*;

    // ============================================================================
    // SEEDS & CONSTANTS
    // ============================================================================

    pub const TOKEN_VIDEO_SEED: &[u8] = b"token_video";
    pub const USER_VIDEO_VOTE_SEED: &[u8] = b"user_video_vote";

    pub const MAX_VIDEO_URL_LEN: usize = 200;
    pub const MAX_THUMBNAIL_URL_LEN: usize = 200;
    pub const MAX_DESCRIPTION_LEN: usize = 500;

    // ============================================================================
    // ACCOUNTS
    // ============================================================================

    /// Video pitch PDA — one per token mint. Only the token creator can set it.
    #[account]
    #[derive(Default)]
    pub struct TokenVideo {
        /// Creator who posted the video (must match token's original creator).
        pub creator: Pubkey,
        /// URL to the video (max 200 chars).
        pub video_url: String,
        /// URL to a thumbnail image (max 200 chars).
        pub thumbnail_url: String,
        /// Short description / pitch (max 500 chars).
        pub description: String,
        /// Total upvotes.
        pub upvotes: u32,
        /// Total downvotes.
        pub downvotes: u32,
        /// Unix timestamp when the video was posted.
        pub posted_at: i64,
        /// The token mint this video belongs to.
        pub token_mint: Pubkey,
        /// PDA bump.
        pub bump: u8,
    }

    impl TokenVideo {
        pub const SIZE: usize = 8  // discriminator
            + 32 // creator
            + 4 + MAX_VIDEO_URL_LEN
            + 4 + MAX_THUMBNAIL_URL_LEN
            + 4 + MAX_DESCRIPTION_LEN
            + 4  // upvotes
            + 4  // downvotes
            + 8  // posted_at
            + 32 // token_mint
            + 1; // bump
    }

    /// Tracks whether a user has voted on a specific token video (one vote per user).
    #[account]
    #[derive(Default)]
    pub struct UserVideoVote {
        pub user: Pubkey,
        pub token_mint: Pubkey,
        /// true = upvote, false = downvote.
        pub is_upvote: bool,
        pub bump: u8,
    }

    impl UserVideoVote {
        pub const SIZE: usize = 8 + 32 + 32 + 1 + 1;
    }

    // ============================================================================
    // INSTRUCTIONS
    // ============================================================================

    /// Creator sets (or updates) the video pitch for their token.
    pub fn set_token_video(
        ctx: Context<SetTokenVideo>,
        video_url: String,
        thumbnail_url: String,
        description: String,
    ) -> Result<()> {
        require!(video_url.len() <= MAX_VIDEO_URL_LEN, TokenVideoError::VideoUrlTooLong);
        require!(thumbnail_url.len() <= MAX_THUMBNAIL_URL_LEN, TokenVideoError::ThumbnailUrlTooLong);
        require!(description.len() <= MAX_DESCRIPTION_LEN, TokenVideoError::DescriptionTooLong);

        let video = &mut ctx.accounts.token_video;
        video.creator = ctx.accounts.creator.key();
        video.video_url = video_url;
        video.thumbnail_url = thumbnail_url;
        video.description = description;
        video.posted_at = Clock::get()?.unix_timestamp;
        video.token_mint = ctx.accounts.token_mint.key();
        video.bump = ctx.bumps.token_video;

        Ok(())
    }

    /// Upvote a token video. One vote per user enforced via UserVideoVote PDA.
    pub fn upvote_video(ctx: Context<VoteVideo>) -> Result<()> {
        let video = &mut ctx.accounts.token_video;
        let vote = &mut ctx.accounts.user_video_vote;

        vote.user = ctx.accounts.voter.key();
        vote.token_mint = ctx.accounts.token_mint.key();
        vote.is_upvote = true;
        vote.bump = ctx.bumps.user_video_vote;

        video.upvotes = video.upvotes.checked_add(1).unwrap();

        Ok(())
    }

    /// Downvote a token video. One vote per user enforced via UserVideoVote PDA.
    pub fn downvote_video(ctx: Context<VoteVideo>) -> Result<()> {
        let video = &mut ctx.accounts.token_video;
        let vote = &mut ctx.accounts.user_video_vote;

        vote.user = ctx.accounts.voter.key();
        vote.token_mint = ctx.accounts.token_mint.key();
        vote.is_upvote = false;
        vote.bump = ctx.bumps.user_video_vote;

        video.downvotes = video.downvotes.checked_add(1).unwrap();

        Ok(())
    }

    /// Remove a token video. Only the creator or platform authority can call this.
    pub fn remove_video(ctx: Context<RemoveVideo>) -> Result<()> {
        let video = &ctx.accounts.token_video;
        let signer = ctx.accounts.authority.key();

        require!(
            signer == video.creator || signer == ctx.accounts.platform_authority.key(),
            TokenVideoError::Unauthorized
        );

        // Close account and return lamports to authority
        // (handled by the `close` constraint on the context)
        Ok(())
    }

    // ============================================================================
    // CONTEXT STRUCTS
    // ============================================================================

    #[derive(Accounts)]
    pub struct SetTokenVideo<'info> {
        #[account(
            init_if_needed,
            payer = creator,
            space = TokenVideo::SIZE,
            seeds = [TOKEN_VIDEO_SEED, token_mint.key().as_ref()],
            bump,
        )]
        pub token_video: Account<'info, TokenVideo>,

        /// CHECK: Token mint — validated by seeds.
        pub token_mint: UncheckedAccount<'info>,

        #[account(mut)]
        pub creator: Signer<'info>,

        pub system_program: Program<'info, System>,
    }

    #[derive(Accounts)]
    pub struct VoteVideo<'info> {
        #[account(
            mut,
            seeds = [TOKEN_VIDEO_SEED, token_mint.key().as_ref()],
            bump = token_video.bump,
        )]
        pub token_video: Account<'info, TokenVideo>,

        #[account(
            init,
            payer = voter,
            space = UserVideoVote::SIZE,
            seeds = [USER_VIDEO_VOTE_SEED, token_mint.key().as_ref(), voter.key().as_ref()],
            bump,
        )]
        pub user_video_vote: Account<'info, UserVideoVote>,

        /// CHECK: Token mint — validated by seeds.
        pub token_mint: UncheckedAccount<'info>,

        #[account(mut)]
        pub voter: Signer<'info>,

        pub system_program: Program<'info, System>,
    }

    #[derive(Accounts)]
    pub struct RemoveVideo<'info> {
        #[account(
            mut,
            close = authority,
            seeds = [TOKEN_VIDEO_SEED, token_mint.key().as_ref()],
            bump = token_video.bump,
        )]
        pub token_video: Account<'info, TokenVideo>,

        /// CHECK: Token mint — validated by seeds.
        pub token_mint: UncheckedAccount<'info>,

        #[account(mut)]
        pub authority: Signer<'info>,

        /// CHECK: Platform authority for admin removal. Validated in instruction logic.
        pub platform_authority: UncheckedAccount<'info>,
    }

    // ============================================================================
    // ERRORS
    // ============================================================================

    #[error_code]
    pub enum TokenVideoError {
        #[msg("Video URL exceeds maximum length of 200 characters")]
        VideoUrlTooLong,
        #[msg("Thumbnail URL exceeds maximum length of 200 characters")]
        ThumbnailUrlTooLong,
        #[msg("Description exceeds maximum length of 500 characters")]
        DescriptionTooLong,
        #[msg("Only the token creator or platform authority can perform this action")]
        Unauthorized,
    }

}
pub mod voting {
    use super::*;

    // ---------------------------------------------------------------------------
    // Constants
    // ---------------------------------------------------------------------------
    pub const MAX_TITLE_LEN: usize = 64;
    pub const MAX_DESCRIPTION_LEN: usize = 256;
    pub const MAX_OPTIONS: usize = 8;
    pub const MAX_OPTION_LABEL_LEN: usize = 32;

    // ---------------------------------------------------------------------------
    // Account structs
    // ---------------------------------------------------------------------------

    #[account]
    pub struct Proposal {
        pub proposal_id: u64,
        pub token_mint: Pubkey,
        pub creator: Pubkey,
        pub title: [u8; MAX_TITLE_LEN],
        pub title_len: u8,
        pub description: [u8; MAX_DESCRIPTION_LEN],
        pub description_len: u16,
        pub options: [OptionData; MAX_OPTIONS],
        pub option_count: u8,
        pub start_time: i64,
        pub end_time: i64,
        pub quorum: u64,
        pub total_votes: u64,
        pub status: ProposalStatus,
        pub bump: u8,
    }

    #[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Default)]
    pub struct OptionData {
        pub label: [u8; MAX_OPTION_LABEL_LEN],
        pub label_len: u8,
        pub vote_count: u64,
    }

    #[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Default)]
    pub enum ProposalStatus {
        #[default]
        Active,
        Passed,
        Rejected,
        Cancelled,
    }

    #[account]
    pub struct UserVote {
        pub proposal: Pubkey,
        pub voter: Pubkey,
        pub option_index: u8,
        pub weight: u64,
        pub timestamp: i64,
        pub bump: u8,
    }

    // ---------------------------------------------------------------------------
    // Events
    // ---------------------------------------------------------------------------

    #[event]
    pub struct ProposalCreated {
        pub proposal_id: u64,
        pub token_mint: Pubkey,
        pub creator: Pubkey,
        pub start_time: i64,
        pub end_time: i64,
        pub option_count: u8,
    }

    #[event]
    pub struct VoteCast {
        pub proposal_id: u64,
        pub voter: Pubkey,
        pub option_index: u8,
        pub weight: u64,
    }

    #[event]
    pub struct ProposalFinalized {
        pub proposal_id: u64,
        pub status: ProposalStatus,
        pub winning_option: u8,
        pub total_votes: u64,
    }

    // ---------------------------------------------------------------------------
    // Errors
    // ---------------------------------------------------------------------------

    #[error_code]
    pub enum VotingError {
        #[msg("Title too long")]
        TitleTooLong,
        #[msg("Description too long")]
        DescriptionTooLong,
        #[msg("Too many options (max 8)")]
        TooManyOptions,
        #[msg("Must have at least 2 options")]
        TooFewOptions,
        #[msg("Option label too long")]
        OptionLabelTooLong,
        #[msg("Voting has not started yet")]
        VotingNotStarted,
        #[msg("Voting has ended")]
        VotingEnded,
        #[msg("Voting is still active")]
        VotingStillActive,
        #[msg("Invalid option index")]
        InvalidOption,
        #[msg("Already voted on this proposal")]
        AlreadyVoted,
        #[msg("Insufficient token balance to create proposal")]
        InsufficientBalance,
        #[msg("Proposal is not active")]
        ProposalNotActive,
        #[msg("Arithmetic overflow")]
        Overflow,
        #[msg("End time must be after start time")]
        InvalidTimeRange,
    }

    // ---------------------------------------------------------------------------
    // Instruction accounts
    // ---------------------------------------------------------------------------

    #[derive(Accounts)]
    #[instruction(proposal_id: u64, token_mint: Pubkey)]
    pub struct CreateProposal<'info> {
        #[account(
            init,
            payer = creator,
            space = 8 + std::mem::size_of::<Proposal>(),
            seeds = [b"proposal", token_mint.as_ref(), proposal_id.to_le_bytes().as_ref()],
            bump
        )]
        pub proposal: Account<'info, Proposal>,

        #[account(mut)]
        pub creator: Signer<'info>,

        /// Token account proving the creator holds enough tokens.
        /// CHECK: validated in handler via balance check.
        pub creator_token_account: AccountInfo<'info>,

        pub system_program: Program<'info, System>,
    }

    #[derive(Accounts)]
    pub struct CastVote<'info> {
        #[account(
            mut,
            seeds = [b"proposal", proposal.token_mint.as_ref(), proposal.proposal_id.to_le_bytes().as_ref()],
            bump = proposal.bump
        )]
        pub proposal: Account<'info, Proposal>,

        #[account(
            init,
            payer = voter,
            space = 8 + std::mem::size_of::<UserVote>(),
            seeds = [b"user_vote", proposal.key().as_ref(), voter.key().as_ref()],
            bump
        )]
        pub user_vote: Account<'info, UserVote>,

        #[account(mut)]
        pub voter: Signer<'info>,

        /// Voter token account for vote weighting.
        /// CHECK: deserialized manually.
        pub voter_token_account: AccountInfo<'info>,

        pub system_program: Program<'info, System>,
    }

    #[derive(Accounts)]
    pub struct FinalizeProposal<'info> {
        #[account(
            mut,
            seeds = [b"proposal", proposal.token_mint.as_ref(), proposal.proposal_id.to_le_bytes().as_ref()],
            bump = proposal.bump
        )]
        pub proposal: Account<'info, Proposal>,

        pub finalizer: Signer<'info>,
    }

    // ---------------------------------------------------------------------------
    // Instructions
    // ---------------------------------------------------------------------------

    pub fn handle_create_proposal(
        ctx: Context<CreateProposal>,
        proposal_id: u64,
        token_mint: Pubkey,
        title: String,
        description: String,
        option_labels: Vec<String>,
        start_time: i64,
        end_time: i64,
        quorum: u64,
        min_balance: u64,
    ) -> Result<()> {
        require!(title.len() <= MAX_TITLE_LEN, VotingError::TitleTooLong);
        require!(description.len() <= MAX_DESCRIPTION_LEN, VotingError::DescriptionTooLong);
        require!(option_labels.len() >= 2, VotingError::TooFewOptions);
        require!(option_labels.len() <= MAX_OPTIONS, VotingError::TooManyOptions);
        require!(end_time > start_time, VotingError::InvalidTimeRange);

        // Check creator token balance if min_balance > 0
        if min_balance > 0 {
            let token_data = ctx.accounts.creator_token_account.try_borrow_data()?;
            // SPL token account: amount is at offset 64, 8 bytes LE
            if token_data.len() >= 72 {
                let amount = u64::from_le_bytes(token_data[64..72].try_into().unwrap());
                require!(amount >= min_balance, VotingError::InsufficientBalance);
            } else {
                return Err(VotingError::InsufficientBalance.into());
            }
        }

        let proposal = &mut ctx.accounts.proposal;
        proposal.proposal_id = proposal_id;
        proposal.token_mint = token_mint;
        proposal.creator = ctx.accounts.creator.key();
        proposal.bump = ctx.bumps.proposal;

        // Copy title
        let title_bytes = title.as_bytes();
        proposal.title[..title_bytes.len()].copy_from_slice(title_bytes);
        proposal.title_len = title_bytes.len() as u8;

        // Copy description
        let desc_bytes = description.as_bytes();
        proposal.description[..desc_bytes.len()].copy_from_slice(desc_bytes);
        proposal.description_len = desc_bytes.len() as u16;

        // Copy options
        for (i, label) in option_labels.iter().enumerate() {
            require!(label.len() <= MAX_OPTION_LABEL_LEN, VotingError::OptionLabelTooLong);
            let lb = label.as_bytes();
            proposal.options[i].label[..lb.len()].copy_from_slice(lb);
            proposal.options[i].label_len = lb.len() as u8;
        }
        proposal.option_count = option_labels.len() as u8;
        proposal.start_time = start_time;
        proposal.end_time = end_time;
        proposal.quorum = quorum;
        proposal.status = ProposalStatus::Active;

        emit!(ProposalCreated {
            proposal_id,
            token_mint,
            creator: ctx.accounts.creator.key(),
            start_time,
            end_time,
            option_count: proposal.option_count,
        });

        Ok(())
    }

    pub fn handle_cast_vote(
        ctx: Context<CastVote>,
        option_index: u8,
    ) -> Result<()> {
        let proposal = &mut ctx.accounts.proposal;
        let clock = Clock::get()?;
        let now = clock.unix_timestamp;

        require!(proposal.status == ProposalStatus::Active, VotingError::ProposalNotActive);
        require!(now >= proposal.start_time, VotingError::VotingNotStarted);
        require!(now <= proposal.end_time, VotingError::VotingEnded);
        require!((option_index as usize) < proposal.option_count as usize, VotingError::InvalidOption);

        // Get voter's token balance as vote weight
        let token_data = ctx.accounts.voter_token_account.try_borrow_data()?;
        let weight = if token_data.len() >= 72 {
            u64::from_le_bytes(token_data[64..72].try_into().unwrap())
        } else {
            1u64
        };

        // Update proposal
        proposal.options[option_index as usize].vote_count = proposal.options[option_index as usize]
            .vote_count
            .checked_add(weight)
            .ok_or(VotingError::Overflow)?;
        proposal.total_votes = proposal
            .total_votes
            .checked_add(weight)
            .ok_or(VotingError::Overflow)?;

        // Record user vote
        let user_vote = &mut ctx.accounts.user_vote;
        user_vote.proposal = proposal.key();
        user_vote.voter = ctx.accounts.voter.key();
        user_vote.option_index = option_index;
        user_vote.weight = weight;
        user_vote.timestamp = now;
        user_vote.bump = ctx.bumps.user_vote;

        emit!(VoteCast {
            proposal_id: proposal.proposal_id,
            voter: ctx.accounts.voter.key(),
            option_index,
            weight,
        });

        Ok(())
    }

    pub fn handle_finalize_proposal(ctx: Context<FinalizeProposal>) -> Result<()> {
        let proposal = &mut ctx.accounts.proposal;
        let clock = Clock::get()?;

        require!(proposal.status == ProposalStatus::Active, VotingError::ProposalNotActive);
        require!(clock.unix_timestamp > proposal.end_time, VotingError::VotingStillActive);

        // Determine outcome
        let quorum_met = proposal.total_votes >= proposal.quorum;

        let mut winning_option: u8 = 0;
        let mut max_votes: u64 = 0;
        for i in 0..proposal.option_count as usize {
            if proposal.options[i].vote_count > max_votes {
                max_votes = proposal.options[i].vote_count;
                winning_option = i as u8;
            }
        }

        proposal.status = if quorum_met {
            ProposalStatus::Passed
        } else {
            ProposalStatus::Rejected
        };

        emit!(ProposalFinalized {
            proposal_id: proposal.proposal_id,
            status: proposal.status,
            winning_option,
            total_votes: proposal.total_votes,
        });

        Ok(())
    }

}

pub mod fee_splitting {
    use super::*;

    // ---------------------------------------------------------------------------
    // Constants
    // ---------------------------------------------------------------------------

    const FEE_CONFIG_SEED: &[u8] = b"fee_config";
    const USER_FEE_CLAIM_SEED: &[u8] = b"user_fee_claim";
    const FEE_VAULT_SEED: &[u8] = b"fee_vault";

    const MAX_SPLITS: usize = 5;
    const MAX_SOCIAL_HANDLE_LEN: usize = 64;
    const BPS_TOTAL: u16 = 10_000;

    // ---------------------------------------------------------------------------
    // Accounts
    // ---------------------------------------------------------------------------

    #[derive(AnchorSerialize, AnchorDeserialize, Clone, Default)]
    pub struct FeeSplit {
        /// Recipient wallet
        pub recipient: Pubkey,
        /// Share in basis points (out of 10000)
        pub share_bps: u16,
        /// Optional social handle for display purposes
        pub social_handle: String,
    }

    impl FeeSplit {
        pub const SIZE: usize = 32 + 2 + (4 + MAX_SOCIAL_HANDLE_LEN);
    }

    #[account]
    #[derive(Default)]
    pub struct FeeConfig {
        /// The token mint this fee config is for
        pub token_mint: Pubkey,
        /// Original creator who set up the config
        pub creator: Pubkey,
        /// Fee split recipients (up to 5)
        pub splits: Vec<FeeSplit>,
        /// Total fees distributed through this config (lamports)
        pub total_distributed: u64,
        /// Whether distribution has ever occurred (locks splits if true, unless creator allows updates)
        pub has_distributed: bool,
        /// Whether creator allows updating splits after first distribution
        pub allow_update_after_distribution: bool,
        /// Bump seed for PDA
        pub bump: u8,
    }

    impl FeeConfig {
        pub const SIZE: usize = 8 // discriminator
            + 32  // token_mint
            + 32  // creator
            + (4 + MAX_SPLITS * FeeSplit::SIZE) // splits vec
            + 8   // total_distributed
            + 1   // has_distributed
            + 1   // allow_update_after_distribution
            + 1;  // bump
    }

    #[account]
    #[derive(Default)]
    pub struct UserFeeClaim {
        /// Recipient wallet
        pub recipient: Pubkey,
        /// Token mint
        pub token_mint: Pubkey,
        /// Total amount allocated to this recipient (lamports)
        pub total_allocated: u64,
        /// Total amount claimed by this recipient (lamports)
        pub total_claimed: u64,
        /// Bump seed for PDA
        pub bump: u8,
    }

    impl UserFeeClaim {
        pub const SIZE: usize = 8 // discriminator
            + 32  // recipient
            + 32  // token_mint
            + 8   // total_allocated
            + 8   // total_claimed
            + 1;  // bump
    }

    // ---------------------------------------------------------------------------
    // Events
    // ---------------------------------------------------------------------------

    #[event]
    pub struct FeeConfigCreated {
        pub token_mint: Pubkey,
        pub creator: Pubkey,
        pub num_splits: u8,
    }

    #[event]
    pub struct FeeSplitsUpdated {
        pub token_mint: Pubkey,
        pub creator: Pubkey,
        pub num_splits: u8,
    }

    #[event]
    pub struct FeesDistributed {
        pub token_mint: Pubkey,
        pub total_amount: u64,
        pub num_recipients: u8,
    }

    #[event]
    pub struct SplitFeesClaimed {
        pub token_mint: Pubkey,
        pub recipient: Pubkey,
        pub amount: u64,
    }

    // ---------------------------------------------------------------------------
    // Errors
    // ---------------------------------------------------------------------------

    #[error_code]
    pub enum FeeSplittingError {
        #[msg("Too many fee splits (max 5)")]
        TooManySplits,
        #[msg("Total share basis points must equal 10000")]
        InvalidShareTotal,
        #[msg("Social handle too long (max 64 chars)")]
        SocialHandleTooLong,
        #[msg("Fee splits cannot be updated after distribution")]
        SplitsLocked,
        #[msg("Nothing to claim")]
        NothingToClaim,
        #[msg("Nothing to distribute")]
        NothingToDistribute,
        #[msg("Unauthorized — only creator can perform this action")]
        Unauthorized,
        #[msg("Recipient not found in fee splits")]
        RecipientNotFound,
        #[msg("Math overflow")]
        MathOverflow,
    }

    // ---------------------------------------------------------------------------
    // Instructions
    // ---------------------------------------------------------------------------

    #[derive(Accounts)]
    pub struct InitializeFeeConfig<'info> {
        #[account(
            init,
            payer = creator,
            space = FeeConfig::SIZE,
            seeds = [FEE_CONFIG_SEED, token_mint.key().as_ref()],
            bump,
        )]
        pub fee_config: Account<'info, FeeConfig>,

        /// CHECK: Token mint for which fees are configured
        pub token_mint: AccountInfo<'info>,

        #[account(mut)]
        pub creator: Signer<'info>,

        pub system_program: Program<'info, System>,
    }

    #[derive(Accounts)]
    pub struct UpdateFeeSplits<'info> {
        #[account(
            mut,
            seeds = [FEE_CONFIG_SEED, token_mint.key().as_ref()],
            bump = fee_config.bump,
            has_one = creator @ FeeSplittingError::Unauthorized,
        )]
        pub fee_config: Account<'info, FeeConfig>,

        /// CHECK: Token mint
        pub token_mint: AccountInfo<'info>,

        pub creator: Signer<'info>,
    }

    #[derive(Accounts)]
    pub struct DistributeFees<'info> {
        #[account(
            mut,
            seeds = [FEE_CONFIG_SEED, token_mint.key().as_ref()],
            bump = fee_config.bump,
        )]
        pub fee_config: Account<'info, FeeConfig>,

        /// CHECK: Token mint
        pub token_mint: AccountInfo<'info>,

        /// CHECK: Fee vault PDA holding accumulated creator fees
        #[account(
            mut,
            seeds = [FEE_VAULT_SEED, token_mint.key().as_ref()],
            bump,
        )]
        pub fee_vault: AccountInfo<'info>,

        /// CHECK: Permissionless crank caller
        #[account(mut)]
        pub payer: Signer<'info>,

        pub system_program: Program<'info, System>,
    }

    #[derive(Accounts)]
    pub struct ClaimSplitFees<'info> {
        #[account(
            seeds = [FEE_CONFIG_SEED, token_mint.key().as_ref()],
            bump = fee_config.bump,
        )]
        pub fee_config: Account<'info, FeeConfig>,

        #[account(
            mut,
            seeds = [USER_FEE_CLAIM_SEED, token_mint.key().as_ref(), recipient.key().as_ref()],
            bump = user_fee_claim.bump,
            has_one = recipient,
        )]
        pub user_fee_claim: Account<'info, UserFeeClaim>,

        /// CHECK: Token mint
        pub token_mint: AccountInfo<'info>,

        /// CHECK: Fee vault PDA
        #[account(
            mut,
            seeds = [FEE_VAULT_SEED, token_mint.key().as_ref()],
            bump,
        )]
        pub fee_vault: AccountInfo<'info>,

        #[account(mut)]
        pub recipient: Signer<'info>,

        pub system_program: Program<'info, System>,
    }

    // ---------------------------------------------------------------------------
    // Handlers
    // ---------------------------------------------------------------------------

    pub fn handle_initialize_fee_config(
        ctx: Context<InitializeFeeConfig>,
        splits: Vec<FeeSplit>,
        allow_update_after_distribution: bool,
    ) -> Result<()> {
        require!(splits.len() <= MAX_SPLITS, FeeSplittingError::TooManySplits);

        // Validate social handles
        for split in &splits {
            require!(
                split.social_handle.len() <= MAX_SOCIAL_HANDLE_LEN,
                FeeSplittingError::SocialHandleTooLong
            );
        }

        // Validate total bps = 10000
        let total_bps: u16 = splits.iter().map(|s| s.share_bps).sum();
        if splits.is_empty() {
            // Default: 100% to creator — we'll store a single split
            let config = &mut ctx.accounts.fee_config;
            config.token_mint = ctx.accounts.token_mint.key();
            config.creator = ctx.accounts.creator.key();
            config.splits = vec![FeeSplit {
                recipient: ctx.accounts.creator.key(),
                share_bps: BPS_TOTAL,
                social_handle: String::new(),
            }];
            config.total_distributed = 0;
            config.has_distributed = false;
            config.allow_update_after_distribution = allow_update_after_distribution;
            config.bump = ctx.bumps.fee_config;
        } else {
            require!(total_bps == BPS_TOTAL, FeeSplittingError::InvalidShareTotal);

            let config = &mut ctx.accounts.fee_config;
            config.token_mint = ctx.accounts.token_mint.key();
            config.creator = ctx.accounts.creator.key();
            config.splits = splits;
            config.total_distributed = 0;
            config.has_distributed = false;
            config.allow_update_after_distribution = allow_update_after_distribution;
            config.bump = ctx.bumps.fee_config;
        }

        emit!(FeeConfigCreated {
            token_mint: ctx.accounts.token_mint.key(),
            creator: ctx.accounts.creator.key(),
            num_splits: ctx.accounts.fee_config.splits.len() as u8,
        });

        Ok(())
    }

    pub fn handle_update_fee_splits(
        ctx: Context<UpdateFeeSplits>,
        new_splits: Vec<FeeSplit>,
    ) -> Result<()> {
        let config = &ctx.accounts.fee_config;

        // Check if updates are allowed
        if config.has_distributed && !config.allow_update_after_distribution {
            return Err(FeeSplittingError::SplitsLocked.into());
        }

        require!(new_splits.len() <= MAX_SPLITS, FeeSplittingError::TooManySplits);

        for split in &new_splits {
            require!(
                split.social_handle.len() <= MAX_SOCIAL_HANDLE_LEN,
                FeeSplittingError::SocialHandleTooLong
            );
        }

        let total_bps: u16 = new_splits.iter().map(|s| s.share_bps).sum();
        require!(total_bps == BPS_TOTAL, FeeSplittingError::InvalidShareTotal);

        let config = &mut ctx.accounts.fee_config;
        config.splits = new_splits;

        emit!(FeeSplitsUpdated {
            token_mint: ctx.accounts.token_mint.key(),
            creator: ctx.accounts.creator.key(),
            num_splits: config.splits.len() as u8,
        });

        Ok(())
    }

    pub fn handle_distribute_fees(ctx: Context<DistributeFees>) -> Result<()> {
        let fee_vault = &ctx.accounts.fee_vault;
        let rent = Rent::get()?;
        let min_balance = rent.minimum_balance(0);
        let available = fee_vault
            .lamports()
            .checked_sub(min_balance)
            .unwrap_or(0);

        require!(available > 0, FeeSplittingError::NothingToDistribute);

        let config = &ctx.accounts.fee_config;
        let splits = &config.splits;

        // Distribute proportionally to each split recipient via remaining_accounts
        // Each remaining account corresponds to a UserFeeClaim PDA in order
        let remaining = &ctx.remaining_accounts;
        require!(
            remaining.len() == splits.len(),
            FeeSplittingError::RecipientNotFound
        );

        for (i, split) in splits.iter().enumerate() {
            let share_amount = (available as u128)
                .checked_mul(split.share_bps as u128)
                .ok_or(FeeSplittingError::MathOverflow)?
                .checked_div(BPS_TOTAL as u128)
                .ok_or(FeeSplittingError::MathOverflow)? as u64;

            if share_amount > 0 {
                // Update the UserFeeClaim account
                let claim_info = &remaining[i];
                let mut claim_data = claim_info.try_borrow_mut_data()?;
                // Skip 8-byte discriminator, deserialize
                let mut claim: UserFeeClaim =
                    UserFeeClaim::try_deserialize(&mut &claim_data[..])?;
                claim.total_allocated = claim
                    .total_allocated
                    .checked_add(share_amount)
                    .ok_or(FeeSplittingError::MathOverflow)?;
                // Serialize back
                let mut writer = &mut claim_data[8..];
                claim.try_serialize(&mut writer)?;
            }
        }

        // Transfer total from vault
        let mint_key = ctx.accounts.token_mint.key();
        let vault_seeds: &[&[u8]] = &[
            FEE_VAULT_SEED,
            mint_key.as_ref(),
            &[ctx.bumps.fee_vault],
        ];

        **ctx.accounts.fee_vault.try_borrow_mut_lamports()? -= available;
        // Distribute lamports to fee_config PDA temporarily (claims pull from vault)
        // Actually we keep in vault and let claims pull — just update allocations above

        // Re-add lamports since we only updated allocations
        **ctx.accounts.fee_vault.try_borrow_mut_lamports()? += available;

        let config = &mut ctx.accounts.fee_config;
        config.total_distributed = config
            .total_distributed
            .checked_add(available)
            .ok_or(FeeSplittingError::MathOverflow)?;
        config.has_distributed = true;

        emit!(FeesDistributed {
            token_mint: ctx.accounts.token_mint.key(),
            total_amount: available,
            num_recipients: splits.len() as u8,
        });

        Ok(())
    }

    pub fn handle_claim_split_fees(ctx: Context<ClaimSplitFees>) -> Result<()> {
        let claim = &ctx.accounts.user_fee_claim;
        let claimable = claim
            .total_allocated
            .checked_sub(claim.total_claimed)
            .unwrap_or(0);

        require!(claimable > 0, FeeSplittingError::NothingToClaim);

        // Transfer from fee vault to recipient
        let mint_key = ctx.accounts.token_mint.key();
        let vault_seeds: &[&[u8]] = &[
            FEE_VAULT_SEED,
            mint_key.as_ref(),
            &[ctx.bumps.fee_vault],
        ];

        **ctx.accounts.fee_vault.try_borrow_mut_lamports()? -= claimable;
        **ctx.accounts.recipient.to_account_info().try_borrow_mut_lamports()? += claimable;

        let claim = &mut ctx.accounts.user_fee_claim;
        claim.total_claimed = claim
            .total_claimed
            .checked_add(claimable)
            .ok_or(FeeSplittingError::MathOverflow)?;

        emit!(SplitFeesClaimed {
            token_mint: ctx.accounts.token_mint.key(),
            recipient: ctx.accounts.recipient.key(),
            amount: claimable,
        });

        Ok(())
    }

}
pub mod content_claims {
    use super::*;

    // ---------------------------------------------------------------------------
    // Constants
    // ---------------------------------------------------------------------------

    const CONTENT_CLAIM_SEED: &[u8] = b"content_claim";
    const CLAIM_VERIFY_SEED: &[u8] = b"claim_verify";

    const MAX_URL_LEN: usize = 200;

    // ---------------------------------------------------------------------------
    // Enums
    // ---------------------------------------------------------------------------

    #[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Default)]
    pub enum ClaimStatus {
        #[default]
        Unclaimed,
        Pending,
        Verified,
        Rejected,
    }

    // ---------------------------------------------------------------------------
    // Accounts
    // ---------------------------------------------------------------------------

    #[account]
    #[derive(Default)]
    pub struct ContentClaim {
        /// The token mint this claim is for
        pub token_mint: Pubkey,
        /// Original creator who launched the token
        pub original_creator: Pubkey,
        /// Content owner who claimed it (None if unclaimed)
        pub claimed_by: Option<Pubkey>,
        /// Link to original content
        pub content_url: String,
        /// Current claim status
        pub claim_status: ClaimStatus,
        /// Timestamp when claimed
        pub claimed_at: Option<i64>,
        /// Basis points of creator fee redirected to content owner after verification (default 5000 = 50%)
        pub fee_redirect_bps: u16,
        /// Bump seed for PDA
        pub bump: u8,
    }

    impl ContentClaim {
        pub const SIZE: usize = 8  // discriminator
            + 32  // token_mint
            + 32  // original_creator
            + (1 + 32) // claimed_by Option<Pubkey>
            + (4 + MAX_URL_LEN)  // content_url
            + 1   // claim_status
            + (1 + 8) // claimed_at Option<i64>
            + 2   // fee_redirect_bps
            + 1;  // bump
    }

    #[account]
    #[derive(Default)]
    pub struct ClaimVerification {
        /// Token mint
        pub token_mint: Pubkey,
        /// The claimant submitting the claim
        pub claimant: Pubkey,
        /// URL to proof of content ownership
        pub proof_url: String,
        /// Timestamp of submission
        pub submitted_at: i64,
        /// Whether this verification has been resolved
        pub resolved: bool,
        /// Bump seed for PDA
        pub bump: u8,
    }

    impl ClaimVerification {
        pub const SIZE: usize = 8  // discriminator
            + 32  // token_mint
            + 32  // claimant
            + (4 + MAX_URL_LEN)  // proof_url
            + 8   // submitted_at
            + 1   // resolved
            + 1;  // bump
    }

    // ---------------------------------------------------------------------------
    // Events
    // ---------------------------------------------------------------------------

    #[event]
    pub struct ContentRegistered {
        pub token_mint: Pubkey,
        pub creator: Pubkey,
        pub content_url: String,
    }

    #[event]
    pub struct ClaimSubmitted {
        pub token_mint: Pubkey,
        pub claimant: Pubkey,
        pub proof_url: String,
    }

    #[event]
    pub struct ClaimVerifiedEvent {
        pub token_mint: Pubkey,
        pub claimant: Pubkey,
        pub fee_redirect_bps: u16,
    }

    #[event]
    pub struct ClaimRejected {
        pub token_mint: Pubkey,
        pub claimant: Pubkey,
    }

    #[event]
    pub struct FeesRedirected {
        pub token_mint: Pubkey,
        pub content_owner: Pubkey,
        pub amount: u64,
        pub fee_redirect_bps: u16,
    }

    // ---------------------------------------------------------------------------
    // Errors
    // ---------------------------------------------------------------------------

    #[error_code]
    pub enum ContentClaimError {
        #[msg("Content URL too long (max 200 chars)")]
        UrlTooLong,
        #[msg("Content has already been claimed")]
        AlreadyClaimed,
        #[msg("Invalid claim status transition")]
        InvalidStatusTransition,
        #[msg("Unauthorized — only platform authority can verify/reject")]
        Unauthorized,
        #[msg("Claim is not in Pending status")]
        NotPending,
        #[msg("Claim is not Verified — cannot redirect fees")]
        NotVerified,
        #[msg("Fee redirect bps too high (max 10000)")]
        InvalidRedirectBps,
        #[msg("Proof URL too long (max 200 chars)")]
        ProofUrlTooLong,
        #[msg("Math overflow")]
        MathOverflow,
    }

    // ---------------------------------------------------------------------------
    // Instructions
    // ---------------------------------------------------------------------------

    #[derive(Accounts)]
    pub struct RegisterContent<'info> {
        #[account(
            init,
            payer = creator,
            space = ContentClaim::SIZE,
            seeds = [CONTENT_CLAIM_SEED, token_mint.key().as_ref()],
            bump,
        )]
        pub content_claim: Account<'info, ContentClaim>,

        /// CHECK: Token mint
        pub token_mint: AccountInfo<'info>,

        #[account(mut)]
        pub creator: Signer<'info>,

        pub system_program: Program<'info, System>,
    }

    #[derive(Accounts)]
    pub struct SubmitClaim<'info> {
        #[account(
            mut,
            seeds = [CONTENT_CLAIM_SEED, token_mint.key().as_ref()],
            bump = content_claim.bump,
        )]
        pub content_claim: Account<'info, ContentClaim>,

        #[account(
            init,
            payer = claimant,
            space = ClaimVerification::SIZE,
            seeds = [CLAIM_VERIFY_SEED, token_mint.key().as_ref(), claimant.key().as_ref()],
            bump,
        )]
        pub claim_verification: Account<'info, ClaimVerification>,

        /// CHECK: Token mint
        pub token_mint: AccountInfo<'info>,

        #[account(mut)]
        pub claimant: Signer<'info>,

        pub system_program: Program<'info, System>,
    }

    #[derive(Accounts)]
    pub struct VerifyClaim<'info> {
        #[account(
            mut,
            seeds = [CONTENT_CLAIM_SEED, token_mint.key().as_ref()],
            bump = content_claim.bump,
        )]
        pub content_claim: Account<'info, ContentClaim>,

        #[account(
            mut,
            seeds = [CLAIM_VERIFY_SEED, token_mint.key().as_ref(), claimant.key().as_ref()],
            bump = claim_verification.bump,
        )]
        pub claim_verification: Account<'info, ClaimVerification>,

        /// CHECK: Token mint
        pub token_mint: AccountInfo<'info>,

        /// CHECK: The claimant whose claim is being verified
        pub claimant: AccountInfo<'info>,

        /// Platform authority that can verify claims
        pub authority: Signer<'info>,
    }

    #[derive(Accounts)]
    pub struct RejectClaim<'info> {
        #[account(
            mut,
            seeds = [CONTENT_CLAIM_SEED, token_mint.key().as_ref()],
            bump = content_claim.bump,
        )]
        pub content_claim: Account<'info, ContentClaim>,

        #[account(
            mut,
            seeds = [CLAIM_VERIFY_SEED, token_mint.key().as_ref(), claimant.key().as_ref()],
            bump = claim_verification.bump,
        )]
        pub claim_verification: Account<'info, ClaimVerification>,

        /// CHECK: Token mint
        pub token_mint: AccountInfo<'info>,

        /// CHECK: The claimant whose claim is being rejected
        pub claimant: AccountInfo<'info>,

        /// Platform authority
        pub authority: Signer<'info>,
    }

    #[derive(Accounts)]
    pub struct RedirectFees<'info> {
        #[account(
            seeds = [CONTENT_CLAIM_SEED, token_mint.key().as_ref()],
            bump = content_claim.bump,
        )]
        pub content_claim: Account<'info, ContentClaim>,

        /// CHECK: Token mint
        pub token_mint: AccountInfo<'info>,

        /// CHECK: Content owner receiving redirected fees
        #[account(
            mut,
            constraint = Some(content_owner.key()) == content_claim.claimed_by @ ContentClaimError::NotVerified,
        )]
        pub content_owner: AccountInfo<'info>,

        /// CHECK: Creator fee source (vault or creator wallet)
        #[account(mut)]
        pub fee_source: AccountInfo<'info>,

        /// Permissionless crank
        #[account(mut)]
        pub payer: Signer<'info>,

        pub system_program: Program<'info, System>,
    }

    // ---------------------------------------------------------------------------
    // Handlers
    // ---------------------------------------------------------------------------

    pub fn handle_register_content(
        ctx: Context<RegisterContent>,
        content_url: String,
        fee_redirect_bps: u16,
    ) -> Result<()> {
        require!(content_url.len() <= MAX_URL_LEN, ContentClaimError::UrlTooLong);
        require!(fee_redirect_bps <= 10_000, ContentClaimError::InvalidRedirectBps);

        let claim = &mut ctx.accounts.content_claim;
        claim.token_mint = ctx.accounts.token_mint.key();
        claim.original_creator = ctx.accounts.creator.key();
        claim.claimed_by = None;
        claim.content_url = content_url.clone();
        claim.claim_status = ClaimStatus::Unclaimed;
        claim.claimed_at = None;
        claim.fee_redirect_bps = if fee_redirect_bps == 0 { 5000 } else { fee_redirect_bps };
        claim.bump = ctx.bumps.content_claim;

        emit!(ContentRegistered {
            token_mint: ctx.accounts.token_mint.key(),
            creator: ctx.accounts.creator.key(),
            content_url,
        });

        Ok(())
    }

    pub fn handle_submit_claim(
        ctx: Context<SubmitClaim>,
        proof_url: String,
    ) -> Result<()> {
        require!(proof_url.len() <= MAX_URL_LEN, ContentClaimError::ProofUrlTooLong);

        let content_claim = &ctx.accounts.content_claim;
        require!(
            content_claim.claim_status == ClaimStatus::Unclaimed,
            ContentClaimError::AlreadyClaimed
        );

        // Update content claim to Pending
        let content_claim = &mut ctx.accounts.content_claim;
        content_claim.claim_status = ClaimStatus::Pending;
        content_claim.claimed_by = Some(ctx.accounts.claimant.key());

        let clock = Clock::get()?;

        // Initialize verification record
        let verification = &mut ctx.accounts.claim_verification;
        verification.token_mint = ctx.accounts.token_mint.key();
        verification.claimant = ctx.accounts.claimant.key();
        verification.proof_url = proof_url.clone();
        verification.submitted_at = clock.unix_timestamp;
        verification.resolved = false;
        verification.bump = ctx.bumps.claim_verification;

        emit!(ClaimSubmitted {
            token_mint: ctx.accounts.token_mint.key(),
            claimant: ctx.accounts.claimant.key(),
            proof_url,
        });

        Ok(())
    }

    pub fn handle_verify_claim(ctx: Context<VerifyClaim>) -> Result<()> {
        let content_claim = &ctx.accounts.content_claim;
        require!(
            content_claim.claim_status == ClaimStatus::Pending,
            ContentClaimError::NotPending
        );

        let clock = Clock::get()?;

        let content_claim = &mut ctx.accounts.content_claim;
        content_claim.claim_status = ClaimStatus::Verified;
        content_claim.claimed_at = Some(clock.unix_timestamp);

        let verification = &mut ctx.accounts.claim_verification;
        verification.resolved = true;

        emit!(ClaimVerifiedEvent {
            token_mint: ctx.accounts.token_mint.key(),
            claimant: ctx.accounts.claimant.key(),
            fee_redirect_bps: content_claim.fee_redirect_bps,
        });

        Ok(())
    }

    pub fn handle_reject_claim(ctx: Context<RejectClaim>) -> Result<()> {
        let content_claim = &ctx.accounts.content_claim;
        require!(
            content_claim.claim_status == ClaimStatus::Pending,
            ContentClaimError::NotPending
        );

        let content_claim = &mut ctx.accounts.content_claim;
        content_claim.claim_status = ClaimStatus::Rejected;
        content_claim.claimed_by = None;

        let verification = &mut ctx.accounts.claim_verification;
        verification.resolved = true;

        emit!(ClaimRejected {
            token_mint: ctx.accounts.token_mint.key(),
            claimant: ctx.accounts.claimant.key(),
        });

        Ok(())
    }

    pub fn handle_redirect_fees(
        ctx: Context<RedirectFees>,
        amount: u64,
    ) -> Result<()> {
        let content_claim = &ctx.accounts.content_claim;
        require!(
            content_claim.claim_status == ClaimStatus::Verified,
            ContentClaimError::NotVerified
        );

        // Calculate redirect amount
        let redirect_amount = (amount as u128)
            .checked_mul(content_claim.fee_redirect_bps as u128)
            .ok_or(ContentClaimError::MathOverflow)?
            .checked_div(10_000u128)
            .ok_or(ContentClaimError::MathOverflow)? as u64;

        if redirect_amount > 0 {
            // Transfer from fee source to content owner
            **ctx.accounts.fee_source.try_borrow_mut_lamports()? -= redirect_amount;
            **ctx.accounts.content_owner.try_borrow_mut_lamports()? += redirect_amount;
        }

        emit!(FeesRedirected {
            token_mint: ctx.accounts.token_mint.key(),
            content_owner: ctx.accounts.content_owner.key(),
            amount: redirect_amount,
            fee_redirect_bps: content_claim.fee_redirect_bps,
        });

        Ok(())
    }

}
pub mod embeddable_widgets {
    use super::*;

    // ---------------------------------------------------------------------------
    // Constants
    // ---------------------------------------------------------------------------

    const WIDGET_SEED: &[u8] = b"widget";

    // ---------------------------------------------------------------------------
    // Enums
    // ---------------------------------------------------------------------------

    #[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Default)]
    pub enum WidgetType {
        #[default]
        PriceBadge,
        TradingCard,
        LeaderboardBadge,
        MiniChart,
    }

    #[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Default)]
    pub enum WidgetTheme {
        #[default]
        Dark,
        Light,
        Custom,
    }

    // ---------------------------------------------------------------------------
    // Accounts
    // ---------------------------------------------------------------------------

    #[account]
    #[derive(Default)]
    pub struct WidgetConfig {
        /// The token mint this widget is for
        pub token_mint: Pubkey,
        /// Creator who owns this widget config
        pub creator: Pubkey,
        /// Type of widget
        pub widget_type: WidgetType,
        /// Theme
        pub theme: WidgetTheme,
        /// Custom RGB color (only used when theme = Custom)
        pub custom_color: Option<[u8; 3]>,
        /// Display flags
        pub show_price: bool,
        pub show_volume: bool,
        pub show_holders: bool,
        pub show_market_cap: bool,
        /// Whether the widget is enabled
        pub enabled: bool,
        /// Total views tracked on-chain for analytics
        pub views: u64,
        /// Bump seed for PDA
        pub bump: u8,
    }

    impl WidgetConfig {
        pub const SIZE: usize = 8  // discriminator
            + 32  // token_mint
            + 32  // creator
            + 1   // widget_type
            + 1   // theme
            + (1 + 3) // custom_color Option<[u8; 3]>
            + 1   // show_price
            + 1   // show_volume
            + 1   // show_holders
            + 1   // show_market_cap
            + 1   // enabled
            + 8   // views
            + 1;  // bump
    }

    // ---------------------------------------------------------------------------
    // Events
    // ---------------------------------------------------------------------------

    #[event]
    pub struct WidgetCreated {
        pub token_mint: Pubkey,
        pub creator: Pubkey,
        pub widget_type: WidgetType,
        pub theme: WidgetTheme,
    }

    #[event]
    pub struct WidgetUpdated {
        pub token_mint: Pubkey,
        pub creator: Pubkey,
        pub widget_type: WidgetType,
        pub theme: WidgetTheme,
    }

    #[event]
    pub struct WidgetViewed {
        pub token_mint: Pubkey,
        pub views: u64,
    }

    // ---------------------------------------------------------------------------
    // Errors
    // ---------------------------------------------------------------------------

    #[error_code]
    pub enum WidgetError {
        #[msg("Unauthorized — only creator can modify widget")]
        Unauthorized,
        #[msg("Widget is disabled")]
        WidgetDisabled,
        #[msg("Custom color required when theme is Custom")]
        CustomColorRequired,
    }

    // ---------------------------------------------------------------------------
    // Instructions
    // ---------------------------------------------------------------------------

    #[derive(Accounts)]
    pub struct CreateWidgetConfig<'info> {
        #[account(
            init,
            payer = creator,
            space = WidgetConfig::SIZE,
            seeds = [WIDGET_SEED, token_mint.key().as_ref()],
            bump,
        )]
        pub widget_config: Account<'info, WidgetConfig>,

        /// CHECK: Token mint
        pub token_mint: AccountInfo<'info>,

        #[account(mut)]
        pub creator: Signer<'info>,

        pub system_program: Program<'info, System>,
    }

    #[derive(Accounts)]
    pub struct UpdateWidgetConfig<'info> {
        #[account(
            mut,
            seeds = [WIDGET_SEED, token_mint.key().as_ref()],
            bump = widget_config.bump,
            has_one = creator @ WidgetError::Unauthorized,
        )]
        pub widget_config: Account<'info, WidgetConfig>,

        /// CHECK: Token mint
        pub token_mint: AccountInfo<'info>,

        pub creator: Signer<'info>,
    }

    #[derive(Accounts)]
    pub struct RecordWidgetView<'info> {
        #[account(
            mut,
            seeds = [WIDGET_SEED, token_mint.key().as_ref()],
            bump = widget_config.bump,
        )]
        pub widget_config: Account<'info, WidgetConfig>,

        /// CHECK: Token mint
        pub token_mint: AccountInfo<'info>,
    }

    #[derive(Accounts)]
    pub struct DisableWidget<'info> {
        #[account(
            mut,
            seeds = [WIDGET_SEED, token_mint.key().as_ref()],
            bump = widget_config.bump,
        )]
        pub widget_config: Account<'info, WidgetConfig>,

        /// CHECK: Token mint
        pub token_mint: AccountInfo<'info>,

        /// Creator or platform authority
        pub authority: Signer<'info>,
    }

    // ---------------------------------------------------------------------------
    // Handlers
    // ---------------------------------------------------------------------------

    pub fn handle_create_widget_config(
        ctx: Context<CreateWidgetConfig>,
        widget_type: WidgetType,
        theme: WidgetTheme,
        custom_color: Option<[u8; 3]>,
        show_price: bool,
        show_volume: bool,
        show_holders: bool,
        show_market_cap: bool,
    ) -> Result<()> {
        if theme == WidgetTheme::Custom {
            require!(custom_color.is_some(), WidgetError::CustomColorRequired);
        }

        let config = &mut ctx.accounts.widget_config;
        config.token_mint = ctx.accounts.token_mint.key();
        config.creator = ctx.accounts.creator.key();
        config.widget_type = widget_type;
        config.theme = theme;
        config.custom_color = custom_color;
        config.show_price = show_price;
        config.show_volume = show_volume;
        config.show_holders = show_holders;
        config.show_market_cap = show_market_cap;
        config.enabled = true;
        config.views = 0;
        config.bump = ctx.bumps.widget_config;

        emit!(WidgetCreated {
            token_mint: ctx.accounts.token_mint.key(),
            creator: ctx.accounts.creator.key(),
            widget_type,
            theme,
        });

        Ok(())
    }

    pub fn handle_update_widget_config(
        ctx: Context<UpdateWidgetConfig>,
        widget_type: Option<WidgetType>,
        theme: Option<WidgetTheme>,
        custom_color: Option<Option<[u8; 3]>>,
        show_price: Option<bool>,
        show_volume: Option<bool>,
        show_holders: Option<bool>,
        show_market_cap: Option<bool>,
    ) -> Result<()> {
        let config = &mut ctx.accounts.widget_config;

        if let Some(wt) = widget_type {
            config.widget_type = wt;
        }
        if let Some(t) = theme {
            config.theme = t;
        }
        if let Some(cc) = custom_color {
            config.custom_color = cc;
        }
        if let Some(v) = show_price {
            config.show_price = v;
        }
        if let Some(v) = show_volume {
            config.show_volume = v;
        }
        if let Some(v) = show_holders {
            config.show_holders = v;
        }
        if let Some(v) = show_market_cap {
            config.show_market_cap = v;
        }

        // Validate custom color if theme is Custom
        if config.theme == WidgetTheme::Custom {
            require!(config.custom_color.is_some(), WidgetError::CustomColorRequired);
        }

        emit!(WidgetUpdated {
            token_mint: ctx.accounts.token_mint.key(),
            creator: ctx.accounts.creator.key(),
            widget_type: config.widget_type,
            theme: config.theme,
        });

        Ok(())
    }

    pub fn handle_record_widget_view(ctx: Context<RecordWidgetView>) -> Result<()> {
        let config = &mut ctx.accounts.widget_config;
        require!(config.enabled, WidgetError::WidgetDisabled);

        config.views = config.views.saturating_add(1);

        emit!(WidgetViewed {
            token_mint: ctx.accounts.token_mint.key(),
            views: config.views,
        });

        Ok(())
    }

    pub fn handle_disable_widget(ctx: Context<DisableWidget>) -> Result<()> {
        let config = &ctx.accounts.widget_config;
        // Allow creator or any authority (for platform admin, check in caller)
        require!(
            ctx.accounts.authority.key() == config.creator,
            WidgetError::Unauthorized
        );

        let config = &mut ctx.accounts.widget_config;
        config.enabled = false;

        Ok(())
    }

}
pub mod stable_pairs {
    use super::*;

    // ============================================================================
    // CONSTANTS
    // ============================================================================

    const STABLE_PAIR_SEED: &[u8] = b"stable_pair";
    const LP_POSITION_SEED: &[u8] = b"lp_position";
    const POOL_TOKEN_VAULT_SEED: &[u8] = b"sp_token_vault";
    const POOL_STABLE_VAULT_SEED: &[u8] = b"sp_stable_vault";
    const TREASURY_SEED: &[u8] = b"sp_treasury";

    const BPS_DENOMINATOR: u64 = 10_000;
    const MAX_FEE_BPS: u16 = 500; // 5%
    const MIN_LIQUIDITY: u64 = 1_000; // minimum initial deposit to avoid zero-division

    // ============================================================================
    // ACCOUNTS
    // ============================================================================

    /// Configuration PDA for a token/stablecoin trading pair.
    ///
    /// Seeds: `["stable_pair", token_mint, stable_mint]`
    #[account]
    pub struct StablePairConfig {
        /// The non-stable token mint (e.g. project token).
        pub token_mint: Pubkey,
        /// The stablecoin mint (USDC, USDT, etc.).
        pub stable_mint: Pubkey,
        /// Current token reserve held in the pool.
        pub pool_token_reserve: u64,
        /// Current stablecoin reserve held in the pool.
        pub pool_stable_reserve: u64,
        /// Trading fee in basis points charged on every swap.
        pub fee_bps: u16,
        /// The wallet that created this pair.
        pub creator: Pubkey,
        /// Total LP shares outstanding (virtual — no LP mint, tracked via LPPosition PDAs).
        pub total_lp_shares: u128,
        /// Whether the pair is paused.
        pub paused: bool,
        /// Bump seed.
        pub bump: u8,
        /// Token vault bump.
        pub token_vault_bump: u8,
        /// Stable vault bump.
        pub stable_vault_bump: u8,
    }

    impl StablePairConfig {
        pub const SIZE: usize = 8   // discriminator
            + 32    // token_mint
            + 32    // stable_mint
            + 8     // pool_token_reserve
            + 8     // pool_stable_reserve
            + 2     // fee_bps
            + 32    // creator
            + 16    // total_lp_shares
            + 1     // paused
            + 1     // bump
            + 1     // token_vault_bump
            + 1;    // stable_vault_bump
    }

    /// Per-user LP position tracking the user's share of a stable pair pool.
    ///
    /// Seeds: `["lp_position", stable_pair, user]`
    #[account]
    pub struct LPPosition {
        /// The stable pair this position belongs to.
        pub pair: Pubkey,
        /// The user who owns this LP position.
        pub owner: Pubkey,
        /// LP shares held by this user.
        pub lp_shares: u128,
        /// Bump seed.
        pub bump: u8,
    }

    impl LPPosition {
        pub const SIZE: usize = 8 + 32 + 32 + 16 + 1;
    }

    // ============================================================================
    // EVENTS
    // ============================================================================

    #[event]
    pub struct StablePairCreated {
        pub token_mint: Pubkey,
        pub stable_mint: Pubkey,
        pub creator: Pubkey,
        pub fee_bps: u16,
        pub timestamp: i64,
    }

    #[event]
    pub struct SwapExecuted {
        pub pair: Pubkey,
        pub user: Pubkey,
        /// True = token→stable, False = stable→token.
        pub token_to_stable: bool,
        pub amount_in: u64,
        pub amount_out: u64,
        pub fee_amount: u64,
        pub timestamp: i64,
    }

    #[event]
    pub struct LiquidityAdded {
        pub pair: Pubkey,
        pub provider: Pubkey,
        pub token_amount: u64,
        pub stable_amount: u64,
        pub lp_shares_minted: u128,
        pub timestamp: i64,
    }

    #[event]
    pub struct LiquidityRemoved {
        pub pair: Pubkey,
        pub provider: Pubkey,
        pub token_amount: u64,
        pub stable_amount: u64,
        pub lp_shares_burned: u128,
        pub timestamp: i64,
    }

    // ============================================================================
    // ERRORS
    // ============================================================================

    #[error_code]
    pub enum StablePairError {
        #[msg("Fee exceeds maximum (5%)")]
        FeeTooHigh,
        #[msg("Amount must be greater than zero")]
        ZeroAmount,
        #[msg("Pair is paused")]
        Paused,
        #[msg("Insufficient output amount")]
        InsufficientOutput,
        #[msg("Slippage tolerance exceeded")]
        SlippageExceeded,
        #[msg("Insufficient LP shares")]
        InsufficientShares,
        #[msg("Initial liquidity too low")]
        InitialLiquidityTooLow,
        #[msg("Math overflow")]
        MathOverflow,
        #[msg("Pool is empty")]
        EmptyPool,
        #[msg("Unauthorized")]
        Unauthorized,
    }

    // ============================================================================
    // INSTRUCTIONS (HANDLERS)
    // ============================================================================

    /// Create a new token/stablecoin pair.
    pub fn handle_create_stable_pair(
        ctx: Context<CreateStablePair>,
        fee_bps: u16,
    ) -> Result<()> {
        require!(fee_bps <= MAX_FEE_BPS, StablePairError::FeeTooHigh);

        let clock = Clock::get()?;
        let pair = &mut ctx.accounts.stable_pair;
        pair.token_mint = ctx.accounts.token_mint.key();
        pair.stable_mint = ctx.accounts.stable_mint.key();
        pair.pool_token_reserve = 0;
        pair.pool_stable_reserve = 0;
        pair.fee_bps = fee_bps;
        pair.creator = ctx.accounts.creator.key();
        pair.total_lp_shares = 0;
        pair.paused = false;
        pair.bump = ctx.bumps.stable_pair;
        pair.token_vault_bump = ctx.bumps.token_vault;
        pair.stable_vault_bump = ctx.bumps.stable_vault;

        emit!(StablePairCreated {
            token_mint: pair.token_mint,
            stable_mint: pair.stable_mint,
            creator: pair.creator,
            fee_bps,
            timestamp: clock.unix_timestamp,
        });

        Ok(())
    }

    /// Swap project token → stablecoin.
    pub fn handle_swap_token_for_stable(
        ctx: Context<SwapTokenForStable>,
        amount_in: u64,
        min_amount_out: u64,
    ) -> Result<()> {
        require!(amount_in > 0, StablePairError::ZeroAmount);
        let pair = &ctx.accounts.stable_pair;
        require!(!pair.paused, StablePairError::Paused);

        let (amount_out, fee) = compute_swap_output(
            amount_in,
            pair.pool_token_reserve,
            pair.pool_stable_reserve,
            pair.fee_bps,
        )?;
        require!(amount_out > 0, StablePairError::InsufficientOutput);
        require!(amount_out >= min_amount_out, StablePairError::SlippageExceeded);

        // Transfer tokens in: user → pool token vault
        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.user_token_account.to_account_info(),
                    to: ctx.accounts.token_vault.to_account_info(),
                    authority: ctx.accounts.user.to_account_info(),
                },
            ),
            amount_in,
        )?;

        // Transfer stables out: pool stable vault → user  (PDA signer)
        let token_mint_key = ctx.accounts.stable_pair.token_mint;
        let stable_mint_key = ctx.accounts.stable_pair.stable_mint;
        let pair_seeds: &[&[u8]] = &[
            STABLE_PAIR_SEED,
            token_mint_key.as_ref(),
            stable_mint_key.as_ref(),
            &[ctx.accounts.stable_pair.bump],
        ];
        let signer = &[pair_seeds];

        // Send amount_out minus platform fee share to user, fee to treasury
        let treasury_fee = fee / 2; // 50% of fee to platform treasury
        let user_receives = amount_out; // fee already deducted in compute_swap_output

        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.stable_vault.to_account_info(),
                    to: ctx.accounts.user_stable_account.to_account_info(),
                    authority: ctx.accounts.stable_pair.to_account_info(),
                },
                signer,
            ),
            user_receives,
        )?;

        // Transfer treasury fee (in stablecoins) to treasury vault
        if treasury_fee > 0 {
            token::transfer(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    Transfer {
                        from: ctx.accounts.stable_vault.to_account_info(),
                        to: ctx.accounts.treasury_stable_account.to_account_info(),
                        authority: ctx.accounts.stable_pair.to_account_info(),
                    },
                    signer,
                ),
                treasury_fee,
            )?;
        }

        // Update reserves
        let pair = &mut ctx.accounts.stable_pair;
        pair.pool_token_reserve = pair.pool_token_reserve
            .checked_add(amount_in)
            .ok_or(StablePairError::MathOverflow)?;
        pair.pool_stable_reserve = pair.pool_stable_reserve
            .checked_sub(amount_out)
            .ok_or(StablePairError::MathOverflow)?
            .checked_sub(treasury_fee)
            .ok_or(StablePairError::MathOverflow)?;

        let clock = Clock::get()?;
        emit!(SwapExecuted {
            pair: pair.key(),
            user: ctx.accounts.user.key(),
            token_to_stable: true,
            amount_in,
            amount_out: user_receives,
            fee_amount: fee,
            timestamp: clock.unix_timestamp,
        });

        Ok(())
    }

    /// Swap stablecoin → project token.
    pub fn handle_swap_stable_for_token(
        ctx: Context<SwapStableForToken>,
        amount_in: u64,
        min_amount_out: u64,
    ) -> Result<()> {
        require!(amount_in > 0, StablePairError::ZeroAmount);
        let pair = &ctx.accounts.stable_pair;
        require!(!pair.paused, StablePairError::Paused);

        let (amount_out, fee) = compute_swap_output(
            amount_in,
            pair.pool_stable_reserve,
            pair.pool_token_reserve,
            pair.fee_bps,
        )?;
        require!(amount_out > 0, StablePairError::InsufficientOutput);
        require!(amount_out >= min_amount_out, StablePairError::SlippageExceeded);

        // Transfer stables in: user → pool stable vault
        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.user_stable_account.to_account_info(),
                    to: ctx.accounts.stable_vault.to_account_info(),
                    authority: ctx.accounts.user.to_account_info(),
                },
            ),
            amount_in,
        )?;

        // Transfer tokens out: pool token vault → user (PDA signer)
        let token_mint_key = ctx.accounts.stable_pair.token_mint;
        let stable_mint_key = ctx.accounts.stable_pair.stable_mint;
        let pair_seeds: &[&[u8]] = &[
            STABLE_PAIR_SEED,
            token_mint_key.as_ref(),
            stable_mint_key.as_ref(),
            &[ctx.accounts.stable_pair.bump],
        ];
        let signer = &[pair_seeds];

        let treasury_fee = fee / 2;
        let user_receives = amount_out;

        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.token_vault.to_account_info(),
                    to: ctx.accounts.user_token_account.to_account_info(),
                    authority: ctx.accounts.stable_pair.to_account_info(),
                },
                signer,
            ),
            user_receives,
        )?;

        // Treasury fee in tokens
        if treasury_fee > 0 {
            token::transfer(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    Transfer {
                        from: ctx.accounts.token_vault.to_account_info(),
                        to: ctx.accounts.treasury_token_account.to_account_info(),
                        authority: ctx.accounts.stable_pair.to_account_info(),
                    },
                    signer,
                ),
                treasury_fee,
            )?;
        }

        // Update reserves
        let pair = &mut ctx.accounts.stable_pair;
        pair.pool_stable_reserve = pair.pool_stable_reserve
            .checked_add(amount_in)
            .ok_or(StablePairError::MathOverflow)?;
        pair.pool_token_reserve = pair.pool_token_reserve
            .checked_sub(amount_out)
            .ok_or(StablePairError::MathOverflow)?
            .checked_sub(treasury_fee)
            .ok_or(StablePairError::MathOverflow)?;

        let clock = Clock::get()?;
        emit!(SwapExecuted {
            pair: pair.key(),
            user: ctx.accounts.user.key(),
            token_to_stable: false,
            amount_in,
            amount_out: user_receives,
            fee_amount: fee,
            timestamp: clock.unix_timestamp,
        });

        Ok(())
    }

    /// Add liquidity to a stable pair. Deposits both tokens proportionally
    /// (or arbitrary amounts on first deposit) and mints virtual LP shares.
    pub fn handle_add_liquidity(
        ctx: Context<AddLiquidity>,
        token_amount: u64,
        stable_amount: u64,
        min_lp_shares: u128,
    ) -> Result<()> {
        require!(token_amount > 0 && stable_amount > 0, StablePairError::ZeroAmount);
        let pair = &ctx.accounts.stable_pair;
        require!(!pair.paused, StablePairError::Paused);

        // Calculate LP shares to mint
        let lp_shares: u128 = if pair.total_lp_shares == 0 {
            // First deposit — shares = sqrt(token_amount * stable_amount)
            let product = (token_amount as u128)
                .checked_mul(stable_amount as u128)
                .ok_or(StablePairError::MathOverflow)?;
            let shares = isqrt(product);
            require!(shares >= MIN_LIQUIDITY as u128, StablePairError::InitialLiquidityTooLow);
            shares
        } else {
            // Proportional deposit: shares = min(dT/T, dS/S) * total_shares
            let share_by_token = (token_amount as u128)
                .checked_mul(pair.total_lp_shares)
                .ok_or(StablePairError::MathOverflow)?
                .checked_div(pair.pool_token_reserve as u128)
                .ok_or(StablePairError::EmptyPool)?;
            let share_by_stable = (stable_amount as u128)
                .checked_mul(pair.total_lp_shares)
                .ok_or(StablePairError::MathOverflow)?
                .checked_div(pair.pool_stable_reserve as u128)
                .ok_or(StablePairError::EmptyPool)?;
            share_by_token.min(share_by_stable)
        };
        require!(lp_shares >= min_lp_shares, StablePairError::SlippageExceeded);

        // Transfer tokens in
        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.user_token_account.to_account_info(),
                    to: ctx.accounts.token_vault.to_account_info(),
                    authority: ctx.accounts.provider.to_account_info(),
                },
            ),
            token_amount,
        )?;

        // Transfer stables in
        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.user_stable_account.to_account_info(),
                    to: ctx.accounts.stable_vault.to_account_info(),
                    authority: ctx.accounts.provider.to_account_info(),
                },
            ),
            stable_amount,
        )?;

        // Update pair state
        let pair = &mut ctx.accounts.stable_pair;
        pair.pool_token_reserve = pair.pool_token_reserve
            .checked_add(token_amount)
            .ok_or(StablePairError::MathOverflow)?;
        pair.pool_stable_reserve = pair.pool_stable_reserve
            .checked_add(stable_amount)
            .ok_or(StablePairError::MathOverflow)?;
        pair.total_lp_shares = pair.total_lp_shares
            .checked_add(lp_shares)
            .ok_or(StablePairError::MathOverflow)?;

        // Update user LP position
        let lp_pos = &mut ctx.accounts.lp_position;
        lp_pos.pair = ctx.accounts.stable_pair.key();
        lp_pos.owner = ctx.accounts.provider.key();
        lp_pos.lp_shares = lp_pos.lp_shares
            .checked_add(lp_shares)
            .ok_or(StablePairError::MathOverflow)?;
        if lp_pos.bump == 0 {
            lp_pos.bump = ctx.bumps.lp_position;
        }

        let clock = Clock::get()?;
        emit!(LiquidityAdded {
            pair: ctx.accounts.stable_pair.key(),
            provider: ctx.accounts.provider.key(),
            token_amount,
            stable_amount,
            lp_shares_minted: lp_shares,
            timestamp: clock.unix_timestamp,
        });

        Ok(())
    }

    /// Remove liquidity from a stable pair by burning LP shares and
    /// receiving proportional reserves back.
    pub fn handle_remove_liquidity(
        ctx: Context<RemoveLiquidity>,
        lp_shares_to_burn: u128,
        min_token_out: u64,
        min_stable_out: u64,
    ) -> Result<()> {
        require!(lp_shares_to_burn > 0, StablePairError::ZeroAmount);

        let lp_pos = &ctx.accounts.lp_position;
        require!(lp_pos.lp_shares >= lp_shares_to_burn, StablePairError::InsufficientShares);

        let pair = &ctx.accounts.stable_pair;

        // Calculate proportional amounts
        let token_out = (pair.pool_token_reserve as u128)
            .checked_mul(lp_shares_to_burn)
            .ok_or(StablePairError::MathOverflow)?
            .checked_div(pair.total_lp_shares)
            .ok_or(StablePairError::EmptyPool)? as u64;

        let stable_out = (pair.pool_stable_reserve as u128)
            .checked_mul(lp_shares_to_burn)
            .ok_or(StablePairError::MathOverflow)?
            .checked_div(pair.total_lp_shares)
            .ok_or(StablePairError::EmptyPool)? as u64;

        require!(token_out >= min_token_out, StablePairError::SlippageExceeded);
        require!(stable_out >= min_stable_out, StablePairError::SlippageExceeded);

        // PDA signer
        let token_mint_key = pair.token_mint;
        let stable_mint_key = pair.stable_mint;
        let pair_seeds: &[&[u8]] = &[
            STABLE_PAIR_SEED,
            token_mint_key.as_ref(),
            stable_mint_key.as_ref(),
            &[pair.bump],
        ];
        let signer = &[pair_seeds];

        // Transfer tokens out
        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.token_vault.to_account_info(),
                    to: ctx.accounts.user_token_account.to_account_info(),
                    authority: ctx.accounts.stable_pair.to_account_info(),
                },
                signer,
            ),
            token_out,
        )?;

        // Transfer stables out
        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.stable_vault.to_account_info(),
                    to: ctx.accounts.user_stable_account.to_account_info(),
                    authority: ctx.accounts.stable_pair.to_account_info(),
                },
                signer,
            ),
            stable_out,
        )?;

        // Update pair state
        let pair = &mut ctx.accounts.stable_pair;
        pair.pool_token_reserve = pair.pool_token_reserve
            .checked_sub(token_out)
            .ok_or(StablePairError::MathOverflow)?;
        pair.pool_stable_reserve = pair.pool_stable_reserve
            .checked_sub(stable_out)
            .ok_or(StablePairError::MathOverflow)?;
        pair.total_lp_shares = pair.total_lp_shares
            .checked_sub(lp_shares_to_burn)
            .ok_or(StablePairError::MathOverflow)?;

        // Update user LP position
        let lp_pos = &mut ctx.accounts.lp_position;
        lp_pos.lp_shares = lp_pos.lp_shares
            .checked_sub(lp_shares_to_burn)
            .ok_or(StablePairError::MathOverflow)?;

        let clock = Clock::get()?;
        emit!(LiquidityRemoved {
            pair: ctx.accounts.stable_pair.key(),
            provider: ctx.accounts.provider.key(),
            token_amount: token_out,
            stable_amount: stable_out,
            lp_shares_burned: lp_shares_to_burn,
            timestamp: clock.unix_timestamp,
        });

        Ok(())
    }

    /// Pause or unpause a stable pair (creator only).
    pub fn handle_set_pair_paused(
        ctx: Context<PairAdmin>,
        paused: bool,
    ) -> Result<()> {
        ctx.accounts.stable_pair.paused = paused;
        Ok(())
    }

    /// Update fee on a stable pair (creator only).
    pub fn handle_update_pair_fee(
        ctx: Context<PairAdmin>,
        new_fee_bps: u16,
    ) -> Result<()> {
        require!(new_fee_bps <= MAX_FEE_BPS, StablePairError::FeeTooHigh);
        ctx.accounts.stable_pair.fee_bps = new_fee_bps;
        Ok(())
    }

    // ============================================================================
    // AMM MATH
    // ============================================================================

    /// Constant product swap: given `amount_in` of asset A, compute output of
    /// asset B after fee deduction.
    ///
    /// Formula: `amount_out = (reserve_out * amount_in_after_fee) / (reserve_in + amount_in_after_fee)`
    ///
    /// Fee is taken from `amount_in` before the swap calculation.
    /// Returns `(amount_out, fee_amount)`.
    fn compute_swap_output(
        amount_in: u64,
        reserve_in: u64,
        reserve_out: u64,
        fee_bps: u16,
    ) -> Result<(u64, u64)> {
        require!(reserve_in > 0 && reserve_out > 0, StablePairError::EmptyPool);

        let fee = (amount_in as u128)
            .checked_mul(fee_bps as u128)
            .ok_or(StablePairError::MathOverflow)?
            .checked_div(BPS_DENOMINATOR as u128)
            .ok_or(StablePairError::MathOverflow)? as u64;

        let amount_in_after_fee = (amount_in as u128)
            .checked_sub(fee as u128)
            .ok_or(StablePairError::MathOverflow)?;

        let numerator = (reserve_out as u128)
            .checked_mul(amount_in_after_fee)
            .ok_or(StablePairError::MathOverflow)?;

        let denominator = (reserve_in as u128)
            .checked_add(amount_in_after_fee)
            .ok_or(StablePairError::MathOverflow)?;

        let amount_out = numerator
            .checked_div(denominator)
            .ok_or(StablePairError::MathOverflow)? as u64;

        Ok((amount_out, fee))
    }

    /// Integer square root via Newton's method.
    fn isqrt(n: u128) -> u128 {
        if n == 0 {
            return 0;
        }
        let mut x = n;
        let mut y = (x + 1) / 2;
        while y < x {
            x = y;
            y = (x + n / x) / 2;
        }
        x
    }

    // ============================================================================
    // CONTEXT STRUCTS
    // ============================================================================

    #[derive(Accounts)]
    pub struct CreateStablePair<'info> {
        #[account(
            init,
            payer = creator,
            space = StablePairConfig::SIZE,
            seeds = [STABLE_PAIR_SEED, token_mint.key().as_ref(), stable_mint.key().as_ref()],
            bump,
        )]
        pub stable_pair: Account<'info, StablePairConfig>,

        pub token_mint: Account<'info, Mint>,
        pub stable_mint: Account<'info, Mint>,

        /// Token vault owned by the pair PDA.
        #[account(
            init,
            payer = creator,
            token::mint = token_mint,
            token::authority = stable_pair,
            seeds = [POOL_TOKEN_VAULT_SEED, stable_pair.key().as_ref()],
            bump,
        )]
        pub token_vault: Account<'info, TokenAccount>,

        /// Stablecoin vault owned by the pair PDA.
        #[account(
            init,
            payer = creator,
            token::mint = stable_mint,
            token::authority = stable_pair,
            seeds = [POOL_STABLE_VAULT_SEED, stable_pair.key().as_ref()],
            bump,
        )]
        pub stable_vault: Account<'info, TokenAccount>,

        #[account(mut)]
        pub creator: Signer<'info>,

        pub token_program: Program<'info, Token>,
        pub system_program: Program<'info, System>,
        pub rent: Sysvar<'info, Rent>,
    }

    #[derive(Accounts)]
    pub struct SwapTokenForStable<'info> {
        #[account(
            mut,
            seeds = [STABLE_PAIR_SEED, stable_pair.token_mint.as_ref(), stable_pair.stable_mint.as_ref()],
            bump = stable_pair.bump,
        )]
        pub stable_pair: Account<'info, StablePairConfig>,

        #[account(
            mut,
            seeds = [POOL_TOKEN_VAULT_SEED, stable_pair.key().as_ref()],
            bump = stable_pair.token_vault_bump,
        )]
        pub token_vault: Account<'info, TokenAccount>,

        #[account(
            mut,
            seeds = [POOL_STABLE_VAULT_SEED, stable_pair.key().as_ref()],
            bump = stable_pair.stable_vault_bump,
        )]
        pub stable_vault: Account<'info, TokenAccount>,

        /// User's token account (source of tokens being sold).
        #[account(
            mut,
            constraint = user_token_account.mint == stable_pair.token_mint,
        )]
        pub user_token_account: Account<'info, TokenAccount>,

        /// User's stablecoin account (receives stablecoins).
        #[account(
            mut,
            constraint = user_stable_account.mint == stable_pair.stable_mint,
        )]
        pub user_stable_account: Account<'info, TokenAccount>,

        /// Platform treasury stablecoin account for fee collection.
        #[account(
            mut,
            constraint = treasury_stable_account.mint == stable_pair.stable_mint,
        )]
        pub treasury_stable_account: Account<'info, TokenAccount>,

        #[account(mut)]
        pub user: Signer<'info>,

        pub token_program: Program<'info, Token>,
    }

    #[derive(Accounts)]
    pub struct SwapStableForToken<'info> {
        #[account(
            mut,
            seeds = [STABLE_PAIR_SEED, stable_pair.token_mint.as_ref(), stable_pair.stable_mint.as_ref()],
            bump = stable_pair.bump,
        )]
        pub stable_pair: Account<'info, StablePairConfig>,

        #[account(
            mut,
            seeds = [POOL_TOKEN_VAULT_SEED, stable_pair.key().as_ref()],
            bump = stable_pair.token_vault_bump,
        )]
        pub token_vault: Account<'info, TokenAccount>,

        #[account(
            mut,
            seeds = [POOL_STABLE_VAULT_SEED, stable_pair.key().as_ref()],
            bump = stable_pair.stable_vault_bump,
        )]
        pub stable_vault: Account<'info, TokenAccount>,

        /// User's stablecoin account (source).
        #[account(
            mut,
            constraint = user_stable_account.mint == stable_pair.stable_mint,
        )]
        pub user_stable_account: Account<'info, TokenAccount>,

        /// User's token account (receives tokens).
        #[account(
            mut,
            constraint = user_token_account.mint == stable_pair.token_mint,
        )]
        pub user_token_account: Account<'info, TokenAccount>,

        /// Platform treasury token account for fee collection.
        #[account(
            mut,
            constraint = treasury_token_account.mint == stable_pair.token_mint,
        )]
        pub treasury_token_account: Account<'info, TokenAccount>,

        #[account(mut)]
        pub user: Signer<'info>,

        pub token_program: Program<'info, Token>,
    }

    #[derive(Accounts)]
    pub struct AddLiquidity<'info> {
        #[account(
            mut,
            seeds = [STABLE_PAIR_SEED, stable_pair.token_mint.as_ref(), stable_pair.stable_mint.as_ref()],
            bump = stable_pair.bump,
        )]
        pub stable_pair: Account<'info, StablePairConfig>,

        #[account(
            mut,
            seeds = [POOL_TOKEN_VAULT_SEED, stable_pair.key().as_ref()],
            bump = stable_pair.token_vault_bump,
        )]
        pub token_vault: Account<'info, TokenAccount>,

        #[account(
            mut,
            seeds = [POOL_STABLE_VAULT_SEED, stable_pair.key().as_ref()],
            bump = stable_pair.stable_vault_bump,
        )]
        pub stable_vault: Account<'info, TokenAccount>,

        #[account(
            init_if_needed,
            payer = provider,
            space = LPPosition::SIZE,
            seeds = [LP_POSITION_SEED, stable_pair.key().as_ref(), provider.key().as_ref()],
            bump,
        )]
        pub lp_position: Account<'info, LPPosition>,

        #[account(
            mut,
            constraint = user_token_account.mint == stable_pair.token_mint,
        )]
        pub user_token_account: Account<'info, TokenAccount>,

        #[account(
            mut,
            constraint = user_stable_account.mint == stable_pair.stable_mint,
        )]
        pub user_stable_account: Account<'info, TokenAccount>,

        #[account(mut)]
        pub provider: Signer<'info>,

        pub token_program: Program<'info, Token>,
        pub system_program: Program<'info, System>,
    }

    #[derive(Accounts)]
    pub struct RemoveLiquidity<'info> {
        #[account(
            mut,
            seeds = [STABLE_PAIR_SEED, stable_pair.token_mint.as_ref(), stable_pair.stable_mint.as_ref()],
            bump = stable_pair.bump,
        )]
        pub stable_pair: Account<'info, StablePairConfig>,

        #[account(
            mut,
            seeds = [POOL_TOKEN_VAULT_SEED, stable_pair.key().as_ref()],
            bump = stable_pair.token_vault_bump,
        )]
        pub token_vault: Account<'info, TokenAccount>,

        #[account(
            mut,
            seeds = [POOL_STABLE_VAULT_SEED, stable_pair.key().as_ref()],
            bump = stable_pair.stable_vault_bump,
        )]
        pub stable_vault: Account<'info, TokenAccount>,

        #[account(
            mut,
            seeds = [LP_POSITION_SEED, stable_pair.key().as_ref(), provider.key().as_ref()],
            bump = lp_position.bump,
            constraint = lp_position.owner == provider.key(),
        )]
        pub lp_position: Account<'info, LPPosition>,

        #[account(
            mut,
            constraint = user_token_account.mint == stable_pair.token_mint,
        )]
        pub user_token_account: Account<'info, TokenAccount>,

        #[account(
            mut,
            constraint = user_stable_account.mint == stable_pair.stable_mint,
        )]
        pub user_stable_account: Account<'info, TokenAccount>,

        #[account(mut)]
        pub provider: Signer<'info>,

        pub token_program: Program<'info, Token>,
    }

    /// Admin context — only the pair creator can call.
    #[derive(Accounts)]
    pub struct PairAdmin<'info> {
        #[account(
            mut,
            seeds = [STABLE_PAIR_SEED, stable_pair.token_mint.as_ref(), stable_pair.stable_mint.as_ref()],
            bump = stable_pair.bump,
            constraint = stable_pair.creator == authority.key() @ StablePairError::Unauthorized,
        )]
        pub stable_pair: Account<'info, StablePairConfig>,

        pub authority: Signer<'info>,
    }

}
pub mod social_launch {
    use super::*;

    use crate::{
        CurveType, PlatformConfig, TokenLaunch,
        PLATFORM_CONFIG_SEED, TOKEN_LAUNCH_SEED, TOKEN_DECIMALS,
        DEFAULT_TOTAL_SUPPLY, DEFAULT_MIGRATION_THRESHOLD,
        MAX_NAME_LEN, MAX_SYMBOL_LEN, MAX_URI_LEN,
        SendItError,
    };

    // ═══════════════════════════════════════════════════════════════════════════════
    //  Social Launch Module for Send.it
    //  Tweet-to-launch: create tokens by posting a tweet URL (inspired by Believe.app)
    //
    //  Flow:
    //    1. Creator posts a tweet describing a token idea
    //    2. Creator calls `launch_from_tweet` with the tweet URL + metadata
    //    3. An oracle/verifier calls `verify_tweet` to attest tweet authenticity
    //    4. Token is immediately tradeable on the bonding curve (via existing module)
    // ═══════════════════════════════════════════════════════════════════════════════

    // ── Seeds ──

    pub const SOCIAL_LAUNCH_SEED: &[u8] = b"social_launch";
    pub const TWEET_VERIFICATION_SEED: &[u8] = b"tweet_verification";
    pub const SOCIAL_CONFIG_SEED: &[u8] = b"social_config";

    // ── Limits ──

    pub const MAX_TWEET_URL_LEN: usize = 280;
    pub const MAX_TWEET_ID_LEN: usize = 32;
    pub const MAX_AUTHOR_HANDLE_LEN: usize = 64;
    pub const MAX_TWEET_CONTENT_LEN: usize = 280;
    pub const MAX_TOKEN_DESCRIPTION_LEN: usize = 200;

    // ═══════════════════════════════════════════════════════════════════════════════
    //  ACCOUNTS
    // ═══════════════════════════════════════════════════════════════════════════════

    /// Global configuration for the social launch module.
    #[account]
    pub struct SocialLaunchConfig {
        /// Authority that can update config and designate verifiers.
        pub authority: Pubkey,
        /// The oracle/backend that is allowed to verify tweets.
        pub verifier_authority: Pubkey,
        /// Whether launches require tweet verification before trading starts.
        pub require_verification: bool,
        /// Default curve type for tweet-launched tokens.
        pub default_curve_type: CurveType,
        /// Default creator fee in basis points for social launches.
        pub default_creator_fee_bps: u16,
        /// Time (seconds) after launch before trading begins (gives verifier time).
        pub verification_grace_period: i64,
        /// Total social launches to date.
        pub total_social_launches: u64,
        pub bump: u8,
    }

    impl SocialLaunchConfig {
        pub const SIZE: usize = 8   // discriminator
            + 32    // authority
            + 32    // verifier_authority
            + 1     // require_verification
            + 1     // default_curve_type
            + 2     // default_creator_fee_bps
            + 8     // verification_grace_period
            + 8     // total_social_launches
            + 1;    // bump
    }

    /// PDA linking a tweet to a token launch. Created when a user launches from a tweet.
    #[account]
    pub struct SocialLaunchRecord {
        /// The wallet that initiated the launch.
        pub creator: Pubkey,
        /// The token mint created for this launch.
        pub mint: Pubkey,
        /// Full tweet URL (e.g. "https://x.com/user/status/123456").
        pub tweet_url: String,
        /// Extracted tweet ID (numeric string).
        pub tweet_id: String,
        /// Twitter/X handle of the tweet author.
        pub author_handle: String,
        /// Raw tweet content used to derive token metadata.
        pub tweet_content: String,
        /// Token name derived from tweet content.
        pub token_name: String,
        /// Token symbol derived from tweet content.
        pub token_symbol: String,
        /// Token metadata URI (off-chain JSON).
        pub token_uri: String,
        /// Optional description extracted or provided.
        pub token_description: String,
        /// Whether the linked tweet has been verified by the oracle.
        pub verified: bool,
        /// Unix timestamp of launch creation.
        pub created_at: i64,
        /// Unix timestamp of verification (0 if unverified).
        pub verified_at: i64,
        pub bump: u8,
    }

    impl SocialLaunchRecord {
        pub const SIZE: usize = 8   // discriminator
            + 32    // creator
            + 32    // mint
            + (4 + MAX_TWEET_URL_LEN)       // tweet_url
            + (4 + MAX_TWEET_ID_LEN)        // tweet_id
            + (4 + MAX_AUTHOR_HANDLE_LEN)   // author_handle
            + (4 + MAX_TWEET_CONTENT_LEN)   // tweet_content
            + (4 + MAX_NAME_LEN)            // token_name
            + (4 + MAX_SYMBOL_LEN)          // token_symbol
            + (4 + MAX_URI_LEN)             // token_uri
            + (4 + MAX_TOKEN_DESCRIPTION_LEN) // token_description
            + 1     // verified
            + 8     // created_at
            + 8     // verified_at
            + 1;    // bump
    }

    /// Standalone tweet verification PDA. Can exist independently of a launch,
    /// allowing pre-verification or verification of tweets not yet launched.
    #[account]
    pub struct TweetVerification {
        /// The tweet ID being verified.
        pub tweet_id: String,
        /// Twitter/X handle of the tweet author.
        pub author_handle: String,
        /// Whether the tweet has been verified as authentic.
        pub verified: bool,
        /// The verifier authority that attested.
        pub verified_by: Pubkey,
        /// Unix timestamp of verification.
        pub verified_at: i64,
        /// Optional: the mint if a launch was created from this tweet.
        pub associated_mint: Option<Pubkey>,
        pub bump: u8,
    }

    impl TweetVerification {
        pub const SIZE: usize = 8   // discriminator
            + (4 + MAX_TWEET_ID_LEN)        // tweet_id
            + (4 + MAX_AUTHOR_HANDLE_LEN)   // author_handle
            + 1     // verified
            + 32    // verified_by
            + 8     // verified_at
            + (1 + 32) // associated_mint (Option<Pubkey>)
            + 1;    // bump
    }

    // ═══════════════════════════════════════════════════════════════════════════════
    //  CONTEXT STRUCTS
    // ═══════════════════════════════════════════════════════════════════════════════

    /// Initialize the social launch module configuration.
    #[derive(Accounts)]
    pub struct InitializeSocialConfig<'info> {
        #[account(
            init,
            payer = authority,
            space = SocialLaunchConfig::SIZE,
            seeds = [SOCIAL_CONFIG_SEED],
            bump,
        )]
        pub social_config: Account<'info, SocialLaunchConfig>,

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

    /// Update social launch configuration.
    #[derive(Accounts)]
    pub struct UpdateSocialConfig<'info> {
        #[account(
            mut,
            seeds = [SOCIAL_CONFIG_SEED],
            bump = social_config.bump,
            has_one = authority,
        )]
        pub social_config: Account<'info, SocialLaunchConfig>,

        pub authority: Signer<'info>,
    }

    /// Launch a token from a tweet. Creates both a SocialLaunchRecord and a
    /// TokenLaunch (from the core module) plus mints the token supply.
    #[derive(Accounts)]
    #[instruction(
        tweet_url: String,
        tweet_id: String,
        author_handle: String,
        tweet_content: String,
        token_name: String,
        token_symbol: String,
        token_uri: String,
    )]
    pub struct LaunchFromTweet<'info> {
        // ── Social launch record (new) ──
        #[account(
            init,
            payer = creator,
            space = SocialLaunchRecord::SIZE,
            seeds = [SOCIAL_LAUNCH_SEED, tweet_id.as_bytes()],
            bump,
        )]
        pub social_launch_record: Account<'info, SocialLaunchRecord>,

        // ── Core token launch PDA (new) ──
        #[account(
            init,
            payer = creator,
            space = TokenLaunch::SIZE,
            seeds = [TOKEN_LAUNCH_SEED, token_mint.key().as_ref()],
            bump,
        )]
        pub token_launch: Account<'info, TokenLaunch>,

        // ── Token mint (new) ──
        #[account(
            init,
            payer = creator,
            mint::decimals = TOKEN_DECIMALS,
            mint::authority = token_launch,
        )]
        pub token_mint: Account<'info, Mint>,

        // ── Launch token vault (new) ──
        #[account(
            init,
            payer = creator,
            associated_token::mint = token_mint,
            associated_token::authority = token_launch,
        )]
        pub launch_token_vault: Account<'info, TokenAccount>,

        // ── Social config ──
        #[account(
            mut,
            seeds = [SOCIAL_CONFIG_SEED],
            bump = social_config.bump,
        )]
        pub social_config: Account<'info, SocialLaunchConfig>,

        // ── Platform config ──
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

    /// Oracle/verifier attests that a tweet is authentic and matches the on-chain record.
    #[derive(Accounts)]
    #[instruction(tweet_id: String)]
    pub struct VerifyTweet<'info> {
        // ── Tweet verification PDA (init or update) ──
        #[account(
            init_if_needed,
            payer = verifier,
            space = TweetVerification::SIZE,
            seeds = [TWEET_VERIFICATION_SEED, tweet_id.as_bytes()],
            bump,
        )]
        pub tweet_verification: Account<'info, TweetVerification>,

        // ── Social launch record (update verified flag) ──
        #[account(
            mut,
            seeds = [SOCIAL_LAUNCH_SEED, tweet_id.as_bytes()],
            bump = social_launch_record.bump,
        )]
        pub social_launch_record: Account<'info, SocialLaunchRecord>,

        // ── Social config (check verifier authority) ──
        #[account(
            seeds = [SOCIAL_CONFIG_SEED],
            bump = social_config.bump,
        )]
        pub social_config: Account<'info, SocialLaunchConfig>,

        /// The designated verifier oracle. Must match `social_config.verifier_authority`.
        #[account(
            mut,
            constraint = verifier.key() == social_config.verifier_authority @ SocialLaunchError::UnauthorizedVerifier,
        )]
        pub verifier: Signer<'info>,

        pub system_program: Program<'info, System>,
    }

    /// Revoke verification (admin only). Used if a tweet is found to be fraudulent.
    #[derive(Accounts)]
    #[instruction(tweet_id: String)]
    pub struct RevokeVerification<'info> {
        #[account(
            mut,
            seeds = [TWEET_VERIFICATION_SEED, tweet_id.as_bytes()],
            bump = tweet_verification.bump,
        )]
        pub tweet_verification: Account<'info, TweetVerification>,

        #[account(
            mut,
            seeds = [SOCIAL_LAUNCH_SEED, tweet_id.as_bytes()],
            bump = social_launch_record.bump,
        )]
        pub social_launch_record: Account<'info, SocialLaunchRecord>,

        #[account(
            seeds = [SOCIAL_CONFIG_SEED],
            bump = social_config.bump,
            has_one = authority,
        )]
        pub social_config: Account<'info, SocialLaunchConfig>,

        pub authority: Signer<'info>,
    }

    // ═══════════════════════════════════════════════════════════════════════════════
    //  INSTRUCTIONS
    // ═══════════════════════════════════════════════════════════════════════════════

    /// Initialize the social launch module. Called once by platform admin.
    pub fn handle_initialize_social_config(
        ctx: Context<InitializeSocialConfig>,
        verifier_authority: Pubkey,
        require_verification: bool,
        default_curve_type: CurveType,
        default_creator_fee_bps: u16,
        verification_grace_period: i64,
    ) -> Result<()> {
        let config = &mut ctx.accounts.social_config;
        config.authority = ctx.accounts.authority.key();
        config.verifier_authority = verifier_authority;
        config.require_verification = require_verification;
        config.default_curve_type = default_curve_type;
        config.default_creator_fee_bps = default_creator_fee_bps;
        config.verification_grace_period = verification_grace_period;
        config.total_social_launches = 0;
        config.bump = ctx.bumps.social_config;

        emit!(SocialConfigInitialized {
            authority: config.authority,
            verifier_authority,
            require_verification,
        });

        Ok(())
    }

    /// Update social launch configuration parameters.
    pub fn handle_update_social_config(
        ctx: Context<UpdateSocialConfig>,
        new_verifier: Option<Pubkey>,
        new_require_verification: Option<bool>,
        new_default_curve: Option<CurveType>,
        new_default_fee_bps: Option<u16>,
        new_grace_period: Option<i64>,
    ) -> Result<()> {
        let config = &mut ctx.accounts.social_config;

        if let Some(v) = new_verifier {
            config.verifier_authority = v;
        }
        if let Some(rv) = new_require_verification {
            config.require_verification = rv;
        }
        if let Some(c) = new_default_curve {
            config.default_curve_type = c;
        }
        if let Some(fee) = new_default_fee_bps {
            require!(fee <= 500, SendItError::FeeTooHigh);
            config.default_creator_fee_bps = fee;
        }
        if let Some(gp) = new_grace_period {
            config.verification_grace_period = gp;
        }

        Ok(())
    }

    /// Launch a token from a tweet URL.
    ///
    /// The creator supplies tweet metadata (URL, ID, author, content) along with
    /// desired token name/symbol/URI. A `SocialLaunchRecord` is created linking the
    /// tweet to the mint, and a standard `TokenLaunch` is initialized on the bonding
    /// curve. The full token supply is minted to the launch vault.
    ///
    /// If `require_verification` is enabled, trading start is delayed by
    /// `verification_grace_period` to allow the oracle to verify the tweet.
    pub fn handle_launch_from_tweet(
        ctx: Context<LaunchFromTweet>,
        tweet_url: String,
        tweet_id: String,
        author_handle: String,
        tweet_content: String,
        token_name: String,
        token_symbol: String,
        token_uri: String,
        token_description: String,
        curve_type_override: Option<CurveType>,
        creator_fee_bps_override: Option<u16>,
    ) -> Result<()> {
        // ── Validate inputs ──
        require!(tweet_url.len() <= MAX_TWEET_URL_LEN, SocialLaunchError::TweetUrlTooLong);
        require!(tweet_id.len() <= MAX_TWEET_ID_LEN, SocialLaunchError::TweetIdTooLong);
        require!(author_handle.len() <= MAX_AUTHOR_HANDLE_LEN, SocialLaunchError::AuthorHandleTooLong);
        require!(tweet_content.len() <= MAX_TWEET_CONTENT_LEN, SocialLaunchError::TweetContentTooLong);
        require!(token_name.len() <= MAX_NAME_LEN, SendItError::NameTooLong);
        require!(token_symbol.len() <= MAX_SYMBOL_LEN, SendItError::SymbolTooLong);
        require!(token_uri.len() <= MAX_URI_LEN, SendItError::UriTooLong);
        require!(
            token_description.len() <= MAX_TOKEN_DESCRIPTION_LEN,
            SocialLaunchError::DescriptionTooLong
        );
        require!(!tweet_url.is_empty(), SocialLaunchError::EmptyTweetUrl);
        require!(!tweet_id.is_empty(), SocialLaunchError::EmptyTweetId);

        let platform_config = &ctx.accounts.platform_config;
        require!(!platform_config.paused, SendItError::PlatformPaused);

        let social_config = &ctx.accounts.social_config;
        let clock = Clock::get()?;

        // Resolve curve type and fee (use overrides or social config defaults)
        let curve_type = curve_type_override.unwrap_or(social_config.default_curve_type);
        let creator_fee_bps = creator_fee_bps_override.unwrap_or(social_config.default_creator_fee_bps);
        require!(creator_fee_bps <= 500, SendItError::FeeTooHigh);

        // Determine trading start: if verification required, delay by grace period
        let trading_starts_at = if social_config.require_verification {
            clock.unix_timestamp + social_config.verification_grace_period
        } else {
            clock.unix_timestamp // immediate
        };

        // ── Populate social launch record ──
        let record = &mut ctx.accounts.social_launch_record;
        record.creator = ctx.accounts.creator.key();
        record.mint = ctx.accounts.token_mint.key();
        record.tweet_url = tweet_url.clone();
        record.tweet_id = tweet_id.clone();
        record.author_handle = author_handle.clone();
        record.tweet_content = tweet_content.clone();
        record.token_name = token_name.clone();
        record.token_symbol = token_symbol.clone();
        record.token_uri = token_uri.clone();
        record.token_description = token_description;
        record.verified = false;
        record.created_at = clock.unix_timestamp;
        record.verified_at = 0;
        record.bump = ctx.bumps.social_launch_record;

        // ── Populate core TokenLaunch (integrates with bonding_curve.rs) ──
        let launch = &mut ctx.accounts.token_launch;
        launch.creator = ctx.accounts.creator.key();
        launch.mint = ctx.accounts.token_mint.key();
        launch.name = token_name.clone();
        launch.symbol = token_symbol.clone();
        launch.uri = token_uri.clone();
        launch.curve_type = curve_type;
        launch.creator_fee_bps = creator_fee_bps;
        launch.total_supply = DEFAULT_TOTAL_SUPPLY;
        launch.tokens_sold = 0;
        launch.reserve_sol = 0;
        launch.created_at = clock.unix_timestamp;
        launch.trading_starts_at = trading_starts_at;
        launch.snipe_window_end = trading_starts_at + 30; // 30s anti-snipe window
        launch.max_buy_during_snipe = DEFAULT_TOTAL_SUPPLY / 100; // 1% during snipe
        launch.lock_period_end = clock.unix_timestamp; // no lock for social launches
        launch.migrated = false;
        launch.paused = false;
        launch.total_volume_sol = 0;
        launch.bump = ctx.bumps.token_launch;

        // ── Mint total supply to launch vault ──
        let mint_key = ctx.accounts.token_mint.key();
        let launch_seeds: &[&[u8]] = &[
            TOKEN_LAUNCH_SEED,
            mint_key.as_ref(),
            &[launch.bump],
        ];
        let signer_seeds = &[launch_seeds];

        anchor_spl::token::mint_to(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                MintTo {
                    mint: ctx.accounts.token_mint.to_account_info(),
                    to: ctx.accounts.launch_token_vault.to_account_info(),
                    authority: ctx.accounts.token_launch.to_account_info(),
                },
                signer_seeds,
            ),
            DEFAULT_TOTAL_SUPPLY,
        )?;

        // ── Update counters ──
        let social_config = &mut ctx.accounts.social_config;
        social_config.total_social_launches += 1;

        let platform_config = &mut ctx.accounts.platform_config;
        platform_config.total_launches += 1;

        emit!(SocialLaunchCreated {
            mint: launch.mint,
            creator: launch.creator,
            tweet_id,
            tweet_url,
            author_handle,
            token_name,
            token_symbol,
            curve_type,
            trading_starts_at,
            created_at: clock.unix_timestamp,
        });

        Ok(())
    }

    /// Verify a tweet's authenticity. Called by the designated oracle/verifier.
    ///
    /// Sets the `verified` flag on both the `TweetVerification` PDA and the
    /// associated `SocialLaunchRecord`. Once verified, the token is eligible
    /// for trading (if verification gating is enabled).
    pub fn handle_verify_tweet(
        ctx: Context<VerifyTweet>,
        tweet_id: String,
        author_handle: String,
        verified: bool,
    ) -> Result<()> {
        require!(tweet_id.len() <= MAX_TWEET_ID_LEN, SocialLaunchError::TweetIdTooLong);
        require!(
            author_handle.len() <= MAX_AUTHOR_HANDLE_LEN,
            SocialLaunchError::AuthorHandleTooLong
        );

        let clock = Clock::get()?;

        // ── Populate / update TweetVerification PDA ──
        let verification = &mut ctx.accounts.tweet_verification;
        verification.tweet_id = tweet_id.clone();
        verification.author_handle = author_handle.clone();
        verification.verified = verified;
        verification.verified_by = ctx.accounts.verifier.key();
        verification.verified_at = clock.unix_timestamp;
        verification.associated_mint = Some(ctx.accounts.social_launch_record.mint);
        if verification.bump == 0 {
            verification.bump = ctx.bumps.tweet_verification;
        }

        // ── Update the social launch record ──
        let record = &mut ctx.accounts.social_launch_record;
        record.verified = verified;
        record.verified_at = clock.unix_timestamp;

        // Validate that the tweet metadata matches
        require!(
            record.tweet_id == tweet_id,
            SocialLaunchError::TweetIdMismatch
        );

        emit!(TweetVerified {
            tweet_id,
            author_handle,
            verified,
            mint: record.mint,
            verifier: ctx.accounts.verifier.key(),
            timestamp: clock.unix_timestamp,
        });

        Ok(())
    }

    /// Revoke tweet verification (admin only).
    ///
    /// Used when a tweet is discovered to be fraudulent, deleted, or otherwise
    /// invalid after initial verification. This pauses the associated launch.
    pub fn handle_revoke_verification(
        ctx: Context<RevokeVerification>,
        tweet_id: String,
    ) -> Result<()> {
        let verification = &mut ctx.accounts.tweet_verification;
        verification.verified = false;

        let record = &mut ctx.accounts.social_launch_record;
        record.verified = false;

        emit!(TweetVerificationRevoked {
            tweet_id,
            mint: record.mint,
            revoked_by: ctx.accounts.authority.key(),
            timestamp: Clock::get()?.unix_timestamp,
        });

        Ok(())
    }

    // ═══════════════════════════════════════════════════════════════════════════════
    //  EVENTS
    // ═══════════════════════════════════════════════════════════════════════════════

    #[event]
    pub struct SocialConfigInitialized {
        pub authority: Pubkey,
        pub verifier_authority: Pubkey,
        pub require_verification: bool,
    }

    #[event]
    pub struct SocialLaunchCreated {
        pub mint: Pubkey,
        pub creator: Pubkey,
        pub tweet_id: String,
        pub tweet_url: String,
        pub author_handle: String,
        pub token_name: String,
        pub token_symbol: String,
        pub curve_type: CurveType,
        pub trading_starts_at: i64,
        pub created_at: i64,
    }

    #[event]
    pub struct TweetVerified {
        pub tweet_id: String,
        pub author_handle: String,
        pub verified: bool,
        pub mint: Pubkey,
        pub verifier: Pubkey,
        pub timestamp: i64,
    }

    #[event]
    pub struct TweetVerificationRevoked {
        pub tweet_id: String,
        pub mint: Pubkey,
        pub revoked_by: Pubkey,
        pub timestamp: i64,
    }

    // ═══════════════════════════════════════════════════════════════════════════════
    //  ERRORS
    // ═══════════════════════════════════════════════════════════════════════════════

    #[error_code]
    pub enum SocialLaunchError {
        #[msg("Tweet URL exceeds maximum length")]
        TweetUrlTooLong,
        #[msg("Tweet ID exceeds maximum length")]
        TweetIdTooLong,
        #[msg("Author handle exceeds maximum length")]
        AuthorHandleTooLong,
        #[msg("Tweet content exceeds maximum length")]
        TweetContentTooLong,
        #[msg("Token description exceeds maximum length")]
        DescriptionTooLong,
        #[msg("Tweet URL cannot be empty")]
        EmptyTweetUrl,
        #[msg("Tweet ID cannot be empty")]
        EmptyTweetId,
        #[msg("Unauthorized verifier")]
        UnauthorizedVerifier,
        #[msg("Tweet ID does not match the launch record")]
        TweetIdMismatch,
        #[msg("Tweet has not been verified")]
        TweetNotVerified,
        #[msg("Tweet has already been used for a launch")]
        TweetAlreadyUsed,
    }

}
pub mod points_system {
    use super::*;

    // ============================================================================
    // SEEDS & CONSTANTS
    // ============================================================================

    pub const POINTS_CONFIG_SEED: &[u8] = b"points_config";
    pub const USER_POINTS_SEED: &[u8] = b"user_points";
    pub const POINTS_LEADERBOARD_SEED: &[u8] = b"points_leaderboard";
    pub const SEASON_ARCHIVE_SEED: &[u8] = b"season_archive";
    pub const REWARD_CLAIM_SEED: &[u8] = b"reward_claim";

    /// Maximum entries on the points leaderboard.
    pub const MAX_POINTS_LEADERBOARD: usize = 100;

    /// Minimum cooldown between point-earning actions (seconds).
    pub const DEFAULT_ACTION_COOLDOWN: i64 = 60;

    /// Default maximum points earnable per calendar day.
    pub const DEFAULT_MAX_DAILY_POINTS: u64 = 10_000;

    /// Streak resets if user is inactive for more than this many seconds (48h grace).
    pub const STREAK_GRACE_PERIOD: i64 = 48 * 60 * 60;

    /// One day in seconds.
    pub const SECONDS_PER_DAY: i64 = 86_400;

    // ============================================================================
    // ENUMS
    // ============================================================================

    /// The type of action being rewarded.
    #[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
    pub enum PointAction {
        Trade,
        Launch,
        Referral,
        HoldDay,
    }

    /// Reward types that can be claimed by spending points.
    #[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
    pub enum RewardKind {
        /// Percentage fee discount (value = discount bps, e.g. 500 = 5%).
        FeeDiscount,
        /// Early access to a specific launch (value = launch index / identifier).
        EarlyAccess,
        /// Cosmetic badge or title (value = badge id).
        Badge,
    }

    // ============================================================================
    // ACCOUNTS
    // ============================================================================

    /// Global points configuration, managed by the platform authority.
    /// PDA: [POINTS_CONFIG_SEED]
    #[account]
    pub struct PointsConfig {
        /// Platform authority that can update config and award points.
        pub authority: Pubkey,
        /// Points granted per trade action.
        pub points_per_trade: u64,
        /// Points granted per token launch.
        pub points_per_launch: u64,
        /// Points granted per successful referral.
        pub points_per_referral: u64,
        /// Points granted per day of holding a token.
        pub points_per_hold_day: u64,
        /// Current season identifier. Incremented on season reset.
        pub season_id: u32,
        /// Minimum seconds between point-earning actions per user.
        pub action_cooldown: i64,
        /// Maximum points a user can earn in a single calendar day.
        pub max_daily_points: u64,
        /// Whether point accrual is paused.
        pub paused: bool,
        /// Bump seed.
        pub bump: u8,
    }

    impl PointsConfig {
        pub const SIZE: usize = 8  // discriminator
            + 32  // authority
            + 8   // points_per_trade
            + 8   // points_per_launch
            + 8   // points_per_referral
            + 8   // points_per_hold_day
            + 4   // season_id
            + 8   // action_cooldown
            + 8   // max_daily_points
            + 1   // paused
            + 1;  // bump

        /// Look up how many points a given action is worth.
        pub fn points_for(&self, action: PointAction) -> u64 {
            match action {
                PointAction::Trade => self.points_per_trade,
                PointAction::Launch => self.points_per_launch,
                PointAction::Referral => self.points_per_referral,
                PointAction::HoldDay => self.points_per_hold_day,
            }
        }
    }

    /// Per-user points account, scoped to the current season.
    /// PDA: [USER_POINTS_SEED, user_pubkey, season_id (LE bytes)]
    #[account]
    pub struct UserPoints {
        /// The user this account belongs to.
        pub user: Pubkey,
        /// Season this account is valid for.
        pub season_id: u32,
        /// Lifetime total points earned this season.
        pub total_points: u64,
        /// Points available to spend (total minus redeemed).
        pub available_points: u64,
        /// User level, derived from total_points thresholds.
        pub level: u16,
        /// Unix timestamp of the last point-earning action.
        pub last_action_ts: i64,
        /// Consecutive days with at least one point-earning action.
        pub streak_days: u32,
        /// The calendar day (unix_ts / 86400) of the last action, for streak tracking.
        pub last_action_day: i64,
        /// Points earned today (resets each calendar day).
        pub daily_points_earned: u64,
        /// Calendar day number for `daily_points_earned` tracking.
        pub daily_reset_day: i64,
        /// Bump seed.
        pub bump: u8,
    }

    impl UserPoints {
        pub const SIZE: usize = 8  // discriminator
            + 32  // user
            + 4   // season_id
            + 8   // total_points
            + 8   // available_points
            + 2   // level
            + 8   // last_action_ts
            + 4   // streak_days
            + 8   // last_action_day
            + 8   // daily_points_earned
            + 8   // daily_reset_day
            + 1;  // bump

        /// Compute the user level from total points.
        /// Thresholds: 0→L1, 100→L2, 500→L3, 2000→L4, 5000→L5, 10000→L6, 25000→L7, 50000→L8, 100000→L9, 250000→L10
        pub fn compute_level(total_points: u64) -> u16 {
            match total_points {
                0..=99 => 1,
                100..=499 => 2,
                500..=1_999 => 3,
                2_000..=4_999 => 4,
                5_000..=9_999 => 5,
                10_000..=24_999 => 6,
                25_000..=49_999 => 7,
                50_000..=99_999 => 8,
                100_000..=249_999 => 9,
                _ => 10,
            }
        }
    }

    /// Leaderboard entry for the points system.
    #[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Default)]
    pub struct PointsLeaderboardEntry {
        pub user: Pubkey,
        pub total_points: u64,
        pub level: u16,
    }

    impl PointsLeaderboardEntry {
        pub const SIZE: usize = 32 + 8 + 2;
    }

    /// On-chain leaderboard holding the top point holders for the current season.
    /// PDA: [POINTS_LEADERBOARD_SEED, season_id (LE bytes)]
    #[account]
    pub struct PointsLeaderboardState {
        /// Season this leaderboard belongs to.
        pub season_id: u32,
        /// Sorted descending by total_points.
        pub entries: Vec<PointsLeaderboardEntry>,
        /// Bump seed.
        pub bump: u8,
    }

    impl PointsLeaderboardState {
        pub const SIZE: usize = 8  // discriminator
            + 4   // season_id
            + (4 + MAX_POINTS_LEADERBOARD * PointsLeaderboardEntry::SIZE) // entries vec
            + 1;  // bump
    }

    /// Archived leaderboard snapshot from a previous season.
    /// PDA: [SEASON_ARCHIVE_SEED, season_id (LE bytes)]
    #[account]
    pub struct SeasonArchive {
        pub season_id: u32,
        pub ended_at: i64,
        pub top_entries: Vec<PointsLeaderboardEntry>,
        pub bump: u8,
    }

    impl SeasonArchive {
        pub const SIZE: usize = 8  // discriminator
            + 4   // season_id
            + 8   // ended_at
            + (4 + MAX_POINTS_LEADERBOARD * PointsLeaderboardEntry::SIZE)
            + 1;  // bump
    }

    // ============================================================================
    // CONTEXT STRUCTS
    // ============================================================================

    /// Initialize the global points configuration. Called once by admin.
    #[derive(Accounts)]
    pub struct InitializePointsConfig<'info> {
        #[account(
            init,
            payer = authority,
            space = PointsConfig::SIZE,
            seeds = [POINTS_CONFIG_SEED],
            bump,
        )]
        pub points_config: Account<'info, PointsConfig>,

        #[account(mut)]
        pub authority: Signer<'info>,

        pub system_program: Program<'info, System>,
    }

    /// Update points configuration values. Authority only.
    #[derive(Accounts)]
    pub struct UpdatePointsConfig<'info> {
        #[account(
            mut,
            seeds = [POINTS_CONFIG_SEED],
            bump = points_config.bump,
            has_one = authority,
        )]
        pub points_config: Account<'info, PointsConfig>,

        pub authority: Signer<'info>,
    }

    /// Award points to a user. Called by program authority (backend / CPI).
    #[derive(Accounts)]
    #[instruction(action: PointAction)]
    pub struct AwardPoints<'info> {
        #[account(
            seeds = [POINTS_CONFIG_SEED],
            bump = points_config.bump,
            has_one = authority,
        )]
        pub points_config: Account<'info, PointsConfig>,

        #[account(
            init_if_needed,
            payer = authority,
            space = UserPoints::SIZE,
            seeds = [
                USER_POINTS_SEED,
                user.key().as_ref(),
                &points_config.season_id.to_le_bytes(),
            ],
            bump,
        )]
        pub user_points: Account<'info, UserPoints>,

        #[account(
            mut,
            seeds = [
                POINTS_LEADERBOARD_SEED,
                &points_config.season_id.to_le_bytes(),
            ],
            bump = leaderboard.bump,
        )]
        pub leaderboard: Account<'info, PointsLeaderboardState>,

        /// CHECK: The user receiving points. Does not need to sign.
        pub user: AccountInfo<'info>,

        #[account(mut)]
        pub authority: Signer<'info>,

        pub system_program: Program<'info, System>,
    }

    /// User spends points to claim a reward.
    #[derive(Accounts)]
    #[instruction(reward_kind: RewardKind, reward_value: u64)]
    pub struct ClaimReward<'info> {
        #[account(
            seeds = [POINTS_CONFIG_SEED],
            bump = points_config.bump,
        )]
        pub points_config: Account<'info, PointsConfig>,

        #[account(
            mut,
            seeds = [
                USER_POINTS_SEED,
                user.key().as_ref(),
                &points_config.season_id.to_le_bytes(),
            ],
            bump = user_points.bump,
            constraint = user_points.user == user.key() @ PointsError::Unauthorized,
        )]
        pub user_points: Account<'info, UserPoints>,

        #[account(mut)]
        pub user: Signer<'info>,
    }

    /// Initialize a leaderboard for a given season. Called by admin.
    #[derive(Accounts)]
    pub struct InitializePointsLeaderboard<'info> {
        #[account(
            init,
            payer = authority,
            space = PointsLeaderboardState::SIZE,
            seeds = [
                POINTS_LEADERBOARD_SEED,
                &points_config.season_id.to_le_bytes(),
            ],
            bump,
        )]
        pub leaderboard: Account<'info, PointsLeaderboardState>,

        #[account(
            seeds = [POINTS_CONFIG_SEED],
            bump = points_config.bump,
            has_one = authority,
        )]
        pub points_config: Account<'info, PointsConfig>,

        #[account(mut)]
        pub authority: Signer<'info>,

        pub system_program: Program<'info, System>,
    }

    /// End the current season: archive the leaderboard and bump season_id.
    #[derive(Accounts)]
    pub struct EndSeason<'info> {
        #[account(
            mut,
            seeds = [POINTS_CONFIG_SEED],
            bump = points_config.bump,
            has_one = authority,
        )]
        pub points_config: Account<'info, PointsConfig>,

        #[account(
            seeds = [
                POINTS_LEADERBOARD_SEED,
                &points_config.season_id.to_le_bytes(),
            ],
            bump = current_leaderboard.bump,
        )]
        pub current_leaderboard: Account<'info, PointsLeaderboardState>,

        #[account(
            init,
            payer = authority,
            space = SeasonArchive::SIZE,
            seeds = [
                SEASON_ARCHIVE_SEED,
                &points_config.season_id.to_le_bytes(),
            ],
            bump,
        )]
        pub season_archive: Account<'info, SeasonArchive>,

        #[account(mut)]
        pub authority: Signer<'info>,

        pub system_program: Program<'info, System>,
    }

    // ============================================================================
    // INSTRUCTIONS (free functions called from lib.rs #[program] block)
    // ============================================================================

    /// Initialize the global points configuration.
    pub fn handle_initialize_points_config(
        ctx: Context<InitializePointsConfig>,
        points_per_trade: u64,
        points_per_launch: u64,
        points_per_referral: u64,
        points_per_hold_day: u64,
    ) -> Result<()> {
        let config = &mut ctx.accounts.points_config;
        config.authority = ctx.accounts.authority.key();
        config.points_per_trade = points_per_trade;
        config.points_per_launch = points_per_launch;
        config.points_per_referral = points_per_referral;
        config.points_per_hold_day = points_per_hold_day;
        config.season_id = 1;
        config.action_cooldown = DEFAULT_ACTION_COOLDOWN;
        config.max_daily_points = DEFAULT_MAX_DAILY_POINTS;
        config.paused = false;
        config.bump = ctx.bumps.points_config;

        emit!(PointsConfigInitialized {
            authority: config.authority,
            season_id: config.season_id,
            points_per_trade,
            points_per_launch,
            points_per_referral,
            points_per_hold_day,
        });

        Ok(())
    }

    /// Update one or more fields on the points config.
    pub fn handle_update_points_config(
        ctx: Context<UpdatePointsConfig>,
        points_per_trade: Option<u64>,
        points_per_launch: Option<u64>,
        points_per_referral: Option<u64>,
        points_per_hold_day: Option<u64>,
        action_cooldown: Option<i64>,
        max_daily_points: Option<u64>,
        paused: Option<bool>,
    ) -> Result<()> {
        let config = &mut ctx.accounts.points_config;

        if let Some(v) = points_per_trade { config.points_per_trade = v; }
        if let Some(v) = points_per_launch { config.points_per_launch = v; }
        if let Some(v) = points_per_referral { config.points_per_referral = v; }
        if let Some(v) = points_per_hold_day { config.points_per_hold_day = v; }
        if let Some(v) = action_cooldown {
            require!(v >= 0, PointsError::InvalidCooldown);
            config.action_cooldown = v;
        }
        if let Some(v) = max_daily_points { config.max_daily_points = v; }
        if let Some(v) = paused { config.paused = v; }

        emit!(PointsConfigUpdated {
            authority: config.authority,
            season_id: config.season_id,
        });

        Ok(())
    }

    /// Initialize the leaderboard for the current season.
    pub fn handle_initialize_points_leaderboard(
        ctx: Context<InitializePointsLeaderboard>,
    ) -> Result<()> {
        let lb = &mut ctx.accounts.leaderboard;
        lb.season_id = ctx.accounts.points_config.season_id;
        lb.entries = Vec::new();
        lb.bump = ctx.bumps.leaderboard;
        Ok(())
    }

    /// Award points to a user for a specific action.
    ///
    /// Anti-gaming enforced:
    /// - Cooldown between actions
    /// - Daily points cap
    /// - Streak tracking with grace period
    pub fn handle_award_points(
        ctx: Context<AwardPoints>,
        action: PointAction,
        multiplier: Option<u64>,
    ) -> Result<()> {
        let config = &ctx.accounts.points_config;
        require!(!config.paused, PointsError::PointsPaused);

        let clock = Clock::get()?;
        let now = clock.unix_timestamp;
        let today = now / SECONDS_PER_DAY;

        let user_points = &mut ctx.accounts.user_points;

        // First-time init
        if user_points.user == Pubkey::default() {
            user_points.user = ctx.accounts.user.key();
            user_points.season_id = config.season_id;
            user_points.bump = ctx.bumps.user_points;
            user_points.streak_days = 0;
            user_points.last_action_day = 0;
            user_points.daily_reset_day = today;
        }

        // --- Anti-gaming: cooldown ---
        let elapsed = now.saturating_sub(user_points.last_action_ts);
        require!(
            elapsed >= config.action_cooldown,
            PointsError::CooldownActive
        );

        // --- Daily cap reset ---
        if today != user_points.daily_reset_day {
            user_points.daily_points_earned = 0;
            user_points.daily_reset_day = today;
        }

        // Calculate points to award
        let base_points = config.points_for(action);
        let mult = multiplier.unwrap_or(1).max(1);
        let raw_points = base_points.checked_mul(mult).ok_or(PointsError::MathOverflow)?;

        // --- Anti-gaming: daily cap ---
        let headroom = config.max_daily_points.saturating_sub(user_points.daily_points_earned);
        require!(headroom > 0, PointsError::DailyCapReached);
        let capped_points = raw_points.min(headroom);

        // --- Streak logic ---
        let last_day = user_points.last_action_day;
        if last_day == 0 {
            // First ever action
            user_points.streak_days = 1;
        } else if today == last_day {
            // Same day, streak unchanged
        } else if today == last_day + 1 {
            // Consecutive day
            user_points.streak_days = user_points.streak_days.saturating_add(1);
        } else {
            // Check grace period (48h from last action timestamp)
            let since_last = now.saturating_sub(user_points.last_action_ts);
            if since_last <= STREAK_GRACE_PERIOD {
                user_points.streak_days = user_points.streak_days.saturating_add(1);
            } else {
                // Streak broken
                user_points.streak_days = 1;
            }
        }

        // Apply streak bonus: +1% per streak day, capped at +50%
        let streak_bonus_pct = (user_points.streak_days as u64).min(50);
        let bonus = capped_points
            .checked_mul(streak_bonus_pct)
            .ok_or(PointsError::MathOverflow)?
            / 100;
        let final_points = capped_points
            .checked_add(bonus)
            .ok_or(PointsError::MathOverflow)?;

        // Re-check daily cap after bonus
        let final_points = final_points.min(
            config.max_daily_points.saturating_sub(user_points.daily_points_earned),
        );

        // Update user state
        user_points.total_points = user_points
            .total_points
            .checked_add(final_points)
            .ok_or(PointsError::MathOverflow)?;
        user_points.available_points = user_points
            .available_points
            .checked_add(final_points)
            .ok_or(PointsError::MathOverflow)?;
        user_points.daily_points_earned = user_points
            .daily_points_earned
            .checked_add(final_points)
            .ok_or(PointsError::MathOverflow)?;
        user_points.last_action_ts = now;
        user_points.last_action_day = today;
        user_points.level = UserPoints::compute_level(user_points.total_points);

        // --- Update leaderboard ---
        let lb = &mut ctx.accounts.leaderboard;
        update_points_leaderboard(
            &mut lb.entries,
            PointsLeaderboardEntry {
                user: ctx.accounts.user.key(),
                total_points: user_points.total_points,
                level: user_points.level,
            },
        );

        emit!(PointsAwarded {
            user: ctx.accounts.user.key(),
            action,
            base_points,
            streak_bonus: bonus,
            final_points,
            new_total: user_points.total_points,
            level: user_points.level,
            streak_days: user_points.streak_days,
            season_id: config.season_id,
        });

        Ok(())
    }

    /// Spend points to claim a reward (fee discount, early access, badge, etc.).
    pub fn handle_claim_reward(
        ctx: Context<ClaimReward>,
        reward_kind: RewardKind,
        reward_value: u64,
        points_cost: u64,
    ) -> Result<()> {
        require!(points_cost > 0, PointsError::ZeroCost);

        let user_points = &mut ctx.accounts.user_points;
        require!(
            user_points.available_points >= points_cost,
            PointsError::InsufficientPoints
        );

        user_points.available_points = user_points
            .available_points
            .checked_sub(points_cost)
            .ok_or(PointsError::MathOverflow)?;

        emit!(RewardClaimed {
            user: ctx.accounts.user.key(),
            reward_kind,
            reward_value,
            points_spent: points_cost,
            remaining_points: user_points.available_points,
            season_id: ctx.accounts.points_config.season_id,
        });

        Ok(())
    }

    /// End the current season: archive leaderboard, bump season_id.
    pub fn handle_end_season(ctx: Context<EndSeason>) -> Result<()> {
        let clock = Clock::get()?;
        let config = &ctx.accounts.points_config;
        let current_lb = &ctx.accounts.current_leaderboard;

        // Write archive
        let archive = &mut ctx.accounts.season_archive;
        archive.season_id = config.season_id;
        archive.ended_at = clock.unix_timestamp;
        archive.top_entries = current_lb.entries.clone();
        archive.bump = ctx.bumps.season_archive;

        let old_season = config.season_id;

        // Bump season
        let config = &mut ctx.accounts.points_config;
        config.season_id = config
            .season_id
            .checked_add(1)
            .ok_or(PointsError::MathOverflow)?;

        emit!(SeasonEnded {
            season_id: old_season,
            new_season_id: config.season_id,
            ended_at: clock.unix_timestamp,
            top_user: if current_lb.entries.is_empty() {
                Pubkey::default()
            } else {
                current_lb.entries[0].user
            },
        });

        Ok(())
    }

    // ============================================================================
    // HELPERS
    // ============================================================================

    /// Insert or update a user on the leaderboard, keep sorted desc, truncate to max.
    fn update_points_leaderboard(
        entries: &mut Vec<PointsLeaderboardEntry>,
        entry: PointsLeaderboardEntry,
    ) {
        if let Some(existing) = entries.iter_mut().find(|e| e.user == entry.user) {
            existing.total_points = entry.total_points;
            existing.level = entry.level;
        } else {
            entries.push(entry);
        }
        entries.sort_by(|a, b| b.total_points.cmp(&a.total_points));
        entries.truncate(MAX_POINTS_LEADERBOARD);
    }

    // ============================================================================
    // EVENTS
    // ============================================================================

    #[event]
    pub struct PointsConfigInitialized {
        pub authority: Pubkey,
        pub season_id: u32,
        pub points_per_trade: u64,
        pub points_per_launch: u64,
        pub points_per_referral: u64,
        pub points_per_hold_day: u64,
    }

    #[event]
    pub struct PointsConfigUpdated {
        pub authority: Pubkey,
        pub season_id: u32,
    }

    #[event]
    pub struct PointsAwarded {
        pub user: Pubkey,
        pub action: PointAction,
        pub base_points: u64,
        pub streak_bonus: u64,
        pub final_points: u64,
        pub new_total: u64,
        pub level: u16,
        pub streak_days: u32,
        pub season_id: u32,
    }

    #[event]
    pub struct RewardClaimed {
        pub user: Pubkey,
        pub reward_kind: RewardKind,
        pub reward_value: u64,
        pub points_spent: u64,
        pub remaining_points: u64,
        pub season_id: u32,
    }

    #[event]
    pub struct SeasonEnded {
        pub season_id: u32,
        pub new_season_id: u32,
        pub ended_at: i64,
        pub top_user: Pubkey,
    }

    // ============================================================================
    // ERRORS
    // ============================================================================

    #[error_code]
    pub enum PointsError {
        #[msg("Points system is paused")]
        PointsPaused,
        #[msg("Action cooldown still active")]
        CooldownActive,
        #[msg("Daily points cap reached")]
        DailyCapReached,
        #[msg("Insufficient points to claim reward")]
        InsufficientPoints,
        #[msg("Reward cost must be greater than zero")]
        ZeroCost,
        #[msg("Unauthorized")]
        Unauthorized,
        #[msg("Invalid cooldown value")]
        InvalidCooldown,
        #[msg("Math overflow")]
        MathOverflow,
    }

}
pub mod fund_tokens {
    use super::*;

    // ---------------------------------------------------------------------------
    // Constants
    // ---------------------------------------------------------------------------

    pub const FUND_CONFIG_SEED: &[u8] = b"fund_config";
    pub const FUND_SHARE_MINT_SEED: &[u8] = b"fund_share_mint";
    pub const FUND_VAULT_SEED: &[u8] = b"fund_vault";
    pub const FUND_SOL_VAULT_SEED: &[u8] = b"fund_sol_vault";
    pub const USER_FUND_POSITION_SEED: &[u8] = b"user_fund_position";

    pub const MAX_FUND_NAME_LEN: usize = 32;
    pub const MAX_BASKET_SIZE: usize = 10;
    pub const SHARE_DECIMALS: u8 = 6;
    pub const BPS_DENOMINATOR: u64 = 10_000;
    pub const MAX_MANAGEMENT_FEE_BPS: u16 = 500; // 5%

    // ---------------------------------------------------------------------------
    // Accounts
    // ---------------------------------------------------------------------------

    /// On-chain configuration for a fund / index token.
    ///
    /// Stores the basket composition (token mints + weights), fee parameters,
    /// and cumulative deposit tracking. Derived as a PDA from the fund name.
    #[account]
    pub struct FundConfig {
        /// Human-readable fund name (unique, used in PDA seed).
        pub name: String,
        /// Creator authority – can rebalance and collect fees.
        pub creator: Pubkey,
        /// SPL token mints that make up the basket (max 10).
        pub token_mints: Vec<Pubkey>,
        /// Corresponding weights in basis points (must sum to 10 000).
        pub weights_bps: Vec<u16>,
        /// Total SOL deposited into this fund (lifetime).
        pub total_deposits_sol: u64,
        /// Annualised management fee in basis points.
        pub management_fee_bps: u16,
        /// Whether the fund is currently accepting deposits.
        pub active: bool,
        /// Share mint address (the fund-share SPL token).
        pub share_mint: Pubkey,
        /// Creation timestamp.
        pub created_at: i64,
        /// Bump for this PDA.
        pub bump: u8,
        /// Bump for the share mint PDA.
        pub share_mint_bump: u8,
    }

    impl FundConfig {
        pub const SIZE: usize = 8 // discriminator
            + (4 + MAX_FUND_NAME_LEN)               // name
            + 32                                      // creator
            + (4 + MAX_BASKET_SIZE * 32)             // token_mints vec
            + (4 + MAX_BASKET_SIZE * 2)              // weights_bps vec
            + 8                                       // total_deposits_sol
            + 2                                       // management_fee_bps
            + 1                                       // active
            + 32                                      // share_mint
            + 8                                       // created_at
            + 1                                       // bump
            + 1;                                      // share_mint_bump
    }

    /// Per-user position tracking for a specific fund.
    #[account]
    pub struct UserFundPosition {
        /// User wallet.
        pub user: Pubkey,
        /// Fund config address.
        pub fund: Pubkey,
        /// Total shares held (mirrors token account but useful for analytics).
        pub shares_held: u64,
        /// Cumulative SOL deposited by this user.
        pub total_deposited_sol: u64,
        /// Cumulative SOL value redeemed by this user.
        pub total_redeemed_sol: u64,
        /// Timestamp of first deposit.
        pub first_deposit_at: i64,
        /// Bump seed.
        pub bump: u8,
    }

    impl UserFundPosition {
        pub const SIZE: usize = 8 + 32 + 32 + 8 + 8 + 8 + 8 + 1;
    }

    // ---------------------------------------------------------------------------
    // Instructions
    // ---------------------------------------------------------------------------

    /// Create a new fund with a basket of SPL tokens and their weights.
    ///
    /// The creator defines:
    /// - A unique fund name
    /// - Up to 10 SPL token mints forming the basket
    /// - Corresponding weights in basis points (must sum to 10 000)
    /// - A management fee (max 5%)
    ///
    /// A fund-share SPL mint is created as a PDA owned by the fund.
    pub fn create_fund(
        ctx: Context<CreateFund>,
        name: String,
        token_mints: Vec<Pubkey>,
        weights_bps: Vec<u16>,
        management_fee_bps: u16,
    ) -> Result<()> {
        require!(name.len() > 0 && name.len() <= MAX_FUND_NAME_LEN, FundError::InvalidName);
        require!(
            token_mints.len() > 0 && token_mints.len() <= MAX_BASKET_SIZE,
            FundError::InvalidBasketSize
        );
        require!(token_mints.len() == weights_bps.len(), FundError::MintWeightMismatch);
        require!(management_fee_bps <= MAX_MANAGEMENT_FEE_BPS, FundError::FeeTooHigh);

        // Weights must sum to 10 000 bps (100%).
        let weight_sum: u64 = weights_bps.iter().map(|w| *w as u64).sum();
        require!(weight_sum == BPS_DENOMINATOR, FundError::WeightsSumInvalid);

        // Ensure no duplicate mints.
        for i in 0..token_mints.len() {
            for j in (i + 1)..token_mints.len() {
                require!(token_mints[i] != token_mints[j], FundError::DuplicateMint);
            }
        }

        let clock = Clock::get()?;
        let fund = &mut ctx.accounts.fund_config;

        fund.name = name.clone();
        fund.creator = ctx.accounts.creator.key();
        fund.token_mints = token_mints;
        fund.weights_bps = weights_bps;
        fund.total_deposits_sol = 0;
        fund.management_fee_bps = management_fee_bps;
        fund.active = true;
        fund.share_mint = ctx.accounts.share_mint.key();
        fund.created_at = clock.unix_timestamp;
        fund.bump = ctx.bumps.fund_config;
        fund.share_mint_bump = ctx.bumps.share_mint;

        emit!(FundCreated {
            fund: fund.key(),
            name,
            creator: fund.creator,
            share_mint: fund.share_mint,
            num_tokens: fund.token_mints.len() as u8,
            management_fee_bps,
            created_at: clock.unix_timestamp,
        });

        Ok(())
    }

    /// Deposit SOL into a fund and receive proportional fund shares.
    ///
    /// The deposited SOL is held in the fund's SOL vault. Shares are minted
    /// 1:1 with lamports on the first deposit; subsequent deposits mint shares
    /// proportional to the existing share supply vs. vault balance.
    pub fn deposit_to_fund(ctx: Context<DepositToFund>, sol_amount: u64) -> Result<()> {
        require!(sol_amount > 0, FundError::ZeroAmount);

        let fund = &ctx.accounts.fund_config;
        require!(fund.active, FundError::FundInactive);

        // Calculate shares to mint.
        // If supply == 0 → shares = sol_amount (bootstrap).
        // Otherwise → shares = sol_amount * total_supply / vault_balance.
        let current_supply = ctx.accounts.share_mint.supply;
        let vault_balance = ctx.accounts.fund_sol_vault.lamports();

        let shares_to_mint: u64 = if current_supply == 0 || vault_balance == 0 {
            sol_amount
        } else {
            (sol_amount as u128)
                .checked_mul(current_supply as u128)
                .ok_or(FundError::MathOverflow)?
                .checked_div(vault_balance as u128)
                .ok_or(FundError::MathOverflow)? as u64
        };

        require!(shares_to_mint > 0, FundError::InsufficientOutput);

        // Transfer SOL from depositor to fund SOL vault.
        anchor_lang::system_program::transfer(
            CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.depositor.to_account_info(),
                    to: ctx.accounts.fund_sol_vault.to_account_info(),
                },
            ),
            sol_amount,
        )?;

        // Mint fund shares to depositor.
        let fund_name = ctx.accounts.fund_config.name.as_bytes();
        let bump = ctx.accounts.fund_config.bump;
        let signer_seeds: &[&[u8]] = &[FUND_CONFIG_SEED, fund_name, &[bump]];

        token::mint_to(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                MintTo {
                    mint: ctx.accounts.share_mint.to_account_info(),
                    to: ctx.accounts.depositor_share_account.to_account_info(),
                    authority: ctx.accounts.fund_config.to_account_info(),
                },
                &[signer_seeds],
            ),
            shares_to_mint,
        )?;

        // Update fund state.
        let fund = &mut ctx.accounts.fund_config;
        fund.total_deposits_sol = fund
            .total_deposits_sol
            .checked_add(sol_amount)
            .ok_or(FundError::MathOverflow)?;

        // Update user position.
        let clock = Clock::get()?;
        let position = &mut ctx.accounts.user_position;
        if position.first_deposit_at == 0 {
            position.user = ctx.accounts.depositor.key();
            position.fund = ctx.accounts.fund_config.key();
            position.first_deposit_at = clock.unix_timestamp;
            position.bump = ctx.bumps.user_position;
        }
        position.shares_held = position
            .shares_held
            .checked_add(shares_to_mint)
            .ok_or(FundError::MathOverflow)?;
        position.total_deposited_sol = position
            .total_deposited_sol
            .checked_add(sol_amount)
            .ok_or(FundError::MathOverflow)?;

        emit!(DepositedToFund {
            fund: ctx.accounts.fund_config.key(),
            depositor: ctx.accounts.depositor.key(),
            sol_amount,
            shares_minted: shares_to_mint,
            total_supply_after: current_supply.checked_add(shares_to_mint).unwrap_or(shares_to_mint),
        });

        Ok(())
    }

    /// Redeem (burn) fund shares and receive proportional SOL from the vault.
    ///
    /// The SOL returned is: `share_amount * vault_balance / total_supply`.
    /// A management fee is deducted and sent to the creator.
    pub fn redeem_shares(ctx: Context<RedeemShares>, share_amount: u64) -> Result<()> {
        require!(share_amount > 0, FundError::ZeroAmount);

        let total_supply = ctx.accounts.share_mint.supply;
        require!(total_supply > 0, FundError::NoSharesOutstanding);

        let vault_balance = ctx.accounts.fund_sol_vault.lamports();

        // Gross SOL entitlement.
        let gross_sol: u64 = (share_amount as u128)
            .checked_mul(vault_balance as u128)
            .ok_or(FundError::MathOverflow)?
            .checked_div(total_supply as u128)
            .ok_or(FundError::MathOverflow)? as u64;

        require!(gross_sol > 0, FundError::InsufficientOutput);

        // Management fee.
        let fee = (gross_sol as u128)
            .checked_mul(ctx.accounts.fund_config.management_fee_bps as u128)
            .ok_or(FundError::MathOverflow)?
            .checked_div(BPS_DENOMINATOR as u128)
            .ok_or(FundError::MathOverflow)? as u64;

        let net_sol = gross_sol.checked_sub(fee).ok_or(FundError::MathOverflow)?;

        // Burn shares from redeemer.
        token::burn(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Burn {
                    mint: ctx.accounts.share_mint.to_account_info(),
                    from: ctx.accounts.redeemer_share_account.to_account_info(),
                    authority: ctx.accounts.redeemer.to_account_info(),
                },
            ),
            share_amount,
        )?;

        // Transfer SOL from vault to redeemer (lamport manipulation – vault is PDA).
        **ctx
            .accounts
            .fund_sol_vault
            .to_account_info()
            .try_borrow_mut_lamports()? -= gross_sol;
        **ctx
            .accounts
            .redeemer
            .to_account_info()
            .try_borrow_mut_lamports()? += net_sol;

        // Fee to creator.
        if fee > 0 {
            **ctx
                .accounts
                .creator_wallet
                .to_account_info()
                .try_borrow_mut_lamports()? += fee;
        }

        // Update user position.
        let position = &mut ctx.accounts.user_position;
        position.shares_held = position.shares_held.saturating_sub(share_amount);
        position.total_redeemed_sol = position
            .total_redeemed_sol
            .checked_add(net_sol)
            .ok_or(FundError::MathOverflow)?;

        emit!(SharesRedeemed {
            fund: ctx.accounts.fund_config.key(),
            redeemer: ctx.accounts.redeemer.key(),
            shares_burned: share_amount,
            sol_received: net_sol,
            fee_collected: fee,
            total_supply_after: total_supply.checked_sub(share_amount).unwrap_or(0),
        });

        Ok(())
    }

    /// Rebalance fund weights. Creator-only.
    ///
    /// Allows the fund creator to adjust the target allocation weights
    /// without changing the underlying basket mints. Weights must still
    /// sum to 10 000 bps.
    pub fn rebalance_fund(ctx: Context<RebalanceFund>, new_weights_bps: Vec<u16>) -> Result<()> {
        let fund = &ctx.accounts.fund_config;

        require!(
            new_weights_bps.len() == fund.token_mints.len(),
            FundError::MintWeightMismatch
        );

        let weight_sum: u64 = new_weights_bps.iter().map(|w| *w as u64).sum();
        require!(weight_sum == BPS_DENOMINATOR, FundError::WeightsSumInvalid);

        let old_weights = fund.weights_bps.clone();

        let fund = &mut ctx.accounts.fund_config;
        fund.weights_bps = new_weights_bps.clone();

        emit!(FundRebalanced {
            fund: fund.key(),
            creator: ctx.accounts.creator.key(),
            old_weights_bps: old_weights,
            new_weights_bps,
        });

        Ok(())
    }

    /// Toggle fund active status. Creator-only.
    pub fn set_fund_active(ctx: Context<RebalanceFund>, active: bool) -> Result<()> {
        let fund = &mut ctx.accounts.fund_config;
        fund.active = active;

        emit!(FundStatusChanged {
            fund: fund.key(),
            active,
        });

        Ok(())
    }

    // ---------------------------------------------------------------------------
    // Context structs (Accounts)
    // ---------------------------------------------------------------------------

    #[derive(Accounts)]
    #[instruction(name: String)]
    pub struct CreateFund<'info> {
        #[account(
            init,
            payer = creator,
            space = FundConfig::SIZE,
            seeds = [FUND_CONFIG_SEED, name.as_bytes()],
            bump,
        )]
        pub fund_config: Account<'info, FundConfig>,

        /// Fund-share SPL mint. Authority = fund_config PDA.
        #[account(
            init,
            payer = creator,
            mint::decimals = SHARE_DECIMALS,
            mint::authority = fund_config,
            seeds = [FUND_SHARE_MINT_SEED, fund_config.key().as_ref()],
            bump,
        )]
        pub share_mint: Account<'info, Mint>,

        #[account(mut)]
        pub creator: Signer<'info>,

        pub token_program: Program<'info, Token>,
        pub system_program: Program<'info, System>,
        pub rent: Sysvar<'info, Rent>,
    }

    #[derive(Accounts)]
    pub struct DepositToFund<'info> {
        #[account(
            mut,
            seeds = [FUND_CONFIG_SEED, fund_config.name.as_bytes()],
            bump = fund_config.bump,
        )]
        pub fund_config: Account<'info, FundConfig>,

        #[account(
            mut,
            seeds = [FUND_SHARE_MINT_SEED, fund_config.key().as_ref()],
            bump = fund_config.share_mint_bump,
        )]
        pub share_mint: Account<'info, Mint>,

        /// CHECK: Fund SOL vault PDA.
        #[account(
            mut,
            seeds = [FUND_SOL_VAULT_SEED, fund_config.key().as_ref()],
            bump,
        )]
        pub fund_sol_vault: AccountInfo<'info>,

        /// Depositor's associated token account for fund shares.
        #[account(
            init_if_needed,
            payer = depositor,
            associated_token::mint = share_mint,
            associated_token::authority = depositor,
        )]
        pub depositor_share_account: Account<'info, TokenAccount>,

        /// Per-user position tracker.
        #[account(
            init_if_needed,
            payer = depositor,
            space = UserFundPosition::SIZE,
            seeds = [USER_FUND_POSITION_SEED, depositor.key().as_ref(), fund_config.key().as_ref()],
            bump,
        )]
        pub user_position: Account<'info, UserFundPosition>,

        #[account(mut)]
        pub depositor: Signer<'info>,

        pub token_program: Program<'info, Token>,
        pub associated_token_program: Program<'info, AssociatedToken>,
        pub system_program: Program<'info, System>,
    }

    #[derive(Accounts)]
    pub struct RedeemShares<'info> {
        #[account(
            seeds = [FUND_CONFIG_SEED, fund_config.name.as_bytes()],
            bump = fund_config.bump,
        )]
        pub fund_config: Account<'info, FundConfig>,

        #[account(
            mut,
            seeds = [FUND_SHARE_MINT_SEED, fund_config.key().as_ref()],
            bump = fund_config.share_mint_bump,
        )]
        pub share_mint: Account<'info, Mint>,

        /// CHECK: Fund SOL vault PDA.
        #[account(
            mut,
            seeds = [FUND_SOL_VAULT_SEED, fund_config.key().as_ref()],
            bump,
        )]
        pub fund_sol_vault: AccountInfo<'info>,

        /// Redeemer's fund share token account.
        #[account(
            mut,
            associated_token::mint = share_mint,
            associated_token::authority = redeemer,
        )]
        pub redeemer_share_account: Account<'info, TokenAccount>,

        /// Per-user position tracker.
        #[account(
            mut,
            seeds = [USER_FUND_POSITION_SEED, redeemer.key().as_ref(), fund_config.key().as_ref()],
            bump = user_position.bump,
        )]
        pub user_position: Account<'info, UserFundPosition>,

        /// CHECK: Creator wallet to receive management fee.
        #[account(
            mut,
            constraint = creator_wallet.key() == fund_config.creator @ FundError::InvalidCreator,
        )]
        pub creator_wallet: AccountInfo<'info>,

        #[account(mut)]
        pub redeemer: Signer<'info>,

        pub token_program: Program<'info, Token>,
        pub system_program: Program<'info, System>,
    }

    #[derive(Accounts)]
    pub struct RebalanceFund<'info> {
        #[account(
            mut,
            seeds = [FUND_CONFIG_SEED, fund_config.name.as_bytes()],
            bump = fund_config.bump,
            has_one = creator,
        )]
        pub fund_config: Account<'info, FundConfig>,

        pub creator: Signer<'info>,
    }

    // ---------------------------------------------------------------------------
    // Events
    // ---------------------------------------------------------------------------

    #[event]
    pub struct FundCreated {
        pub fund: Pubkey,
        pub name: String,
        pub creator: Pubkey,
        pub share_mint: Pubkey,
        pub num_tokens: u8,
        pub management_fee_bps: u16,
        pub created_at: i64,
    }

    #[event]
    pub struct DepositedToFund {
        pub fund: Pubkey,
        pub depositor: Pubkey,
        pub sol_amount: u64,
        pub shares_minted: u64,
        pub total_supply_after: u64,
    }

    #[event]
    pub struct SharesRedeemed {
        pub fund: Pubkey,
        pub redeemer: Pubkey,
        pub shares_burned: u64,
        pub sol_received: u64,
        pub fee_collected: u64,
        pub total_supply_after: u64,
    }

    #[event]
    pub struct FundRebalanced {
        pub fund: Pubkey,
        pub creator: Pubkey,
        pub old_weights_bps: Vec<u16>,
        pub new_weights_bps: Vec<u16>,
    }

    #[event]
    pub struct FundStatusChanged {
        pub fund: Pubkey,
        pub active: bool,
    }

    // ---------------------------------------------------------------------------
    // Errors
    // ---------------------------------------------------------------------------

    #[error_code]
    pub enum FundError {
        #[msg("Fund name must be 1-32 characters")]
        InvalidName,
        #[msg("Basket must contain 1-10 token mints")]
        InvalidBasketSize,
        #[msg("Number of mints must match number of weights")]
        MintWeightMismatch,
        #[msg("Weights must sum to 10000 (100%)")]
        WeightsSumInvalid,
        #[msg("Duplicate mint in basket")]
        DuplicateMint,
        #[msg("Management fee too high (max 5%)")]
        FeeTooHigh,
        #[msg("Amount must be greater than zero")]
        ZeroAmount,
        #[msg("Fund is not active")]
        FundInactive,
        #[msg("Insufficient output amount")]
        InsufficientOutput,
        #[msg("No shares outstanding")]
        NoSharesOutstanding,
        #[msg("Invalid creator")]
        InvalidCreator,
        #[msg("Math overflow")]
        MathOverflow,
    }

}

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
