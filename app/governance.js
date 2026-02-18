/**
 * Send.it DAO Governance — Frontend JS
 * 
 * Integrates with SPL Governance (Realms) on-chain for real DAO governance.
 * Falls back to sample proposals if on-chain fetch fails.
 */

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------
const NETWORK = 'devnet';
const RPC_URL = 'https://api.devnet.solana.com';
const GOVERNANCE_PROGRAM_ID = 'GovER5Lthms3bLBqWub97yVrMmEogzX7xNjdXpPPCVZw';
const GOV_PROGRAM_PK = new solanaWeb3.PublicKey(GOVERNANCE_PROGRAM_ID);
const SENDIT_MINT = 'F8qWTN8JfyDCvj4RoCHuvNMVbTV9XQksLuziA8PYpump';
const SENDIT_PROGRAM = 'HTKq18cATdwCZb6XM66Mhn8JWKCFTrZqH6zU1zip88Zx';

// DAO Addresses (LIVE on devnet)
const REALM_ADDRESS = new solanaWeb3.PublicKey('BCa4t2Z43MhW98DznzMKTqQy9ibtCLdJ2TEnbBNqirZ6');
const GOVERNANCE_ADDRESS = new solanaWeb3.PublicKey('EMwc1eS5L4YYUuBdKvFWeERMG2GuKHWHi7VCikYRXWZ1');
const GOV_TOKEN_MINT = new solanaWeb3.PublicKey('DbUFXig3HSfM6N1U3ckruf6xRTeZZo3gpCUr4EEBFhBq');
const TOKEN_OWNER_RECORD = new solanaWeb3.PublicKey('3hKXvJXdapZwFgoY7MHhr1dKWNxyRBoZBxBQsXEHNLs7');

// State
let connection = null;
let wallet = null;
let walletPubkey = null;
let currentTab = 'all';
let onChainProposals = [];
let useOnChain = false;

// ---------------------------------------------------------------------------
// SPL Governance Account Parsing
// ---------------------------------------------------------------------------

// Account types in SPL Governance v3
const GOV_ACCOUNT_TYPE = {
  Realm: 16,            // RealmV2
  TokenOwnerRecord: 17, // TokenOwnerRecordV2
  Governance: 18,       // GovernanceV2
  ProposalV2: 14,
  VoteRecordV2: 12,
  ProposalTransactionV2: 13,
  SignatoryRecordV2: 22,
};

// Proposal state enum
const PROPOSAL_STATE = {
  0: 'active',     // Draft
  1: 'active',     // SigningOff
  2: 'active',     // Voting
  3: 'passed',     // Succeeded
  4: 'rejected',   // Defeated
  5: 'active',     // Executing
  6: 'passed',     // Completed
  7: 'cancelled',  // Cancelled
};

const PROPOSAL_STATE_LABEL = {
  0: 'Draft',
  1: 'Signing Off',
  2: 'Voting',
  3: 'Succeeded',
  4: 'Defeated',
  5: 'Executing',
  6: 'Completed',
  7: 'Cancelled',
};

/**
 * Parse a ProposalV2 account from raw bytes (SPL Governance v3)
 */
