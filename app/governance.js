/**
 * Send.it DAO Governance — Frontend JS
 * 
 * Integrates with SPL Governance (Realms) for on-chain DAO governance.
 * SENDIT token holders create proposals, vote, and manage protocol parameters.
 */

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------
const NETWORK = 'devnet';
const RPC_URL = 'https://api.devnet.solana.com';
const GOVERNANCE_PROGRAM_ID = 'GovER5Lthms3bLBqWub97yVrMmEogzX7xNjdXpPPCVZw';
const SENDIT_MINT = 'F8qWTN8JfyDCvj4RoCHuvNMVbTV9XQksLuziA8PYpump';
const SENDIT_PROGRAM = 'HTKq18cATdwCZb6XM66Mhn8JWKCFTrZqH6zU1zip88Zx';

// Will be set after DAO creation
let REALM_ADDRESS = null;
let GOVERNANCE_ADDRESS = null;

// State
let connection = null;
let wallet = null;
let walletPubkey = null;
let currentTab = 'all';

// Sample proposals (will be replaced with on-chain data)
const sampleProposals = [
  {
    id: 'SIP-001',
    title: 'Reduce Swap Fee from 1% to 0.5%',
    description: 'Proposal to reduce the AMM swap fee from 100bps to 50bps to increase trading volume and attract more liquidity. The LP share would go from 0.3% to 0.2%, and protocol fee from 0.7% to 0.3%.',
    category: 'parameter',
    status: 'active',
    options: [
      { label: 'For', votes: 4250000, class: 'for' },
      { label: 'Against', votes: 1800000, class: 'against' },
      { label: 'Abstain', votes: 350000, class: 'abstain' },
    ],
    quorum: 10000000,
    totalVotes: 6400000,
    voters: 23,
    creator: 'G3QL...dNP9',
    startTime: Date.now() - 86400000,
    endTime: Date.now() + 172800000,
  },
  {
    id: 'SIP-002',
    title: 'Add PYUSD Stablecoin Pool',
    description: 'Create a SENDIT/PYUSD trading pair to offer stablecoin swaps. This would leverage our existing PYUSD vault integration and provide lower-volatility trading options.',
    category: 'parameter',
    status: 'active',
    options: [
      { label: 'For', votes: 5600000, class: 'for' },
      { label: 'Against', votes: 800000, class: 'against' },
      { label: 'Abstain', votes: 200000, class: 'abstain' },
    ],
    quorum: 10000000,
    totalVotes: 6600000,
    voters: 31,
    creator: 'G86j...cLXR',
    startTime: Date.now() - 48000000,
    endTime: Date.now() + 120000000,
  },
  {
    id: 'SIP-003',
    title: 'Allocate 5% Treasury for Bug Bounty Program',
    description: 'Fund a public bug bounty program using 5% of protocol treasury. Rewards: Critical $5k, High $2k, Medium $500. Managed through Sec3 partnership.',
    category: 'treasury',
    status: 'passed',
    options: [
      { label: 'For', votes: 8200000, class: 'for' },
      { label: 'Against', votes: 1200000, class: 'against' },
      { label: 'Abstain', votes: 600000, class: 'abstain' },
    ],
    quorum: 8000000,
    totalVotes: 10000000,
    voters: 45,
    creator: 'G3QL...dNP9',
    startTime: Date.now() - 400000000,
    endTime: Date.now() - 100000000,
  },
  {
    id: 'SIP-004',
    title: 'Upgrade Program to Include Limit Orders',
    description: 'Proposal to deploy the limit orders module from the full 31-module suite to the slim devnet program. This adds on-chain limit orders alongside the existing AMM.',
    category: 'upgrade',
    status: 'rejected',
    options: [
      { label: 'For', votes: 3100000, class: 'for' },
      { label: 'Against', votes: 5400000, class: 'against' },
      { label: 'Abstain', votes: 500000, class: 'abstain' },
    ],
    quorum: 8000000,
    totalVotes: 9000000,
    voters: 38,
    creator: 'G86j...cLXR',
    startTime: Date.now() - 500000000,
    endTime: Date.now() - 200000000,
  },
  {
    id: 'SIP-005',
    title: 'Community Airdrop for Early Devnet Testers',
    description: 'Reward early devnet testers with a SENDIT airdrop. 1M SENDIT distributed proportionally to wallets that interacted with the devnet program before mainnet launch.',
    category: 'community',
    status: 'active',
    options: [
      { label: 'For', votes: 3800000, class: 'for' },
      { label: 'Against', votes: 900000, class: 'against' },
      { label: 'Abstain', votes: 300000, class: 'abstain' },
    ],
    quorum: 10000000,
    totalVotes: 5000000,
    voters: 18,
    creator: 'G3QL...dNP9',
    startTime: Date.now() - 20000000,
    endTime: Date.now() + 240000000,
  },
];

// ---------------------------------------------------------------------------
// Init
// ---------------------------------------------------------------------------
window.addEventListener('DOMContentLoaded', () => {
  connection = new solanaWeb3.Connection(RPC_URL, 'confirmed');
  renderProposals(sampleProposals);
  updateStats(sampleProposals);
});

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
// Render
// ---------------------------------------------------------------------------
function renderProposals(proposals) {
  const filtered = currentTab === 'all' 
    ? proposals 
    : proposals.filter(p => p.status === currentTab);
  
  const container = document.getElementById('proposalsList');
  
  if (filtered.length === 0) {
    container.innerHTML = `
      <div class="connect-prompt">
        <p>No ${currentTab === 'all' ? '' : currentTab + ' '}proposals found</p>
      </div>`;
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
      active: 'badge-active',
      passed: 'badge-passed',
      rejected: 'badge-rejected',
      pending: 'badge-pending',
    }[p.status] || 'badge-pending';

    const categoryIcons = {
      parameter: '⚙️',
      treasury: '💰',
      upgrade: '🔧',
      community: '🤝',
      other: '📋',
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
}

function updateStats(proposals) {
  const active = proposals.filter(p => p.status === 'active').length;
  const passed = proposals.filter(p => p.status === 'passed').length;
  const total = proposals.length;
  const voters = [...new Set(proposals.flatMap(p => Array(p.voters).fill(0)))].length || 
    proposals.reduce((s, p) => s + p.voters, 0);
  const passRate = total > 0 ? ((passed / (total - active)) * 100).toFixed(0) : 0;

  document.getElementById('totalProposals').textContent = total;
  document.getElementById('activeProposals').textContent = active;
  document.getElementById('totalVoters').textContent = voters;
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
  renderProposals(sampleProposals);
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
  showToast(`Vote "${labels[optionIndex]}" cast on ${proposalId}! (UI Preview — Devnet)`, 'success');
  
  // In production, this would call SPL Governance withCastVote
  // const tx = new solanaWeb3.Transaction();
  // await withCastVote(tx.instructions, ...);
  // const sig = await wallet.signAndSendTransaction(tx);
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
  
  showToast(`Proposal "${title}" created! (UI Preview — Devnet)`, 'success');
  closeCreateProposal();
  
  // In production:
  // const tx = new solanaWeb3.Transaction();
  // await withCreateProposal(tx.instructions, ...);
  // const sig = await wallet.signAndSendTransaction(tx);
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

// Close modal on overlay click
document.addEventListener('click', (e) => {
  if (e.target.id === 'createForm') closeCreateProposal();
});
