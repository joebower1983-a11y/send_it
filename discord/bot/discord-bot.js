require('dotenv').config();
const {
  Client, GatewayIntentBits, EmbedBuilder, SlashCommandBuilder,
  REST, Routes, ActivityType
} = require('discord.js');

// ─── Config ───────────────────────────────────────────────────────────────────
const CONFIG = {
  token: process.env.DISCORD_BOT_TOKEN,
  guildId: process.env.DISCORD_GUILD_ID,
  channels: {
    tokenLaunches: process.env.CHANNEL_TOKEN_LAUNCHES,
    whaleAlerts: process.env.CHANNEL_WHALE_ALERTS,
    general: process.env.CHANNEL_GENERAL,
  },
  rpcUrl: process.env.RPC_URL || 'https://api.mainnet-beta.solana.com',
  apiUrl: process.env.SENDIT_API_URL || 'https://api.sendit.app',
  appUrl: process.env.SENDIT_APP_URL || 'https://sendit.app',
  whaleThreshold: parseFloat(process.env.WHALE_THRESHOLD) || 1,
};

// ─── Client Setup ─────────────────────────────────────────────────────────────
const client = new Client({
  intents: [
    GatewayIntentBits.Guilds,
    GatewayIntentBits.GuildMembers,
    GatewayIntentBits.GuildMessages,
  ],
});

// ─── Slash Commands ───────────────────────────────────────────────────────────
const commands = [
  new SlashCommandBuilder()
    .setName('price')
    .setDescription('Get the price of a token')
    .addStringOption(opt =>
      opt.setName('token').setDescription('Token address or symbol').setRequired(true)
    ),
  new SlashCommandBuilder()
    .setName('trending')
    .setDescription('Show top 10 trending tokens on Send.it'),
  new SlashCommandBuilder()
    .setName('stats')
    .setDescription('Show Send.it platform stats'),
  new SlashCommandBuilder()
    .setName('launch')
    .setDescription('Get the link to launch a token on Send.it'),
];

// ─── Register Commands ────────────────────────────────────────────────────────
async function registerCommands() {
  const rest = new REST({ version: '10' }).setToken(CONFIG.token);
  try {
    await rest.put(
      Routes.applicationGuildCommands(client.user.id, CONFIG.guildId),
      { body: commands.map(c => c.toJSON()) }
    );
    console.log('✅ Slash commands registered');
  } catch (err) {
    console.error('❌ Failed to register commands:', err);
  }
}

// ─── API Helpers ──────────────────────────────────────────────────────────────
async function apiFetch(endpoint) {
  try {
    const res = await fetch(`${CONFIG.apiUrl}${endpoint}`);
    if (!res.ok) throw new Error(`API ${res.status}`);
    return await res.json();
  } catch (err) {
    console.error(`API error (${endpoint}):`, err.message);
    return null;
  }
}

// ─── Embeds ───────────────────────────────────────────────────────────────────
function tokenLaunchEmbed(token) {
  return new EmbedBuilder()
    .setColor(0x00FF88)
    .setTitle('🚀 New Token Launched!')
    .setDescription(`**${token.name}** (${token.symbol})`)
    .addFields(
      { name: 'Contract', value: `\`${token.address}\``, inline: false },
      { name: 'Initial Price', value: `${token.price || 'N/A'} SOL`, inline: true },
      { name: 'Liquidity', value: `${token.liquidity || 'N/A'} SOL`, inline: true },
      { name: 'Creator', value: `\`${token.creator || 'Unknown'}\``, inline: false },
    )
    .setURL(`${CONFIG.appUrl}/token/${token.address}`)
    .setTimestamp()
    .setFooter({ text: 'Send.it 🚀 • DYOR — Not financial advice' });
}

