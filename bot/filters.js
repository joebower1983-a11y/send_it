const BOT_TOKEN = process.env.TELEGRAM_BOT_TOKEN || "8562369283:AAEG2hfV6vOCzSwcxEmpHtVBYxRxBYS_ejI";
const BASE = `https://api.telegram.org/bot${BOT_TOKEN}`;

const MINT = "F8qWTN8JfyDCvj4RoCHuvNMVbTV9XQksLuziA8PYpump";

const responses = {
  "/price": `📊 *SENDIT Token*\n\n• Mint: \`${MINT}\`\n• Chain: Solana\n• Platform: Pump\\.fun\n\n[View on Pump\\.fun](https://pump.fun/coin/${MINT})\n[View on DexScreener](https://dexscreener.com/solana/${MINT})`,

  "/links": `🔗 *Official Links*\n\n🟢 [Pump\\.fun Token](https://pump.fun/coin/${MINT})\n📦 [GitHub](https://github.com/joebower1983-a11y/send_it)\n🌐 [Live Demo](https://send-it-seven-sigma.vercel.app)\n💬 [Discord](https://discord.gg/vKRTyG85)\n🐦 [Twitter](https://twitter.com/SendItSolana420)\n📱 [Telegram](https://t.me/+Xw4E2sJ0Z3Q5ZDYx)`,

  "/tokeninfo": `💰 *SENDIT Token Info*\n\n• Name: Send It\n• Ticker: SENDIT\n• Chain: Solana\n• Mint: \`${MINT}\`\n\n*Fee Structure \\(launchpad\\):*\n• 1% platform fee → treasury\n• 1% creator fee → token creators\n• Holder rewards → redistributed\n\n*Modules:* 29 on\\-chain \\| 13k\\+ lines of Rust`,

  "/rules": `📜 *Group Rules*\n\n1️⃣ Be respectful\n2️⃣ No scams, phishing, or unsolicited DMs\n3️⃣ No shilling other projects\n4️⃣ Nothing here is financial advice — DYOR\n5️⃣ English only\n6️⃣ No spam\n7️⃣ Have fun and send it\\! 🚀\n\n_Breaking rules \\= warn → mute → ban_`,

  "/website": `🌐 *Send\\.it Website*\n\n• Main: [send\\-it\\-seven\\-sigma\\.vercel\\.app](https://send-it-seven-sigma.vercel.app)\n• GitHub Pages: [joebower1983\\-a11y\\.github\\.io/send\\_it](https://joebower1983-a11y.github.io/send_it/)\n• Pitch Deck: [View](https://joebower1983-a11y.github.io/send_it/pitch-deck.html)`,

  "/chart": `📈 *SENDIT Chart*\n\n[DexScreener](https://dexscreener.com/solana/${MINT})\n[Pump\\.fun](https://pump.fun/coin/${MINT})\n[Birdeye](https://birdeye.so/token/${MINT}?chain=solana)`,

  "/buy": `🛒 *How to Buy SENDIT*\n\n1\\. Get a Solana wallet \\(Phantom, Solflare\\)\n2\\. Fund it with SOL\n3\\. Go to [Pump\\.fun](https://pump.fun/coin/${MINT})\n4\\. Connect wallet and buy\\!\n\n⚠️ _DYOR \\- This is not financial advice_`,

  "/socials": `📱 *Send\\.it Socials*\n\n🐦 Twitter: [@SendItSolana420](https://twitter.com/SendItSolana420)\n💬 Discord: [discord\\.gg/vKRTyG85](https://discord.gg/vKRTyG85)\n📱 Telegram: [Join Group](https://t.me/+Xw4E2sJ0Z3Q5ZDYx)\n📦 GitHub: [send\\_it](https://github.com/joebower1983-a11y/send_it)`,

  "/whitepaper": `📄 *Send\\.it Whitepaper v2\\.0*\n\nRead the full whitepaper covering all 29 modules:\n[View on GitHub](https://github.com/joebower1983-a11y/send_it/blob/main/docs/WHITEPAPER.md)`,

  "/ca": `📋 *Contract Address*\n\n\`F8qWTN8JfyDCvj4RoCHuvNMVbTV9XQksLuziA8PYpump\`\n\n[Buy on Pump\\.fun](https://pump.fun/coin/F8qWTN8JfyDCvj4RoCHuvNMVbTV9XQksLuziA8PYpump)`,

  "/filters": `🤖 *Bot Commands*\n\n📊 /price — Token price \\& stats\n📋 /ca — Contract address\n🔗 /links — Official links\n💰 /tokeninfo — Contract \\& fee info\n📜 /rules — Group rules\n🌐 /website — Send\\.it website\n📈 /chart — Price charts\n🛒 /buy — How to buy SENDIT\n📱 /socials — Social media links\n📄 /whitepaper — Read the whitepaper\n🗺️ /roadmap — Project roadmap\n🚨 /raids — Raid coordinator\n📣 /shill — Copy\\-paste shill message\n🤖 /filters — This list\n\n🛡️ *Mod Commands \\(admin/mod only\\):*\n/warn — Warn a user \\(reply\\)\n/mute \\[min\\] — Mute user \\(reply, default 60min\\)\n/unmute — Unmute user \\(reply\\)\n/ban — Ban user \\(reply\\)\n/unban — Unban user \\(reply\\)\n\n⚔️ *Raid Leader Commands \\(mod/owner\\):*\n/addraidleader — Add raid leader \\(reply\\)\n/removeraidleader — Remove raid leader \\(reply\\)\n/raidleaders — List raid leaders\n\n📣 *Roles \\(mod\\):*\n/shiller — Give Shiller 📣 badge \\(reply\\)\n/unshiller — Remove Shiller badge \\(reply\\)\n/fundraiser — Give Fundraiser 💰 badge \\(reply\\)\n/unfundraiser — Remove Fundraiser badge \\(reply\\)\n/pm — Give Project Manager 📋 badge \\(reply\\)\n/unpm — Remove Project Manager badge \\(reply\\)\n/raider — Give Raider ⚔️ badge \\(reply\\)\n/unraider — Remove Raider badge \\(reply\\)\n\n👑 *Owner Commands:*\n/addmod — Add bot moderator \\(reply\\)\n/removemod — Remove bot moderator \\(reply\\)\n/modlist — List all bot moderators\n\n🏆 *Contest Commands:*\n/contest — Show active contests \\& help\n/contest shill start|enter|entries|end\n/contest raid start|leaderboard|end\n/contest meme start|enter|entries|end\n/contest invite start|leaderboard|end\n/contest end all — End all contests \\(mod\\)`,

  "/shill": `📣 *Copy \\& paste this everywhere:*\n\n\\-\\-\\-\\-\\-\\-\\-\\-\\-\\-\\-\\-\\-\\-\\-\\-\\-\\-\\-\\-\n\n🚀 Send\\.it — The fairest token launchpad on Solana\n\n✅ No insiders \\| No presales \\| Anti\\-snipe\n✅ 29 on\\-chain modules \\| 13k\\+ lines of Rust\n✅ Auto Raydium migration\n✅ Creator rewards \\+ holder rewards\n\n📋 CA: F8qWTN8JfyDCvj4RoCHuvNMVbTV9XQksLuziA8PYpump\n\n🐦 twitter\\.com/SendItSolana420\n💬 t\\.me/\\+Xw4E2sJ0Z3Q5ZDYx\n💎 discord\\.gg/vKRTyG85\n📈 pump\\.fun/coin/F8qWTN8JfyDCvj4RoCHuvNMVbTV9XQksLuziA8PYpump\n\n\\-\\-\\-\\-\\-\\-\\-\\-\\-\\-\\-\\-\\-\\-\\-\\-\\-\\-\\-\\-\n\nSend it\\! 🔥`,

  "/roadmap": `🗺️ *Send\\.it Roadmap*\n\n*Q1 2026* ← WE ARE HERE\n• Core program \\+ community building\n• Token launch on Pump\\.fun ✅\n• Grant applications ✅\n\n*Q2 2026*\n• Mainnet deployment\n• First token launches\n• Mobile PWA\n\n*Q3 2026*\n• DeFi suite live \\(staking, lending, perps\\)\n• Solana dApp Store\n\n*Q4 2026*\n• Cross\\-chain bridge\n• DAO governance\n• Ecosystem partnerships`
};

