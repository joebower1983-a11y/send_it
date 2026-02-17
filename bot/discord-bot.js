/**
 * Send.it Discord Bot
 * Features: mod commands, points, spam filter, welcome, devnet info
 * 
 * Run: DISCORD_TOKEN=... node bot/discord-bot.js
 * Or use pm2/systemd for persistence
 */

const { Client, GatewayIntentBits, Events, EmbedBuilder, PermissionFlagsBits } = require('discord.js');

// ─── Config ───
const TOKEN = process.env.DISCORD_TOKEN;
const GUILD_ID = '1471992185959354400';
const ACCENT = 0x00c896;

// Mod IDs (Discord user IDs — update these as needed)
const MOD_IDS = new Set([
  // Add Discord user IDs of mods here
]);

// ─── Points (in-memory, persists to file) ───
const fs = require('fs');
const POINTS_FILE = './bot/data/discord-points.json';
let points = {};

function loadPoints() {
  try {
    if (fs.existsSync(POINTS_FILE)) {
      points = JSON.parse(fs.readFileSync(POINTS_FILE, 'utf8'));
    }
  } catch (e) { console.error('Points load error:', e.message); }
}

function savePoints() {
  try {
    fs.mkdirSync('./bot/data', { recursive: true });
    fs.writeFileSync(POINTS_FILE, JSON.stringify(points, null, 2));
  } catch (e) { console.error('Points save error:', e.message); }
}

function addPoints(userId, username, amount) {
  if (!points[userId]) points[userId] = { username, points: 0 };
  points[userId].username = username;
  points[userId].points += amount;
  savePoints();
  return points[userId].points;
}

function getPoints(userId) {
  return points[userId]?.points || 0;
}

function getLeaderboard(limit = 10) {
  return Object.entries(points)
    .sort((a, b) => b[1].points - a[1].points)
    .slice(0, limit)
    .map(([id, data], i) => `${i + 1}. **${data.username}** — ${data.points.toLocaleString()} pts`);
}

// ─── Spam Filter ───
const msgHistory = new Map(); // userId -> [{content, time}]
const SPAM_WINDOW = 5000; // 5 seconds
const SPAM_THRESHOLD = 4; // 4 messages in window
const LINK_PATTERN = /(?:https?:\/\/|www\.)(?!senditsolana\.io|discord\.gg\/vKRTyG85|github\.com\/joebower1983|pump\.fun|dexscreener\.com)[^\s]+/i;

function isSpam(message) {
  const userId = message.author.id;
  
  // Mods exempt
  if (MOD_IDS.has(userId)) return false;
  if (message.member?.permissions?.has(PermissionFlagsBits.ManageMessages)) return false;
  
  const now = Date.now();
  if (!msgHistory.has(userId)) msgHistory.set(userId, []);
  const history = msgHistory.get(userId);
  history.push({ content: message.content, time: now });
  
  // Clean old entries
  const recent = history.filter(m => now - m.time < SPAM_WINDOW);
  msgHistory.set(userId, recent);
  
  // Rate limit check
  if (recent.length >= SPAM_THRESHOLD) return true;
  
  // Suspicious link check (new accounts posting links)
  const joinedAgo = now - (message.member?.joinedTimestamp || 0);
  if (joinedAgo < 86400000 && LINK_PATTERN.test(message.content)) return true; // <24h old + external link
  
  return false;
}

// ─── Bot Setup ───
const client = new Client({
  intents: [
    GatewayIntentBits.Guilds,
    GatewayIntentBits.GuildMessages,
    GatewayIntentBits.MessageContent,
    GatewayIntentBits.GuildMembers,
  ]
});

// ─── Ready ───
client.once(Events.ClientReady, (c) => {
  console.log(`✅ Send.it Discord bot online as ${c.user.tag}`);
  c.user.setActivity('senditsolana.io | /help', { type: 3 }); // Watching
  loadPoints();
});

