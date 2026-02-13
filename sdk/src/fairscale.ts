// ═══════════════════════════════════════════════
//  FairScale Client SDK for Send.it
//  Fetches and caches reputation scores from FairScale API
// ═══════════════════════════════════════════════

const FAIRSCALE_BASE_URL = "https://api.fairscale.xyz";
const CACHE_TTL_MS = 60 * 60 * 1000; // 1 hour

export type ReputationTier = "bronze" | "silver" | "gold" | "platinum";

export interface FairScoreResponse {
  fairscore: number;       // 0-100
  tier: ReputationTier;
  badges: string[];
  social_score: number;
  features: {
    tx_count: number;
    active_days: number;
    wallet_age_days: number;
    [key: string]: number;
  };
}

interface CacheEntry {
  data: FairScoreResponse;
  timestamp: number;
}

const scoreCache = new Map<string, CacheEntry>();

function getCached(wallet: string): FairScoreResponse | null {
  const entry = scoreCache.get(wallet);
  if (entry && Date.now() - entry.timestamp < CACHE_TTL_MS) {
    return entry.data;
  }
  if (entry) scoreCache.delete(wallet);
  return null;
}

function setCache(wallet: string, data: FairScoreResponse): void {
  scoreCache.set(wallet, { data, timestamp: Date.now() });
}

/**
 * Fetch FairScore for a wallet address from the FairScale API.
 * Results are cached for 1 hour.
 */
export async function fetchFairScore(
  wallet: string,
  apiKey: string
): Promise<FairScoreResponse> {
  const cached = getCached(wallet);
  if (cached) return cached;

  const res = await fetch(
    `${FAIRSCALE_BASE_URL}/score?wallet=${encodeURIComponent(wallet)}`,
    { headers: { fairkey: apiKey } }
  );

  if (!res.ok) {
    throw new Error(`FairScale API error: ${res.status} ${res.statusText}`);
  }

  const data: FairScoreResponse = await res.json();
  setCache(wallet, data);
  return data;
}

/**
 * Get the reputation tier for a wallet.
 */
export async function getTier(
  wallet: string,
  apiKey: string
): Promise<ReputationTier> {
  const score = await fetchFairScore(wallet, apiKey);
  return score.tier;
}

/**
 * Check if a wallet meets the minimum score to launch a token.
 */
export async function checkLaunchEligibility(
  wallet: string,
  apiKey: string,
  minScore: number = 30
): Promise<boolean> {
  const score = await fetchFairScore(wallet, apiKey);
  return score.fairscore >= minScore;
}

/**
 * Get fee discount percentage based on reputation tier.
 */
export function getFeeDiscount(tier: ReputationTier): number {
  const discounts: Record<ReputationTier, number> = {
    bronze: 0,
    silver: 5,
    gold: 10,
    platinum: 20,
  };
  return discounts[tier] ?? 0;
}

/**
 * Get trust level label for UI display.
 */
export function getTrustLevel(
  fairscore: number | null
): { label: string; emoji: string; color: string } {
  if (fairscore === null || fairscore === undefined) {
    return { label: "Unscored", emoji: "⚫", color: "#555570" };
  }
  if (fairscore >= 70) {
    return { label: "High Trust", emoji: "🟢", color: "#00ff88" };
  }
  if (fairscore >= 40) {
    return { label: "Medium", emoji: "🟡", color: "#ffcc00" };
  }
  return { label: "Low", emoji: "🔴", color: "#ff2d78" };
}

/**
 * Get tier badge color for UI.
 */
export function getTierColor(tier: ReputationTier): string {
  const colors: Record<ReputationTier, string> = {
    bronze: "#cd7f32",
    silver: "#c0c0c0",
    gold: "#ffd700",
    platinum: "#e5e4e2",
  };
  return colors[tier] ?? "#555570";
}

/**
 * Clear the score cache (useful for forcing refresh).
 */
export function clearCache(): void {
  scoreCache.clear();
}

/**
 * Invalidate cache for a specific wallet.
 */
export function invalidateCache(wallet: string): void {
  scoreCache.delete(wallet);
}