// Anti-spam: block common scam patterns
const SPAM_PATTERNS = [
  /airdrop.*claim/i,
  /connect.*wallet.*verify/i,
  /dm me for/i,
  /send \d+ sol/i,
  /free (nft|token|sol|crypto)/i,
  /t\.me\/(?!.*SendIt)/i, // other telegram links
  /bit\.ly|tinyurl/i,
];

// Bot moderators and raid leaders
// Hardcoded mods persist across Vercel cold starts (in-memory Set resets otherwise)
const botMods = new Set([
  6541770845,  // ZED⚡️
  6312896742,  // Crypto
  6260568591,  // Shuñ2.0🗿
]);
const raidLeaders = new Set(); // can start raids but not mod
const OWNER_IDS = [7920028061]; // Joe's Telegram ID

function isOwner(userId) {
  return OWNER_IDS.includes(userId);
}

function isMod(userId) {
  return botMods.has(userId) || isOwner(userId);
}

// Contest system
const contests = {
  shill: { active: false, entries: new Map(), endsAt: null, chatId: null },
  raid: { active: false, scores: new Map(), endsAt: null, chatId: null },
  meme: { active: false, entries: [], endsAt: null, chatId: null },
  invite: { active: false, scores: new Map(), endsAt: null, chatId: null }
};

