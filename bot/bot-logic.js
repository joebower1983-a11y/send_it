/**
 * Shared bot logic — extracted from filters.js for use by both
 * the long-polling dev server and the Vercel webhook handler.
 *
 * LIMITATION: All in-memory state (pendingCaptcha, contests, activeRaids,
 * botMods, raidLeaders) resets on every serverless invocation. Stateful
 * features (captcha timeouts, active raids, contest scores) won't persist
 * across requests in serverless mode. A database (e.g. Upstash Redis)
 * would be needed for full functionality.
 */

const points = require("./points");

const BOT_TOKEN = process.env.TELEGRAM_BOT_TOKEN || "8562369283:AAEG2hfV6vOCzSwcxEmpHtVBYxRxBYS_ejI";
const BASE = `https://api.telegram.org/bot${BOT_TOKEN}`;

const MINT = "F8qWTN8JfyDCvj4RoCHuvNMVbTV9XQksLuziA8PYpump";

const responses = {
  "/price": `📊 *SENDIT Token*\n\n• Mint: \`${MINT}\`\n• Chain: Solana\n• Platform: Pump\\.fun\n\n[View on Pump\\.fun](https://pump.fun/coin/${MINT})\n[View on DexScreener](https://dexscreener.com/solana/${MINT})`,
  "/links": `🔗 *Official Links*\n\n🟢 [Pump\\.fun Token](https://pump.fun/coin/${MINT})\n📦 [GitHub](https://github.com/joebower1983-a11y/send_it)\n🌐 [Live Demo](https://senditsolana.io)\n💬 [Discord](https://discord.gg/vKRTyG85)\n🐦 [Twitter](https://twitter.com/SendItSolana420)\n📱 [Telegram](https://t.me/+Xw4E2sJ0Z3Q5ZDYx)`,
  "/tokeninfo": `💰 *SENDIT Token Info*\n\n• Name: Send It\n• Ticker: SENDIT\n• Chain: Solana\n• Mint: \`${MINT}\`\n\n*Fee Structure \\(launchpad\\):*\n• 1% platform fee → treasury\n• 1% creator fee → token creators\n• Holder rewards → redistributed\n\n*Modules:* 29 on\\-chain \\| 13k\\+ lines of Rust`,
  "/rules": `📜 *Group Rules*\n\n1️⃣ Be respectful\n2️⃣ No scams, phishing, or unsolicited DMs\n3️⃣ No shilling other projects\n4️⃣ Nothing here is financial advice — DYOR\n5️⃣ English only\n6️⃣ No spam\n7️⃣ Have fun and send it\\! 🚀\n\n_Breaking rules \\= warn → mute → ban_`,
  "/website": `🌐 *Send\\.it Website*\n\n• Main: [send\\-it\\-seven\\-sigma\\.vercel\\.app](https://senditsolana.io)\n• GitHub Pages: [joebower1983\\-a11y\\.github\\.io/send\\_it](https://joebower1983-a11y.github.io/send_it/)\n• Pitch Deck: [View](https://joebower1983-a11y.github.io/send_it/pitch-deck.html)`,
  "/chart": `📈 *SENDIT Chart*\n\n[DexScreener](https://dexscreener.com/solana/${MINT})\n[Pump\\.fun](https://pump.fun/coin/${MINT})\n[Birdeye](https://birdeye.so/token/${MINT}?chain=solana)`,
  "/buy": `🛒 *How to Buy SENDIT*\n\n1\\. Get a Solana wallet \\(Phantom, Solflare\\)\n2\\. Fund it with SOL\n3\\. Go to [Pump\\.fun](https://pump.fun/coin/${MINT})\n4\\. Connect wallet and buy\\!\n\n⚠️ _DYOR \\- This is not financial advice_`,
  "/socials": `📱 *Send\\.it Socials*\n\n🐦 Twitter: [@SendItSolana420](https://twitter.com/SendItSolana420)\n💬 Discord: [discord\\.gg/vKRTyG85](https://discord.gg/vKRTyG85)\n📱 Telegram: [Join Group](https://t.me/+Xw4E2sJ0Z3Q5ZDYx)\n📦 GitHub: [send\\_it](https://github.com/joebower1983-a11y/send_it)`,
  "/whitepaper": `📄 *Send\\.it Whitepaper v2\\.0*\n\nRead the full whitepaper covering all 29 modules:\n[View on GitHub](https://github.com/joebower1983-a11y/send_it/blob/main/docs/WHITEPAPER.md)`,
  "/ca": `📋 *Contract Address*\n\n\`F8qWTN8JfyDCvj4RoCHuvNMVbTV9XQksLuziA8PYpump\`\n\n[Buy on Pump\\.fun](https://pump.fun/coin/F8qWTN8JfyDCvj4RoCHuvNMVbTV9XQksLuziA8PYpump)`,
  "/filters": "🤖 <b>Bot Commands</b>\n\n📊 /price — Token price &amp; stats\n📋 /ca — Contract address\n🔗 /links — Official links\n💰 /tokeninfo — Contract &amp; fee info\n📜 /rules — Group rules\n🌐 /website — Send.it website\n📈 /chart — Price charts\n🛒 /buy — How to buy SENDIT\n📱 /socials — Social media links\n📄 /whitepaper — Read the whitepaper\n🗺️ /roadmap — Project roadmap\n🟢 /devnet — Devnet program status\n🚨 /raids — Raid coordinator\n📣 /shill — Copy-paste shill message\n🤖 /filters — This list\n\n🏆 <b>Points System:</b>\n/checkin — Daily check-in (+5 pts)\n/points — Check your balance\n/leaderboard — Top 10 holders\n\n<i>Earn points:</i>\n• Daily check-in: 5 pts\n• First message of the day: 2 pts\n• Invite a member: 25 pts\n• Mod award: 15 pts\n• Bug report: 50 pts\n\n🛡️ <b>Mod Commands (admin/mod only):</b>\n/warn — Warn a user (reply)\n/mute [min] — Mute user (reply, default 60min)\n/unmute — Unmute user (reply)\n/ban — Ban user (reply)\n/unban — Unban user (reply)\n/award — Award 15 pts (reply)\n/bugreport — Award 50 pts for bug report (reply)\n\n⚔️ <b>Raid Leader Commands (mod/owner):</b>\n/addraidleader — Add raid leader (reply)\n/removeraidleader — Remove raid leader (reply)\n/raidleaders — List raid leaders\n\n📣 <b>Roles (mod):</b>\n/shiller — Give Shiller 📣 badge (reply)\n/unshiller — Remove Shiller badge (reply)\n/fundraiser — Give Fundraiser 💰 badge (reply)\n/unfundraiser — Remove Fundraiser badge (reply)\n/pm — Give Project Manager 📋 badge (reply)\n/unpm — Remove Project Manager badge (reply)\n/raider — Give Raider ⚔️ badge (reply)\n/unraider — Remove Raider badge (reply)\n/communitylead — Give Community Lead 🌟 badge (reply)\n/uncommunitylead — Remove Community Lead badge (reply)\n\n👑 <b>Owner Commands:</b>\n/addmod — Add bot moderator (reply)\n/removemod — Remove bot moderator (reply)\n/modlist — List all bot moderators\n\n🏆 <b>Contest Commands:</b>\n/contest — Show active contests &amp; help\n/contest shill start|enter|entries|end\n/contest raid start|leaderboard|end\n/contest meme start|enter|entries|end\n/contest invite start|leaderboard|end\n/contest end all — End all contests (mod)",
  "/shill": `📣 *Copy \\& paste this everywhere:*\n\n\\-\\-\\-\\-\\-\\-\\-\\-\\-\\-\\-\\-\\-\\-\\-\\-\\-\\-\\-\\-\n\n🚀 Send\\.it — The fairest token launchpad on Solana\n\n✅ No insiders \\| No presales \\| Anti\\-snipe\n✅ 29 on\\-chain modules \\| 13k\\+ lines of Rust\n✅ Auto Raydium migration\n✅ Creator rewards \\+ holder rewards\n\n📋 CA: F8qWTN8JfyDCvj4RoCHuvNMVbTV9XQksLuziA8PYpump\n\n🐦 twitter\\.com/SendItSolana420\n💬 t\\.me/\\+Xw4E2sJ0Z3Q5ZDYx\n💎 discord\\.gg/vKRTyG85\n📈 pump\\.fun/coin/F8qWTN8JfyDCvj4RoCHuvNMVbTV9XQksLuziA8PYpump\n\n\\-\\-\\-\\-\\-\\-\\-\\-\\-\\-\\-\\-\\-\\-\\-\\-\\-\\-\\-\\-\n\nSend it\\! 🔥`,
  "/devnet": `🟢 *Send\\.it Devnet Status*\n\n*Program:* \`98Vxqk2dHjLsUb4svNaZWwVZxt9DZZwkRQZZNQmYRm1L\`\n[View on Solscan](https://solscan.io/account/98Vxqk2dHjLsUb4svNaZWwVZxt9DZZwkRQZZNQmYRm1L?cluster=devnet)\n\n*Verified Instructions:*\n✅ initialize\\_platform — config \\+ fees\n✅ create\\_token — bonding curve launch\n✅ buy — bonding curve pricing \\+ fee split\n✅ sell — reverse curve \\+ SOL refund\n\n*SENDIT Token:* \`F8qWTN8JfyDCvj4RoCHuvNMVbTV9XQksLuziA8PYpump\`\n\n*Stats:*\n• 31 on\\-chain modules \\(16k lines Rust\\)\n• 5IVE DSL port \\(6k lines, 25KB bytecode\\)\n• Full DeFi loop tested on\\-chain\n\n🔜 Full 31\\-module deploy coming soon`,
  "/roadmap": `🗺️ *Send\\.it Roadmap*\n\n*Q1 2026* ← WE ARE HERE\n• Core program \\+ community building\n• Token launch on Pump\\.fun ✅\n• Grant applications ✅\n\n*Q2 2026*\n• Mainnet deployment\n• First token launches\n• Mobile PWA\n\n*Q3 2026*\n• DeFi suite live \\(staking, lending, perps\\)\n• Solana dApp Store\n\n*Q4 2026*\n• Cross\\-chain bridge\n• DAO governance\n• Ecosystem partnerships`
};

