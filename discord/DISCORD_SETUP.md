# Send.it 🚀 — Discord Server Setup Guide

## 1. Create the Server

1. Open Discord → click **+** (Add a Server) → **Create My Own** → **For a club or community**
2. Server name: **Send.it 🚀**
3. Upload server icon (your Send.it logo)
4. Go to **Server Settings → Community** → Enable Community Server (unlocks Announcement channels, Welcome Screen, Server Discovery)

---

## 2. Channel Structure

Create the following categories and channels:

### 📢 ANNOUNCEMENTS
| Channel | Purpose | Permissions |
|---------|---------|-------------|
| `#announcements` | Official announcements (Announcement type) | Admin/Mod send only |
| `#updates` | Platform updates & changelogs | Admin/Mod send only |
| `#roadmap` | Roadmap milestones & progress | Admin/Mod send only |

### 💬 GENERAL
| Channel | Purpose |
|---------|---------|
| `#general` | Main chat |
| `#introductions` | New members introduce themselves |
| `#memes` | Memes & shitposts |

### 🚀 TRADING
| Channel | Purpose |
|---------|---------|
| `#token-launches` | Bot-posted new token alerts |
| `#calls` | Community token calls |
| `#price-discussion` | Price talk & analysis |
| `#whale-alerts` | Bot-posted whale trade alerts (>1 SOL) |

### 🛠️ DEVELOPMENT
| Channel | Purpose |
|---------|---------|
| `#dev-updates` | Developer announcements |
| `#bug-reports` | Bug reports (use Forum channel type) |
| `#feature-requests` | Feature requests (use Forum channel type) |
| `#github` | GitHub webhook feed |

### 🤖 BOTS
| Channel | Purpose |
|---------|---------|
| `#bot-commands` | Run bot commands here |
| `#price-bot` | Price check results |
| `#alerts` | Custom alert configurations |

### 🎮 COMMUNITY
| Channel | Purpose |
|---------|---------|
| `#giveaways` | Giveaway events |
| `#contests` | Trading contests & competitions |
| `#governance-votes` | Community governance proposals |

### 🎫 SUPPORT
| Channel | Purpose |
|---------|---------|
| `#support` | Get help (use Forum channel type) |
| `#faq` | Read-only FAQ |

---

## 3. Roles

Create roles in this order (highest to lowest):

| Role | Color | Permissions | How to Get |
|------|-------|-------------|------------|
| **Admin** | 🔴 Red `#E74C3C` | Administrator | Manually assigned |
| **Mod** | 🟠 Orange `#E67E22` | Manage Messages, Kick, Ban, Mute | Manually assigned |
| **Developer** | 🟣 Purple `#9B59B6` | Access to #dev channels | Manually assigned |
| **OG** | 🟡 Gold `#F1C40F` | Early member badge | Assigned to first 500 members |
| **Whale** | 🔵 Blue `#3498DB` | Access to whale-only channels | Collab.Land token-gate (≥100 SOL volume) |
| **Diamond Hands** | 💎 Cyan `#1ABC9C` | Badge | Collab.Land token-gate (held token ≥30 days) |
| **Degen** | 🟢 Green `#2ECC71` | Verified member | Pass verification |

### Role Permissions Quick Setup
- **@everyone**: Can read #announcements, #faq, #rules. Cannot send in announcement channels.
- **Degen** (verified): Can send in all general/trading/community channels.
- **Whale/OG**: Access to exclusive hidden channels if desired.
- Lock #token-launches and #whale-alerts to bot-only posting.

---

## 4. Verification System

### Option A: Captcha Bot (Recommended)
1. Invite [Captcha.bot](https://captcha.bot/) 
2. Configure:
   - Verification channel: create `#verify`
   - On verify: assign **Degen** role
   - Unverified users can only see `#verify` and `#rules`

### Option B: MEE6 Verification
1. Use MEE6's welcome plugin with reaction-based verification
2. New members react to rules message → get Degen role

---

## 5. Suggested Bots

| Bot | Purpose | Link |
|-----|---------|------|
| **MEE6** | Leveling, auto-mod, welcome messages | [mee6.xyz](https://mee6.xyz) |
| **Carl-bot** | Reaction roles, embeds, logging, auto-mod | [carl.gg](https://carl.gg) |
| **Collab.Land** | Token-gating (Whale & Diamond Hands roles) | [collab.land](https://collab.land) |
| **Send.it Bot** | Custom bot (see `/bot` folder) | Self-hosted |
| **GitHub Bot** | Webhook to #github | Discord webhook integration |

### Collab.Land Token-Gating Setup
1. Invite Collab.Land → run `/collabland setup`
2. Create token-gate rules:
   - **Whale**: Wallet has ≥100 SOL trading volume on Send.it
   - **Diamond Hands**: Held any token launched on Send.it ≥30 days
3. Users connect wallet via `/collabland verify`

---

## 6. Welcome Message & Rules

Set up the **Welcome Screen** (Server Settings → Community → Welcome Screen):
- Description: *"Welcome to Send.it 🚀 — The fastest Solana token launcher"*
- Channels to highlight:
  - `#rules` — Read the rules
  - `#introductions` — Say hi
  - `#general` — Start chatting
  - `#token-launches` — Watch new launches

### Rules (post in #rules or use welcome embed)
1. 🚫 No spam, scams, or phishing links
2. 🚫 No impersonation of team or mods
3. 🚫 No NSFW content
4. 💬 English only in main channels
5. 🤝 Be respectful — no harassment or hate speech
6. 📢 No unsolicited DMs or promotions
7. 🚀 DYOR — nothing here is financial advice
8. 🔒 Never share your private keys or seed phrase
9. 🎯 Use the right channels for the right topics
10. 🛡️ Report scams/issues to mods immediately

---

## 7. Auto-Mod Setup

### Discord AutoMod (built-in)
- Block known spam phrases
- Block excessive mentions (>5)
- Block known invite links (except your own)

### Carl-bot Auto-Mod
- Anti-raid: auto-ban if >10 joins in 30 seconds
- Anti-spam: mute if >5 messages in 5 seconds  
- Link filter: whitelist only send.it domains

---

## 8. GitHub Webhook (for #github)

1. Go to `#github` channel settings → Integrations → Webhooks → New Webhook
2. Copy the webhook URL
3. In your GitHub repo → Settings → Webhooks → Add webhook
4. Paste Discord webhook URL + `/github` at the end
5. Select events: Pushes, Pull Requests, Issues

---

## 9. Server Icon & Banner

- **Icon**: Send.it logo (512x512 PNG)
- **Banner**: Branded banner with tagline (960x540 PNG)
- **Invite Splash**: Branded splash image (Boost Level 1 required)

---

## 10. Quick Checklist

- [ ] Server created with correct name and icon
- [ ] All categories and channels created
- [ ] Roles created with correct colors and permissions
- [ ] Announcement channels set to read-only for members
- [ ] Bot channels locked to bot posting only
- [ ] Captcha.bot or verification system active
- [ ] MEE6 / Carl-bot configured
- [ ] Collab.Land token-gating set up
- [ ] Send.it custom bot deployed
- [ ] Welcome screen configured
- [ ] Rules posted
- [ ] AutoMod enabled
- [ ] GitHub webhook connected
- [ ] Invite link created (`discord.gg/sendit`)
