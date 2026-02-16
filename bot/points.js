/**
 * Send.it Points System
 * Persistent via Upstash Redis (free tier)
 *
 * Points:
 *   /checkin     → 5 pts (once per 20h)
 *   invite       → 25 pts (auto on new member join)
 *   first msg    → 2 pts (once per day)
 *   mod award    → 15 pts (/award command)
 *   bug report   → 50 pts (/bugreport, mod confirms)
 */

const { Redis } = require("@upstash/redis");

let redis = null;

function getRedis() {
  if (!redis) {
    const url = process.env.UPSTASH_REDIS_REST_URL;
    const token = process.env.UPSTASH_REDIS_REST_TOKEN;
    if (!url || !token) {
      console.error("Points: Missing UPSTASH_REDIS_REST_URL or UPSTASH_REDIS_REST_TOKEN");
      return null;
    }
    redis = new Redis({ url, token });
  }
  return redis;
}

// Key helpers
const userKey = (userId) => `points:${userId}`;
const checkinKey = (userId) => `checkin:${userId}`;
const dailyMsgKey = (userId, date) => `dailymsg:${userId}:${date}`;
const nameKey = (userId) => `name:${userId}`;
const leaderboardKey = () => "leaderboard";

function todayStr() {
  return new Date().toISOString().slice(0, 10);
}

/**
 * Get user's points
 */
async function getPoints(userId) {
  const r = getRedis();
  if (!r) return 0;
  const pts = await r.get(userKey(userId));
  return parseInt(pts) || 0;
}

/**
 * Add points to user and update leaderboard
 */
async function addPoints(userId, amount, name) {
  const r = getRedis();
  if (!r) return 0;
  const newTotal = await r.incrby(userKey(userId), amount);
  // Update name cache
  if (name) await r.set(nameKey(userId), name);
  // Update sorted set leaderboard
  await r.zadd(leaderboardKey(), { score: newTotal, member: String(userId) });
  return newTotal;
}

/**
 * Daily check-in — 5 pts, once per 20 hours
 */
async function checkin(userId, name) {
  const r = getRedis();
  if (!r) return { ok: false, reason: "Redis not configured" };

  const key = checkinKey(userId);
  const last = await r.get(key);

  if (last) {
    const elapsed = Date.now() - parseInt(last);
    const hoursLeft = Math.ceil((20 * 3600000 - elapsed) / 3600000);
    if (elapsed < 20 * 3600000) {
      return { ok: false, reason: `already_checked_in`, hoursLeft };
    }
  }

  await r.set(key, String(Date.now()));
  const total = await addPoints(userId, 5, name);
  return { ok: true, earned: 5, total };
}

/**
 * First message of the day — 2 pts, once per calendar day
 */
async function firstMessage(userId, name) {
  const r = getRedis();
  if (!r) return null;

  const key = dailyMsgKey(userId, todayStr());
  const already = await r.get(key);
  if (already) return null;

  await r.set(key, "1", { ex: 86400 }); // expires in 24h
  const total = await addPoints(userId, 2, name);
  return { earned: 2, total };
}

/**
 * Invite reward — 25 pts
 */
async function inviteReward(userId, name) {
  const total = await addPoints(userId, 25, name);
  return { earned: 25, total };
}

/**
 * Mod award — 15 pts
 */
async function modAward(userId, name) {
  const total = await addPoints(userId, 15, name);
  return { earned: 15, total };
}

/**
 * Bug report — 50 pts
 */
async function bugReward(userId, name) {
  const total = await addPoints(userId, 50, name);
  return { earned: 50, total };
}

/**
 * Get top N from leaderboard
 */
async function getLeaderboard(limit = 10) {
  const r = getRedis();
  if (!r) return [];

  // Get top members with scores (highest first)
  const results = await r.zrange(leaderboardKey(), 0, limit - 1, { rev: true, withScores: true });

  // results is [member, score, member, score, ...]
  const entries = [];
  for (let i = 0; i < results.length; i += 2) {
    const userId = results[i];
    const score = results[i + 1];
    const name = await r.get(nameKey(userId)) || "Unknown";
    entries.push({ userId, name, points: parseInt(score) });
  }
  return entries;
}

module.exports = {
  getPoints,
  addPoints,
  checkin,
  firstMessage,
  inviteReward,
  modAward,
  bugReward,
  getLeaderboard
};
