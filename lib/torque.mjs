/**
 * Send.it × Torque Loyalty Integration
 * 
 * Full integration with Torque SDK for campaign management,
 * user tracking, and reward distribution. Also provides a
 * standalone loyalty engine that works via on-chain event parsing
 * when the Torque API is unavailable.
 * 
 * @module torque
 */

import { Connection, PublicKey, Keypair } from '@solana/web3.js';
import bs58 from 'bs58';
import nacl from 'tweetnacl';

// ── Constants ────────────────────────────────────────────────────────
const TORQUE_API      = 'https://api.torque.so';
const TORQUE_APP      = 'https://app.torque.so';
const SENDIT_PROGRAM  = 'HTKq18cATdwCZb6XM66Mhn8JWKCFTrZqH6zU1zip88Zx';
const SENDIT_MINT     = 'F8qWTN8JfyDCvj4RoCHuvNMVbTV9XQksLuziA8PYpump';
const RPC_DEVNET      = 'https://api.devnet.solana.com';

// ── Loyalty Tiers ────────────────────────────────────────────────────
export const LOYALTY_TIERS = [
  { name: 'Bronze',   emoji: '🥉', minPoints: 0,      multiplier: 1.0,  color: '#cd7f32' },
  { name: 'Silver',   emoji: '🥈', minPoints: 1000,   multiplier: 1.25, color: '#c0c0c0' },
  { name: 'Gold',     emoji: '🥇', minPoints: 5000,   multiplier: 1.5,  color: '#ffd700' },
  { name: 'Platinum', emoji: '💎', minPoints: 25000,  multiplier: 2.0,  color: '#e5e4e2' },
  { name: 'Diamond',  emoji: '👑', minPoints: 100000, multiplier: 3.0,  color: '#b9f2ff' },
];

// ── Point Values ─────────────────────────────────────────────────────
export const POINT_VALUES = {
  swap_sol:          10,   // per 0.1 SOL swapped
  buy_bonding:       15,   // per 0.1 SOL on bonding curve
  sell_bonding:       5,   // per 0.1 SOL sold
  add_liquidity:     50,   // per 0.1 SOL added as LP
  remove_liquidity: -20,   // penalty for early removal
  stake_tokens:      25,   // per 1M tokens staked
  stake_duration:     1,   // per day staked (bonus)
  create_profile:   100,   // one-time Tapestry profile
  follow_user:        5,
  post_content:      10,
  launch_token:     500,   // one-time per token launched
  referral_signup:  200,
  referral_volume:    5,   // per 0.1 SOL of referee volume
  dao_vote:          50,   // per governance vote cast
  dao_proposal:     200,   // per proposal created
};

// ── Anchor Discriminators for Send.it program ────────────────────────
// sha256("global:<fn_name>")[0..8]
const DISCRIMINATORS = {
  buy:              'f223c68952e1f2b6',
  sell:             'd33e5e5e3edcab90',
  swap:             'f8c69e91e17587c8',
  stake:            '26d4c1727e411b7c',
  unstake:          '90b2627574652e4e',
  create_token:     '848c70d3ee3c6526',
  create_pool:      'e992d18ecf6840bc',
  add_liquidity:    '7a5ffee3dd0b53e5',
  remove_liquidity: '5055d14818ceb016',
};