async function handleContestCommand(msg, chatId, text) {
  const parts = text.split(/\s+/);
  const type = parts[1]?.toLowerCase();
  const sub = parts[2]?.toLowerCase();

  // /contest — show active contests & help
  if (!type) {
    const active = Object.entries(contests).filter(([,c]) => c.active);
    let txt = "🏆 *Contest System*\n\n";
    if (active.length) {
      txt += "*Active Contests:*\n";
      for (const [name, c] of active) {
        const left = Math.max(0, Math.round((c.endsAt - Date.now()) / 3600000));
        txt += `• ${name.charAt(0).toUpperCase() + name.slice(1)} — ${left}h remaining\n`;
      }
    } else {
      txt += "_No active contests_\n";
    }
    txt += "\n*Commands:*\n";
    txt += "/contest shill start|enter|entries|end\n";
    txt += "/contest raid start|leaderboard|end\n";
    txt += "/contest meme start|enter|entries|end\n";
    txt += "/contest invite start|leaderboard|end\n";
    txt += "/contest end all — end all contests";
    await fetch(`${BASE}/sendMessage`, { method: "POST", headers: {"Content-Type":"application/json"}, body: JSON.stringify({chat_id: chatId, text: txt, reply_to_message_id: msg.message_id}) });
    return;
  }

  // /contest end all
  if (type === "end" && sub === "all") {
    if (!isMod(msg.from.id)) {
      await fetch(`${BASE}/sendMessage`, { method: "POST", headers: {"Content-Type":"application/json"}, body: JSON.stringify({chat_id: chatId, text: "⛔ Mods only.", reply_to_message_id: msg.message_id}) });
      return;
    }
    for (const c of Object.values(contests)) {
      c.active = false; c.endsAt = null;
    }
    await fetch(`${BASE}/sendMessage`, { method: "POST", headers: {"Content-Type":"application/json"}, body: JSON.stringify({chat_id: chatId, text: "🏆 All contests ended."}) });
    return;
  }

  // SHILL CONTEST
  if (type === "shill") {
    const c = contests.shill;
    if (sub === "start") {
      if (!isMod(msg.from.id)) { await fetch(`${BASE}/sendMessage`, { method: "POST", headers: {"Content-Type":"application/json"}, body: JSON.stringify({chat_id: chatId, text: "⛔ Mods only.", reply_to_message_id: msg.message_id}) }); return; }
      const hours = parseInt(parts[3]) || 72;
      c.active = true; c.entries = new Map(); c.endsAt = Date.now() + hours * 3600000; c.chatId = chatId;
      await fetch(`${BASE}/sendMessage`, { method: "POST", headers: {"Content-Type":"application/json"}, body: JSON.stringify({chat_id: chatId, text: `🏆 Shill Contest started! ${hours}h to go.\n\nSubmit your tweet with:\n/contest shill enter <url>`}) });
    } else if (sub === "enter") {
      if (!c.active) { await fetch(`${BASE}/sendMessage`, { method: "POST", headers: {"Content-Type":"application/json"}, body: JSON.stringify({chat_id: chatId, text: "No active shill contest.", reply_to_message_id: msg.message_id}) }); return; }
      const url = parts[3];
      if (!url) { await fetch(`${BASE}/sendMessage`, { method: "POST", headers: {"Content-Type":"application/json"}, body: JSON.stringify({chat_id: chatId, text: "Usage: /contest shill enter <tweet_url>", reply_to_message_id: msg.message_id}) }); return; }
      c.entries.set(msg.from.id, { url, name: msg.from.first_name || "Anon", timestamp: Date.now() });
      await fetch(`${BASE}/sendMessage`, { method: "POST", headers: {"Content-Type":"application/json"}, body: JSON.stringify({chat_id: chatId, text: `✅ Entry recorded! (${c.entries.size} total entries)`, reply_to_message_id: msg.message_id}) });
    } else if (sub === "entries") {
      if (!c.active && c.entries.size === 0) { await fetch(`${BASE}/sendMessage`, { method: "POST", headers: {"Content-Type":"application/json"}, body: JSON.stringify({chat_id: chatId, text: "No shill contest entries.", reply_to_message_id: msg.message_id}) }); return; }
      let txt = `📣 Shill Contest Entries (${c.entries.size}):\n\n`;
      let i = 1;
      for (const [, e] of c.entries) { txt += `${i++}. ${e.name} — ${e.url}\n`; }
      await fetch(`${BASE}/sendMessage`, { method: "POST", headers: {"Content-Type":"application/json"}, body: JSON.stringify({chat_id: chatId, text: txt, reply_to_message_id: msg.message_id}) });
    } else if (sub === "end") {
      if (!isMod(msg.from.id)) { await fetch(`${BASE}/sendMessage`, { method: "POST", headers: {"Content-Type":"application/json"}, body: JSON.stringify({chat_id: chatId, text: "⛔ Mods only.", reply_to_message_id: msg.message_id}) }); return; }
      c.active = false;
      let txt = `🏆 Shill Contest ended! ${c.entries.size} entries:\n\n`;
      let i = 1;
      for (const [, e] of c.entries) { txt += `${i++}. ${e.name} — ${e.url}\n`; }
      txt += "\nMods: judge entries and announce the winner!";
      await fetch(`${BASE}/sendMessage`, { method: "POST", headers: {"Content-Type":"application/json"}, body: JSON.stringify({chat_id: chatId, text: txt}) });
    }
    return;
  }

  // RAID CONTEST
  if (type === "raid") {
    const c = contests.raid;
    if (sub === "start") {
      if (!isMod(msg.from.id)) { await fetch(`${BASE}/sendMessage`, { method: "POST", headers: {"Content-Type":"application/json"}, body: JSON.stringify({chat_id: chatId, text: "⛔ Mods only.", reply_to_message_id: msg.message_id}) }); return; }
      const days = parseInt(parts[3]) || 7;
      c.active = true; c.scores = new Map(); c.endsAt = Date.now() + days * 86400000; c.chatId = chatId;
      await fetch(`${BASE}/sendMessage`, { method: "POST", headers: {"Content-Type":"application/json"}, body: JSON.stringify({chat_id: chatId, text: `🏆 Raid Contest started! ${days} days to go.\n\nComplete raids with /raids done to earn points!`}) });
    } else if (sub === "leaderboard") {
      if (c.scores.size === 0) { await fetch(`${BASE}/sendMessage`, { method: "POST", headers: {"Content-Type":"application/json"}, body: JSON.stringify({chat_id: chatId, text: "No raid contest scores yet.", reply_to_message_id: msg.message_id}) }); return; }
      const sorted = [...c.scores.values()].sort((a, b) => b.points - a.points).slice(0, 10);
      let txt = "🏆 Raid Contest Leaderboard:\n\n";
      sorted.forEach((e, i) => { txt += `${i + 1}. ${e.name} — ${e.points} pts\n`; });
      await fetch(`${BASE}/sendMessage`, { method: "POST", headers: {"Content-Type":"application/json"}, body: JSON.stringify({chat_id: chatId, text: txt, reply_to_message_id: msg.message_id}) });
    } else if (sub === "end") {
      if (!isMod(msg.from.id)) { await fetch(`${BASE}/sendMessage`, { method: "POST", headers: {"Content-Type":"application/json"}, body: JSON.stringify({chat_id: chatId, text: "⛔ Mods only.", reply_to_message_id: msg.message_id}) }); return; }
      c.active = false;
      const sorted = [...c.scores.values()].sort((a, b) => b.points - a.points).slice(0, 10);
      let txt = "🏆 Raid Contest OVER! Final standings:\n\n";
      sorted.forEach((e, i) => { txt += `${["🥇","🥈","🥉"][i] || `${i+1}.`} ${e.name} — ${e.points} pts\n`; });
      if (sorted.length === 0) txt += "No participants.";
      await fetch(`${BASE}/sendMessage`, { method: "POST", headers: {"Content-Type":"application/json"}, body: JSON.stringify({chat_id: chatId, text: txt}) });
    }
    return;
  }

  // MEME CONTEST
  if (type === "meme") {
    const c = contests.meme;
    if (sub === "start") {
      if (!isMod(msg.from.id)) { await fetch(`${BASE}/sendMessage`, { method: "POST", headers: {"Content-Type":"application/json"}, body: JSON.stringify({chat_id: chatId, text: "⛔ Mods only.", reply_to_message_id: msg.message_id}) }); return; }
      const hours = parseInt(parts[3]) || 72;
      c.active = true; c.entries = []; c.endsAt = Date.now() + hours * 3600000; c.chatId = chatId;
      await fetch(`${BASE}/sendMessage`, { method: "POST", headers: {"Content-Type":"application/json"}, body: JSON.stringify({chat_id: chatId, text: `🏆 Meme Contest started! ${hours}h to go.\n\nReply to a photo/image with:\n/contest meme enter`}) });
    } else if (sub === "enter") {
      if (!c.active) { await fetch(`${BASE}/sendMessage`, { method: "POST", headers: {"Content-Type":"application/json"}, body: JSON.stringify({chat_id: chatId, text: "No active meme contest.", reply_to_message_id: msg.message_id}) }); return; }
      const reply = msg.reply_to_message;
      if (!reply || (!reply.photo && !reply.document)) { await fetch(`${BASE}/sendMessage`, { method: "POST", headers: {"Content-Type":"application/json"}, body: JSON.stringify({chat_id: chatId, text: "↩️ Reply to a photo/image to enter!", reply_to_message_id: msg.message_id}) }); return; }
      c.entries.push({ userId: msg.from.id, name: msg.from.first_name || "Anon", messageId: reply.message_id, timestamp: Date.now() });
      await fetch(`${BASE}/sendMessage`, { method: "POST", headers: {"Content-Type":"application/json"}, body: JSON.stringify({chat_id: chatId, text: `✅ Meme entry recorded! (${c.entries.length} total)`, reply_to_message_id: msg.message_id}) });
    } else if (sub === "entries") {
      await fetch(`${BASE}/sendMessage`, { method: "POST", headers: {"Content-Type":"application/json"}, body: JSON.stringify({chat_id: chatId, text: `🎨 Meme Contest: ${c.entries.length} entries so far.`, reply_to_message_id: msg.message_id}) });
    } else if (sub === "end") {
      if (!isMod(msg.from.id)) { await fetch(`${BASE}/sendMessage`, { method: "POST", headers: {"Content-Type":"application/json"}, body: JSON.stringify({chat_id: chatId, text: "⛔ Mods only.", reply_to_message_id: msg.message_id}) }); return; }
      c.active = false;
      let txt = `🏆 Meme Contest ended! ${c.entries.length} entries:\n\n`;
      c.entries.forEach((e, i) => { txt += `${i + 1}. ${e.name}\n`; });
      txt += "\nMods: judge entries and announce the winner!";
      await fetch(`${BASE}/sendMessage`, { method: "POST", headers: {"Content-Type":"application/json"}, body: JSON.stringify({chat_id: chatId, text: txt}) });
    }
    return;
  }

  // INVITE CONTEST
  if (type === "invite") {
    const c = contests.invite;
    if (sub === "start") {
      if (!isMod(msg.from.id)) { await fetch(`${BASE}/sendMessage`, { method: "POST", headers: {"Content-Type":"application/json"}, body: JSON.stringify({chat_id: chatId, text: "⛔ Mods only.", reply_to_message_id: msg.message_id}) }); return; }
      const days = parseInt(parts[3]) || 7;
      c.active = true; c.scores = new Map(); c.endsAt = Date.now() + days * 86400000; c.chatId = chatId;
      const startTxt = `🏆 *Invite Contest started!* ${days} days to go.\n\nInvite friends — they'll be tracked automatically!\n\n💰 *Prize Pool (Points):*\n🥇 1st Place: 5,000 pts\n🥈 2nd Place: 2,500 pts\n🥉 3rd Place: 1,000 pts\n⭐ Top 10: 500 pts each\n✅ All participants (1+ invite): 100 pts\n\nCheck standings: /contest invite leaderboard`;
      await fetch(`${BASE}/sendMessage`, { method: "POST", headers: {"Content-Type":"application/json"}, body: JSON.stringify({chat_id: chatId, text: startTxt, parse_mode: "Markdown"}) });
    } else if (sub === "leaderboard") {
      if (c.scores.size === 0) { await fetch(`${BASE}/sendMessage`, { method: "POST", headers: {"Content-Type":"application/json"}, body: JSON.stringify({chat_id: chatId, text: "No invite scores yet.", reply_to_message_id: msg.message_id}) }); return; }
      const sorted = [...c.scores.values()].sort((a, b) => b.points - a.points).slice(0, 10);
      let txt = "🏆 Invite Contest Leaderboard:\n\n";
      sorted.forEach((e, i) => { txt += `${i + 1}. ${e.name} — ${e.points} invites\n`; });
      await fetch(`${BASE}/sendMessage`, { method: "POST", headers: {"Content-Type":"application/json"}, body: JSON.stringify({chat_id: chatId, text: txt, reply_to_message_id: msg.message_id}) });
    } else if (sub === "end") {
      if (!isMod(msg.from.id)) { await fetch(`${BASE}/sendMessage`, { method: "POST", headers: {"Content-Type":"application/json"}, body: JSON.stringify({chat_id: chatId, text: "⛔ Mods only.", reply_to_message_id: msg.message_id}) }); return; }
      c.active = false;
      const sorted = [...c.scores.values()].sort((a, b) => b.points - a.points);
      const top10 = sorted.slice(0, 10);

      // Award points based on placement
      const pointsAwarded = new Map();
      sorted.forEach((e, i) => {
        let pts = 0;
        if (i === 0) pts = 5000;       // 🥇 1st place
        else if (i === 1) pts = 2500;  // 🥈 2nd place
        else if (i === 2) pts = 1000;  // 🥉 3rd place
        else if (i < 10) pts = 500;    // Top 10
        else if (e.points > 0) pts = 100; // All participants with 1+ invite
        if (pts > 0) pointsAwarded.set(e.name, pts);
      });

      let txt = "🏆 Invite Contest OVER! Final standings:\n\n";
      if (top10.length > 0) {
        txt += "📊 *Results & Points Awarded:*\n\n";
        top10.forEach((e, i) => {
          const medal = ["🥇","🥈","🥉"][i] || `${i+1}.`;
          const pts = pointsAwarded.get(e.name) || 0;
          txt += `${medal} ${e.name} — ${e.points} invites → *+${pts} points*\n`;
        });
        const remaining = sorted.slice(10).filter(e => e.points > 0);
        if (remaining.length > 0) {
          txt += `\n...and ${remaining.length} more participants each earn *+100 points*\n`;
        }
        txt += "\n💰 *Prize Pool:*\n";
        txt += "🥇 1st: 5,000 pts | 🥈 2nd: 2,500 pts | 🥉 3rd: 1,000 pts\n";
        txt += "Top 10: 500 pts | All participants: 100 pts\n";
        txt += "\nPoints will be credited to your Send.it account! 🚀";
      } else {
        txt += "No invites tracked.";
      }
      await fetch(`${BASE}/sendMessage`, { method: "POST", headers: {"Content-Type":"application/json"}, body: JSON.stringify({chat_id: chatId, text: txt, parse_mode: "Markdown"}) });
    }
    return;
  }
}