function parseProposalV2(data, pubkey) {
  // ProposalV2 layout (v3):
  // 0: u8 account_type (6)
  // 1-32: Pubkey governance
  // 33-64: Pubkey governing_token_mint
  // 65: u8 state
  // 66-73: u64 token_owner_record (skip)
  // ... complex layout, parse key fields

  if (data[0] !== GOV_ACCOUNT_TYPE.ProposalV2) return null;

  const state = data[65];
  
  // Parse name: offset varies. In v3, after fixed fields (~168 bytes), there's a borsh string.
  // We'll find the name by looking for the borsh string length prefix.
  // Simplified: search for reasonable string data after the fixed header.
  let name = '';
  let description = '';
  
  try {
    // Scan for title string — starts after fixed header + vote options (~offset 200-300)
    // Title is typically the longest meaningful string after the options array
    const strings = [];
    let scanPos = 100;
    while (scanPos < data.length - 4) {
      const found = findBorshString(data, scanPos);
      if (found) {
        strings.push(found);
        scanPos = found.end;
      } else {
        scanPos++;
      }
    }
    // Title is usually the string with length > 10 that comes after vote option labels
    const meaningful = strings.filter(s => s.value.length > 10);
    if (meaningful.length >= 1) name = meaningful[0].value;
    if (meaningful.length >= 2) description = meaningful[1].value;
  } catch (e) {
    // Fallback
  }

  // Parse vote counts from the options array
  // In v3, options are after description as a borsh Vec<ProposalOption>
  // Each ProposalOption: string label + u64 vote_weight + Vec<ProposalTransaction> + ...
  
  // For now, extract what we can from the state
  return {
    pubkey: pubkey,
    state,
    stateLabel: PROPOSAL_STATE_LABEL[state] || 'Unknown',
    status: PROPOSAL_STATE[state] || 'active',
    name: name || `Proposal ${pubkey.slice(0, 8)}...`,
    description: description || '',
    governance: bs58Encode(data.slice(1, 33)),
    tokenMint: bs58Encode(data.slice(33, 65)),
  };
}

/**
 * Find a borsh-encoded string (u32 length + utf8 bytes) in raw data
 */
function findBorshString(data, startOffset) {
  for (let i = startOffset; i < Math.min(data.length - 4, startOffset + 200); i++) {
    const len = data[i] | (data[i + 1] << 8) | (data[i + 2] << 16) | (data[i + 3] << 24);
    if (len > 0 && len < 256 && i + 4 + len <= data.length) {
      const bytes = data.slice(i + 4, i + 4 + len);
      // Check if it looks like UTF-8 text
      const isText = bytes.every(b => (b >= 32 && b < 127) || b === 10 || b === 13);
      if (isText && len >= 3) {
        return {
          value: new TextDecoder().decode(bytes),
          end: i + 4 + len,
        };
      }
    }
  }
  return null;
}

/**
 * Simple base58 encode (for display only — not cryptographic)
 */
function bs58Encode(bytes) {
  // Use web3.js PublicKey for proper encoding
  try {
    return new solanaWeb3.PublicKey(bytes).toBase58();
  } catch {
    return Array.from(bytes).map(b => b.toString(16).padStart(2, '0')).join('');
  }
}

// ---------------------------------------------------------------------------
// On-Chain Data Fetching
// ---------------------------------------------------------------------------

async function fetchOnChainGovernance() {
  try {
    // Verify realm exists
    const realmInfo = await connection.getAccountInfo(REALM_ADDRESS);
    if (!realmInfo) {
      console.log('Realm not found on-chain');
      return false;
    }
    console.log('✅ Realm found, owner:', realmInfo.owner.toBase58(), 'size:', realmInfo.data.length);

    // Verify governance exists
    const govInfo = await connection.getAccountInfo(GOVERNANCE_ADDRESS);
    if (!govInfo) {
      console.log('Governance not found on-chain');
      return false;
    }
    console.log('✅ Governance found, size:', govInfo.data.length);

    // Update DAO config display
    updateDAOConfig({
      realm: REALM_ADDRESS.toBase58(),
      governance: GOVERNANCE_ADDRESS.toBase58(),
      tokenMint: GOV_TOKEN_MINT.toBase58(),
      voteThreshold: '60%',
      minTokensToPropose: '1,000,000 SENDIT-GOV',
      votingPeriod: '3 days',
      status: 'LIVE',
    });

    // Fetch proposals (getProgramAccounts filtered by governance)
    const proposals = await fetchProposals();
    if (proposals.length > 0) {
      onChainProposals = proposals;
      useOnChain = true;
      console.log(`✅ Found ${proposals.length} on-chain proposals`);
    } else {
      console.log('No proposals yet — showing sample proposals');
    }

    return true;
  } catch (e) {
    console.error('Failed to fetch on-chain data:', e);
    return false;
  }
}

