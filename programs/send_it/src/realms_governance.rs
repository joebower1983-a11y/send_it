use anchor_lang::prelude::*;
use anchor_lang::solana_program::{
    instruction::Instruction,
    program::invoke_signed,
    system_program,
};

// ---------------------------------------------------------------------------
// SPL Governance Program ID (v3)
// ---------------------------------------------------------------------------
pub const SPL_GOVERNANCE_ID: Pubkey = anchor_lang::solana_program::pubkey!("GovER5Lthms3bLBqWub97yVrMmEogzX7xNjdXpPPCVZw");

// ---------------------------------------------------------------------------
// Seeds
// ---------------------------------------------------------------------------
pub const GOVERNANCE_BRIDGE_SEED: &[u8] = b"gov_bridge";

// ---------------------------------------------------------------------------
// Account: GovernanceBridge
// Stores config linking Send.it to a Realms DAO
// ---------------------------------------------------------------------------
#[account]
pub struct GovernanceBridge {
    /// The Realm pubkey on SPL Governance
    pub realm: Pubkey,
    /// Governance instance within the realm
    pub governance: Pubkey,
    /// The community token mint used for voting power
    pub community_mint: Pubkey,
    /// Send.it platform authority who can update this bridge
    pub authority: Pubkey,
    /// Minimum SENDIT token balance required to create a proposal
    pub min_proposal_balance: u64,
    /// Whether the bridge is active
    pub active: bool,
    pub bump: u8,
}

impl GovernanceBridge {
    pub const SIZE: usize = 8 + 32 + 32 + 32 + 32 + 8 + 1 + 1;
}

// ---------------------------------------------------------------------------
// Account: ProposalRecord
// Tracks proposals created through Send.it → Realms bridge
// ---------------------------------------------------------------------------
#[account]
pub struct ProposalRecord {
    /// The SPL Governance proposal address
    pub governance_proposal: Pubkey,
    /// Who created it through Send.it
    pub creator: Pubkey,
    /// Title (for indexing)
    pub title: [u8; 64],
    pub title_len: u8,
    /// Created timestamp
    pub created_at: i64,
    pub bump: u8,
}

impl ProposalRecord {
    pub const SIZE: usize = 8 + 32 + 32 + 64 + 1 + 8 + 1;
}

// ---------------------------------------------------------------------------
// Account: VoteRecord
// Tracks votes cast through Send.it → Realms bridge
// ---------------------------------------------------------------------------
#[account]
pub struct VoteRecord {
    /// The SPL Governance proposal voted on
    pub governance_proposal: Pubkey,
    /// The voter
    pub voter: Pubkey,
    /// Vote choice (0 = Approve, 1 = Deny, 2 = Abstain, 3 = Veto)
    pub vote_choice: u8,
    /// Vote weight at time of voting
    pub weight: u64,
    /// Timestamp
    pub voted_at: i64,
    pub bump: u8,
}

impl VoteRecord {
    pub const SIZE: usize = 8 + 32 + 32 + 1 + 8 + 8 + 1;
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------
#[error_code]
pub enum RealmsError {
    #[msg("Governance bridge is not active")]
    BridgeInactive,
    #[msg("Insufficient token balance to create proposal")]
    InsufficientBalance,
    #[msg("Title too long (max 64 bytes)")]
    TitleTooLong,
    #[msg("Description too long")]
    DescriptionTooLong,
    #[msg("Invalid vote choice")]
    InvalidVoteChoice,
    #[msg("SPL Governance CPI failed")]
    GovernanceCpiFailed,
    #[msg("Invalid realm account")]
    InvalidRealm,
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------
#[event]
pub struct GovernanceBridgeInitialized {
    pub realm: Pubkey,
    pub governance: Pubkey,
    pub community_mint: Pubkey,
}

#[event]
pub struct RealmsProposalCreated {
    pub governance_proposal: Pubkey,
    pub creator: Pubkey,
    pub title: String,
}

#[event]
pub struct RealmsVoteCast {
    pub governance_proposal: Pubkey,
    pub voter: Pubkey,
    pub vote_choice: u8,
    pub weight: u64,
}

// ---------------------------------------------------------------------------
// Instruction Accounts: Initialize Bridge
// ---------------------------------------------------------------------------
#[derive(Accounts)]
pub struct InitGovernanceBridge<'info> {
    #[account(
        init,
        payer = authority,
        space = GovernanceBridge::SIZE,
        seeds = [GOVERNANCE_BRIDGE_SEED],
        bump,
    )]
    pub bridge: Account<'info, GovernanceBridge>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