// ─── Welcome ───
client.on(Events.GuildMemberAdd, (member) => {
  const channel = member.guild.systemChannel;
  if (!channel) return;
  
  const embed = new EmbedBuilder()
    .setColor(ACCENT)
    .setTitle('🚀 Welcome to Send.it!')
    .setDescription(`Hey ${member}, welcome to the fairest token launchpad on Solana!\n\n` +
      '**Quick Links:**\n' +
      '• 🌐 [Website](https://senditsolana.io)\n' +
      '• 🚀 [Launchpad](https://senditsolana.io/launchpad.html)\n' +
      '• 💱 [Trading](https://senditsolana.io/trading.html)\n' +
      '• 📦 [GitHub](https://github.com/joebower1983-a11y/send_it)\n\n' +
      'Type `!help` to see all commands. Enjoy your stay! 🐕')
    .setThumbnail(member.user.displayAvatarURL())
    .setTimestamp();
  
  channel.send({ embeds: [embed] });
});

// ─── Message Handler ───
client.on(Events.MessageCreate, async (message) => {
  if (message.author.bot) return;
  
  // Spam filter
  if (isSpam(message)) {
    try {
      await message.delete();
      const warn = await message.channel.send(`⚠️ ${message.author}, slow down or no external links for new accounts.`);
      setTimeout(() => warn.delete().catch(() => {}), 5000);
    } catch (e) {}
    return;
  }
  
  // Passive points (1 point per message, max 1 per 30s)
  const userId = message.author.id;
  const now = Date.now();
  const lastMsg = msgHistory.get(`pts_${userId}`) || 0;
  if (now - lastMsg > 30000) {
    addPoints(userId, message.author.username, 1);
    msgHistory.set(`pts_${userId}`, now);
  }
  
  const content = message.content.trim().toLowerCase();
  
  // ─── Commands ───
  
  if (content === '!help') {
    const embed = new EmbedBuilder()
      .setColor(ACCENT)
      .setTitle('🐕 Send.it Bot Commands')
      .addFields(
        { name: '📊 Info', value: '`!devnet` — Program info\n`!ca` — Contract address\n`!links` — All links\n`!stats` — Protocol stats', inline: true },
        { name: '🏆 Points', value: '`!points` — Your points\n`!leaderboard` — Top 10\n`!daily` — Daily check-in', inline: true },
        { name: '🛡️ Mod', value: '`!warn @user` — Warn\n`!mute @user` — Timeout 10m\n`!addpoints @user N` — Give points', inline: true },
      )
      .setFooter({ text: 'Send.it — The fairest launchpad on Solana' });
    return message.reply({ embeds: [embed] });
  }
  
  if (content === '!devnet') {
    const embed = new EmbedBuilder()
      .setColor(ACCENT)
      .setTitle('⛓️ Send.it Devnet Program')
      .addFields(
        { name: 'Program ID', value: '```HTKq18cATdwCZb6XM66Mhn8JWKCFTrZqH6zU1zip88Zx```' },
        { name: 'Instructions', value: '11 (init, update, create_token, buy, sell, stake, unstake, create_pool, swap, add_liquidity, remove_liquidity)', inline: false },
        { name: 'Security', value: 'Sec3 X-Ray: **0 vulnerabilities** ✅', inline: true },
        { name: 'Lines of Rust', value: '~16,000', inline: true },
        { name: 'Modules', value: '31', inline: true },
      )
      .setURL('https://solscan.io/account/HTKq18cATdwCZb6XM66Mhn8JWKCFTrZqH6zU1zip88Zx?cluster=devnet');
    return message.reply({ embeds: [embed] });
  }
  
  if (content === '!ca') {
    const embed = new EmbedBuilder()
      .setColor(ACCENT)
      .setTitle('📋 SENDIT Contract Address')
      .setDescription('```F8qWTN8JfyDCvj4RoCHuvNMVbTV9XQksLuziA8PYpump```')
      .addFields(
        { name: 'Buy', value: '[Pump.fun](https://pump.fun/coin/F8qWTN8JfyDCvj4RoCHuvNMVbTV9XQksLuziA8PYpump) · [DexScreener](https://dexscreener.com/solana/F8qWTN8JfyDCvj4RoCHuvNMVbTV9XQksLuziA8PYpump)', inline: false }
      );
    return message.reply({ embeds: [embed] });
  }
  
  if (content === '!links') {
    const embed = new EmbedBuilder()
      .setColor(ACCENT)
      .setTitle('🔗 Send.it Links')
      .setDescription(
        '🌐 [Website](https://senditsolana.io)\n' +
        '🚀 [Launchpad](https://senditsolana.io/launchpad.html)\n' +
        '💱 [Trading / AMM](https://senditsolana.io/trading.html)\n' +
        '🔒 [Staking](https://senditsolana.io/staking.html)\n' +
        '👥 [Social Hub](https://senditsolana.io/social.html)\n' +
        '📦 [GitHub](https://github.com/joebower1983-a11y/send_it)\n' +
        '📄 [Pitch Deck](https://senditsolana.io/pitch-deck.html)\n' +
        '📜 [Whitepaper v2.2](https://github.com/joebower1983-a11y/send_it/blob/main/docs/WHITEPAPER.md)\n' +
        '🐦 [Twitter](https://twitter.com/SendItSolana420)\n' +
        '💬 [Telegram](https://t.me/+Xw4E2sJ0Z3Q5ZDYx)'
      );
    return message.reply({ embeds: [embed] });
  }
  
  if (content === '!stats') {
    const embed = new EmbedBuilder()
      .setColor(ACCENT)
      .setTitle('📊 Protocol Stats')
      .addFields(
        { name: 'Modules', value: '31', inline: true },
        { name: 'Lines of Rust', value: '16k+', inline: true },
        { name: 'Frontend Pages', value: '12', inline: true },
        { name: 'Devnet Instructions', value: '11', inline: true },
        { name: 'Vulnerabilities', value: '0 ✅', inline: true },
        { name: 'AMM Fee', value: '1% (0.3% LP / 0.7% protocol)', inline: true },
      );
    return message.reply({ embeds: [embed] });
  }
  
  // ─── Points Commands ───
  
  if (content === '!points') {
    const pts = getPoints(userId);
    return message.reply(`🏆 **${message.author.username}** — ${pts.toLocaleString()} points`);
  }
  
  if (content === '!leaderboard' || content === '!lb') {
    const lb = getLeaderboard();
    if (lb.length === 0) return message.reply('No points yet! Start chatting to earn.');
    const embed = new EmbedBuilder()
      .setColor(ACCENT)
      .setTitle('🏆 Points Leaderboard')
      .setDescription(lb.join('\n'));
    return message.reply({ embeds: [embed] });
  }
  
  if (content === '!daily') {
    const key = `daily_${userId}`;
    const last = msgHistory.get(key) || 0;
    const now = Date.now();
    if (now - last < 86400000) {
      const remaining = Math.ceil((86400000 - (now - last)) / 3600000);
      return message.reply(`⏰ Already claimed! Come back in ~${remaining}h.`);
    }
    msgHistory.set(key, now);
    const total = addPoints(userId, message.author.username, 50);
    return message.reply(`✅ Daily check-in! **+50 points** (Total: ${total.toLocaleString()})`);
  }
  
  // ─── Mod Commands ───
  
  if (content.startsWith('!warn') && message.member?.permissions?.has(PermissionFlagsBits.ManageMessages)) {
    const target = message.mentions.users.first();
    if (!target) return message.reply('Usage: `!warn @user`');
    const embed = new EmbedBuilder()
      .setColor(0xff2d78)
      .setDescription(`⚠️ **${target.username}** has been warned by ${message.author.username}. Please follow the rules.`);
    return message.channel.send({ embeds: [embed] });
  }
  
  if (content.startsWith('!mute') && message.member?.permissions?.has(PermissionFlagsBits.ModerateMembers)) {
    const target = message.mentions.members?.first();
    if (!target) return message.reply('Usage: `!mute @user`');
    try {
      await target.timeout(600000, `Muted by ${message.author.username}`); // 10 min
      return message.reply(`🔇 **${target.user.username}** muted for 10 minutes.`);
    } catch (e) {
      return message.reply(`❌ Can't mute: ${e.message}`);
    }
  }
  
  if (content.startsWith('!addpoints') && message.member?.permissions?.has(PermissionFlagsBits.ManageMessages)) {
    const target = message.mentions.users.first();
    const args = content.split(/\s+/);
    const amount = parseInt(args[args.length - 1]);
    if (!target || isNaN(amount)) return message.reply('Usage: `!addpoints @user 100`');
    const total = addPoints(target.id, target.username, amount);
    return message.reply(`✅ Gave **${amount}** points to **${target.username}** (Total: ${total.toLocaleString()})`);
  }
});

// ─── Login ───
if (!TOKEN) {
  console.error('❌ Set DISCORD_TOKEN environment variable');
  process.exit(1);
}
client.login(TOKEN);