// ── Campaign Templates ───────────────────────────────────────────────
export const CAMPAIGN_TEMPLATES = {
  weeklyTrading: {
    campaignName: 'Send.it Weekly Trading Leaderboard',
    campaignType: 'BOUNTY',
    campaignDescription: 'Top traders by swap volume earn SENDIT rewards. Tracked by Torque.',
    landingPage: 'https://senditsolana.io/app/trading.html',
    eventConfig: [{
      type: 'SWAP',
      requirement: {
        inToken: null,
        outToken: SENDIT_MINT,
        usdcValue: 1, // minimum $1 swap
      },
    }],
    conversionCount: 100,
    userRewardType: 'POINTS',
    userPayoutPerConversion: 10,
  },
  lpDeposit: {
    campaignName: 'Send.it LP Deposit Rebate',
    campaignType: 'BOUNTY',
    campaignDescription: 'Add liquidity to any Send.it pool and earn 10% bonus in points.',
    landingPage: 'https://senditsolana.io/app/trading.html',
    eventConfig: [{
      type: 'CUSTOM_EVENT',
      requirement: {
        eventName: 'sendit_add_liquidity',
        formEnabled: false,
        fields: [
          { name: 'sol_amount', type: 'number', validation: { min: 0.01 } },
          { name: 'pool', type: 'string', validation: {} },
        ],
      },
    }],
    conversionCount: 500,
    userRewardType: 'POINTS',
    userPayoutPerConversion: 50,
  },
  stakingBonus: {
    campaignName: 'Send.it Diamond Hands Staking',
    campaignType: 'BOUNTY',
    campaignDescription: 'Stake SENDIT for 30+ days to earn 2x point multiplier.',
    landingPage: 'https://senditsolana.io/app/staking.html',
    eventConfig: [{
      type: 'CUSTOM_EVENT',
      requirement: {
        eventName: 'sendit_stake',
        formEnabled: false,
        fields: [
          { name: 'amount', type: 'number', validation: { min: 1000000 } },
          { name: 'duration_days', type: 'number', validation: { min: 30 } },
        ],
      },
    }],
    conversionCount: 200,
    userRewardType: 'POINTS',
    userPayoutPerConversion: 25,
  },
  daoVote: {
    campaignName: 'Send.it Governance Voter',
    campaignType: 'BOUNTY',
    campaignDescription: 'Vote on Send.it DAO proposals via Realms to earn loyalty points.',
    landingPage: 'https://senditsolana.io/app/governance.html',
    eventConfig: [{
      type: 'REALMS_VOTE',
      requirement: {
        daoPubKey: 'BCa4t2Z43MhW98DznzMKTqQy9ibtCLdJ2TEnbBNqirZ6',
        proposalPubKey: '', // any proposal
      },
    }],
    conversionCount: 1000,
    userRewardType: 'POINTS',
    userPayoutPerConversion: 50,
  },
  socialRaffle: {
    campaignName: 'Send.it Community Raffle',
    campaignType: 'BOUNTY',
    campaignDescription: 'Create a Tapestry profile + follow 3 users to enter weekly raffle.',
    landingPage: 'https://senditsolana.io/app/social.html',
    eventConfig: [{
      type: 'CLICK',
      requirement: {
        targetUrl: 'https://senditsolana.io/app/social.html',
        requireSignature: true,
      },
    }],
    conversionCount: 50,
    userRewardType: 'POINTS',
    userPayoutPerConversion: 100,
  },
};

// ── Tier Helpers ─────────────────────────────────────────────────────

export function getUserTier(points) {
  let tier = LOYALTY_TIERS[0];
  for (const t of LOYALTY_TIERS) {
    if (points >= t.minPoints) tier = t;
  }
  return tier;
}

export function getNextTier(points) {
  for (const t of LOYALTY_TIERS) {
    if (points < t.minPoints) return t;
  }
  return null; // already max tier
}

export function getTierProgress(points) {
  const current = getUserTier(points);
  const next = getNextTier(points);
  if (!next) return { current, next: null, progress: 100 };
  const range = next.minPoints - current.minPoints;
  const progress = Math.min(100, ((points - current.minPoints) / range) * 100);
  return { current, next, progress };
}

// ── Points Calculator ────────────────────────────────────────────────

export function calculatePoints(activity) {
  let points = 0;
  if (activity.swapVolumeSol)    points += Math.floor(activity.swapVolumeSol / 0.1) * POINT_VALUES.swap_sol;
  if (activity.buyVolumeSol)     points += Math.floor(activity.buyVolumeSol / 0.1) * POINT_VALUES.buy_bonding;
  if (activity.sellVolumeSol)    points += Math.floor(activity.sellVolumeSol / 0.1) * POINT_VALUES.sell_bonding;
  if (activity.lpAddedSol)       points += Math.floor(activity.lpAddedSol / 0.1) * POINT_VALUES.add_liquidity;
  if (activity.tokensStaked)     points += Math.floor(activity.tokensStaked / 1_000_000) * POINT_VALUES.stake_tokens;
  if (activity.stakeDays)        points += activity.stakeDays * POINT_VALUES.stake_duration;
  if (activity.hasProfile)       points += POINT_VALUES.create_profile;
  if (activity.followCount)      points += activity.followCount * POINT_VALUES.follow_user;
  if (activity.postCount)        points += activity.postCount * POINT_VALUES.post_content;
  if (activity.tokensLaunched)   points += activity.tokensLaunched * POINT_VALUES.launch_token;
  if (activity.referralSignups)  points += activity.referralSignups * POINT_VALUES.referral_signup;
  if (activity.referralVolumeSol) points += Math.floor(activity.referralVolumeSol / 0.1) * POINT_VALUES.referral_volume;
  if (activity.daoVotes)         points += activity.daoVotes * POINT_VALUES.dao_vote;
  if (activity.daoProposals)     points += activity.daoProposals * POINT_VALUES.dao_proposal;
  
  // Apply tier multiplier
  const tier = getUserTier(points);
  return Math.floor(points * tier.multiplier);
}

