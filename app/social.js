/**
 * Send.it Social Layer — Tapestry Integration
 * Provides social profiles, follows, content, and feeds for token launches
 */

const TAPESTRY_API = 'https://api.usetapestry.dev/v1';
let TAPESTRY_KEY = ''; // Set via initSocial()

export function initSocial(apiKey) {
  TAPESTRY_KEY = apiKey;
}

// Create or find a profile for a wallet
export async function getOrCreateProfile(walletAddress, username) {
  const res = await fetch(`${TAPESTRY_API}/profiles/findOrCreate?apiKey=${TAPESTRY_KEY}`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      walletAddress,
      username: username || walletAddress.slice(0, 8),
      bio: '',
      blockchain: 'SOLANA',
      execution: 'FAST_UNCONFIRMED'
    })
  });
  return res.json();
}

// Get a profile by wallet
export async function getProfile(walletAddress) {
  const res = await fetch(`${TAPESTRY_API}/profiles/${walletAddress}?apiKey=${TAPESTRY_KEY}`);
  return res.json();
}

// Follow a user (e.g., follow a token creator when you buy)
export async function followUser(followerWallet, targetWallet) {
  const res = await fetch(`${TAPESTRY_API}/follows?apiKey=${TAPESTRY_KEY}`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      startId: followerWallet,
      endId: targetWallet,
      blockchain: 'SOLANA',
      execution: 'FAST_UNCONFIRMED'
    })
  });
  return res.json();
}

// Get followers of a user
export async function getFollowers(walletAddress) {
  const res = await fetch(`${TAPESTRY_API}/followers/${walletAddress}?apiKey=${TAPESTRY_KEY}`);
  return res.json();
}

// Get who a user follows
export async function getFollowing(walletAddress) {
  const res = await fetch(`${TAPESTRY_API}/following/${walletAddress}?apiKey=${TAPESTRY_KEY}`);
  return res.json();
}

// Create a content node (e.g., token launch announcement)
export async function createPost(walletAddress, content, properties = {}) {
  const res = await fetch(`${TAPESTRY_API}/contents?apiKey=${TAPESTRY_KEY}`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      authorId: walletAddress,
      content,
      properties,
      blockchain: 'SOLANA',
      execution: 'FAST_UNCONFIRMED'
    })
  });
  return res.json();
}

// Create a token launch post
export async function postTokenLaunch(creatorWallet, tokenName, tokenSymbol, mintAddress) {
  return createPost(creatorWallet, `🚀 Launched ${tokenName} ($${tokenSymbol}) on Send.it!\n\nMint: ${mintAddress}\nBuy now on the bonding curve!`, {
    type: 'token_launch',
    mint: mintAddress,
    name: tokenName,
    symbol: tokenSymbol
  });
}

// Create a trade post (buy/sell)
export async function postTrade(walletAddress, action, tokenSymbol, amount, mintAddress) {
  const emoji = action === 'buy' ? '💚' : '🔴';
  return createPost(walletAddress, `${emoji} ${action === 'buy' ? 'Bought' : 'Sold'} ${amount} $${tokenSymbol}`, {
    type: 'trade',
    action,
    mint: mintAddress,
    amount
  });
}

// Like a content node
export async function likeContent(walletAddress, contentId) {
  const res = await fetch(`${TAPESTRY_API}/likes?apiKey=${TAPESTRY_KEY}`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      userId: walletAddress,
      contentId,
      blockchain: 'SOLANA',
      execution: 'FAST_UNCONFIRMED'
    })
  });
  return res.json();
}

// Add a comment to a content node (e.g., comment on token page)
export async function addComment(walletAddress, contentId, comment) {
  const res = await fetch(`${TAPESTRY_API}/comments?apiKey=${TAPESTRY_KEY}`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      authorId: walletAddress,
      contentId,
      content: comment,
      blockchain: 'SOLANA',
      execution: 'FAST_UNCONFIRMED'
    })
  });
  return res.json();
}

// Get social feed for a user (posts from people they follow)
export async function getFeed(walletAddress) {
  const res = await fetch(`${TAPESTRY_API}/feed/${walletAddress}?apiKey=${TAPESTRY_KEY}`);
  return res.json();
}
