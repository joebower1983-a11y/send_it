/**
 * Send.it Loyalty — Browser-side Torque integration
 * 
 * Connects wallet → fetches on-chain activity → calculates points →
 * displays tier, leaderboard, and active campaigns.
 * Falls back to local parsing when Torque API is unavailable.
 */

// ── Constants ────────────────────────────────────────────────────────
const SENDIT_PROGRAM = 'HTKq18cATdwCZb6XM66Mhn8JWKCFTrZqH6zU1zip88Zx';
const SENDIT_MINT    = 'F8qWTN8JfyDCvj4RoCHuvNMVbTV9XQksLuziA8PYpump';
const RPC_DEVNET     = 'https://api.devnet.solana.com';
const TAPESTRY_API   = 'https://api.usetapestry.dev/api/v1';
const TAPESTRY_APP   = '601a8251-9c95-4456-97af-c1e79b5c0259';

const TIERS = [
  { name: 'Bronze',   emoji: '🥉', min: 0,      mult: 1.0,  color: '#cd7f32' },
  { name: 'Silver',   emoji: '🥈', min: 1000,   mult: 1.25, color: '#c0c0c0' },
  { name: 'Gold',     emoji: '🥇', min: 5000,   mult: 1.5,  color: '#ffd700' },
  { name: 'Platinum', emoji: '💎', min: 25000,  mult: 2.0,  color: '#e5e4e2' },
  { name: 'Diamond',  emoji: '👑', min: 100000, mult: 3.0,  color: '#b9f2ff' },
];

const POINTS = {
  swap: 10, buy: 15, sell: 5, add_liq: 50,
  stake: 25, stake_day: 1, launch: 500,
  profile: 100, follow: 5, post: 10,
  referral: 200, ref_vol: 5, vote: 50,
};

// Anchor discriminators (first 8 bytes of sha256("global:<name>"))
const DISC = {
  'f223c68952e1f2b6': 'buy',
  'd33e5e5e3edcab90': 'sell',
  'f8c69e91e17587c8': 'swap',
  '26d4c1727e411b7c': 'stake',
  '90b2627574652e4e': 'unstake',
  '848c70d3ee3c6526': 'create_token',
  'e992d18ecf6840bc': 'create_pool',
  '7a5ffee3dd0b53e5': 'add_liquidity',
  '5055d14818ceb016': 'remove_liquidity',
};

// ── State ────────────────────────────────────────────────────────────
let walletPubkey = null;
let connection = null;
let userActivity = null;
let userPoints = 0;

// ── Init ─────────────────────────────────────────────────────────────
document.addEventListener('DOMContentLoaded', () => {
  // Check if wallet was previously connected
  if (window.solana?.isPhantom && window.solana.isConnected) {
    handleConnect(window.solana.publicKey.toBase58());
  }
});

async function connectWallet() {
  if (!window.solana?.isPhantom) {
    alert('Please install Phantom wallet to use Send.it Loyalty');
    return;
  }
  try {
    const resp = await window.solana.connect();
    handleConnect(resp.publicKey.toBase58());
  } catch (e) {
    console.error('Wallet connect failed:', e);
  }
}

async function handleConnect(pubkey) {
  walletPubkey = pubkey;
  document.getElementById('connectBtn').textContent = pubkey.slice(0, 4) + '...' + pubkey.slice(-4);
  document.getElementById('tierCard').classList.add('connected');

  // Show loading state
  document.getElementById('tierPoints').textContent = '...';
  document.getElementById('tierName').textContent = 'Loading...';

  try {
    // Fetch on-chain activity
    userActivity = await fetchOnChainActivity(pubkey);
    
    // Fetch Tapestry social data
    const social = await fetchTapestryData(pubkey);
    if (social) {
      userActivity.hasProfile = true;
      userActivity.followCount = social.following || 0;
    }

    // Calculate points
    userPoints = calculatePoints(userActivity);

    // Update UI
    updateTierDisplay(userPoints);
    updateActivityBreakdown(userActivity);
    highlightUserInLeaderboard(pubkey, userPoints);
    renderActivityFeed(userActivity.transactions);

    // Show activity section
    const actSection = document.getElementById('activitySection');
    if (actSection) actSection.style.display = 'block';
  } catch (e) {
    console.error('Failed to load activity:', e);
    document.getElementById('tierPoints').textContent = '0';
    document.getElementById('tierName').textContent = 'Bronze';
  }
}

// ── On-Chain Activity Parser ─────────────────────────────────────────