// ── On-Chain Event Parser ────────────────────────────────────────────

/**
 * Parse Send.it program transactions to extract activity for a wallet.
 * Works independently of Torque — reads directly from Solana.
 */
export async function parseOnChainActivity(walletPubkey, rpcUrl = RPC_DEVNET) {
  const connection = new Connection(rpcUrl, 'confirmed');
  const pubkey = new PublicKey(walletPubkey);
  const programId = new PublicKey(SENDIT_PROGRAM);

  const activity = {
    swapVolumeSol: 0,
    buyVolumeSol: 0,
    sellVolumeSol: 0,
    lpAddedSol: 0,
    tokensStaked: 0,
    stakeDays: 0,
    tokensLaunched: 0,
    daoVotes: 0,
    transactions: [],
  };

  try {
    // Fetch recent signatures for the wallet
    const sigs = await connection.getSignaturesForAddress(pubkey, { limit: 100 });
    
    for (const sig of sigs) {
      try {
        const tx = await connection.getTransaction(sig.signature, {
          maxSupportedTransactionVersion: 0,
        });
        if (!tx?.meta || tx.meta.err) continue;

        // Check if this tx interacts with our program
        const accountKeys = tx.transaction.message.staticAccountKeys || 
                           tx.transaction.message.accountKeys;
        const hasProgram = accountKeys.some(k => k.toBase58() === SENDIT_PROGRAM);
        if (!hasProgram) continue;

        // Parse instruction data to identify action type
        const instructions = tx.transaction.message.compiledInstructions || 
                            tx.transaction.message.instructions;
        
        for (const ix of instructions) {
          const programIdx = ix.programIdIndex;
          if (accountKeys[programIdx]?.toBase58() !== SENDIT_PROGRAM) continue;

          const data = ix.data instanceof Uint8Array ? ix.data : Buffer.from(ix.data, 'base64');
          const disc = Buffer.from(data.slice(0, 8)).toString('hex');

          // Calculate SOL change for this tx
          const preBalances = tx.meta.preBalances;
          const postBalances = tx.meta.postBalances;
          const walletIdx = accountKeys.findIndex(k => k.toBase58() === walletPubkey);
          const solChange = walletIdx >= 0 
            ? (postBalances[walletIdx] - preBalances[walletIdx]) / 1e9 
            : 0;

          const record = {
            signature: sig.signature,
            blockTime: tx.blockTime,
            solChange,
          };

          switch (disc) {
            case DISCRIMINATORS.buy:
              record.action = 'buy';
              activity.buyVolumeSol += Math.abs(solChange);
              break;
            case DISCRIMINATORS.sell:
              record.action = 'sell';
              activity.sellVolumeSol += Math.abs(solChange);
              break;
            case DISCRIMINATORS.swap:
              record.action = 'swap';
              activity.swapVolumeSol += Math.abs(solChange);
              break;
            case DISCRIMINATORS.add_liquidity:
              record.action = 'add_liquidity';
              activity.lpAddedSol += Math.abs(solChange);
              break;
            case DISCRIMINATORS.stake:
              record.action = 'stake';
              // Token amount is in instruction data after discriminator
              if (data.length >= 16) {
                const amount = Number(data.readBigUInt64LE(8));
                activity.tokensStaked += amount / 1e6; // assuming 6 decimals
              }
              break;
            case DISCRIMINATORS.create_token:
              record.action = 'create_token';
              activity.tokensLaunched += 1;
              break;
            case DISCRIMINATORS.create_pool:
              record.action = 'create_pool';
              break;
            default:
              record.action = 'unknown';
          }

          activity.transactions.push(record);
        }
      } catch (e) {
        // Skip failed tx parses
      }
    }
  } catch (e) {
    console.error('Failed to parse on-chain activity:', e.message);
  }

  return activity;
}

// ── Torque SDK Wrapper (Server-Side with Keypair) ────────────────────

/**
 * Initialize Torque SDK with a Keypair (for server-side/CLI usage).
 * Handles SIWS auth flow programmatically.
 */