const SPAM_PATTERNS = [
  /airdrop.*claim/i,
  /connect.*wallet.*verify/i,
  /dm me for/i,
  /send \d+ sol/i,
  /free (nft|token|sol|crypto)/i,
  /t\.me\/(?!.*SendIt)/i,
  /bit\.ly|tinyurl/i,
];

// Hardcoded defaults persist across Vercel cold starts
const botMods = new Set([
  6541770845,  // ZED⚡️
  6312896742,  // Crypto
  6260568591,  // Shuñ2.0🗿
]);
const raidLeaders = new Set();
const OWNER_IDS = [7920028061];
const contests = {
  shill: { active: false, entries: new Map(), endsAt: null, chatId: null },
  raid: { active: false, scores: new Map(), endsAt: null, chatId: null },
  meme: { active: false, entries: [], endsAt: null, chatId: null },
  invite: { active: false, scores: new Map(), endsAt: null, chatId: null }
};
const activeRaids = [];
const raidHistory = [];
const captchaWhitelist = new Set([6260568591]);
const pendingCaptcha = new Map();

function isOwner(userId) { return OWNER_IDS.includes(userId); }
function isMod(userId) { return botMods.has(userId) || isOwner(userId); }

function generateCaptcha() {
  const a = Math.floor(Math.random() * 10) + 1;
  const b = Math.floor(Math.random() * 10) + 1;
  return { question: `${a} + ${b}`, answer: String(a + b) };
}