async function fetchOnChainActivity(pubkey) {
  const activity = {
    swapVolumeSol: 0, buyVolumeSol: 0, sellVolumeSol: 0,
    lpAddedSol: 0, tokensStaked: 0, stakeDays: 0,
    tokensLaunched: 0, daoVotes: 0, hasProfile: false,
    followCount: 0, transactions: [],
  };

  try {
    // Use web3.js from CDN (loaded in HTML)
    const conn = new solanaWeb3.Connection(RPC_DEVNET, 'confirmed');
    const pk = new solanaWeb3.PublicKey(pubkey);
    const programPk = new solanaWeb3.PublicKey(SENDIT_PROGRAM);

    const sigs = await conn.getSignaturesForAddress(pk, { limit: 100 });

    for (const sig of sigs) {
      try {
        const tx = await conn.getTransaction(sig.signature, {
          maxSupportedTransactionVersion: 0,
        });
        if (!tx?.meta || tx.meta.err) continue;

        const keys = tx.transaction.message.staticAccountKeys ||
                     tx.transaction.message.accountKeys;
        if (!keys.some(k => k.toBase58() === SENDIT_PROGRAM)) continue;

        const instructions = tx.transaction.message.compiledInstructions ||
                            tx.transaction.message.instructions;

        for (const ix of instructions) {
          if (keys[ix.programIdIndex]?.toBase58() !== SENDIT_PROGRAM) continue;

          const data = ix.data instanceof Uint8Array ? ix.data : 
            Uint8Array.from(atob(ix.data), c => c.charCodeAt(0));
          const discHex = Array.from(data.slice(0, 8))
            .map(b => b.toString(16).padStart(2, '0')).join('');
          const action = DISC[discHex] || 'unknown';

          // Calculate SOL change
          const walletIdx = keys.findIndex(k => k.toBase58() === pubkey);
          const solChange = walletIdx >= 0
            ? Math.abs((tx.meta.postBalances[walletIdx] - tx.meta.preBalances[walletIdx]) / 1e9)
            : 0;

          switch (action) {
            case 'buy':           activity.buyVolumeSol += solChange; break;
            case 'sell':          activity.sellVolumeSol += solChange; break;
            case 'swap':          activity.swapVolumeSol += solChange; break;
            case 'add_liquidity': activity.lpAddedSol += solChange; break;
            case 'stake':         activity.tokensStaked += 1000000; break; // estimate
            case 'create_token':  activity.tokensLaunched += 1; break;
          }

          activity.transactions.push({
            signature: sig.signature,
            action,
            solChange,
            time: tx.blockTime,
          });
        }
      } catch (e) { /* skip */ }
    }
  } catch (e) {
    console.error('On-chain parse error:', e);
  }

  return activity;
}

// ── Tapestry Social Data ─────────────────────────────────────────────

async function fetchTapestryData(pubkey) {
  try {
    const resp = await fetch(
      `${TAPESTRY_API}/profiles/${pubkey}?apiKey=${TAPESTRY_APP}`
    );
    if (!resp.ok) return null;
    const data = await resp.json();
    return {
      username: data.profile?.username,
      following: data.socialCounts?.following || 0,
      followers: data.socialCounts?.followers || 0,
    };
  } catch {
    return null;
  }
}

// ── Points Calculation ───────────────────────────────────────────────

function calculatePoints(a) {
  let pts = 0;
  pts += Math.floor((a.swapVolumeSol || 0) / 0.1) * POINTS.swap;
  pts += Math.floor((a.buyVolumeSol || 0) / 0.1) * POINTS.buy;
  pts += Math.floor((a.sellVolumeSol || 0) / 0.1) * POINTS.sell;
  pts += Math.floor((a.lpAddedSol || 0) / 0.1) * POINTS.add_liq;
  pts += Math.floor((a.tokensStaked || 0) / 1e6) * POINTS.stake;
  pts += (a.stakeDays || 0) * POINTS.stake_day;
  pts += (a.tokensLaunched || 0) * POINTS.launch;
  if (a.hasProfile) pts += POINTS.profile;
  pts += (a.followCount || 0) * POINTS.follow;
  pts += (a.daoVotes || 0) * POINTS.vote;

  // Apply tier multiplier
  const tier = getTier(pts);
  return Math.floor(pts * tier.mult);
}

function getTier(pts) {
  let t = TIERS[0];
  for (const tier of TIERS) {
    if (pts >= tier.min) t = tier;
  }
  return t;
}

function getNextTier(pts) {
  for (const t of TIERS) {
    if (pts < t.min) return t;
  }
  return null;
}

// ── UI Updates ───────────────────────────────────────────────────────

