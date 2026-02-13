const { generateWallet, getBalance, launchToken, buyToken, sellToken, getTokenPrice } = require('./services/solana');
const { shortAddr, solFmt, usdFmt, pnlEmoji, buyButtons, settingsKeyboard } = require('./helpers');

function registerCommands(bot) {

  // ── /start ──
  bot.start(async (ctx) => {
    const user = ctx.store.getUser(ctx.chat.id);

    if (!user.wallet) {
      const wallet = generateWallet();
      ctx.store.setUser(ctx.chat.id, { wallet: wallet.publicKey, privateKey: wallet.privateKey });
      return ctx.replyWithHTML([
        `🚀 <b>Welcome to Send.it!</b>`,
        ``,
        `Your wallet has been created:`,
        `<code>${wallet.publicKey}</code>`,
        ``,
        `💡 Send SOL to this address to get started.`,
        ``,
        `<b>Commands:</b>`,
        `/launch &lt;name&gt; &lt;symbol&gt; — Create a token`,
        `/buy &lt;token&gt; &lt;amount&gt; — Buy tokens`,
        `/sell &lt;token&gt; &lt;amount&gt; — Sell tokens`,
        `/trending — Top tokens by volume`,
        `/new — Recently launched`,
        `/price &lt;token&gt; — Check price`,
        `/portfolio — Your holdings & PnL`,
        `/settings — Configure slippage & sniper`,
      ].join('\n'));
    }

    const bal = await getBalance(user.wallet);
    return ctx.replyWithHTML([
      `👋 <b>Welcome back!</b>`,
      ``,
      `💳 Wallet: <code>${user.wallet}</code>`,
      `💰 Balance: <b>${solFmt(bal)} SOL</b>`,
      ``,
      `What do you want to do?`,
    ].join('\n'), {
      reply_markup: {
        inline_keyboard: [
          [{ text: '🔥 Trending', callback_data: 'cmd:trending' }, { text: '🆕 New', callback_data: 'cmd:new' }],
          [{ text: '💼 Portfolio', callback_data: 'cmd:portfolio' }, { text: '⚙️ Settings', callback_data: 'cmd:settings' }],
        ],
      },
    });
  });

  // ── /launch ──
  bot.command('launch', async (ctx) => {
    const user = ctx.store.getUser(ctx.chat.id);
    if (!user.wallet) return ctx.reply('⚠️ Run /start first to create a wallet.');

    const args = ctx.message.text.split(/\s+/).slice(1);
    if (args.length < 2) return ctx.replyWithHTML('Usage: /launch &lt;name&gt; &lt;symbol&gt;\nExample: <code>/launch DogeCoin DOGE</code>');

    const name = args.slice(0, -1).join(' ');
    const symbol = args[args.length - 1].toUpperCase();

    await ctx.replyWithHTML('⏳ Launching token...');
    try {
      const result = await launchToken(user, name, symbol);
      ctx.store.addToken({ mint: result.mint, name, symbol, creator: user.wallet });

      return ctx.replyWithHTML([
        `✅ <b>Token Launched!</b>`,
        ``,
        `📛 <b>${name}</b> ($${symbol})`,
        `🔗 Mint: <code>${result.mint}</code>`,
        `📝 Tx: <code>${result.txSig}</code>`,
        ``,
        `🛒 Buy now:`,
      ].join('\n'), buyButtons(result.mint));
    } catch (e) {
      return ctx.reply('❌ Launch failed: ' + e.message);
    }
  });

  // ── /buy ──
  bot.command('buy', async (ctx) => {
    const user = ctx.store.getUser(ctx.chat.id);
    if (!user.wallet) return ctx.reply('⚠️ Run /start first.');

    const args = ctx.message.text.split(/\s+/).slice(1);
    if (args.length < 1) return ctx.replyWithHTML('Usage: /buy &lt;token_mint&gt; [amount_sol]\nExample: <code>/buy ABC123 0.5</code>');

    const mint = args[0];
    const amount = parseFloat(args[1]) || user.settings.defaultBuy;

    await ctx.replyWithHTML(`⏳ Buying ${solFmt(amount)} SOL of tokens...`);
    try {
      const result = await buyToken(user, mint, amount);
      // Update holdings
      if (!user.holdings[mint]) user.holdings[mint] = { amount: 0, avgCost: 0 };
      const h = user.holdings[mint];
      const totalCost = h.avgCost * h.amount + amount;
      h.amount += result.tokensReceived;
      h.avgCost = totalCost / h.amount;
      ctx.store.setUser(ctx.chat.id, { holdings: user.holdings });
      ctx.store.addTrade({ chatId: ctx.chat.id, mint, side: 'buy', sol: amount, tokens: result.tokensReceived, price: result.price, txSig: result.txSig });

      return ctx.replyWithHTML([
        `✅ <b>Buy Successful!</b>`,
        ``,
        `🪙 Got: <b>${result.tokensReceived.toLocaleString()}</b> tokens`,
        `💰 Spent: <b>${solFmt(amount)} SOL</b>`,
        `📈 Price: ${result.price.toFixed(10)} SOL`,
        `📝 Tx: <code>${result.txSig}</code>`,
      ].join('\n'), buyButtons(mint));
    } catch (e) {
      return ctx.reply('❌ Buy failed: ' + e.message);
    }
  });

  // ── /sell ──
  bot.command('sell', async (ctx) => {
    const user = ctx.store.getUser(ctx.chat.id);
    if (!user.wallet) return ctx.reply('⚠️ Run /start first.');

    const args = ctx.message.text.split(/\s+/).slice(1);
    if (args.length < 1) return ctx.replyWithHTML('Usage: /sell &lt;token_mint&gt; [amount]\nOmit amount to sell all.');

    const mint = args[0];
    const holding = user.holdings[mint];
    if (!holding || holding.amount <= 0) return ctx.reply('⚠️ You don\'t hold this token.');

    const amount = parseFloat(args[1]) || holding.amount;

    await ctx.replyWithHTML(`⏳ Selling ${amount.toLocaleString()} tokens...`);
    try {
      const result = await sellToken(user, mint, amount);
      holding.amount -= amount;
      if (holding.amount <= 0) delete user.holdings[mint];
      ctx.store.setUser(ctx.chat.id, { holdings: user.holdings });
      ctx.store.addTrade({ chatId: ctx.chat.id, mint, side: 'sell', sol: result.solReceived, tokens: amount, price: result.price, txSig: result.txSig });

      return ctx.replyWithHTML([
        `✅ <b>Sell Successful!</b>`,
        ``,
        `🪙 Sold: <b>${amount.toLocaleString()}</b> tokens`,
        `💰 Received: <b>${solFmt(result.solReceived)} SOL</b>`,
        `📝 Tx: <code>${result.txSig}</code>`,
      ].join('\n'));
    } catch (e) {
      return ctx.reply('❌ Sell failed: ' + e.message);
    }
  });

  // ── /trending ──
  bot.command('trending', async (ctx) => {
    const tokens = ctx.store.trending(10);
    if (!tokens.length) return ctx.reply('📊 No tokens yet. Be the first — /launch');

    const lines = tokens.map((t, i) => {
      return `${i + 1}. <b>${t.name}</b> ($${t.symbol}) — Vol: $${(t.volume24h || 0).toLocaleString()}`;
    });

    return ctx.replyWithHTML([
      `🔥 <b>Trending Tokens</b>`,
      ``,
      ...lines,
    ].join('\n'));
  });

  // ── /new ──
  bot.command('new', async (ctx) => {
    const tokens = ctx.store.newest(10);
    if (!tokens.length) return ctx.reply('🆕 No tokens yet. Launch one with /launch');

    const lines = tokens.map((t, i) => {
      const ago = Math.floor((Date.now() - t.createdAt) / 60000);
      return `${i + 1}. <b>${t.name}</b> ($${t.symbol}) — ${ago}m ago`;
    });

    return ctx.replyWithHTML([
      `🆕 <b>Recently Launched</b>`,
      ``,
      ...lines,
    ].join('\n'));
  });

  // ── /price ──
  bot.command('price', async (ctx) => {
    const args = ctx.message.text.split(/\s+/).slice(1);
    if (!args[0]) return ctx.replyWithHTML('Usage: /price &lt;token_mint&gt;');

    const mint = args[0];
    const token = ctx.store.getToken(mint);
    const info = await getTokenPrice(mint);

    const name = token ? `${token.name} ($${token.symbol})` : shortAddr(mint);

    return ctx.replyWithHTML([
      `📊 <b>${name}</b>`,
      ``,
      `💰 Price: ${info.price.toFixed(10)} SOL`,
      `📈 MCap: $${info.mcap.toLocaleString()}`,
      `💧 Liquidity: $${info.liquidity.toLocaleString()}`,
      `📊 24h Volume: $${info.volume24h.toLocaleString()}`,
      `🎓 Graduation: ${info.graduationPct}%`,
    ].join('\n'), buyButtons(mint));
  });

  // ── /portfolio ──
  bot.command('portfolio', async (ctx) => {
    const user = ctx.store.getUser(ctx.chat.id);
    if (!user.wallet) return ctx.reply('⚠️ Run /start first.');

    const bal = await getBalance(user.wallet);
    const mints = Object.keys(user.holdings || {});

    if (!mints.length) {
      return ctx.replyWithHTML([
        `💼 <b>Portfolio</b>`,
        ``,
        `💰 SOL Balance: <b>${solFmt(bal)} SOL</b>`,
        ``,
        `No token holdings yet. Start trading!`,
      ].join('\n'));
    }

    const lines = [];
    for (const mint of mints) {
      const h = user.holdings[mint];
      const token = ctx.store.getToken(mint);
      const info = await getTokenPrice(mint);
      const currentVal = h.amount * info.price;
      const costBasis = h.amount * h.avgCost;
      const pnl = costBasis > 0 ? ((currentVal - costBasis) / costBasis) * 100 : 0;
      const name = token ? `${token.name}` : shortAddr(mint);
      lines.push(`${pnlEmoji(pnl)} <b>${name}</b>: ${h.amount.toLocaleString()} tokens — ${pnl >= 0 ? '+' : ''}${pnl.toFixed(1)}%`);
    }

    return ctx.replyWithHTML([
      `💼 <b>Portfolio</b>`,
      ``,
      `💰 SOL: <b>${solFmt(bal)} SOL</b>`,
      ``,
      ...lines,
    ].join('\n'));
  });

  // ── /settings ──
  bot.command('settings', async (ctx) => {
    const user = ctx.store.getUser(ctx.chat.id);
    return ctx.replyWithHTML([
      `⚙️ <b>Settings</b>`,
      ``,
      `Adjust your trading preferences:`,
    ].join('\n'), settingsKeyboard(user.settings));
  });
}

module.exports = { registerCommands };