async function tgApi(method, body) {
  const res = await fetch(`${BASE}/${method}`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body)
  });
  return res.json();
}

async function handleContestCommand(msg, chatId, text) {
  const parts = text.split(/\s+/);
  const type = parts[1]?.toLowerCase();
  const sub = parts[2]?.toLowerCase();

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
    txt += "\n*Commands:*\n/contest shill start\|enter\|entries\|end\n/contest raid start\|leaderboard\|end\n/contest meme start\|enter\|entries\|end\n/contest invite start\|leaderboard\|end\n/contest end all — end all contests";
    await tgApi("sendMessage", { chat_id: chatId, text: txt, reply_to_message_id: msg.message_id });
    return;
  }

  if (type === "end" && sub === "all") {
    if (!isMod(msg.from.id)) { await tgApi("sendMessage", { chat_id: chatId, text: "⛔ Mods only.", reply_to_message_id: msg.message_id }); return; }
    for (const c of Object.values(contests)) { c.active = false; c.endsAt = null; }
    await tgApi("sendMessage", { chat_id: chatId, text: "🏆 All contests ended." });
    return;
  }

  if (type === "shill") {
    const c = contests.shill;
    if (sub === "start") {
      if (!isMod(msg.from.id)) { await tgApi("sendMessage", { chat_id: chatId, text: "⛔ Mods only.", reply_to_message_id: msg.message_id }); return; }
      const hours = parseInt(parts[3]) || 72;
      c.active = true; c.entries = new Map(); c.endsAt = Date.now() + hours * 3600000; c.chatId = chatId;
      await tgApi("sendMessage", { chat_id: chatId, text: `🏆 Shill Contest started! ${hours}h to go.\n\nSubmit your tweet with:\n/contest shill enter <url>` });
    } else if (sub === "enter") {
      if (!c.active) { await tgApi("sendMessage", { chat_id: chatId, text: "No active shill contest.", reply_to_message_id: msg.message_id }); return; }
      const url = parts[3];
      if (!url) { await tgApi("sendMessage", { chat_id: chatId, text: "Usage: /contest shill enter <tweet_url>", reply_to_message_id: msg.message_id }); return; }
      c.entries.set(msg.from.id, { url, name: msg.from.first_name || "Anon", timestamp: Date.now() });
      await tgApi("sendMessage", { chat_id: chatId, text: `✅ Entry recorded! (${c.entries.size} total entries)`, reply_to_message_id: msg.message_id });
    } else if (sub === "entries") {
      if (!c.active && c.entries.size === 0) { await tgApi("sendMessage", { chat_id: chatId, text: "No shill contest entries.", reply_to_message_id: msg.message_id }); return; }
      let txt = `📣 Shill Contest Entries (${c.entries.size}):\n\n`;
      let i = 1;
      for (const [, e] of c.entries) { txt += `${i++}. ${e.name} — ${e.url}\n`; }
      await tgApi("sendMessage", { chat_id: chatId, text: txt, reply_to_message_id: msg.message_id });
    } else if (sub === "end") {
      if (!isMod(msg.from.id)) { await tgApi("sendMessage", { chat_id: chatId, text: "⛔ Mods only.", reply_to_message_id: msg.message_id }); return; }
      c.active = false;
      let txt = `🏆 Shill Contest ended! ${c.entries.size} entries:\n\n`;
      let i = 1;
      for (const [, e] of c.entries) { txt += `${i++}. ${e.name} — ${e.url}\n`; }
      txt += "\nMods: judge entries and announce the winner!";
      await tgApi("sendMessage", { chat_id: chatId, text: txt });
    }
    return;
  }

  if (type === "raid") {
    const c = contests.raid;
    if (sub === "start") {
      if (!isMod(msg.from.id)) { await tgApi("sendMessage", { chat_id: chatId, text: "⛔ Mods only.", reply_to_message_id: msg.message_id }); return; }
      const days = parseInt(parts[3]) || 7;
      c.active = true; c.scores = new Map(); c.endsAt = Date.now() + days * 86400000; c.chatId = chatId;
      await tgApi("sendMessage", { chat_id: chatId, text: `🏆 Raid Contest started! ${days} days to go.\n\nComplete raids with /raids done to earn points!` });
    } else if (sub === "leaderboard") {
      if (c.scores.size === 0) { await tgApi("sendMessage", { chat_id: chatId, text: "No raid contest scores yet.", reply_to_message_id: msg.message_id }); return; }
      const sorted = [...c.scores.values()].sort((a, b) => b.points - a.points).slice(0, 10);
      let txt = "🏆 Raid Contest Leaderboard:\n\n";
      sorted.forEach((e, i) => { txt += `${i + 1}. ${e.name} — ${e.points} pts\n`; });
      await tgApi("sendMessage", { chat_id: chatId, text: txt, reply_to_message_id: msg.message_id });
    } else if (sub === "end") {
      if (!isMod(msg.from.id)) { await tgApi("sendMessage", { chat_id: chatId, text: "⛔ Mods only.", reply_to_message_id: msg.message_id }); return; }
      c.active = false;
      const sorted = [...c.scores.values()].sort((a, b) => b.points - a.points).slice(0, 10);
      let txt = "🏆 Raid Contest OVER! Final standings:\n\n";
      sorted.forEach((e, i) => { txt += `${["🥇","🥈","🥉"][i] || `${i+1}.`} ${e.name} — ${e.points} pts\n`; });
      if (sorted.length === 0) txt += "No participants.";
      await tgApi("sendMessage", { chat_id: chatId, text: txt });
    }
    return;
  }

  if (type === "meme") {
    const c = contests.meme;
    if (sub === "start") {
      if (!isMod(msg.from.id)) { await tgApi("sendMessage", { chat_id: chatId, text: "⛔ Mods only.", reply_to_message_id: msg.message_id }); return; }
      const hours = parseInt(parts[3]) || 72;
      c.active = true; c.entries = []; c.endsAt = Date.now() + hours * 3600000; c.chatId = chatId;
      await tgApi("sendMessage", { chat_id: chatId, text: `🏆 Meme Contest started! ${hours}h to go.\n\nReply to a photo/image with:\n/contest meme enter` });
    } else if (sub === "enter") {
      if (!c.active) { await tgApi("sendMessage", { chat_id: chatId, text: "No active meme contest.", reply_to_message_id: msg.message_id }); return; }
      const reply = msg.reply_to_message;
      if (!reply || (!reply.photo && !reply.document)) { await tgApi("sendMessage", { chat_id: chatId, text: "↩️ Reply to a photo/image to enter!", reply_to_message_id: msg.message_id }); return; }
      c.entries.push({ userId: msg.from.id, name: msg.from.first_name || "Anon", messageId: reply.message_id, timestamp: Date.now() });
      await tgApi("sendMessage", { chat_id: chatId, text: `✅ Meme entry recorded! (${c.entries.length} total)`, reply_to_message_id: msg.message_id });
    } else if (sub === "entries") {
      await tgApi("sendMessage", { chat_id: chatId, text: `🎨 Meme Contest: ${c.entries.length} entries so far.`, reply_to_message_id: msg.message_id });
    } else if (sub === "end") {
      if (!isMod(msg.from.id)) { await tgApi("sendMessage", { chat_id: chatId, text: "⛔ Mods only.", reply_to_message_id: msg.message_id }); return; }
      c.active = false;
      let txt = `🏆 Meme Contest ended! ${c.entries.length} entries:\n\n`;
      c.entries.forEach((e, i) => { txt += `${i + 1}. ${e.name}\n`; });
      txt += "\nMods: judge entries and announce the winner!";
      await tgApi("sendMessage", { chat_id: chatId, text: txt });
    }
    return;
  }

  if (type === "invite") {
    const c = contests.invite;
    if (sub === "start") {
      if (!isMod(msg.from.id)) { await tgApi("sendMessage", { chat_id: chatId, text: "⛔ Mods only.", reply_to_message_id: msg.message_id }); return; }
      const days = parseInt(parts[3]) || 7;
      c.active = true; c.scores = new Map(); c.endsAt = Date.now() + days * 86400000; c.chatId = chatId;
      await tgApi("sendMessage", { chat_id: chatId, text: `🏆 Invite Contest started! ${days} days to go.\n\nInvite friends — they'll be tracked automatically!` });
    } else if (sub === "leaderboard") {
      if (c.scores.size === 0) { await tgApi("sendMessage", { chat_id: chatId, text: "No invite scores yet.", reply_to_message_id: msg.message_id }); return; }
      const sorted = [...c.scores.values()].sort((a, b) => b.points - a.points).slice(0, 10);
      let txt = "🏆 Invite Contest Leaderboard:\n\n";
      sorted.forEach((e, i) => { txt += `${i + 1}. ${e.name} — ${e.points} invites\n`; });
      await tgApi("sendMessage", { chat_id: chatId, text: txt, reply_to_message_id: msg.message_id });
    } else if (sub === "end") {
      if (!isMod(msg.from.id)) { await tgApi("sendMessage", { chat_id: chatId, text: "⛔ Mods only.", reply_to_message_id: msg.message_id }); return; }
      c.active = false;
      const sorted = [...c.scores.values()].sort((a, b) => b.points - a.points).slice(0, 10);
      let txt = "🏆 Invite Contest OVER! Final standings:\n\n";
      sorted.forEach((e, i) => { txt += `${["🥇","🥈","🥉"][i] || `${i+1}.`} ${e.name} — ${e.points} invites\n`; });
      if (sorted.length === 0) txt += "No invites tracked.";
      await tgApi("sendMessage", { chat_id: chatId, text: txt });
    }
    return;
  }
}