// ---------------------------------------------------------------------------
// Instruction Accounts: Create Proposal via Realms
// ---------------------------------------------------------------------------
#[derive(Accounts)]
pub struct CreateRealmsProposal<'info> {
    #[account(
        seeds = [GOVERNANCE_BRIDGE_SEED],
        bump = bridge.bump,
        constraint = bridge.active @ RealmsError::BridgeInactive,
    )]
    pub bridge: Account<'info, GovernanceBridge>,

    #[account(
        init,
        payer = proposer,
        space = ProposalRecord::SIZE,
        seeds = [
            b"proposal_record",
            proposer.key().as_ref(),
            &Clock::get()?.unix_timestamp.to_le_bytes(),
        ],
        bump,
    )]
    pub proposal_record: Account<'info, ProposalRecord>,

    /// The proposer's SENDIT token account — checked for min balance
    /// CHECK: Deserialized manually for balance check
    pub proposer_token_account: AccountInfo<'info>,

    #[account(mut)]
    pub proposer: Signer<'info>,

    // ── SPL Governance accounts (passed through for CPI) ──

    /// CHECK: The Realm account on SPL Governance
    pub realm: AccountInfo<'info>,

    /// CHECK: The Governance instance
    #[account(mut)]
    pub governance: AccountInfo<'info>,

    /// CHECK: Token owner record (proposer's record in the Realm)
    #[account(mut)]
    pub token_owner_record: AccountInfo<'info>,

    /// CHECK: The proposal account to be created by SPL Governance
    #[account(mut)]
    pub governance_proposal: AccountInfo<'info>,

    /// CHECK: Proposal deposit account
    #[account(mut)]
    pub proposal_deposit: AccountInfo<'info>,

    /// CHECK: Community token mint
    pub community_mint: AccountInfo<'info>,

    /// CHECK: Governance authority (proposer must be the governance authority or delegate)
    pub governance_authority: AccountInfo<'info>,

    /// CHECK: SPL Governance program
    #[account(
        constraint = spl_governance_program.key() == SPL_GOVERNANCE_ID @ RealmsError::GovernanceCpiFailed,
    )]
    pub spl_governance_program: AccountInfo<'info>,

    /// CHECK: Payer for rent
    #[account(mut)]
    pub payer: AccountInfo<'info>,

    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

// ---------------------------------------------------------------------------
// Instruction Accounts: Cast Vote via Realms
// ---------------------------------------------------------------------------
#[derive(Accounts)]
pub struct CastRealmsVote<'info> {
    #[account(
        seeds = [GOVERNANCE_BRIDGE_SEED],
        bump = bridge.bump,
        constraint = bridge.active @ RealmsError::BridgeInactive,
    )]
    pub bridge: Account<'info, GovernanceBridge>,

    #[account(
        init,
        payer = voter,
        space = VoteRecord::SIZE,
        seeds = [
            b"vote_record",
            governance_proposal.key().as_ref(),
            voter.key().as_ref(),
        ],
        bump,
    )]
    pub vote_record: Account<'info, VoteRecord>,

    /// Voter's SENDIT token account for weight calculation
    /// CHECK: Deserialized manually
    pub voter_token_account: AccountInfo<'info>,

    #[account(mut)]
    pub voter: Signer<'info>,

    // ── SPL Governance accounts (passed through for CPI) ──

    /// CHECK: Realm account
    pub realm: AccountInfo<'info>,

    /// CHECK: Governance instance
    pub governance: AccountInfo<'info>,

    /// CHECK: The proposal being voted on
    #[account(mut)]
    pub governance_proposal: AccountInfo<'info>,

    /// CHECK: Token owner record for the voter
    #[account(mut)]
    pub token_owner_record: AccountInfo<'info>,

    /// CHECK: Governance authority
    pub governance_authority: AccountInfo<'info>,

    /// CHECK: Vote record account to be created by SPL Governance
    #[account(mut)]
    pub governance_vote_record: AccountInfo<'info>,

    /// CHECK: Community token mint
    pub community_mint: AccountInfo<'info>,

    /// CHECK: SPL Governance program
    #[account(
        constraint = spl_governance_program.key() == SPL_GOVERNANCE_ID @ RealmsError::GovernanceCpiFailed,
    )]
    pub spl_governance_program: AccountInfo<'info>,

    /// CHECK: Payer for rent
    #[account(mut)]
    pub payer: AccountInfo<'info>,

    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