function whaleAlertEmbed(trade) {
  const emoji = trade.type === 'buy' ? '🟢' : '🔴';
  return new EmbedBuilder()
    .setColor(trade.type === 'buy' ? 0x00FF88 : 0xFF4444)
    .setTitle(`🐋 Whale ${trade.type.toUpperCase()}!`)
    .addFields(
      { name: 'Token', value: `**${trade.tokenName}** (${trade.tokenSymbol})`, inline: true },
      { name: 'Amount', value: `${trade.solAmount} SOL`, inline: true },
      { name: 'Wallet', value: `\`${trade.wallet.slice(0, 8)}...${trade.wallet.slice(-4)}\``, inline: false },
    )
    .setURL(`${CONFIG.appUrl}/token/${trade.tokenAddress}`)
    .setTimestamp()
    .setFooter({ text: 'Send.it 🚀 Whale Alerts' });
}

function priceEmbed(token) {
  return new EmbedBuilder()
    .setColor(0x5865F2)
    .setTitle(`💰 ${token.name} (${token.symbol})`)
    .addFields(
      { name: 'Price', value: `${token.price} SOL`, inline: true },
      { name: 'Market Cap', value: `${token.marketCap || 'N/A'}`, inline: true },
      { name: '24h Volume', value: `${token.volume24h || 'N/A'} SOL`, inline: true },
      { name: '24h Change', value: `${token.change24h || 'N/A'}%`, inline: true },
      { name: 'Holders', value: `${token.holders || 'N/A'}`, inline: true },
      { name: 'Liquidity', value: `${token.liquidity || 'N/A'} SOL`, inline: true },
    )
    .setURL(`${CONFIG.appUrl}/token/${token.address}`)
    .setTimestamp()
    .setFooter({ text: 'Send.it 🚀' });
}

function trendingEmbed(tokens) {
  const list = tokens
    .slice(0, 10)
    .map((t, i) => `**${i + 1}.** ${t.name} (${t.symbol}) — ${t.price} SOL • Vol: ${t.volume24h} SOL`)
    .join('\n');

  return new EmbedBuilder()
    .setColor(0xFF6600)
    .setTitle('🔥 Top 10 Trending Tokens')
    .setDescription(list || 'No data available')
    .setTimestamp()
    .setFooter({ text: 'Send.it 🚀' });
}

function statsEmbed(stats) {
  return new EmbedBuilder()
    .setColor(0x9B59B6)
    .setTitle('📊 Send.it Platform Stats')
    .addFields(
      { name: 'Total Tokens Launched', value: `${stats.totalTokens || 'N/A'}`, inline: true },
      { name: 'Total Volume', value: `${stats.totalVolume || 'N/A'} SOL`, inline: true },
      { name: '24h Volume', value: `${stats.volume24h || 'N/A'} SOL`, inline: true },
      { name: 'Active Traders', value: `${stats.activeTraders || 'N/A'}`, inline: true },
      { name: 'Tokens Today', value: `${stats.tokensToday || 'N/A'}`, inline: true },
      { name: 'Total Trades', value: `${stats.totalTrades || 'N/A'}`, inline: true },
    )
    .setTimestamp()
    .setFooter({ text: 'Send.it 🚀' });
}

// ─── Welcome Message ──────────────────────────────────────────────────────────
function welcomeEmbed(member) {
  return new EmbedBuilder()
    .setColor(0x00FF88)
    .setTitle(`Welcome to Send.it! 🚀`)
    .setDescription(
      `Hey ${member}, welcome to the fastest Solana token launcher!\n\n` +
      `🔹 Read the rules in <#rules>\n` +
      `🔹 Introduce yourself in <#introductions>\n` +
      `🔹 Watch new launches in <#token-launches>\n` +
      `🔹 Use \`/price\`, \`/trending\`, \`/stats\` in <#bot-commands>\n\n` +
      `**Let's send it!** 🚀`
    )
    .setThumbnail(member.user.displayAvatarURL({ dynamic: true }))
    .setTimestamp()
    .setFooter({ text: `Member #${member.guild.memberCount}` });
}