// Raid system
const activeRaids = [];
const raidHistory = [];

// Raid commands
async function handleRaidCommand(msg, chatId, text) {
  const parts = text.split(" ");
  const sub = parts[1]?.toLowerCase();
  
  if (sub === "start" && parts[2]) {
    // Admins, mods, or raid leaders can start raids
    let canRaid = isMod(msg.from.id) || raidLeaders.has(msg.from.id);
    if (!canRaid) {
      const adminRes = await fetch(`${BASE}/getChatMember`, { method: "POST", headers: {"Content-Type":"application/json"}, body: JSON.stringify({chat_id: chatId, user_id: msg.from.id}) });
      const adminData = await adminRes.json();
      canRaid = adminData.ok && ["creator", "administrator"].includes(adminData.result?.status);
    }
    if (!canRaid) {
      await fetch(`${BASE}/sendMessage`, { method: "POST", headers: {"Content-Type":"application/json"}, body: JSON.stringify({chat_id: chatId, text: "⛔ Only admins, mods, or raid leaders can start raids.", reply_to_message_id: msg.message_id}) });
      return;
    }
    
    const url = parts[2];
    const duration = parseInt(parts[3]) || 15; // minutes, default 15
    const raid = {
      id: activeRaids.length + raidHistory.length + 1,
      url,
      startedBy: msg.from.first_name || "Admin",
      startedAt: Date.now(),
      endsAt: Date.now() + (duration * 60 * 1000),
      participants: new Set(),
      duration
    };
    activeRaids.push(raid);
    
    const actions = url.includes("twitter.com") || url.includes("x.com") 
      ? "Like ❤️, Retweet 🔄, Comment 💬" 
      : url.includes("t.me") 
      ? "Join & React 🔥" 
      : "Engage & Support 🚀";
    
    await fetch(`${BASE}/sendMessage`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        chat_id: chatId,
        text: `🚨 *RAID TIME\\!* 🚨\n\n🔗 Target: ${url.replace(/[_*[\]()~`>#+\-=|{}.!]/g, '\\$&')}\n\n📋 *Actions:* ${actions.replace(/[_*[\]()~`>#+\-=|{}.!]/g, '\\$&')}\n\n⏰ Duration: ${duration} minutes\n👤 Started by: ${raid.startedBy.replace(/[_*[\]()~`>#+\-=|{}.!]/g, '\\$&')}\n\nType /raids done when you've completed the raid\\!`,
        parse_mode: "MarkdownV2"
      })
    });
    
    // Auto-end raid after duration
    setTimeout(() => {
      const idx = activeRaids.findIndex(r => r.id === raid.id);
      if (idx !== -1) {
        const ended = activeRaids.splice(idx, 1)[0];
        ended.participantCount = ended.participants.size;
        raidHistory.push(ended);
        fetch(`${BASE}/sendMessage`, {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({
            chat_id: chatId,
            text: `✅ Raid #${ended.id} ended\\!\n\n🔗 ${ended.url.replace(/[_*[\]()~`>#+\-=|{}.!]/g, '\\$&')}\n👥 Participants: ${ended.participantCount}\n\nGreat work team\\! 🔥`,
            parse_mode: "MarkdownV2"
          })
        });
      }
    }, duration * 60 * 1000);
    
    console.log(`Raid #${raid.id} started by ${raid.startedBy}: ${url}`);
    
  } else if (sub === "done") {
    if (activeRaids.length === 0) {
      await fetch(`${BASE}/sendMessage`, { method: "POST", headers: {"Content-Type":"application/json"}, body: JSON.stringify({chat_id: chatId, text: "No active raids right now.", reply_to_message_id: msg.message_id}) });
    } else {
      const raid = activeRaids[activeRaids.length - 1];
      raid.participants.add(msg.from.id);
      // Increment raid contest score if active
      if (contests.raid.active) {
        const rc = contests.raid.scores;
        const existing = rc.get(msg.from.id);
        if (existing) { existing.points++; } else { rc.set(msg.from.id, { name: msg.from.first_name || "Anon", points: 1 }); }
      }
      await fetch(`${BASE}/sendMessage`, { method: "POST", headers: {"Content-Type":"application/json"}, body: JSON.stringify({chat_id: chatId, text: `✅ ${msg.from.first_name} completed the raid! (${raid.participants.size} total)`, reply_to_message_id: msg.message_id}) });
    }
    
  } else if (sub === "history") {
    if (raidHistory.length === 0) {
      await fetch(`${BASE}/sendMessage`, { method: "POST", headers: {"Content-Type":"application/json"}, body: JSON.stringify({chat_id: chatId, text: "No past raids yet.", reply_to_message_id: msg.message_id}) });
    } else {
      const last5 = raidHistory.slice(-5).reverse();
      const lines = last5.map(r => `#${r.id} — ${r.participantCount} raiders — ${r.url}`);
      await fetch(`${BASE}/sendMessage`, { method: "POST", headers: {"Content-Type":"application/json"}, body: JSON.stringify({chat_id: chatId, text: `📜 Recent Raids:\n\n${lines.join("\n")}`, reply_to_message_id: msg.message_id}) });
    }
    
  } else {
    // Show active raids or help
    if (activeRaids.length > 0) {
      const raid = activeRaids[activeRaids.length - 1];
      const minsLeft = Math.max(0, Math.round((raid.endsAt - Date.now()) / 60000));
      await fetch(`${BASE}/sendMessage`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          chat_id: chatId,
          text: `🚨 *Active Raid*\n\n🔗 ${raid.url.replace(/[_*[\]()~`>#+\-=|{}.!]/g, '\\$&')}\n⏰ ${minsLeft} min remaining\n👥 ${raid.participants.size} raiders\n\nType /raids done when complete\\!`,
          parse_mode: "MarkdownV2"
        })
      });
    } else {
      await fetch(`${BASE}/sendMessage`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          chat_id: chatId,
          text: `🚨 *Raid Commands*\n\n/raids — Show active raid\n/raids start \\<url\\> \\[minutes\\] — Start a raid\n/raids done — Mark raid as complete\n/raids history — Past raids\n\nExample: /raids start https://x\\.com/tweet 15`,
          parse_mode: "MarkdownV2"
        })
      });
    }
  }
}