async function fetchProposals() {
  try {
    // Fetch all ProposalV2 accounts under our governance
    const accounts = await connection.getProgramAccounts(GOV_PROGRAM_PK, {
      filters: [
        { dataSize: undefined }, // Don't filter by size — proposals vary
        { memcmp: { offset: 1, bytes: GOVERNANCE_ADDRESS.toBase58() } }, // governance pubkey at offset 1
      ],
    });

    const proposals = [];
    for (const { pubkey, account } of accounts) {
      if (account.data[0] === GOV_ACCOUNT_TYPE.ProposalV2) {
        const parsed = parseProposalV2(account.data, pubkey.toBase58());
        if (parsed) proposals.push(parsed);
      }
    }

    return proposals;
  } catch (e) {
    console.error('Failed to fetch proposals:', e);
    return [];
  }
}

function updateDAOConfig(config) {
  const el = document.getElementById('daoConfig');
  if (!el) return;
  
  el.innerHTML = `
    <div class="dao-config-grid">
      <div class="config-item">
        <span class="config-label">Status</span>
        <span class="config-value" style="color:var(--green)">🟢 ${config.status}</span>
      </div>
      <div class="config-item">
        <span class="config-label">Realm</span>
        <a class="config-value config-link" href="https://app.realms.today/dao/${config.realm}?cluster=devnet" target="_blank">${config.realm.slice(0, 8)}...${config.realm.slice(-4)}</a>
      </div>
      <div class="config-item">
        <span class="config-label">Governance</span>
        <a class="config-value config-link" href="https://explorer.solana.com/address/${config.governance}?cluster=devnet" target="_blank">${config.governance.slice(0, 8)}...${config.governance.slice(-4)}</a>
      </div>
      <div class="config-item">
        <span class="config-label">Gov Token</span>
        <a class="config-value config-link" href="https://explorer.solana.com/address/${config.tokenMint}?cluster=devnet" target="_blank">${config.tokenMint.slice(0, 8)}...${config.tokenMint.slice(-4)}</a>
      </div>
      <div class="config-item">
        <span class="config-label">Vote Threshold</span>
        <span class="config-value">${config.voteThreshold}</span>
      </div>
      <div class="config-item">
        <span class="config-label">Min Tokens to Propose</span>
        <span class="config-value">${config.minTokensToPropose}</span>
      </div>
      <div class="config-item">
        <span class="config-label">Voting Period</span>
        <span class="config-value">${config.votingPeriod}</span>
      </div>
      <div class="config-item">
        <span class="config-label">Realms UI</span>
        <a class="config-value config-link" href="https://app.realms.today/dao/${config.realm}?cluster=devnet" target="_blank">Open in Realms ↗</a>
      </div>
    </div>
  `;
}