// ---------------------------------------------------------------------------
// Instructions
// ---------------------------------------------------------------------------

/// Initialize the governance bridge connecting Send.it to a Realms DAO
pub fn handle_init_governance_bridge(
    ctx: Context<InitGovernanceBridge>,
    realm: Pubkey,
    governance: Pubkey,
    community_mint: Pubkey,
    min_proposal_balance: u64,
) -> Result<()> {
    let bridge = &mut ctx.accounts.bridge;
    bridge.realm = realm;
    bridge.governance = governance;
    bridge.community_mint = community_mint;
    bridge.authority = ctx.accounts.authority.key();
    bridge.min_proposal_balance = min_proposal_balance;
    bridge.active = true;
    bridge.bump = ctx.bumps.bridge;

    emit!(GovernanceBridgeInitialized {
        realm,
        governance,
        community_mint,
    });

    Ok(())
}

/// Create a proposal through Send.it that routes to SPL Governance / Realms.
///
/// This performs a CPI to spl-governance's CreateProposal instruction,
/// then records the proposal in Send.it's own state for indexing.
pub fn handle_create_realms_proposal(
    ctx: Context<CreateRealmsProposal>,
    title: String,
    description: String,
    vote_type: u8, // 0 = SingleChoice, 1 = MultiChoice
    options: Vec<String>,
) -> Result<()> {
    require!(title.len() <= 64, RealmsError::TitleTooLong);
    require!(description.len() <= 750, RealmsError::DescriptionTooLong);

    let bridge = &ctx.accounts.bridge;

    // Check proposer has minimum SENDIT balance
    if bridge.min_proposal_balance > 0 {
        let token_data = ctx.accounts.proposer_token_account.try_borrow_data()?;
        if token_data.len() >= 72 {
            let amount = u64::from_le_bytes(token_data[64..72].try_into().unwrap());
            require!(amount >= bridge.min_proposal_balance, RealmsError::InsufficientBalance);
        } else {
            return Err(RealmsError::InsufficientBalance.into());
        }
    }

    // ── Build SPL Governance CreateProposal instruction ──
    // spl-governance v3 CreateProposal layout:
    //   discriminator: [2] (CreateProposal = instruction index 2 in spl-gov v3)
    //   name: String (borsh: 4-byte len + utf8)
    //   description: String
    //   vote_type: enum (0=SingleChoice, 1=MultiChoice)
    //   options: Vec<String>
    //   use_deny_option: bool
    //   proposal_seed: Pubkey (v3 uses proposal_seed for PDA)

    let mut ix_data = Vec::new();
    // spl-governance v3 instruction index for CreateProposal = 2
    ix_data.push(2u8);
    // name (borsh string)
    ix_data.extend_from_slice(&(title.len() as u32).to_le_bytes());
    ix_data.extend_from_slice(title.as_bytes());
    // description (borsh string)
    ix_data.extend_from_slice(&(description.len() as u32).to_le_bytes());
    ix_data.extend_from_slice(description.as_bytes());
    // vote_type
    if vote_type == 0 {
        ix_data.push(0); // SingleChoice
    } else {
        ix_data.push(1); // MultiChoice
        // For multi-choice, borsh encode: { max_voter_options, max_winning_options }
        ix_data.push(options.len() as u8);
        ix_data.push(1); // max_winning_options = 1
    }
    // options vec
    ix_data.extend_from_slice(&(options.len() as u32).to_le_bytes());
    for opt in &options {
        ix_data.extend_from_slice(&(opt.len() as u32).to_le_bytes());
        ix_data.extend_from_slice(opt.as_bytes());
    }
    // use_deny_option
    ix_data.push(1); // true — standard for most governance
    // proposal_seed: use proposer key as seed
    ix_data.extend_from_slice(ctx.accounts.proposer.key().as_ref());

    let create_proposal_ix = Instruction {
        program_id: SPL_GOVERNANCE_ID,
        accounts: vec![
            // 0: realm
            AccountMeta::new_readonly(ctx.accounts.realm.key(), false),
            // 1: proposal (to be created)
            AccountMeta::new(ctx.accounts.governance_proposal.key(), false),
            // 2: governance
            AccountMeta::new(ctx.accounts.governance.key(), false),
            // 3: token_owner_record
            AccountMeta::new(ctx.accounts.token_owner_record.key(), false),
            // 4: governance_authority (signer — the proposer)
            AccountMeta::new_readonly(ctx.accounts.governance_authority.key(), true),
            // 5: payer
            AccountMeta::new(ctx.accounts.payer.key(), true),
            // 6: system_program
            AccountMeta::new_readonly(system_program::id(), false),
            // 7: rent (or RealmConfig in newer versions)
            AccountMeta::new_readonly(ctx.accounts.rent.key(), false),
            // v3 also expects:
            // 8: proposal_deposit (writable)
            AccountMeta::new(ctx.accounts.proposal_deposit.key(), false),
            // 9: community_token_mint
            AccountMeta::new_readonly(ctx.accounts.community_mint.key(), false),
        ],
        data: ix_data,
    };

    // Invoke CPI — proposer signs directly (no PDA signing needed here)
    invoke_signed(
        &create_proposal_ix,
        &[
            ctx.accounts.realm.to_account_info(),
            ctx.accounts.governance_proposal.to_account_info(),
            ctx.accounts.governance.to_account_info(),
            ctx.accounts.token_owner_record.to_account_info(),
            ctx.accounts.governance_authority.to_account_info(),
            ctx.accounts.payer.to_account_info(),
            ctx.accounts.system_program.to_account_info(),
            ctx.accounts.rent.to_account_info(),
            ctx.accounts.proposal_deposit.to_account_info(),
            ctx.accounts.community_mint.to_account_info(),
            ctx.accounts.spl_governance_program.to_account_info(),
        ],
        &[], // no PDA signer seeds needed — proposer signs directly
    ).map_err(|_| RealmsError::GovernanceCpiFailed)?;

    // ── Record the proposal in Send.it state ──
    let record = &mut ctx.accounts.proposal_record;
    record.governance_proposal = ctx.accounts.governance_proposal.key();
    record.creator = ctx.accounts.proposer.key();
    let title_bytes = title.as_bytes();
    record.title[..title_bytes.len()].copy_from_slice(title_bytes);
    record.title_len = title_bytes.len() as u8;
    record.created_at = Clock::get()?.unix_timestamp;
    record.bump = ctx.bumps.proposal_record;

    emit!(RealmsProposalCreated {
        governance_proposal: ctx.accounts.governance_proposal.key(),
        creator: ctx.accounts.proposer.key(),
        title,
    });

    Ok(())
}