// Whitelisted users (skip captcha)
const captchaWhitelist = new Set([6260568591]); // Shun

// Captcha system for new members
const pendingCaptcha = new Map(); // userId -> { chatId, msgId, answer, timeout, joinMsgId }

function generateCaptcha() {
  const a = Math.floor(Math.random() * 10) + 1;
  const b = Math.floor(Math.random() * 10) + 1;
  return { question: `${a} + ${b}`, answer: String(a + b) };
}

async function handleNewMember(msg) {
  const chatId = msg.chat.id;
  // Track invite contest — credit the user who added them (msg.from)
  if (contests.invite.active && msg.from) {
    for (const member of msg.new_chat_members || []) {
      if (member.is_bot || member.id === msg.from.id) continue;
      const ic = contests.invite.scores;
      const existing = ic.get(msg.from.id);
      if (existing) { existing.points++; } else { ic.set(msg.from.id, { name: msg.from.first_name || "Anon", points: 1 }); }
    }
  }
  for (const member of msg.new_chat_members || []) {
    if (member.is_bot || captchaWhitelist.has(member.id)) continue;
    
    const userId = member.id;
    const name = member.first_name || "New member";
    const captcha = generateCaptcha();
    
    // Send captcha challenge via DM to the user
    const dmRes = await fetch(`${BASE}/sendMessage`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        chat_id: userId,
        text: `👋 Welcome to Send.it!\n\n🔒 To verify you're human, solve this:\n\nWhat is ${captcha.question} ?\n\nReply here with the number within 3 minutes.`
      })
    });
    const dmData = await dmRes.json();
    const captchaMsgId = dmData.ok ? dmData.result.message_id : null;
    
    // If DM failed (user hasn't started bot), fall back to group message
    if (!dmData.ok) {
      console.error("DM captcha failed, falling back to group:", JSON.stringify(dmData));
      const threadId = msg.message_thread_id || undefined;
      const fallbackBody = {
        chat_id: chatId,
        text: `👋 Welcome ${name}!\n\n🔒 To verify you're human, solve this:\n\nWhat is ${captcha.question} ?\n\nJust type the number in chat within 3 minutes.`,
      };
      if (threadId) fallbackBody.message_thread_id = threadId;
      const fbRes = await fetch(`${BASE}/sendMessage`, { method: "POST", headers: {"Content-Type":"application/json"}, body: JSON.stringify(fallbackBody) });
      const fbData = await fbRes.json();
      if (fbData.ok) pendingCaptcha.set(userId, { chatId, msgId: fbData.result.message_id, answer: captcha.answer, timeout: null, threadId, isDm: false });
    }
    
    // Set timeout to kick if not solved in 3 minutes
    const timeout = setTimeout(async () => {
      if (pendingCaptcha.has(userId)) {
        pendingCaptcha.delete(userId);
        try {
          await fetch(`${BASE}/banChatMember`, {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ chat_id: chatId, user_id: userId, until_date: Math.floor(Date.now()/1000) + 60 })
          });
          if (captchaMsgId) {
            await fetch(`${BASE}/deleteMessage`, {
              method: "POST",
              headers: { "Content-Type": "application/json" },
              body: JSON.stringify({ chat_id: chatId, message_id: captchaMsgId })
            });
          }
          console.log(`Kicked ${name} (${userId}) - captcha timeout`);
        } catch (e) {}
      }
    }, 180000);
    
    pendingCaptcha.set(userId, { chatId, msgId: captchaMsgId, answer: captcha.answer, timeout, isDm: dmData.ok });
    console.log(`Captcha sent to ${name} (${userId}): ${captcha.question} = ${captcha.answer}`);
  }
}