// ---------------------------------------------------------------------------
// Sample proposals (shown when no on-chain proposals exist)
// ---------------------------------------------------------------------------
const sampleProposals = [
  {
    id: 'SIP-001',
    title: 'Reduce Swap Fee from 1% to 0.5%',
    description: 'Proposal to reduce the AMM swap fee from 100bps to 50bps to increase trading volume and attract more liquidity.',
    category: 'parameter',
    status: 'active',
    options: [
      { label: 'For', votes: 4250000, class: 'for' },
      { label: 'Against', votes: 1800000, class: 'against' },
      { label: 'Abstain', votes: 350000, class: 'abstain' },
    ],
    quorum: 10000000, totalVotes: 6400000, voters: 23,
    creator: 'G3QL...dNP9',
    startTime: Date.now() - 86400000, endTime: Date.now() + 172800000,
  },
  {
    id: 'SIP-002',
    title: 'Add PYUSD Stablecoin Pool',
    description: 'Create a SENDIT/PYUSD trading pair to offer stablecoin swaps via our PYUSD vault integration.',
    category: 'parameter',
    status: 'active',
    options: [
      { label: 'For', votes: 5600000, class: 'for' },
      { label: 'Against', votes: 800000, class: 'against' },
      { label: 'Abstain', votes: 200000, class: 'abstain' },
    ],
    quorum: 10000000, totalVotes: 6600000, voters: 31,
    creator: 'G86j...cLXR',
    startTime: Date.now() - 48000000, endTime: Date.now() + 120000000,
  },
  {
    id: 'SIP-003',
    title: 'Allocate 5% Treasury for Bug Bounty',
    description: 'Fund a public bug bounty program. Critical $5k, High $2k, Medium $500. Managed through Sec3 partnership.',
    category: 'treasury',
    status: 'passed',
    options: [
      { label: 'For', votes: 8200000, class: 'for' },
      { label: 'Against', votes: 1200000, class: 'against' },
      { label: 'Abstain', votes: 600000, class: 'abstain' },
    ],
    quorum: 8000000, totalVotes: 10000000, voters: 45,
    creator: 'G3QL...dNP9',
    startTime: Date.now() - 400000000, endTime: Date.now() - 100000000,
  },
  {
    id: 'SIP-004',
    title: 'Upgrade Program to Include Limit Orders',
    description: 'Deploy the limit orders module from the 31-module suite to the slim devnet program.',
    category: 'upgrade',
    status: 'rejected',
    options: [
      { label: 'For', votes: 3100000, class: 'for' },
      { label: 'Against', votes: 5400000, class: 'against' },
      { label: 'Abstain', votes: 500000, class: 'abstain' },
    ],
    quorum: 8000000, totalVotes: 9000000, voters: 38,
    creator: 'G86j...cLXR',
    startTime: Date.now() - 500000000, endTime: Date.now() - 200000000,
  },
  {
    id: 'SIP-005',
    title: 'Community Airdrop for Early Devnet Testers',
    description: 'Reward early devnet testers with SENDIT. 1M distributed proportionally to wallets that interacted with the devnet program.',
    category: 'community',
    status: 'active',
    options: [
      { label: 'For', votes: 3800000, class: 'for' },
      { label: 'Against', votes: 900000, class: 'against' },
      { label: 'Abstain', votes: 300000, class: 'abstain' },
    ],
    quorum: 10000000, totalVotes: 5000000, voters: 18,
    creator: 'G3QL...dNP9',
    startTime: Date.now() - 20000000, endTime: Date.now() + 240000000,
  },
];

// ---------------------------------------------------------------------------
// Init
// ---------------------------------------------------------------------------
window.addEventListener('DOMContentLoaded', async () => {
  connection = new solanaWeb3.Connection(RPC_URL, 'confirmed');
  
  // Show loading
  const container = document.getElementById('proposalsList');
  container.innerHTML = '<div class="connect-prompt"><p>Loading on-chain governance data...</p></div>';

  // Try to fetch real on-chain data
  const onChain = await fetchOnChainGovernance();
  
  if (onChain && onChainProposals.length > 0) {
    renderOnChainProposals(onChainProposals);
    updateStats(onChainProposals);
  } else {
    // Show sample proposals with label
    renderProposals(sampleProposals);
    updateStats(sampleProposals);
    
    if (onChain) {
      // DAO exists but no proposals yet
      showToast('DAO is live on devnet! No proposals yet — create one via Realms.', 'success');
    }
  }
});

// ---------------------------------------------------------------------------
// Render On-Chain Proposals
// ---------------------------------------------------------------------------
function renderOnChainProposals(proposals) {
  const container = document.getElementById('proposalsList');
  
  if (proposals.length === 0) {
    container.innerHTML = `
      <div class="connect-prompt">
        <p>No proposals yet. Create one via <a href="https://app.realms.today/dao/${REALM_ADDRESS.toBase58()}?cluster=devnet" target="_blank">Realms</a>!</p>
      </div>`;
    return;
  }

  container.innerHTML = proposals.map(p => {
    const badgeClass = {
      active: 'badge-active',
      passed: 'badge-passed',
      rejected: 'badge-rejected',
    }[p.status] || 'badge-pending';

    return `
      <div class="proposal-card" data-status="${p.status}">
        <div class="proposal-header">
          <div>
            <div class="proposal-id">On-Chain • ${p.pubkey.slice(0, 8)}...</div>
            <div class="proposal-title">${p.name}</div>
          </div>
          <span class="badge ${badgeClass}">${p.stateLabel}</span>
        </div>
        <div class="proposal-desc">${p.description || 'No description available.'}</div>
        <div class="proposal-meta">
          <span>🔗 <a href="https://explorer.solana.com/address/${p.pubkey}?cluster=devnet" target="_blank">View on Explorer</a></span>
          <span>🏛 <a href="https://app.realms.today/dao/${REALM_ADDRESS.toBase58()}/proposal/${p.pubkey}?cluster=devnet" target="_blank">View on Realms</a></span>
        </div>
      </div>`;
  }).join('');
}

