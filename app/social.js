/**
 * Send.it Social Layer — Tapestry Integration
 * Wired into social.html for live social features
 */

const TAPESTRY_API = 'https://api.usetapestry.dev/v1';
const TAPESTRY_KEY = '6e42e1b7-f35e-447c-aaab-e5b8c71726f3';

// ─── State ───
let connectedWallet = null;
let userProfile = null;

// ─── API Helpers ───
async function tapestryGet(path) {
  const sep = path.includes('?') ? '&' : '?';
  const res = await fetch(`${TAPESTRY_API}${path}${sep}apiKey=${TAPESTRY_KEY}`);
  if (!res.ok) throw new Error(`API ${res.status}`);
  return res.json();
}

async function tapestryPost(path, body) {
  const sep = path.includes('?') ? '&' : '?';
  const res = await fetch(`${TAPESTRY_API}${path}${sep}apiKey=${TAPESTRY_KEY}`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body)
  });
  if (!res.ok) throw new Error(`API ${res.status}`);
  return res.json();
}

async function tapestryDelete(path, body) {
  const sep = path.includes('?') ? '&' : '?';
  const res = await fetch(`${TAPESTRY_API}${path}${sep}apiKey=${TAPESTRY_KEY}`, {
    method: 'DELETE',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body)
  });
  if (!res.ok) throw new Error(`API ${res.status}`);
  return res.json();
}

// ─── Profile ───
async function findOrCreateProfile(walletAddress, username, bio = '') {
  return tapestryPost('/profiles/findOrCreate', {
    walletAddress,
    username: username || walletAddress.slice(0, 8),
    bio,
    blockchain: 'SOLANA',
    execution: 'FAST_UNCONFIRMED',
    customProperties: [
      { key: 'app', value: 'sendit' },
      { key: 'joinedAt', value: new Date().toISOString() }
    ]
  });
}

async function getProfile(profileId) {
  return tapestryGet(`/profiles/${profileId}`);
}

async function updateProfile(profileId, bio, image) {
  const customProperties = [];
  if (image) customProperties.push({ key: 'profileImage', value: image });
  return tapestryPost(`/profiles/${profileId}`, {
    bio,
    customProperties,
    blockchain: 'SOLANA',
    execution: 'FAST_UNCONFIRMED'
  });
}

// ─── Follows ───
async function followUser(followerId, followeeId) {
  return tapestryPost('/followers', {
    startId: followerId,
    endId: followeeId,
    blockchain: 'SOLANA',
    execution: 'FAST_UNCONFIRMED'
  });
}

async function unfollowUser(followerId, followeeId) {
  return tapestryDelete('/followers', {
    startId: followerId,
    endId: followeeId,
    blockchain: 'SOLANA',
    execution: 'FAST_UNCONFIRMED'
  });
}

async function getFollowerCount(profileId) {
  return tapestryGet(`/profiles/followers/${profileId}/count`);
}

async function getFollowingCount(profileId) {
  return tapestryGet(`/profiles/following/${profileId}/count`);
}

async function getFollowers(profileId, limit = 20) {
  return tapestryGet(`/profiles/followers/${profileId}?limit=${limit}`);
}

async function getFollowing(profileId, limit = 20) {
  return tapestryGet(`/profiles/following/${profileId}?limit=${limit}`);
}

// ─── Content ───
async function createPost(profileId, content, properties = {}) {
  const customProperties = [{ key: 'app', value: 'sendit' }];
  for (const [k, v] of Object.entries(properties)) {
    customProperties.push({ key: k, value: String(v) });
  }
  return tapestryPost('/contents/create', {
    profileId,
    content,
    contentType: 'text',
    customProperties,
    blockchain: 'SOLANA',
    execution: 'FAST_UNCONFIRMED'
  });
}

async function postTokenLaunch(profileId, { mint, name, symbol, uri }) {
  return createPost(profileId, `🚀 Launched ${name} ($${symbol}) on Send.it!\n\nMint: ${mint}\nBuy now on the bonding curve!`, {
    type: 'token_launch',
    mint,
    name,
    symbol,
    uri: uri || ''
  });
}

async function postTrade(profileId, action, symbol, amount, mint) {
  const emoji = action === 'buy' ? '💚' : '🔴';
  return createPost(profileId, `${emoji} ${action === 'buy' ? 'Bought' : 'Sold'} ${amount} $${symbol}`, {
    type: 'trade',
    action,
    mint,
    amount: String(amount)
  });
}

async function getUserPosts(profileId, limit = 20) {
  return tapestryGet(`/contents/profile/${profileId}?limit=${limit}&offset=0`);
}

async function getPost(contentId) {
  return tapestryGet(`/contents/${contentId}`);
}