/// Cast a vote on a Realms proposal through Send.it.
///
/// Routes to spl-governance's CastVote CPI, then records
/// the vote in Send.it state for loyalty/points integration.
pub fn handle_cast_realms_vote(
    ctx: Context<CastRealmsVote>,
    vote_choice: u8, // 0=Approve, 1=Deny, 2=Abstain, 3=Veto
) -> Result<()> {
    require!(vote_choice <= 3, RealmsError::InvalidVoteChoice);

    // Get voter's token balance for weight tracking
    let token_data = ctx.accounts.voter_token_account.try_borrow_data()?;
    let weight = if token_data.len() >= 72 {
        u64::from_le_bytes(token_data[64..72].try_into().unwrap())
    } else {
        0u64
    };

    // ── Build SPL Governance CastVote instruction ──
    // spl-governance v3 CastVote = instruction index 4
    let mut ix_data = Vec::new();
    ix_data.push(4u8); // CastVote discriminator

    // Vote (borsh): enum Vote { Approve(Vec<VoteChoice>), Deny, Abstain, Veto }
    match vote_choice {
        0 => {
            // Approve — with a single choice (rank 0, weight_percentage 100)
            ix_data.push(0); // Approve variant
            // Vec<VoteChoice> — 1 element
            ix_data.extend_from_slice(&1u32.to_le_bytes());
            // VoteChoice { rank: 0, weight_percentage: 100 }
            ix_data.push(0); // rank
            ix_data.push(100); // weight_percentage
        }
        1 => {
            ix_data.push(1); // Deny variant
        }
        2 => {
            ix_data.push(2); // Abstain variant
        }
        3 => {
            ix_data.push(3); // Veto variant
        }
        _ => return Err(RealmsError::InvalidVoteChoice.into()),
    }

    let cast_vote_ix = Instruction {
        program_id: SPL_GOVERNANCE_ID,
        accounts: vec![
            // 0: realm
            AccountMeta::new_readonly(ctx.accounts.realm.key(), false),
            // 1: governance
            AccountMeta::new_readonly(ctx.accounts.governance.key(), false),
            // 2: proposal
            AccountMeta::new(ctx.accounts.governance_proposal.key(), false),
            // 3: token_owner_record (voter's)
            AccountMeta::new(ctx.accounts.token_owner_record.key(), false),
            // 4: governance_vote_record (to be created)
            AccountMeta::new(ctx.accounts.governance_vote_record.key(), false),
            // 5: governance_authority (voter, signer)
            AccountMeta::new_readonly(ctx.accounts.governance_authority.key(), true),
            // 6: payer
            AccountMeta::new(ctx.accounts.payer.key(), true),
            // 7: system_program
            AccountMeta::new_readonly(system_program::id(), false),
            // 8: rent
            AccountMeta::new_readonly(ctx.accounts.rent.key(), false),
        ],
        data: ix_data,
    };

    invoke_signed(
        &cast_vote_ix,
        &[
            ctx.accounts.realm.to_account_info(),
            ctx.accounts.governance.to_account_info(),
            ctx.accounts.governance_proposal.to_account_info(),
            ctx.accounts.token_owner_record.to_account_info(),
            ctx.accounts.governance_vote_record.to_account_info(),
            ctx.accounts.governance_authority.to_account_info(),
            ctx.accounts.payer.to_account_info(),
            ctx.accounts.system_program.to_account_info(),
            ctx.accounts.rent.to_account_info(),
            ctx.accounts.spl_governance_program.to_account_info(),
        ],
        &[],
    ).map_err(|_| RealmsError::GovernanceCpiFailed)?;

    // ── Record the vote in Send.it state ──
    let record = &mut ctx.accounts.vote_record;
    record.governance_proposal = ctx.accounts.governance_proposal.key();
    record.voter = ctx.accounts.voter.key();
    record.vote_choice = vote_choice;
    record.weight = weight;
    record.voted_at = Clock::get()?.unix_timestamp;
    record.bump = ctx.bumps.vote_record;

    emit!(RealmsVoteCast {
        governance_proposal: ctx.accounts.governance_proposal.key(),
        voter: ctx.accounts.voter.key(),
        vote_choice,
        weight,
    });

    Ok(())
}