function updateTierDisplay(points) {
  const tier = getTier(points);
  const next = getNextTier(points);

  document.getElementById('tierEmoji').textContent = tier.emoji;
  document.getElementById('tierName').textContent = tier.name;
  document.getElementById('tierName').style.color = tier.color;
  document.getElementById('tierPoints').textContent = points.toLocaleString();
  document.getElementById('tierMultiplier').textContent = `${tier.mult}x multiplier`;
  document.getElementById('tierCard').style.borderColor = tier.color;

  if (next) {
    const remaining = next.min - points;
    document.getElementById('tierNext').textContent = 
      `${remaining.toLocaleString()} points to ${next.name} ${next.emoji}`;
    const range = next.min - tier.min;
    const progress = ((points - tier.min) / range) * 100;
    document.getElementById('tierFill').style.width = `${Math.min(100, progress)}%`;
  } else {
    document.getElementById('tierNext').textContent = 'Max tier reached! 🎉';
    document.getElementById('tierFill').style.width = '100%';
  }

  // Highlight active tier badge
  document.querySelectorAll('.tier-badge').forEach((badge, i) => {
    const tierIdx = TIERS.findIndex(t => t.name === tier.name);
    badge.classList.toggle('active', i <= tierIdx);
  });

  // Glow effect color
  const glow = document.querySelector('.tier-glow');
  if (glow) {
    glow.style.background = `radial-gradient(circle, ${tier.color}15, transparent 60%)`;
  }
}

function updateActivityBreakdown(activity) {
  // Update stats cards with real data
  const totalVol = (activity.buyVolumeSol + activity.sellVolumeSol + activity.swapVolumeSol);
  const txCount = activity.transactions.length;

  // Update the breakdown section if it exists
  const breakdownEl = document.getElementById('activityBreakdown');
  if (breakdownEl) {
    breakdownEl.innerHTML = `
      <div class="stat-card"><div class="stat-value">${totalVol.toFixed(2)} SOL</div><div class="stat-label">Total Volume</div></div>
      <div class="stat-card"><div class="stat-value">${txCount}</div><div class="stat-label">Transactions</div></div>
      <div class="stat-card"><div class="stat-value">${activity.tokensLaunched}</div><div class="stat-label">Tokens Launched</div></div>
      <div class="stat-card"><div class="stat-value">${(activity.tokensStaked / 1e6).toFixed(1)}M</div><div class="stat-label">Tokens Staked</div></div>
    `;
  }
}

function highlightUserInLeaderboard(pubkey, points) {
  const short = pubkey.slice(0, 4) + '...' + pubkey.slice(-4);
  const tier = getTier(points);

  // Check if user is already in leaderboard
  const rows = document.querySelectorAll('.lb-row');
  let found = false;
  rows.forEach(row => {
    const walletEl = row.querySelector('.lb-wallet');
    if (walletEl && walletEl.textContent.trim() === short) {
      row.classList.add('me');
      found = true;
    }
  });

  // If not in leaderboard, add at bottom
  if (!found && points > 0) {
    const lb = document.querySelector('.leaderboard');
    if (lb) {
      const row = document.createElement('div');
      row.className = 'lb-row me';
      row.innerHTML = `
        <div class="lb-rank">—</div>
        <div class="lb-wallet">${short}</div>
        <div class="lb-tier">${tier.emoji} ${tier.name}</div>
        <div class="lb-points">${points.toLocaleString()}</div>
      `;
      lb.appendChild(row);
    }
  }
}

// ── Recent Activity Feed ─────────────────────────────────────────────

function renderActivityFeed(transactions) {
  const feed = document.getElementById('activityFeed');
  if (!feed || !transactions.length) return;

  const actionEmoji = {
    buy: '🚀', sell: '📉', swap: '💱', add_liquidity: '💧',
    stake: '🔒', unstake: '🔓', create_token: '🎨', create_pool: '🏊',
  };

  feed.innerHTML = transactions.slice(0, 10).map(tx => {
    const emoji = actionEmoji[tx.action] || '📦';
    const time = tx.time ? new Date(tx.time * 1000).toLocaleString() : 'Unknown';
    const sol = tx.solChange > 0 ? `${tx.solChange.toFixed(4)} SOL` : '';
    const sig = tx.signature.slice(0, 8) + '...';
    return `
      <div class="activity-item">
        <span class="activity-emoji">${emoji}</span>
        <span class="activity-action">${tx.action.replace('_', ' ')}</span>
        <span class="activity-sol">${sol}</span>
        <a class="activity-sig" href="https://explorer.solana.com/tx/${tx.signature}?cluster=devnet" target="_blank">${sig}</a>
        <span class="activity-time">${time}</span>
      </div>
    `;
  }).join('');
}

// Expose to global scope for onclick handlers
window.connectWallet = connectWallet;