// ---------------------------------------------------------------------------
// Render Sample Proposals
// ---------------------------------------------------------------------------
function renderProposals(proposals) {
  const filtered = currentTab === 'all' 
    ? proposals 
    : proposals.filter(p => p.status === currentTab);
  
  const container = document.getElementById('proposalsList');
  
  if (filtered.length === 0) {
    container.innerHTML = '<div class="connect-prompt"><p>No proposals found</p></div>';
    return;
  }

  container.innerHTML = filtered.map(p => {
    const total = p.totalVotes || 1;
    const timeLeft = p.endTime - Date.now();
    const timeStr = timeLeft > 0 
      ? `${Math.floor(timeLeft / 86400000)}d ${Math.floor((timeLeft % 86400000) / 3600000)}h left`
      : 'Voting ended';
    const quorumPct = Math.min(100, (p.totalVotes / p.quorum * 100)).toFixed(1);
    
    const badgeClass = {
      active: 'badge-active', passed: 'badge-passed',
      rejected: 'badge-rejected', pending: 'badge-pending',
    }[p.status] || 'badge-pending';

    const categoryIcons = {
      parameter: '⚙️', treasury: '💰', upgrade: '🔧', community: '🤝',
    };

    return `
      <div class="proposal-card" data-status="${p.status}">
        <div class="proposal-header">
          <div>
            <div class="proposal-id">${p.id} • ${categoryIcons[p.category] || '📋'} ${p.category}</div>
            <div class="proposal-title">${p.title}</div>
          </div>
          <span class="badge ${badgeClass}">${p.status}</span>
        </div>
        <div class="proposal-desc">${p.description}</div>
        <div class="options">
          ${p.options.map(opt => {
            const pct = ((opt.votes / total) * 100).toFixed(1);
            return `
              <div class="option">
                <span class="option-label">${opt.label}</span>
                <div class="option-bar-bg">
                  <div class="option-bar ${opt.class}" style="width:${pct}%"></div>
                  <span class="option-pct">${pct}%</span>
                </div>
                <span class="option-votes">${formatVotes(opt.votes)} SENDIT</span>
              </div>`;
          }).join('')}
        </div>
        <div class="quorum-bar">
          <div class="quorum-label">
            <span>Quorum: ${quorumPct}%</span>
            <span>${formatVotes(p.totalVotes)} / ${formatVotes(p.quorum)} SENDIT</span>
          </div>
          <div class="quorum-track">
            <div class="quorum-fill" style="width:${quorumPct}%"></div>
          </div>
        </div>
        ${p.status === 'active' ? `
        <div class="vote-actions">
          <button class="btn btn-green" onclick="castVote('${p.id}', 0)">👍 For</button>
          <button class="btn btn-red" onclick="castVote('${p.id}', 1)">👎 Against</button>
          <button class="btn btn-secondary" onclick="castVote('${p.id}', 2)">🤷 Abstain</button>
        </div>` : ''}
        <div class="proposal-meta">
          <span>👤 ${p.creator}</span>
          <span>🗳 ${p.voters} voters</span>
          <span>⏱ ${timeStr}</span>
        </div>
      </div>`;
  }).join('');

  // Add "UI Preview" label
  container.innerHTML += '<div style="text-align:center;font-size:13px;color:#888;margin-top:16px">UI Preview — Sample proposals. Create real proposals via <a href="https://app.realms.today/dao/' + REALM_ADDRESS.toBase58() + '?cluster=devnet" target="_blank" style="color:#3498db">Realms</a></div>';
}