async function checkCaptchaAnswer(msg) {
  const userId = msg.from.id;
  if (!pendingCaptcha.has(userId)) return false;
  
  const { chatId, msgId, answer, timeout, isDm } = pendingCaptcha.get(userId);
  const text = msg.text.trim();
  
  if (text === answer) {
    // Correct - unrestrict user
    pendingCaptcha.delete(userId);
    clearTimeout(timeout);
    
    // Restore permissions
    await fetch(`${BASE}/restrictChatMember`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        chat_id: chatId,
        user_id: userId,
        permissions: {
          can_send_messages: true,
          can_send_audios: false,
          can_send_documents: false,
          can_send_photos: true,
          can_send_videos: false,
          can_send_video_notes: false,
          can_send_voice_notes: false,
          can_send_polls: false,
          can_send_other_messages: true,
          can_add_web_page_previews: true,
          can_invite_users: true
        }
      })
    });
    
    // Clean up captcha messages
    try {
      if (isDm && msgId) {
        await fetch(`${BASE}/deleteMessage`, { method: "POST", headers: {"Content-Type":"application/json"}, body: JSON.stringify({chat_id: userId, message_id: msgId}) });
      } else if (msgId) {
        await fetch(`${BASE}/deleteMessage`, { method: "POST", headers: {"Content-Type":"application/json"}, body: JSON.stringify({chat_id: chatId, message_id: msgId}) });
        await fetch(`${BASE}/deleteMessage`, { method: "POST", headers: {"Content-Type":"application/json"}, body: JSON.stringify({chat_id: chatId, message_id: msg.message_id}) });
      }
    } catch (e) {}
    
    // Welcome in group
    const welcomeRes = await fetch(`${BASE}/sendMessage`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        chat_id: chatId,
        text: `✅ Verified! Welcome to Send.it, ${msg.from.first_name || "anon"}! 🚀\n\nType /filters to see available commands.`
      })
    });
    
    // Confirm in DM if that's where they answered
    if (isDm) {
      await fetch(`${BASE}/sendMessage`, { method: "POST", headers: {"Content-Type":"application/json"}, body: JSON.stringify({chat_id: userId, text: "✅ Verified! You now have access to the Send.it group."}) });
    }
    
    console.log(`${msg.from.first_name} (${userId}) passed captcha`);
    return true;
  } else {
    // Wrong answer - delete their message, let them try again
    try {
      await fetch(`${BASE}/deleteMessage`, { method: "POST", headers: {"Content-Type":"application/json"}, body: JSON.stringify({chat_id: chatId, message_id: msg.message_id}) });
    } catch (e) {}
    return true;
  }
}

let offset = 0;

