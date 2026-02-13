use anchor_lang::prelude::*;

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