async function handleRaidCommand(msg, chatId, text) {
  const parts = text.split(" ");
  const sub = parts[1]?.toLowerCase();

  if (sub === "start" && parts[2]) {
    let canRaid = isMod(msg.from.id) || raidLeaders.has(msg.from.id);
    if (!canRaid) {
      const adminData = await tgApi("getChatMember", { chat_id: chatId, user_id: msg.from.id });
      canRaid = adminData.ok && ["creator", "administrator"].includes(adminData.result?.status);
    }
    if (!canRaid) {
      await tgApi("sendMessage", { chat_id: chatId, text: "⛔ Only admins, mods, or raid leaders can start raids.", reply_to_message_id: msg.message_id });
      return;
    }
    const url = parts[2];
    const duration = parseInt(parts[3]) || 15;
    const raid = {
      id: activeRaids.length + raidHistory.length + 1,
      url, startedBy: msg.from.first_name || "Admin",
      startedAt: Date.now(), endsAt: Date.now() + (duration * 60 * 1000),
      participants: new Set(), duration
    };
    activeRaids.push(raid);
    const actions = url.includes("twitter.com") || url.includes("x.com")
      ? "Like ❤️, Retweet 🔄, Comment 💬"
      : url.includes("t.me") ? "Join & React 🔥" : "Engage & Support 🚀";
    await tgApi("sendMessage", {
      chat_id: chatId,
      text: `🚨 *RAID TIME\\!* 🚨\n\n🔗 Target: ${url.replace(/[_*[\]()~`>#+\-=|{}.!]/g, '\\$&')}\n\n📋 *Actions:* ${actions.replace(/[_*[\]()~`>#+\-=|{}.!]/g, '\\$&')}\n\n⏰ Duration: ${duration} minutes\n👤 Started by: ${raid.startedBy.replace(/[_*[\]()~`>#+\-=|{}.!]/g, '\\$&')}\n\nType /raids done when you've completed the raid\\!`,
      parse_mode: "MarkdownV2"
    });
    // NOTE: setTimeout won't work in serverless — raid auto-end is polling-only
  } else if (sub === "done") {
    if (activeRaids.length === 0) {
      await tgApi("sendMessage", { chat_id: chatId, text: "No active raids right now.", reply_to_message_id: msg.message_id });
    } else {
      const raid = activeRaids[activeRaids.length - 1];
      raid.participants.add(msg.from.id);
      if (contests.raid.active) {
        const rc = contests.raid.scores;
        const existing = rc.get(msg.from.id);
        if (existing) { existing.points++; } else { rc.set(msg.from.id, { name: msg.from.first_name || "Anon", points: 1 }); }
      }
      await tgApi("sendMessage", { chat_id: chatId, text: `✅ ${msg.from.first_name} completed the raid! (${raid.participants.size} total)`, reply_to_message_id: msg.message_id });
    }
  } else if (sub === "history") {
    if (raidHistory.length === 0) {
      await tgApi("sendMessage", { chat_id: chatId, text: "No past raids yet.", reply_to_message_id: msg.message_id });
    } else {
      const last5 = raidHistory.slice(-5).reverse();
      const lines = last5.map(r => `#${r.id} — ${r.participantCount} raiders — ${r.url}`);
      await tgApi("sendMessage", { chat_id: chatId, text: `📜 Recent Raids:\n\n${lines.join("\n")}`, reply_to_message_id: msg.message_id });
    }
  } else {
    if (activeRaids.length > 0) {
      const raid = activeRaids[activeRaids.length - 1];
      const minsLeft = Math.max(0, Math.round((raid.endsAt - Date.now()) / 60000));
      await tgApi("sendMessage", {
        chat_id: chatId,
        text: `🚨 *Active Raid*\n\n🔗 ${raid.url.replace(/[_*[\]()~`>#+\-=|{}.!]/g, '\\$&')}\n⏰ ${minsLeft} min remaining\n👥 ${raid.participants.size} raiders\n\nType /raids done when complete\\!`,
        parse_mode: "MarkdownV2"
      });
    } else {
      await tgApi("sendMessage", {
        chat_id: chatId,
        text: `🚨 *Raid Commands*\n\n/raids — Show active raid\n/raids start \\<url\\> \\[minutes\\] — Start a raid\n/raids done — Mark raid as complete\n/raids history — Past raids\n\nExample: /raids start https://x\\.com/tweet 15`,
        parse_mode: "MarkdownV2"
      });
    }
  }
}