async function poll() {
  try {
    const res = await fetch(`${BASE}/getUpdates?offset=${offset}&timeout=30`);
    const data = await res.json();
    
    if (!data.ok) return;
    
    for (const update of data.result) {
      offset = update.update_id + 1;
      const msg = update.message;
      if (!msg) continue;
      
      // Handle new members
      if (msg.new_chat_members && msg.new_chat_members.length > 0) {
        await handleNewMember(msg);
        continue;
      }
      
      if (!msg.text) continue;
      
      // Check captcha answers first
      if (pendingCaptcha.has(msg.from?.id)) {
        await checkCaptchaAnswer(msg);
        continue;
      }
      
      const rawText = msg.text.trim();
      // Strip @botname suffix from commands in groups (e.g. /modlist@SendItBot → /modlist)
      const text = rawText.replace(/^(\/\w+)@\w+/, '$1');
      const chatId = msg.chat.id;
      
      // Raid commands
      if (text.startsWith("/raids") || text === "/raid") {
        await handleRaidCommand(msg, chatId, text === "/raid" ? "/raids" : text);
        continue;
      }
      
      // Show user ID
      if (text.startsWith("/myid")) {
        await fetch(`${BASE}/sendMessage`, {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ chat_id: chatId, text: `Your Telegram ID: ${msg.from.id}`, reply_to_message_id: msg.message_id })
        });
        continue;
      }
      
      // Raid leader management (owner/mod)
      if (text.startsWith("/addraidleader") || text.startsWith("/removeraidleader") || text.startsWith("/raidleaders")) {
        const cmd = text.split(" ")[0].toLowerCase();
        
        if (cmd === "/raidleaders") {
          const names = [];
          for (const uid of raidLeaders) {
            try {
              const r = await fetch(`${BASE}/getChatMember`, { method: "POST", headers: {"Content-Type":"application/json"}, body: JSON.stringify({chat_id: chatId, user_id: uid}) });
              const d = await r.json();
              names.push(d.ok ? `• ${d.result.user.first_name} (${uid})` : `• ${uid}`);
            } catch (e) { names.push(`• ${uid}`); }
          }
          await fetch(`${BASE}/sendMessage`, { method: "POST", headers: {"Content-Type":"application/json"}, body: JSON.stringify({chat_id: chatId, text: names.length ? `⚔️ Raid Leaders:\n${names.join("\n")}` : "⚔️ No raid leaders set. Use /addraidleader (reply to user).", reply_to_message_id: msg.message_id}) });
        } else if (!isMod(msg.from.id)) {
          await fetch(`${BASE}/sendMessage`, { method: "POST", headers: {"Content-Type":"application/json"}, body: JSON.stringify({chat_id: chatId, text: "⛔ Only owner/mods can manage raid leaders.", reply_to_message_id: msg.message_id}) });
        } else if (msg.reply_to_message) {
          const targetId = msg.reply_to_message.from.id;
          const targetName = msg.reply_to_message.from.first_name || "User";
          if (cmd === "/addraidleader") {
            raidLeaders.add(targetId);
            await fetch(`${BASE}/sendMessage`, { method: "POST", headers: {"Content-Type":"application/json"}, body: JSON.stringify({chat_id: chatId, text: `⚔️ ${targetName} is now a raid leader!`}) });
            console.log(`Added raid leader: ${targetName} (${targetId})`);
          } else {
            raidLeaders.delete(targetId);
            await fetch(`${BASE}/sendMessage`, { method: "POST", headers: {"Content-Type":"application/json"}, body: JSON.stringify({chat_id: chatId, text: `❌ ${targetName} removed as raid leader.`}) });
          }
        } else {
          await fetch(`${BASE}/sendMessage`, { method: "POST", headers: {"Content-Type":"application/json"}, body: JSON.stringify({chat_id: chatId, text: "↩️ Reply to a user to add/remove them as raid leader.", reply_to_message_id: msg.message_id}) });
        }
        continue;
      }
      
      // Role commands (mod only) — promotes user as admin with custom title & minimal perms
      if (text.startsWith("/shiller") || text.startsWith("/unshiller") || text.startsWith("/fundraiser") || text.startsWith("/unfundraiser") || text.startsWith("/pm") || text.startsWith("/unpm") || text.startsWith("/raider") || text.startsWith("/unraider")) {
        const cmd = text.split(" ")[0].toLowerCase();
        let isAdmin = isMod(msg.from.id);
        if (!isAdmin) {
          try {
            const adminRes = await fetch(`${BASE}/getChatMember`, { method: "POST", headers: {"Content-Type":"application/json"}, body: JSON.stringify({chat_id: chatId, user_id: msg.from.id}) });
            const adminData = await adminRes.json();
            isAdmin = adminData.ok && ["creator", "administrator"].includes(adminData.result?.status);
          } catch (e) {}
        }
        if (!isAdmin) {
          await fetch(`${BASE}/sendMessage`, { method: "POST", headers: {"Content-Type":"application/json"}, body: JSON.stringify({chat_id: chatId, text: "⛔ Mods only.", reply_to_message_id: msg.message_id}) });
        } else if (!msg.reply_to_message) {
          await fetch(`${BASE}/sendMessage`, { method: "POST", headers: {"Content-Type":"application/json"}, body: JSON.stringify({chat_id: chatId, text: "↩️ Reply to a user to assign/remove Shiller role.", reply_to_message_id: msg.message_id}) });
        } else {
          const targetId = msg.reply_to_message.from.id;
          const targetName = msg.reply_to_message.from.first_name || "User";
          const roleTitle = cmd === "/shiller" ? "Shiller 📣" : cmd === "/fundraiser" ? "Fundraiser 💰" : cmd === "/pm" ? "Project Manager 📋" : cmd === "/raider" ? "Raider ⚔️" : null;
          const isAssign = cmd === "/shiller" || cmd === "/fundraiser" || cmd === "/pm" || cmd === "/raider";
          if (isAssign && roleTitle) {
            const res = await fetch(`${BASE}/promoteChatMember`, {
              method: "POST", headers: {"Content-Type":"application/json"},
              body: JSON.stringify({
                chat_id: chatId, user_id: targetId,
                can_manage_chat: false, can_delete_messages: false, can_manage_video_chats: false,
                can_restrict_members: false, can_promote_members: false, can_change_info: false,
                can_invite_users: true, can_pin_messages: false, can_post_stories: false,
                can_edit_stories: false, can_delete_stories: false
              })
            });
            if (res.ok) {
              await fetch(`${BASE}/setChatAdministratorCustomTitle`, {
                method: "POST", headers: {"Content-Type":"application/json"},
                body: JSON.stringify({ chat_id: chatId, user_id: targetId, custom_title: roleTitle })
              });
              await fetch(`${BASE}/sendMessage`, { method: "POST", headers: {"Content-Type":"application/json"}, body: JSON.stringify({chat_id: chatId, text: `${roleTitle} ${targetName} is now a ${roleTitle.split(" ")[0]}!`}) });
              console.log(`${roleTitle} role given to ${targetName} (${targetId})`);
            } else {
              await fetch(`${BASE}/sendMessage`, { method: "POST", headers: {"Content-Type":"application/json"}, body: JSON.stringify({chat_id: chatId, text: "❌ Failed — make sure the bot has promote permissions.", reply_to_message_id: msg.message_id}) });
            }
          } else {
            // /unshiller or /unfundraiser — demote back to regular member
            await fetch(`${BASE}/promoteChatMember`, {
              method: "POST", headers: {"Content-Type":"application/json"},
              body: JSON.stringify({
                chat_id: chatId, user_id: targetId,
                can_manage_chat: false, can_delete_messages: false, can_manage_video_chats: false,
                can_restrict_members: false, can_promote_members: false, can_change_info: false,
                can_invite_users: false, can_pin_messages: false, can_post_stories: false,
                can_edit_stories: false, can_delete_stories: false
              })
            });
            await fetch(`${BASE}/sendMessage`, { method: "POST", headers: {"Content-Type":"application/json"}, body: JSON.stringify({chat_id: chatId, text: `❌ ${targetName} is no longer a Shiller.`}) });
            console.log(`Shiller role removed from ${targetName} (${targetId})`);
          }
        }
        continue;
      }
      
      // Mod management (owner only)
      if (text.startsWith("/addmod") || text.startsWith("/removemod") || text.startsWith("/modlist")) {
        const cmd = text.split(" ")[0].toLowerCase();
        
        if (cmd === "/modlist") {
          const modNames = [];
          for (const modId of botMods) {
            try {
              const r = await fetch(`${BASE}/getChatMember`, { method: "POST", headers: {"Content-Type":"application/json"}, body: JSON.stringify({chat_id: chatId, user_id: modId}) });
              const d = await r.json();
              modNames.push(d.ok ? `• ${d.result.user.first_name} (${modId})` : `• ${modId}`);
            } catch (e) { modNames.push(`• ${modId}`); }
          }
          await fetch(`${BASE}/sendMessage`, {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ chat_id: chatId, text: modNames.length ? `🛡️ Bot Moderators:\n${modNames.join("\n")}` : "🛡️ No bot moderators set. Use /addmod (reply to user) to add one.", reply_to_message_id: msg.message_id })
          });
        } else if (!isOwner(msg.from.id)) {
          await fetch(`${BASE}/sendMessage`, {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ chat_id: chatId, text: "⛔ Owner only command.", reply_to_message_id: msg.message_id })
          });
        } else if (msg.reply_to_message) {
          const targetId = msg.reply_to_message.from.id;
          const targetName = msg.reply_to_message.from.first_name || "User";
          if (cmd === "/addmod") {
            botMods.add(targetId);
            await fetch(`${BASE}/sendMessage`, { method: "POST", headers: {"Content-Type":"application/json"}, body: JSON.stringify({chat_id: chatId, text: `✅ ${targetName} is now a bot moderator.`}) });
            console.log(`Added mod: ${targetName} (${targetId})`);
          } else if (cmd === "/removemod") {
            botMods.delete(targetId);
            await fetch(`${BASE}/sendMessage`, { method: "POST", headers: {"Content-Type":"application/json"}, body: JSON.stringify({chat_id: chatId, text: `❌ ${targetName} removed as bot moderator.`}) });
            console.log(`Removed mod: ${targetName} (${targetId})`);
          }
        } else {
          await fetch(`${BASE}/sendMessage`, { method: "POST", headers: {"Content-Type":"application/json"}, body: JSON.stringify({chat_id: chatId, text: "↩️ Reply to a user to add/remove them as mod.", reply_to_message_id: msg.message_id}) });
        }
        continue;
      }
      
      // Mod commands (admin or bot mod)
      if (text.startsWith("/warn") || text.startsWith("/mute") || text.startsWith("/unmute") || text.startsWith("/ban") || text.startsWith("/unban")) {
        // Check if sender is admin or bot mod
        let isAdmin = isMod(msg.from.id);
        if (!isAdmin) {
          try {
            const adminRes = await fetch(`${BASE}/getChatMember`, {
              method: "POST",
              headers: { "Content-Type": "application/json" },
              body: JSON.stringify({ chat_id: chatId, user_id: msg.from.id })
            });
            const adminData = await adminRes.json();
            isAdmin = adminData.ok && ["creator", "administrator"].includes(adminData.result?.status);
          } catch (e) {}
        }
        
        try {
          if (!isAdmin) {
            await fetch(`${BASE}/sendMessage`, {
              method: "POST",
              headers: { "Content-Type": "application/json" },
              body: JSON.stringify({ chat_id: chatId, text: "⛔ Admin only command.", reply_to_message_id: msg.message_id })
            });
          } else if (msg.reply_to_message) {
            const targetUser = msg.reply_to_message.from;
            const targetId = targetUser.id;
            const targetName = targetUser.first_name || "User";
            const cmd = text.split(" ")[0].toLowerCase();
            
            if (cmd === "/warn") {
              await fetch(`${BASE}/sendMessage`, {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({ chat_id: chatId, text: `⚠️ ${targetName} has been warned. Next offense = mute.`, reply_to_message_id: msg.reply_to_message.message_id })
              });
            } else if (cmd === "/mute") {
              const duration = parseInt(text.split(" ")[1]) || 60; // minutes, default 1hr
              await fetch(`${BASE}/restrictChatMember`, {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({
                  chat_id: chatId, user_id: targetId,
                  until_date: Math.floor(Date.now()/1000) + (duration * 60),
                  permissions: { can_send_messages: false, can_send_audios: false, can_send_documents: false, can_send_photos: false, can_send_videos: false, can_send_video_notes: false, can_send_voice_notes: false, can_send_polls: false, can_send_other_messages: false, can_add_web_page_previews: false }
                })
              });
              await fetch(`${BASE}/sendMessage`, {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({ chat_id: chatId, text: `🔇 ${targetName} muted for ${duration} minutes.` })
              });
            } else if (cmd === "/unmute") {
              await fetch(`${BASE}/restrictChatMember`, {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({
                  chat_id: chatId, user_id: targetId,
                  permissions: { can_send_messages: true, can_send_photos: true, can_send_other_messages: true, can_add_web_page_previews: true, can_invite_users: true }
                })
              });
              await fetch(`${BASE}/sendMessage`, {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({ chat_id: chatId, text: `🔊 ${targetName} has been unmuted.` })
              });
            } else if (cmd === "/ban") {
              await fetch(`${BASE}/banChatMember`, {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({ chat_id: chatId, user_id: targetId })
              });
              await fetch(`${BASE}/sendMessage`, {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({ chat_id: chatId, text: `🚫 ${targetName} has been banned.` })
              });
            } else if (cmd === "/unban") {
              await fetch(`${BASE}/unbanChatMember`, {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({ chat_id: chatId, user_id: targetId, only_if_banned: true })
              });
              await fetch(`${BASE}/sendMessage`, {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({ chat_id: chatId, text: `✅ ${targetName} has been unbanned.` })
              });
            }
            console.log(`Mod action: ${cmd} on ${targetName} by ${msg.from.username || msg.from.id}`);
          } else {
            await fetch(`${BASE}/sendMessage`, {
              method: "POST",
              headers: { "Content-Type": "application/json" },
              body: JSON.stringify({ chat_id: chatId, text: "↩️ Reply to a message to use mod commands.", reply_to_message_id: msg.message_id })
            });
          }
        } catch (e) { console.error("Mod error:", e.message); }
        continue;
      }
      
      // Contest commands
      if (text.startsWith("/contest")) {
        await handleContestCommand(msg, chatId, text);
        continue;
      }
      
      // Check for spam
      if (SPAM_PATTERNS.some(p => p.test(text))) {
        try {
          await fetch(`${BASE}/deleteMessage`, {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ chat_id: chatId, message_id: msg.message_id })
          });
          console.log(`Deleted spam from ${msg.from?.username || msg.from?.id}`);
        } catch (e) {}
        continue;
      }
      
      // Check for commands
      const cmd = text.split("@")[0].split(" ")[0].toLowerCase();
      if (responses[cmd]) {
        await fetch(`${BASE}/sendMessage`, {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({
            chat_id: chatId,
            text: responses[cmd],
            parse_mode: "MarkdownV2",
            disable_web_page_preview: true,
            reply_to_message_id: msg.message_id
          })
        });
        console.log(`Replied to ${cmd} from ${msg.from?.username || msg.from?.id}`);
      }
    }
  } catch (err) {
    console.error("Poll error:", err.message);
  }
}

console.log("🤖 Send.it Bot running with filters...");
setInterval(poll, 1000);
poll();