// ─── Likes ───
async function likePost(profileId, contentId) {
  return tapestryPost('/likes', {
    profileId,
    contentId,
    blockchain: 'SOLANA',
    execution: 'FAST_UNCONFIRMED'
  });
}

async function unlikePost(profileId, contentId) {
  return tapestryDelete('/likes', {
    profileId,
    contentId,
    blockchain: 'SOLANA',
    execution: 'FAST_UNCONFIRMED'
  });
}

// ─── Comments ───
async function commentOnPost(profileId, contentId, text) {
  return tapestryPost('/comments', {
    profileId,
    contentId,
    text,
    blockchain: 'SOLANA',
    execution: 'FAST_UNCONFIRMED'
  });
}

async function getComments(contentId, limit = 50) {
  return tapestryGet(`/comments/${contentId}?limit=${limit}&offset=0`);
}

// ─── Feed ───
async function getPersonalFeed(profileId, limit = 20) {
  return tapestryGet(`/contents/feed/${profileId}?limit=${limit}&offset=0`);
}

// ─── UI Wiring ───

function shortAddr(addr) {
  if (!addr || addr.length < 8) return addr || '???';
  return addr.slice(0, 4) + '...' + addr.slice(-4);
}

function timeAgo(dateStr) {
  const diff = Date.now() - new Date(dateStr).getTime();
  const mins = Math.floor(diff / 60000);
  if (mins < 1) return 'just now';
  if (mins < 60) return `${mins}m ago`;
  const hrs = Math.floor(mins / 60);
  if (hrs < 24) return `${hrs}h ago`;
  return `${Math.floor(hrs / 24)}d ago`;
}

function renderPost(post) {
  const author = post.author?.username || post.profileId || '???';
  const time = post.created_at ? timeAgo(post.created_at) : '';
  const props = {};
  if (post.customProperties) {
    for (const p of post.customProperties) props[p.key] = p.value;
  }
  const isLaunch = props.type === 'token_launch';
  const isTrade = props.type === 'trade';
  
  let badge = '';
  if (isLaunch) badge = '<span class="post-badge launch-badge">🚀 Token Launch</span>';
  else if (isTrade) badge = `<span class="post-badge trade-badge">${props.action === 'buy' ? '💚 Buy' : '🔴 Sell'}</span>`;

  let mintLink = '';
  if (props.mint) {
    mintLink = `<a href="https://solscan.io/token/${props.mint}?cluster=devnet" target="_blank" class="mint-link">${shortAddr(props.mint)}</a>`;
  }

  return `
    <div class="feed-post card" data-id="${post.id || ''}">
      <div class="post-header">
        <div class="post-avatar">${author[0]?.toUpperCase() || '?'}</div>
        <div class="post-meta">
          <span class="post-author">${author}</span>
          ${badge}
          <span class="post-time">${time}</span>
        </div>
      </div>
      <div class="post-content">${escapeHtml(post.content || '')}</div>
      ${mintLink ? `<div class="post-mint">Token: ${mintLink}</div>` : ''}
      <div class="post-actions">
        <button class="post-action-btn like-btn" onclick="handleLike('${post.id}')">
          <i class="fas fa-heart"></i> <span>${post.likes_count || 0}</span>
        </button>
        <button class="post-action-btn comment-btn" onclick="toggleComments('${post.id}')">
          <i class="fas fa-comment"></i> <span>${post.comment_count || 0}</span>
        </button>
        <button class="post-action-btn share-btn" onclick="handleShare('${post.id}')">
          <i class="fas fa-share"></i>
        </button>
        ${isLaunch && props.mint ? `<a href="launchpad.html?mint=${props.mint}" class="post-action-btn trade-action"><i class="fas fa-chart-line"></i> Trade</a>` : ''}
      </div>
      <div class="post-comments-section" id="comments-${post.id}" style="display:none">
        <div class="comments-list" id="comments-list-${post.id}"></div>
        <div class="comment-input-row">
          <input type="text" placeholder="Write a comment..." id="comment-input-${post.id}" onkeydown="if(event.key==='Enter')submitComment('${post.id}')">
          <button class="comment-submit" onclick="submitComment('${post.id}')"><i class="fas fa-paper-plane"></i></button>
        </div>
      </div>
    </div>
  `;
}

function escapeHtml(text) {
  const div = document.createElement('div');
  div.textContent = text;
  return div.innerHTML.replace(/\n/g, '<br>');
}

