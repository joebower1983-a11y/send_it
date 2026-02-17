/**
 * Send.it × Torque Loyalty Integration
 * 
 * Bridges Send.it on-chain activity with Torque's incentive protocol.
 * Tracks trading volume, LP provision, staking, and social engagement
 * to distribute rewards through Torque campaigns.
 */

// Torque API base
const TORQUE_API = 'https://api.torque.so';
const TORQUE_APP = 'https://app.torque.so';

// Send.it program
const SENDIT_PROGRAM = 'HTKq18cATdwCZb6XM66Mhn8JWKCFTrZqH6zU1zip88Zx';
const SENDIT_MINT = 'F8qWTN8JfyDCvj4RoCHuvNMVbTV9XQksLuziA8PYpump';

/**
 * Loyalty tier thresholds based on on-chain activity
 */
export const LOYALTY_TIERS = [
  { name: 'Bronze',   emoji: '🥉', minPoints: 0,      multiplier: 1.0, color: '#cd7f32' },
  { name: 'Silver',   emoji: '🥈', minPoints: 1000,   multiplier: 1.25, color: '#c0c0c0' },
  { name: 'Gold',     emoji: '🥇', minPoints: 5000,   multiplier: 1.5, color: '#ffd700' },
  { name: 'Platinum', emoji: '💎', minPoints: 25000,  multiplier: 2.0, color: '#e5e4e2' },
  { name: 'Diamond',  emoji: '👑', minPoints: 100000, multiplier: 3.0, color: '#b9f2ff' },
];

/**
 * Point values for different on-chain actions
 */
export const POINT_VALUES = {
  // Trading
  swap_sol:          10,   // per 0.1 SOL swapped
  buy_bonding:       15,   // per 0.1 SOL on bonding curve
  sell_bonding:       5,   // per 0.1 SOL sold

  // Liquidity
  add_liquidity:     50,   // per 0.1 SOL added as LP
  remove_liquidity: -20,   // penalty for removing early

  // Staking
  stake_tokens:      25,   // per 1M tokens staked
  stake_duration:     1,   // per day staked (bonus)

  // Social (via Tapestry)
  create_profile:   100,   // one-time
  follow_user:        5,
  post_content:      10,

  // Token creation
  launch_token:     500,   // one-time per token launched

  // Referral
  referral_signup:  200,   // per referred user who trades
  referral_volume:   5,    // per 0.1 SOL of referee's volume
};

/**
 * Campaign templates for Torque integration
 */
export const CAMPAIGN_TEMPLATES = {
  weeklyTrading: {
    name: 'Weekly Trading Leaderboard',
    description: 'Top 10 traders by volume compete for SENDIT rewards',
    type: 'leaderboard',
    duration: 7 * 24 * 3600,
    rewards: {
      '1st': '500,000 SENDIT',
      '2nd': '250,000 SENDIT',
      '3rd': '100,000 SENDIT',
      '4-10': '25,000 SENDIT each',
    },
    criteria: 'swap_volume_sol',
    minActivity: 0.1, // minimum 0.1 SOL traded
  },
  lpRebate: {
    name: 'LP Deposit Rebate',
    description: '10% bonus SENDIT for new liquidity providers',
    type: 'rebate',
    duration: 30 * 24 * 3600,
    rebatePercent: 10,
    maxPerWallet: '1,000,000 SENDIT',
    criteria: 'add_liquidity',
  },
  stakingBonus: {
    name: 'Diamond Hands Staking',
    description: 'Stake for 30+ days and earn 2x point multiplier',
    type: 'bonus',
    duration: 30 * 24 * 3600,
    multiplier: 2,
    criteria: 'stake_duration_days >= 30',
  },
  socialRaffle: {
    name: 'Community Raffle',
    description: 'Create a Tapestry profile + follow 3 users to enter weekly raffle',
    type: 'raffle',
    duration: 7 * 24 * 3600,
    winners: 10,
    reward: '100,000 SENDIT each',
    criteria: 'has_profile AND following_count >= 3',
  },
};

/**
 * Get user's loyalty tier based on points
 */
export function getUserTier(points) {
  let tier = LOYALTY_TIERS[0];
  for (const t of LOYALTY_TIERS) {
    if (points >= t.minPoints) tier = t;
  }
  return tier;
}

/**
 * Calculate points from on-chain activity
 */
export function calculatePoints(activity) {
  let points = 0;

  // Trading volume (in lamports → SOL → 0.1 SOL units)
  if (activity.swapVolumeSol) {
    points += Math.floor(activity.swapVolumeSol / 0.1) * POINT_VALUES.swap_sol;
  }
  if (activity.buyVolumeSol) {
    points += Math.floor(activity.buyVolumeSol / 0.1) * POINT_VALUES.buy_bonding;
  }
  if (activity.sellVolumeSol) {
    points += Math.floor(activity.sellVolumeSol / 0.1) * POINT_VALUES.sell_bonding;
  }

  // Liquidity
  if (activity.lpAddedSol) {
    points += Math.floor(activity.lpAddedSol / 0.1) * POINT_VALUES.add_liquidity;
  }

  // Staking
  if (activity.tokensStaked) {
    points += Math.floor(activity.tokensStaked / 1_000_000) * POINT_VALUES.stake_tokens;
  }
  if (activity.stakeDays) {
    points += activity.stakeDays * POINT_VALUES.stake_duration;
  }

  // Social
  if (activity.hasProfile) points += POINT_VALUES.create_profile;
  if (activity.followCount) points += activity.followCount * POINT_VALUES.follow_user;
  if (activity.postCount) points += activity.postCount * POINT_VALUES.post_content;

  // Token launches
  if (activity.tokensLaunched) {
    points += activity.tokensLaunched * POINT_VALUES.launch_token;
  }

  // Referrals
  if (activity.referralSignups) {
    points += activity.referralSignups * POINT_VALUES.referral_signup;
  }
  if (activity.referralVolumeSol) {
    points += Math.floor(activity.referralVolumeSol / 0.1) * POINT_VALUES.referral_volume;
  }

  return points;
}

/**
 * Format leaderboard from array of {wallet, points} objects
 */
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

/**
 * Torque SDK wrapper — initializes user tracking for Send.it campaigns
 * Requires @torque-labs/torque-ts-sdk in browser context
 */
export async function initTorqueUser(walletAdapter, apiKey) {
  // Dynamic import for browser usage
  const { TorqueSDK } = await import('@torque-labs/torque-ts-sdk');
  
  const sdk = new TorqueSDK({
    apiKey,
    publisherHandle: 'sendit',
    network: 'devnet',
  });

  await sdk.initialize(walletAdapter);
  return sdk;
}

/**
 * Get active Torque campaigns for Send.it
 */
export async function getActiveCampaigns(sdk) {
  if (!sdk?.user) return [];
  
  try {
    const campaigns = await sdk.user.getCampaigns();
    return campaigns.filter(c => c.status === 'ACTIVE');
  } catch (err) {
    console.error('Failed to fetch Torque campaigns:', err);
    return [];
  }
}

/**
 * Check user's progress on a Torque campaign
 */
export async function getCampaignProgress(sdk, campaignId) {
  if (!sdk?.user) return null;
  
  try {
    return await sdk.user.getCampaignProgress(campaignId);
  } catch (err) {
    console.error('Failed to fetch campaign progress:', err);
    return null;
  }
}