// ─── Event: Ready ─────────────────────────────────────────────────────────────
client.once('ready', async () => {
  console.log(`✅ Logged in as ${client.user.tag}`);
  client.user.setActivity('Send.it 🚀', { type: ActivityType.Watching });
  await registerCommands();
  startTokenLaunchPoller();
  startWhaleAlertPoller();
});

// ─── Event: New Member ────────────────────────────────────────────────────────
client.on('guildMemberAdd', async (member) => {
  try {
    const channel = member.guild.channels.cache.get(CONFIG.channels.general);
    if (channel) {
      await channel.send({ embeds: [welcomeEmbed(member)] });
    }
  } catch (err) {
    console.error('Welcome message error:', err);
  }
});

// ─── Event: Slash Commands ────────────────────────────────────────────────────
client.on('interactionCreate', async (interaction) => {
  if (!interaction.isChatInputCommand()) return;

  const { commandName } = interaction;

  if (commandName === 'price') {
    await interaction.deferReply();
    const query = interaction.options.getString('token');
    const data = await apiFetch(`/token/${encodeURIComponent(query)}`);
    if (!data) {
      return interaction.editReply('❌ Token not found or API unavailable.');
    }
    await interaction.editReply({ embeds: [priceEmbed(data)] });
  }

  if (commandName === 'trending') {
    await interaction.deferReply();
    const data = await apiFetch('/tokens/trending');
    if (!data?.tokens?.length) {
      return interaction.editReply('❌ No trending data available.');
    }
    await interaction.editReply({ embeds: [trendingEmbed(data.tokens)] });
  }

  if (commandName === 'stats') {
    await interaction.deferReply();
    const data = await apiFetch('/stats');
    if (!data) {
      return interaction.editReply('❌ Stats unavailable.');
    }
    await interaction.editReply({ embeds: [statsEmbed(data)] });
  }

  if (commandName === 'launch') {
    const embed = new EmbedBuilder()
      .setColor(0x00FF88)
      .setTitle('🚀 Launch a Token on Send.it')
      .setDescription(
        `Ready to send it? Launch your token in seconds!\n\n` +
        `👉 **[Launch Now](${CONFIG.appUrl}/launch)**\n\n` +
        `• No code required\n• Instant Solana deployment\n• Built-in liquidity\n• Fair launch by default`
      )
      .setFooter({ text: 'Send.it 🚀 • DYOR' });
    await interaction.reply({ embeds: [embed] });
  }
});

// ─── Pollers ──────────────────────────────────────────────────────────────────
let lastSeenLaunchId = null;

function startTokenLaunchPoller() {
  setInterval(async () => {
    try {
      const data = await apiFetch('/tokens/latest');
      if (!data?.tokens?.length) return;

      const channel = client.channels.cache.get(CONFIG.channels.tokenLaunches);
      if (!channel) return;

      for (const token of data.tokens) {
        if (token.id === lastSeenLaunchId) break;
        await channel.send({ embeds: [tokenLaunchEmbed(token)] });
      }
      lastSeenLaunchId = data.tokens[0]?.id;
    } catch (err) {
      console.error('Token launch poll error:', err);
    }
  }, 15_000); // Poll every 15 seconds
}

function startWhaleAlertPoller() {
  setInterval(async () => {
    try {
      const data = await apiFetch(`/trades/whales?min_sol=${CONFIG.whaleThreshold}`);
      if (!data?.trades?.length) return;

      const channel = client.channels.cache.get(CONFIG.channels.whaleAlerts);
      if (!channel) return;

      for (const trade of data.trades.slice(0, 5)) {
        await channel.send({ embeds: [whaleAlertEmbed(trade)] });
      }
    } catch (err) {
      console.error('Whale alert poll error:', err);
    }
  }, 30_000); // Poll every 30 seconds
}

// ─── Start ────────────────────────────────────────────────────────────────────
client.login(CONFIG.token);