function renderProfileCard(profile) {
  const username = profile?.username || shortAddr(connectedWallet);
  const bio = profile?.bio || 'Send.it community member';
  return `
    <div class="profile-avatar-lg">${username[0]?.toUpperCase() || '?'}</div>
    <div class="profile-username">${username}</div>
    <div class="profile-bio">${escapeHtml(bio)}</div>
    <div class="profile-stats">
      <div class="stat"><span class="stat-num" id="follower-count">0</span><span class="stat-label">Followers</span></div>
      <div class="stat"><span class="stat-num" id="following-count">0</span><span class="stat-label">Following</span></div>
      <div class="stat"><span class="stat-num" id="post-count">0</span><span class="stat-label">Posts</span></div>
    </div>
    <button class="edit-profile-btn" onclick="showEditProfile()"><i class="fas fa-edit"></i> Edit Profile</button>
  `;
}

// ─── Event Handlers ───

async function handleLike(postId) {
  if (!userProfile) { alert('Connect wallet first'); return; }
  try {
    await likePost(userProfile.username || userProfile.id, postId);
    const btn = document.querySelector(`[data-id="${postId}"] .like-btn`);
    if (btn) {
      btn.classList.toggle('liked');
      const span = btn.querySelector('span');
      span.textContent = parseInt(span.textContent) + 1;
    }
  } catch (e) { console.error('Like failed:', e); }
}

async function toggleComments(postId) {
  const section = document.getElementById(`comments-${postId}`);
  if (!section) return;
  const visible = section.style.display !== 'none';
  section.style.display = visible ? 'none' : 'block';
  if (!visible) {
    try {
      const data = await getComments(postId);
      const list = document.getElementById(`comments-list-${postId}`);
      if (list && data.comments) {
        list.innerHTML = data.comments.map(c => `
          <div class="comment">
            <span class="comment-author">${c.author?.username || c.profileId || '???'}</span>
            <span class="comment-text">${escapeHtml(c.text || c.content || '')}</span>
            <span class="comment-time">${c.created_at ? timeAgo(c.created_at) : ''}</span>
          </div>
        `).join('') || '<div class="no-comments">No comments yet</div>';
      }
    } catch (e) { console.error('Comments fetch failed:', e); }
  }
}

async function submitComment(postId) {
  if (!userProfile) { alert('Connect wallet first'); return; }
  const input = document.getElementById(`comment-input-${postId}`);
  if (!input || !input.value.trim()) return;
  try {
    await commentOnPost(userProfile.username || userProfile.id, postId, input.value.trim());
    input.value = '';
    toggleComments(postId); // close
    toggleComments(postId); // reopen to refresh
  } catch (e) { console.error('Comment failed:', e); }
}

function handleShare(postId) {
  const url = `${window.location.origin}/social.html?post=${postId}`;
  navigator.clipboard.writeText(url).then(() => {
    showToast('Link copied to clipboard!');
  });
}

function showToast(msg) {
  const toast = document.createElement('div');
  toast.className = 'toast';
  toast.textContent = msg;
  document.body.appendChild(toast);
  setTimeout(() => toast.remove(), 3000);
}

async function handleCreatePost() {
  if (!userProfile) { alert('Connect wallet first'); return; }
  const input = document.getElementById('new-post-input');
  if (!input || !input.value.trim()) return;
  try {
    document.getElementById('post-submit-btn').disabled = true;
    await createPost(userProfile.username || userProfile.id, input.value.trim());
    input.value = '';
    showToast('Post created! ✅');
    await loadFeed();
  } catch (e) {
    console.error('Post failed:', e);
    showToast('Post failed — try again');
  } finally {
    document.getElementById('post-submit-btn').disabled = false;
  }
}

async function handleFollow(targetId) {
  if (!userProfile) { alert('Connect wallet first'); return; }
  try {
    await followUser(userProfile.username || userProfile.id, targetId);
    showToast(`Following ${targetId}! ✅`);
  } catch (e) { console.error('Follow failed:', e); }
}

function showEditProfile() {
  const modal = document.getElementById('edit-profile-modal');
  if (modal) modal.style.display = 'flex';
}

async function saveProfile() {
  const username = document.getElementById('edit-username')?.value?.trim();
  const bio = document.getElementById('edit-bio')?.value?.trim();
  if (!username) return;
  try {
    userProfile = await findOrCreateProfile(connectedWallet, username, bio);
    document.getElementById('edit-profile-modal').style.display = 'none';
    renderProfileSection();
    showToast('Profile updated! ✅');
  } catch (e) {
    console.error('Profile update failed:', e);
    showToast('Update failed');
  }
}

// ─── Wallet Connection (Phantom/Solflare) ───

