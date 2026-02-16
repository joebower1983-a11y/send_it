/**
 * Send.it × Tapestry Social Integration
 * 
 * Adds on-chain social features to Send.it token launches:
 * - Creator profiles (wallet-linked)
 * - Follow token creators
 * - Token launch posts (social feed)
 * - Likes & comments on launches
 * 
 * Uses Tapestry (usetapestry.dev) — Solana's social protocol
 * App ID: 601a8251-9c95-4456-97af-c1e79b5c0259
 */

const API_URL = 'https://api.usetapestry.dev/v1';
const APP_ID = '601a8251-9c95-4456-97af-c1e79b5c0259';

export class SendItSocial {
  constructor(apiKey) {
    if (!apiKey) throw new Error('Tapestry API key required');
    this.apiKey = apiKey;
  }

  async _request(path, method = 'GET', body = null) {
    const separator = path.includes('?') ? '&' : '?';
    const url = `${API_URL}${path}${separator}apiKey=${this.apiKey}`;
    const opts = { method, headers: { 'Content-Type': 'application/json' } };
    if (body) opts.body = JSON.stringify(body);
    const res = await fetch(url, opts);
    if (!res.ok) {
      const text = await res.text();
      throw new Error(`Tapestry API ${res.status}: ${text}`);
    }
    return res.json();
  }

  // ─── Profiles ───

  /**
   * Create or find a Send.it user profile linked to their Solana wallet
   */
  async findOrCreateProfile(walletAddress, username, bio = '', image = '') {
    const customProperties = [];
    if (image) customProperties.push({ key: 'profileImage', value: image });
    customProperties.push({ key: 'app', value: 'sendit' });

    return this._request('/profiles/findOrCreate', 'POST', {
      walletAddress,
      username,
      bio,
      blockchain: 'SOLANA',
      execution: 'FAST_UNCONFIRMED',
      customProperties,
    });
  }

  /**
   * Get a user's profile by ID/username
   */
  async getProfile(profileId) {
    return this._request(`/profiles/${profileId}`);
  }

  /**
   * Update profile bio/image
   */
  async updateProfile(profileId, updates) {
    const customProperties = [];
    if (updates.image) customProperties.push({ key: 'profileImage', value: updates.image });
    return this._request(`/profiles/${profileId}`, 'PUT', {
      bio: updates.bio,
      customProperties,
      blockchain: 'SOLANA',
      execution: 'FAST_UNCONFIRMED',
    });
  }

  // ─── Follows ───

  /**
   * Follow a token creator
   */
  async follow(followerId, followeeId) {
    return this._request('/followers', 'POST', {
      startId: followerId,
      endId: followeeId,
      blockchain: 'SOLANA',
      execution: 'FAST_UNCONFIRMED',
    });
  }

  /**
   * Unfollow a user
   */
  async unfollow(followerId, followeeId) {
    return this._request('/followers', 'DELETE', {
      startId: followerId,
      endId: followeeId,
      blockchain: 'SOLANA',
      execution: 'FAST_UNCONFIRMED',
    });
  }

  /**
   * Get follower count
   */
  async getFollowerCount(profileId) {
    return this._request(`/profiles/followers/${profileId}/count`);
  }

  /**
   * Get following count
   */
  async getFollowingCount(profileId) {
    return this._request(`/profiles/following/${profileId}/count`);
  }

  /**
   * Get list of followers
   */
  async getFollowers(profileId, limit = 20, offset = 0) {
    return this._request(`/profiles/followers/${profileId}?limit=${limit}&offset=${offset}`);
  }

  /**
   * Get list of following
   */
  async getFollowing(profileId, limit = 20, offset = 0) {
    return this._request(`/profiles/following/${profileId}?limit=${limit}&offset=${offset}`);
  }

  // ─── Token Launch Posts ───

  /**
   * Create a post for a new token launch
   * Links the launch to the social graph
   */
  async postTokenLaunch(creatorProfileId, { mint, name, symbol, description, uri }) {
    return this._request('/contents/create', 'POST', {
      profileId: creatorProfileId,
      content: description || `${name} ($${symbol}) just launched on Send.it! 🚀`,
      contentType: 'text',
      customProperties: [
        { key: 'type', value: 'token_launch' },
        { key: 'mint', value: mint },
        { key: 'name', value: name },
        { key: 'symbol', value: symbol },
        { key: 'uri', value: uri || '' },
        { key: 'app', value: 'sendit' },
      ],
      blockchain: 'SOLANA',
      execution: 'FAST_UNCONFIRMED',
    });
  }

  /**
   * Create a generic post (user update, comment thread, etc.)
   */
  async createPost(profileId, content, properties = {}) {
    const customProperties = [{ key: 'app', value: 'sendit' }];
    for (const [k, v] of Object.entries(properties)) {
      customProperties.push({ key: k, value: String(v) });
    }
    return this._request('/contents/create', 'POST', {
      profileId,
      content,
      contentType: 'text',
      customProperties,
      blockchain: 'SOLANA',
      execution: 'FAST_UNCONFIRMED',
    });
  }

  /**
   * Get a post by ID
   */
  async getPost(contentId) {
    return this._request(`/contents/${contentId}`);
  }

  /**
   * Get posts by a user
   */
  async getUserPosts(profileId, limit = 20, offset = 0) {
    return this._request(`/contents/profile/${profileId}?limit=${limit}&offset=${offset}`);
  }

  // ─── Likes ───

  /**
   * Like a token launch or post
   */
  async like(profileId, contentId) {
    return this._request('/likes', 'POST', {
      profileId,
      contentId,
      blockchain: 'SOLANA',
      execution: 'FAST_UNCONFIRMED',
    });
  }

  /**
   * Unlike
   */
  async unlike(profileId, contentId) {
    return this._request('/likes', 'DELETE', {
      profileId,
      contentId,
      blockchain: 'SOLANA',
      execution: 'FAST_UNCONFIRMED',
    });
  }

  // ─── Comments ───

  /**
   * Comment on a token launch or post
   */
  async comment(profileId, contentId, text) {
    return this._request('/comments', 'POST', {
      profileId,
      contentId,
      text,
      blockchain: 'SOLANA',
      execution: 'FAST_UNCONFIRMED',
    });
  }

  /**
   * Get comments on a post
   */
  async getComments(contentId, limit = 50, offset = 0) {
    return this._request(`/comments/${contentId}?limit=${limit}&offset=${offset}`);
  }

  // ─── Feed Helpers ───

  /**
   * Get a social feed of token launches (all posts tagged type=token_launch)
   */
  async getTokenLaunchFeed(limit = 20, offset = 0) {
    // Tapestry doesn't have a direct tag filter — get recent content
    // Client-side filter for token_launch type
    // In production, use Tapestry's search/filter when available
    return this._request(`/contents?limit=${limit}&offset=${offset}`);
  }

  /**
   * Get a user's personalized feed (posts from people they follow)
   */
  async getPersonalFeed(profileId, limit = 20, offset = 0) {
    return this._request(`/contents/feed/${profileId}?limit=${limit}&offset=${offset}`);
  }
}

export default SendItSocial;