export async function initTorqueWithKeypair(keypairPath, apiKey, options = {}) {
  const { TorqueSDK } = await import('@torque-labs/torque-ts-sdk');
  const fs = await import('fs');

  const keypairData = JSON.parse(fs.readFileSync(keypairPath, 'utf8'));
  const keypair = Keypair.fromSecretKey(Uint8Array.from(keypairData));

  const sdk = new TorqueSDK({
    apiKey,
    publisherHandle: options.publisherHandle || 'sendit',
    rpc: options.rpc || RPC_DEVNET,
    network: options.network || 'devnet',
  });

  // Create a minimal adapter-like signer for the SDK
  const signer = {
    publicKey: keypair.publicKey,
    signMessage: async (message) => {
      return nacl.sign.detached(message, keypair.secretKey);
    },
    signTransaction: async (tx) => {
      tx.partialSign(keypair);
      return tx;
    },
    signAllTransactions: async (txs) => {
      txs.forEach(tx => tx.partialSign(keypair));
      return txs;
    },
    connected: true,
  };

  await sdk.initialize(signer);
  return { sdk, keypair };
}

/**
 * Create all Send.it campaigns on Torque.
 */
export async function createAllCampaigns(sdk, options = {}) {
  const results = [];
  const now = Math.floor(Date.now() / 1000);
  const oneMonth = 30 * 24 * 3600;

  for (const [key, template] of Object.entries(CAMPAIGN_TEMPLATES)) {
    try {
      const campaignData = {
        ...template,
        startTime: now,
        endTime: now + (options.duration || oneMonth),
      };

      const result = await sdk.api.createCampaign(campaignData);
      results.push({ key, success: true, result });
      console.log(`✅ Created campaign: ${template.campaignName}`);
    } catch (err) {
      results.push({ key, success: false, error: err.message });
      console.error(`❌ Failed to create ${key}: ${err.message}`);
    }
  }

  return results;
}

/**
 * Register custom events with Torque for Send.it on-chain actions.
 */
export async function registerCustomEvents(sdk) {
  const events = [
    {
      name: 'sendit_add_liquidity',
      config: {
        sol_amount: 'number',
        pool: 'string',
        wallet: 'string',
      },
    },
    {
      name: 'sendit_stake',
      config: {
        amount: 'number',
        duration_days: 'number',
        wallet: 'string',
      },
    },
    {
      name: 'sendit_unstake',
      config: {
        amount: 'number',
        wallet: 'string',
      },
    },
    {
      name: 'sendit_launch_token',
      config: {
        token_name: 'string',
        mint: 'string',
        wallet: 'string',
      },
    },
    {
      name: 'sendit_create_pool',
      config: {
        token_mint: 'string',
        initial_sol: 'number',
        wallet: 'string',
      },
    },
  ];

  const results = [];
  for (const event of events) {
    try {
      const result = await sdk.user.createCustomEvent(event);
      results.push({ name: event.name, success: true, id: result.id });
      console.log(`✅ Registered event: ${event.name} (${result.id})`);
    } catch (err) {
      results.push({ name: event.name, success: false, error: err.message });
      console.error(`❌ Failed to register ${event.name}: ${err.message}`);
    }
  }
  return results;
}

// ── Leaderboard ──────────────────────────────────────────────────────

export function formatLeaderboard(entries, limit = 10) {
  return entries
    .sort((a, b) => b.points - a.points)
    .slice(0, limit)
    .map((entry, i) => {
      const tier = getUserTier(entry.points);
      const medals = ['🥇', '🥈', '🥉'];
      const rank = i < 3 ? medals[i] : `#${i + 1}`;
      return {
        rank,
        wallet: entry.wallet,
        walletShort: entry.wallet.slice(0, 4) + '...' + entry.wallet.slice(-4),
        points: entry.points,
        tier,
      };
    });
}

// ── Torque API Health Check ──────────────────────────────────────────

export async function checkTorqueHealth() {
  try {
    const resp = await fetch(`${TORQUE_API}`, { method: 'HEAD', signal: AbortSignal.timeout(5000) });
    return resp.ok || resp.status < 500;
  } catch {
    return false;
  }
}

// ── Export all for use in scripts/UI ─────────────────────────────────
export {
  TORQUE_API,
  TORQUE_APP,
  SENDIT_PROGRAM,
  SENDIT_MINT,
  RPC_DEVNET,
  DISCRIMINATORS,
};