async function handleNewMember(msg) {
  const chatId = msg.chat.id;
  // Invite points (always) + contest tracking
  if (msg.from) {
    for (const member of msg.new_chat_members || []) {
      if (member.is_bot || member.id === msg.from.id) continue;
      // Award 25 pts for invite
      try { await points.inviteReward(msg.from.id, msg.from.first_name || "Anon"); } catch (e) {}
      // Contest tracking
      if (contests.invite.active) {
        const ic = contests.invite.scores;
        const existing = ic.get(msg.from.id);
        if (existing) { existing.points++; } else { ic.set(msg.from.id, { name: msg.from.first_name || "Anon", points: 1 }); }
      }
    }
  }
  for (const member of msg.new_chat_members || []) {
    if (member.is_bot || captchaWhitelist.has(member.id)) continue;
    const userId = member.id;
    const name = member.first_name || "New member";
    const captcha = generateCaptcha();

    const dmData = await tgApi("sendMessage", {
      chat_id: userId,
      text: `👋 Welcome to Send.it!\n\n🔒 To verify you're human, solve this:\n\nWhat is ${captcha.question} ?\n\nReply here with the number within 3 minutes.`
    });
    const captchaMsgId = dmData.ok ? dmData.result.message_id : null;

    if (!dmData.ok) {
      const threadId = msg.message_thread_id || undefined;
      const fallbackBody = {
        chat_id: chatId,
        text: `👋 Welcome ${name}!\n\n🔒 To verify you're human, solve this:\n\nWhat is ${captcha.question} ?\n\nJust type the number in chat within 3 minutes.`,
      };
      if (threadId) fallbackBody.message_thread_id = threadId;
      const fbData = await tgApi("sendMessage", fallbackBody);
      if (fbData.ok) pendingCaptcha.set(userId, { chatId, msgId: fbData.result.message_id, answer: captcha.answer, timeout: null, threadId, isDm: false });
    }

    // NOTE: setTimeout captcha kick won't persist in serverless
    const timeout = setTimeout(async () => {
      if (pendingCaptcha.has(userId)) {
        pendingCaptcha.delete(userId);
        try {
          await tgApi("banChatMember", { chat_id: chatId, user_id: userId, until_date: Math.floor(Date.now()/1000) + 60 });
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
    pendingCaptcha.delete(userId);
    clearTimeout(timeout);
    await tgApi("restrictChatMember", {
      chat_id: chatId, user_id: userId,
      permissions: {
        can_send_messages: true, can_send_audios: false, can_send_documents: false,
        can_send_photos: true, can_send_videos: false, can_send_video_notes: false,
        can_send_voice_notes: false, can_send_polls: false, can_send_other_messages: true,
        can_add_web_page_previews: true, can_invite_users: true
      }
    });
    try {
      if (isDm && msgId) { await tgApi("deleteMessage", { chat_id: userId, message_id: msgId }); }
      else if (msgId) {
        await tgApi("deleteMessage", { chat_id: chatId, message_id: msgId });
        await tgApi("deleteMessage", { chat_id: chatId, message_id: msg.message_id });
      }
    } catch (e) {}
    await tgApi("sendMessage", { chat_id: chatId, text: `✅ Verified! Welcome to Send.it, ${msg.from.first_name || "anon"}! 🚀\n\nType /filters to see available commands.` });
    if (isDm) { await tgApi("sendMessage", { chat_id: userId, text: "✅ Verified! You now have access to the Send.it group." }); }
    console.log(`${msg.from.first_name} (${userId}) passed captcha`);
    return true;
  } else {
    try { await tgApi("deleteMessage", { chat_id: chatId, message_id: msg.message_id }); } catch (e) {}
    return true;
  }
}

/**
 * Main update handler — processes a single Telegram update object.
 * Used by both the polling loop (filters.js) and the webhook (api/webhook.js).
 */
async function handleUpdate(update) {
  const msg = update.message;
  if (!msg) return;

  // Handle new members
  if (msg.new_chat_members && msg.new_chat_members.length > 0) {
    await handleNewMember(msg);
    return;
  }

  if (!msg.text) return;

  // Check captcha answers first
  if (pendingCaptcha.has(msg.from?.id)) {
    await checkCaptchaAnswer(msg);
    return;
  }

  // Strip @botname suffix from commands in groups (e.g. /modlist@SendItBot → /modlist)
  const text = msg.text.trim().replace(/^(\/\w+)@\w+/, '$1');
  const chatId = msg.chat.id;
  const cmd = text.split("@")[0].split(" ")[0].toLowerCase();

  // Raid commands
  if (text.startsWith("/raids") || text === "/raid") {
    await handleRaidCommand(msg, chatId, text === "/raid" ? "/raids" : text);
    return;
  }

  // Show user ID
  if (text.startsWith("/myid")) {
    await tgApi("sendMessage", { chat_id: chatId, text: `Your Telegram ID: ${msg.from.id}`, reply_to_message_id: msg.message_id });
    return;
  }

  // Raid leader management
  if (text.startsWith("/addraidleader") || text.startsWith("/removeraidleader") || text.startsWith("/raidleaders")) {
    const cmd = text.split(" ")[0].toLowerCase();
    if (cmd === "/raidleaders") {
      const names = [];
      for (const uid of raidLeaders) {
        try {
          const d = await tgApi("getChatMember", { chat_id: chatId, user_id: uid });
          names.push(d.ok ? `• ${d.result.user.first_name} (${uid})` : `• ${uid}`);
        } catch (e) { names.push(`• ${uid}`); }
      }
      await tgApi("sendMessage", { chat_id: chatId, text: names.length ? `⚔️ Raid Leaders:\n${names.join("\n")}` : "⚔️ No raid leaders set. Use /addraidleader (reply to user).", reply_to_message_id: msg.message_id });
    } else if (!isMod(msg.from.id)) {
      await tgApi("sendMessage", { chat_id: chatId, text: "⛔ Only owner/mods can manage raid leaders.", reply_to_message_id: msg.message_id });
    } else if (msg.reply_to_message) {
      const targetId = msg.reply_to_message.from.id;
      const targetName = msg.reply_to_message.from.first_name || "User";
      if (cmd === "/addraidleader") {
        raidLeaders.add(targetId);
        await tgApi("sendMessage", { chat_id: chatId, text: `⚔️ ${targetName} is now a raid leader!` });
      } else {
        raidLeaders.delete(targetId);
        await tgApi("sendMessage", { chat_id: chatId, text: `❌ ${targetName} removed as raid leader.` });
      }
    } else {
      await tgApi("sendMessage", { chat_id: chatId, text: "↩️ Reply to a user to add/remove them as raid leader.", reply_to_message_id: msg.message_id });
    }
    return;
  }

  // Role commands
  if (text.startsWith("/shiller") || text.startsWith("/unshiller") || text.startsWith("/fundraiser") || text.startsWith("/unfundraiser") || text.startsWith("/pm") || text.startsWith("/unpm") || text.startsWith("/raider") || text.startsWith("/unraider") || text.startsWith("/communitylead") || text.startsWith("/uncommunitylead")) {
    const cmd = text.split(" ")[0].toLowerCase();
    let isAdmin = isMod(msg.from.id);
    if (!isAdmin) {
      try {
        const adminData = await tgApi("getChatMember", { chat_id: chatId, user_id: msg.from.id });
        isAdmin = adminData.ok && ["creator", "administrator"].includes(adminData.result?.status);
      } catch (e) {}
    }
    if (!isAdmin) {
      await tgApi("sendMessage", { chat_id: chatId, text: "⛔ Mods only.", reply_to_message_id: msg.message_id });
    } else if (!msg.reply_to_message) {
      await tgApi("sendMessage", { chat_id: chatId, text: "↩️ Reply to a user to assign/remove role.", reply_to_message_id: msg.message_id });
    } else {
      const targetId = msg.reply_to_message.from.id;
      const targetName = msg.reply_to_message.from.first_name || "User";
      const roleTitle = cmd === "/shiller" ? "Shiller 📣" : cmd === "/fundraiser" ? "Fundraiser 💰" : cmd === "/pm" ? "Project Manager 📋" : cmd === "/raider" ? "Raider ⚔️" : cmd === "/communitylead" ? "Community Lead 🌟" : null;
      const isAssign = cmd === "/shiller" || cmd === "/fundraiser" || cmd === "/pm" || cmd === "/raider" || cmd === "/communitylead";
      if (isAssign && roleTitle) {
        const res = await tgApi("promoteChatMember", {
          chat_id: chatId, user_id: targetId,
          can_manage_chat: false, can_delete_messages: false, can_manage_video_chats: false,
          can_restrict_members: false, can_promote_members: false, can_change_info: false,
          can_invite_users: true, can_pin_messages: false, can_post_stories: false,
          can_edit_stories: false, can_delete_stories: false
        });
        if (res.ok) {
          await tgApi("setChatAdministratorCustomTitle", { chat_id: chatId, user_id: targetId, custom_title: roleTitle });
          await tgApi("sendMessage", { chat_id: chatId, text: `${roleTitle} ${targetName} is now a ${roleTitle.split(" ")[0]}!` });
        } else {
          await tgApi("sendMessage", { chat_id: chatId, text: "❌ Failed — make sure the bot has promote permissions.", reply_to_message_id: msg.message_id });
        }
      } else {
        await tgApi("promoteChatMember", {
          chat_id: chatId, user_id: targetId,
          can_manage_chat: false, can_delete_messages: false, can_manage_video_chats: false,
          can_restrict_members: false, can_promote_members: false, can_change_info: false,
          can_invite_users: false, can_pin_messages: false, can_post_stories: false,
          can_edit_stories: false, can_delete_stories: false
        });
        await tgApi("sendMessage", { chat_id: chatId, text: `❌ ${targetName} role removed.` });
      }
    }
    return;
  }

  // Mod management (owner only)
  if (text.startsWith("/addmod") || text.startsWith("/removemod") || text.startsWith("/modlist")) {
    const cmd = text.split(" ")[0].toLowerCase();
    if (cmd === "/modlist") {
      const modNames = [];
      for (const modId of botMods) {
        try {
          const d = await tgApi("getChatMember", { chat_id: chatId, user_id: modId });
          modNames.push(d.ok ? `• ${d.result.user.first_name} (${modId})` : `• ${modId}`);
        } catch (e) { modNames.push(`• ${modId}`); }
      }
      await tgApi("sendMessage", { chat_id: chatId, text: modNames.length ? `🛡️ Bot Moderators:\n${modNames.join("\n")}` : "🛡️ No bot moderators set. Use /addmod (reply to user) to add one.", reply_to_message_id: msg.message_id });
    } else if (!isOwner(msg.from.id)) {
      await tgApi("sendMessage", { chat_id: chatId, text: "⛔ Owner only command.", reply_to_message_id: msg.message_id });
    } else if (msg.reply_to_message) {
      const targetId = msg.reply_to_message.from.id;
      const targetName = msg.reply_to_message.from.first_name || "User";
      if (cmd === "/addmod") {
        botMods.add(targetId);
        await tgApi("sendMessage", { chat_id: chatId, text: `✅ ${targetName} is now a bot moderator.` });
      } else {
        botMods.delete(targetId);
        await tgApi("sendMessage", { chat_id: chatId, text: `❌ ${targetName} removed as bot moderator.` });
      }
    } else {
      await tgApi("sendMessage", { chat_id: chatId, text: "↩️ Reply to a user to add/remove them as mod.", reply_to_message_id: msg.message_id });
    }
    return;
  }

  // Mod commands
  if (text.startsWith("/warn") || text.startsWith("/mute") || text.startsWith("/unmute") || text.startsWith("/ban") || text.startsWith("/unban")) {
    let isAdmin = isMod(msg.from.id);
    if (!isAdmin) {
      try {
        const adminData = await tgApi("getChatMember", { chat_id: chatId, user_id: msg.from.id });
        isAdmin = adminData.ok && ["creator", "administrator"].includes(adminData.result?.status);
      } catch (e) {}
    }
    try {
      if (!isAdmin) {
        await tgApi("sendMessage", { chat_id: chatId, text: "⛔ Admin only command.", reply_to_message_id: msg.message_id });
      } else if (msg.reply_to_message) {
        const targetUser = msg.reply_to_message.from;
        const targetId = targetUser.id;
        const targetName = targetUser.first_name || "User";
        const cmd = text.split(" ")[0].toLowerCase();
        if (cmd === "/warn") {
          await tgApi("sendMessage", { chat_id: chatId, text: `⚠️ ${targetName} has been warned. Next offense = mute.`, reply_to_message_id: msg.reply_to_message.message_id });
        } else if (cmd === "/mute") {
          const duration = parseInt(text.split(" ")[1]) || 60;
          await tgApi("restrictChatMember", {
            chat_id: chatId, user_id: targetId,
            until_date: Math.floor(Date.now()/1000) + (duration * 60),
            permissions: { can_send_messages: false, can_send_audios: false, can_send_documents: false, can_send_photos: false, can_send_videos: false, can_send_video_notes: false, can_send_voice_notes: false, can_send_polls: false, can_send_other_messages: false, can_add_web_page_previews: false }
          });
          await tgApi("sendMessage", { chat_id: chatId, text: `🔇 ${targetName} muted for ${duration} minutes.` });
        } else if (cmd === "/unmute") {
          await tgApi("restrictChatMember", {
            chat_id: chatId, user_id: targetId,
            permissions: { can_send_messages: true, can_send_photos: true, can_send_other_messages: true, can_add_web_page_previews: true, can_invite_users: true }
          });
          await tgApi("sendMessage", { chat_id: chatId, text: `🔊 ${targetName} has been unmuted.` });
        } else if (cmd === "/ban") {
          await tgApi("banChatMember", { chat_id: chatId, user_id: targetId });
          await tgApi("sendMessage", { chat_id: chatId, text: `🚫 ${targetName} has been banned.` });
        } else if (cmd === "/unban") {
          await tgApi("unbanChatMember", { chat_id: chatId, user_id: targetId, only_if_banned: true });
          await tgApi("sendMessage", { chat_id: chatId, text: `✅ ${targetName} has been unbanned.` });
        }
        console.log(`Mod action: ${cmd} on ${targetName} by ${msg.from.username || msg.from.id}`);
      } else {
        await tgApi("sendMessage", { chat_id: chatId, text: "↩️ Reply to a message to use mod commands.", reply_to_message_id: msg.message_id });
      }
    } catch (e) { console.error("Mod error:", e.message); }
    return;
  }

  // Contest commands
  if (text.startsWith("/contest")) {
    await handleContestCommand(msg, chatId, text);
    return;
  }

  // Points: first message of the day (silent, no notification)
  if (msg.from) {
    try {
      await points.firstMessage(msg.from.id, msg.from.first_name || "Anon");
    } catch (e) { /* ignore points errors */ }
  }

  // Points commands
  if (cmd === "/checkin") {
    try {
      const result = await points.checkin(msg.from.id, msg.from.first_name || "Anon");
      if (result.ok) {
        await tgApi("sendMessage", { chat_id: chatId, text: `✅ Daily check-in! +${result.earned} pts\n\n🏆 Your total: ${result.total} pts`, reply_to_message_id: msg.message_id });
      } else if (result.reason === "already_checked_in") {
        await tgApi("sendMessage", { chat_id: chatId, text: `⏰ Already checked in! Come back in ~${result.hoursLeft}h`, reply_to_message_id: msg.message_id });
      }
    } catch (e) {
      await tgApi("sendMessage", { chat_id: chatId, text: "❌ Points system offline. Try again later.", reply_to_message_id: msg.message_id });
    }
    return;
  }

  if (cmd === "/points") {
    try {
      const pts = await points.getPoints(msg.from.id);
      await tgApi("sendMessage", { chat_id: chatId, text: `🏆 ${msg.from.first_name || "You"}: ${pts} pts`, reply_to_message_id: msg.message_id });
    } catch (e) {
      await tgApi("sendMessage", { chat_id: chatId, text: "❌ Points system offline.", reply_to_message_id: msg.message_id });
    }
    return;
  }

  if (cmd === "/leaderboard") {
    try {
      const top = await points.getLeaderboard(10);
      if (top.length === 0) {
        await tgApi("sendMessage", { chat_id: chatId, text: "🏆 No points earned yet. Use /checkin to start!", reply_to_message_id: msg.message_id });
      } else {
        const medals = ["🥇", "🥈", "🥉"];
        let txt = "🏆 *Send\\.it Leaderboard*\n\n";
        top.forEach((e, i) => {
          const prefix = medals[i] || `${i + 1}\\.`;
          const name = e.name.replace(/[_*[\]()~`>#+\-=|{}.!]/g, '\\$&');
          txt += `${prefix} ${name} — ${e.points} pts\n`;
        });
        await tgApi("sendMessage", { chat_id: chatId, text: txt, parse_mode: "MarkdownV2", reply_to_message_id: msg.message_id });
      }
    } catch (e) {
      await tgApi("sendMessage", { chat_id: chatId, text: "❌ Points system offline.", reply_to_message_id: msg.message_id });
    }
    return;
  }

  if (cmd === "/award") {
    if (!isMod(msg.from.id)) { await tgApi("sendMessage", { chat_id: chatId, text: "⛔ Mods only.", reply_to_message_id: msg.message_id }); return; }
    if (!msg.reply_to_message) { await tgApi("sendMessage", { chat_id: chatId, text: "↩️ Reply to a user to award 15 pts.", reply_to_message_id: msg.message_id }); return; }
    try {
      const target = msg.reply_to_message.from;
      const result = await points.modAward(target.id, target.first_name || "Anon");
      await tgApi("sendMessage", { chat_id: chatId, text: `🎁 ${target.first_name || "User"} awarded +${result.earned} pts! (Total: ${result.total})` });
    } catch (e) {
      await tgApi("sendMessage", { chat_id: chatId, text: "❌ Points system offline.", reply_to_message_id: msg.message_id });
    }
    return;
  }

  if (cmd === "/bugreport") {
    if (!isMod(msg.from.id)) { await tgApi("sendMessage", { chat_id: chatId, text: "⛔ Mods only — reply to a bug reporter to award 50 pts.", reply_to_message_id: msg.message_id }); return; }
    if (!msg.reply_to_message) { await tgApi("sendMessage", { chat_id: chatId, text: "↩️ Reply to the bug reporter to award 50 pts.", reply_to_message_id: msg.message_id }); return; }
    try {
      const target = msg.reply_to_message.from;
      const result = await points.bugReward(target.id, target.first_name || "Anon");
      await tgApi("sendMessage", { chat_id: chatId, text: `🐛 ${target.first_name || "User"} awarded +${result.earned} pts for bug report! (Total: ${result.total})` });
    } catch (e) {
      await tgApi("sendMessage", { chat_id: chatId, text: "❌ Points system offline.", reply_to_message_id: msg.message_id });
    }
    return;
  }

  // Check for spam
  if (SPAM_PATTERNS.some(p => p.test(text))) {
    try { await tgApi("deleteMessage", { chat_id: chatId, message_id: msg.message_id }); } catch (e) {}
    console.log(`Deleted spam from ${msg.from?.username || msg.from?.id}`);
    return;
  }

  // Check for commands
  if (responses[cmd]) {
    const opts = {
      chat_id: chatId, text: responses[cmd],
      disable_web_page_preview: true,
      reply_to_message_id: msg.message_id
    };
    if (cmd === "/filters") opts.parse_mode = "HTML";
    else opts.parse_mode = "MarkdownV2";
    await tgApi("sendMessage", opts);
    console.log(`Replied to ${cmd} from ${msg.from?.username || msg.from?.id}`);
  }
}

module.exports = { handleUpdate, BOT_TOKEN, BASE };