async function connectWallet() {
  try {
    // Try Phantom first
    if (window.solana?.isPhantom) {
      const resp = await window.solana.connect();
      connectedWallet = resp.publicKey.toString();
    } else if (window.solflare?.isSolflare) {
      await window.solflare.connect();
      connectedWallet = window.solflare.publicKey.toString();
    } else {
      // Demo mode — use a sample wallet
      connectedWallet = 'G3QLffUiguxYTkGWBjtpgKy6WAh2WgQefQKsDscSdNP9';
      showToast('Demo mode — no wallet detected');
    }

    document.getElementById('wallet-btn').innerHTML = `<i class="fas fa-wallet"></i>&nbsp; ${shortAddr(connectedWallet)}`;
    document.getElementById('wallet-btn').classList.add('connected');

    // Create/find profile
    userProfile = await findOrCreateProfile(connectedWallet, connectedWallet.slice(0, 8));
    
    renderProfileSection();
    loadFeed();
    showToast('Wallet connected! ✅');
  } catch (e) {
    console.error('Wallet connect failed:', e);
    showToast('Connection failed');
  }
}

async function renderProfileSection() {
  const el = document.getElementById('profile-card-content');
  if (!el || !userProfile) return;
  el.innerHTML = renderProfileCard(userProfile);
  
  // Load counts
  try {
    const [followers, following] = await Promise.all([
      getFollowerCount(userProfile.username || userProfile.id).catch(() => ({ count: 0 })),
      getFollowingCount(userProfile.username || userProfile.id).catch(() => ({ count: 0 }))
    ]);
    document.getElementById('follower-count').textContent = followers.count || 0;
    document.getElementById('following-count').textContent = following.count || 0;
  } catch {}
}

async function loadFeed() {
  const feedEl = document.getElementById('social-feed');
  if (!feedEl) return;
  
  feedEl.innerHTML = '<div class="loading"><i class="fas fa-spinner fa-spin"></i> Loading feed...</div>';
  
  try {
    let posts = [];
    if (userProfile) {
      try {
        const data = await getPersonalFeed(userProfile.username || userProfile.id);
        posts = data.contents || data.posts || [];
      } catch {}
    }
    
    // If no personal feed or not connected, show user's own posts or placeholder
    if (posts.length === 0 && userProfile) {
      try {
        const data = await getUserPosts(userProfile.username || userProfile.id);
        posts = data.contents || data.posts || [];
      } catch {}
    }
    
    if (posts.length === 0) {
      feedEl.innerHTML = `
        <div class="empty-feed">
          <i class="fas fa-rocket" style="font-size:48px;color:var(--neon);margin-bottom:16px"></i>
          <h3>Your feed is empty</h3>
          <p>Follow creators or make your first post to get started!</p>
        </div>
      `;
    } else {
      feedEl.innerHTML = posts.map(renderPost).join('');
    }
  } catch (e) {
    console.error('Feed load failed:', e);
    feedEl.innerHTML = '<div class="empty-feed"><p>Failed to load feed</p></div>';
  }
}

// ─── Suggested Users (demo) ───
function renderSuggestedUsers() {
  const el = document.getElementById('suggested-users');
  if (!el) return;
  const users = [
    { name: 'SolMaxi', bio: 'Full-time degen, part-time builder', emoji: '🚀' },
    { name: 'DegenQueen', bio: 'NFT collector & token sniper', emoji: '👑' },
    { name: 'CryptoChef', bio: 'Cooking up alpha daily', emoji: '🍳' },
    { name: 'WhaleAlert', bio: 'Tracking the big moves', emoji: '🐋' },
    { name: 'DiamondHands', bio: 'Never selling. Ever.', emoji: '💎' },
  ];
  el.innerHTML = users.map(u => `
    <div class="suggested-user">
      <div class="su-avatar">${u.emoji}</div>
      <div class="su-info">
        <div class="su-name">${u.name}</div>
        <div class="su-bio">${u.bio}</div>
      </div>
      <button class="follow-btn" onclick="handleFollow('${u.name}')"><i class="fas fa-plus"></i> Follow</button>
    </div>
  `).join('');
}

// ─── Init ───
document.addEventListener('DOMContentLoaded', () => {
  const walletBtn = document.getElementById('wallet-btn');
  if (walletBtn) walletBtn.addEventListener('click', connectWallet);
  
  const postBtn = document.getElementById('post-submit-btn');
  if (postBtn) postBtn.addEventListener('click', handleCreatePost);
  
  const saveProfileBtn = document.getElementById('save-profile-btn');
  if (saveProfileBtn) saveProfileBtn.addEventListener('click', saveProfile);
  
  renderSuggestedUsers();
});

// Make functions globally accessible for onclick handlers
window.handleLike = handleLike;
window.toggleComments = toggleComments;
window.submitComment = submitComment;
window.handleShare = handleShare;
window.handleFollow = handleFollow;
window.handleCreatePost = handleCreatePost;
window.showEditProfile = showEditProfile;
window.saveProfile = saveProfile;