function updateStats(proposals) {
  const active = proposals.filter(p => p.status === 'active').length;
  const passed = proposals.filter(p => p.status === 'passed').length;
  const total = proposals.length;
  const voters = proposals.reduce((s, p) => s + (p.voters || 0), 0);
  const passRate = total > active && total > 0 ? ((passed / (total - active)) * 100).toFixed(0) : 0;

  document.getElementById('totalProposals').textContent = total;
  document.getElementById('activeProposals').textContent = active;
  document.getElementById('totalVoters').textContent = voters || '—';
  document.getElementById('passRate').textContent = (isNaN(passRate) ? 0 : passRate) + '%';
}

function formatVotes(n) {
  if (n >= 1000000) return (n / 1000000).toFixed(1) + 'M';
  if (n >= 1000) return (n / 1000).toFixed(0) + 'K';
  return n.toString();
}

// ---------------------------------------------------------------------------
// Tabs
// ---------------------------------------------------------------------------
function switchTab(tab) {
  currentTab = tab;
  document.querySelectorAll('.tab').forEach(t => t.classList.remove('active'));
  event.target.classList.add('active');
  if (useOnChain && onChainProposals.length > 0) {
    renderOnChainProposals(onChainProposals.filter(p => tab === 'all' || p.status === tab));
  } else {
    renderProposals(sampleProposals);
  }
}

// ---------------------------------------------------------------------------
// Wallet
// ---------------------------------------------------------------------------
async function connectWallet() {
  try {
    if (!window.solana || !window.solana.isPhantom) {
      showToast('Please install Phantom wallet', 'error');
      return;
    }
    const resp = await window.solana.connect();
    walletPubkey = resp.publicKey;
    wallet = window.solana;
    document.getElementById('connectBtn').textContent = 
      walletPubkey.toBase58().slice(0, 4) + '...' + walletPubkey.toBase58().slice(-4);
    showToast('Wallet connected!', 'success');
  } catch (err) {
    showToast('Failed to connect: ' + err.message, 'error');
  }
}

// ---------------------------------------------------------------------------
// Voting
// ---------------------------------------------------------------------------
async function castVote(proposalId, optionIndex) {
  if (!walletPubkey) {
    showToast('Connect your wallet first!', 'error');
    return;
  }
  const labels = ['For', 'Against', 'Abstain'];
  showToast(`Vote "${labels[optionIndex]}" on ${proposalId}! (UI Preview — use Realms for on-chain voting)`, 'success');
}

// ---------------------------------------------------------------------------
// Create Proposal
// ---------------------------------------------------------------------------
function openCreateProposal() {
  document.getElementById('createForm').classList.add('active');
}

function closeCreateProposal() {
  document.getElementById('createForm').classList.remove('active');
}

async function submitProposal() {
  if (!walletPubkey) {
    showToast('Connect your wallet first!', 'error');
    return;
  }
  const title = document.getElementById('proposalTitle').value.trim();
  const desc = document.getElementById('proposalDesc').value.trim();
  if (!title || !desc) {
    showToast('Please fill in title and description', 'error');
    return;
  }
  // Redirect to Realms for actual proposal creation
  window.open(`https://app.realms.today/dao/${REALM_ADDRESS.toBase58()}/proposal/new?cluster=devnet`, '_blank');
  closeCreateProposal();
  showToast('Redirecting to Realms to create proposal on-chain...', 'success');
}

// ---------------------------------------------------------------------------
// Toast
// ---------------------------------------------------------------------------
function showToast(msg, type = 'success') {
  const toast = document.getElementById('toast');
  toast.textContent = msg;
  toast.className = `toast ${type}`;
  toast.style.display = 'block';
  setTimeout(() => { toast.style.display = 'none'; }, 4000);
}

document.addEventListener('click', (e) => {
  if (e.target.id === 'createForm') closeCreateProposal();
});
