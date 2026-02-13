const { buyToken, sellToken, getTokenPrice, getBalance } = require('./services/solana');
const { solFmt, shortAddr, pnlEmoji, buyButtons, settingsKeyboard } = require('./helpers');

function registerCallbacks(bot) {

  // ── Quick Buy buttons ──
  bot.action(/^buy:(.+):(.+)$/, async (ctx) => {
    const mint = ctx.match[1];
    const amount = parseFloat(ctx.match[2]);
    const user = ctx.store.getUser(ctx.chat.id);
    if (!user.wallet) return ctx.answerCbQuery('⚠️ Run /start first');

    await ctx.answerCbQuery(`⏳ Buying ${amount} SOL...`);
    try {
      const result = await buyToken(user, mint, amount);
      if (!user.holdings[mint]) user.holdings[mint] = { amount: 0, avgCost: 0 };
      const h = user.holdings[mint];
      const totalCost = h.avgCost * h.amount + amount;
      h.amount += result.tokensReceived;
      h.avgCost = totalCost / h.amount;
      ctx.store.setUser(ctx.chat.id, { holdings: user.holdings });
      ctx.store.addTrade({ chatId: ctx.chat.id, mint, side: 'buy', sol: amount, tokens: result.tokensReceived, price: result.price, txSig: result.txSig });

      return ctx.replyWithHTML([
        `✅ <b>Quick Buy!</b>`,
        `🪙 Got: <b>${result.tokensReceived.toLocaleString()}</b> tokens`,
        `💰 Spent: <b>${solFmt(amount)} SOL</b>`,
        `📝 Tx: <code>${result.txSig}</code>`,
      ].join('\n'), buyButtons(mint));
    } catch (e) {
      return ctx.replyWithHTML('❌ Buy failed: ' + e.message);
    }
  });

  // ── Sell All button ──
  bot.action(/^sell_all:(.+)$/, async (ctx) => {
    const mint = ctx.match[1];
    const user = ctx.store.getUser(ctx.chat.id);
    const holding = user.holdings?.[mint];
    if (!holding || holding.amount <= 0) return ctx.answerCbQuery('⚠️ No holdings');

    await ctx.answerCbQuery('⏳ Selling all...');
    try {
      const result = await sellToken(user, mint, holding.amount);
      const amount = holding.amount;
      delete user.holdings[mint];
      ctx.store.setUser(ctx.chat.id, { holdings: user.holdings });
      ctx.store.addTrade({ chatId: ctx.chat.id, mint, side: 'sell', sol: result.solReceived, tokens: amount, price: result.price, txSig: result.txSig });

      return ctx.replyWithHTML([
        `✅ <b>Sold All!</b>`,
        `🪙 Sold: <b>${amount.toLocaleString()}</b> tokens`,
        `💰 Received: <b>${solFmt(result.solReceived)} SOL</b>`,
      ].join('\n'));
    } catch (e) {
      return ctx.replyWithHTML('❌ Sell failed: ' + e.message);
    }
  });

  // ── Price check button ──
  bot.action(/^price:(.+)$/, async (ctx) => {
    const mint = ctx.match[1];
    const token = ctx.store.getToken(mint);
    const info = await getTokenPrice(mint);
    const name = token ? `${token.name} ($${token.symbol})` : shortAddr(mint);

    await ctx.answerCbQuery();
    return ctx.replyWithHTML([
      `📊 <b>${name}</b>`,
      `💰 Price: ${info.price.toFixed(10)} SOL`,
      `📈 MCap: $${info.mcap.toLocaleString()}`,
      `🎓 Graduation: ${info.graduationPct}%`,
    ].join('\n'), buyButtons(mint));
  });

  // ── Command shortcuts from inline buttons ──
  bot.action('cmd:trending', (ctx) => { ctx.answerCbQuery(); return bot.handleUpdate({ ...ctx.update, message: { ...ctx.callbackQuery.message, text: '/trending', entities: [{ type: 'bot_command', offset: 0, length: 9 }] } }); });
  bot.action('cmd:new', (ctx) => { ctx.answerCbQuery(); return bot.handleUpdate({ ...ctx.update, message: { ...ctx.callbackQuery.message, text: '/new', entities: [{ type: 'bot_command', offset: 0, length: 4 }] } }); });
  bot.action('cmd:portfolio', (ctx) => { ctx.answerCbQuery(); return bot.handleUpdate({ ...ctx.update, message: { ...ctx.callbackQuery.message, text: '/portfolio', entities: [{ type: 'bot_command', offset: 0, length: 10 }] } }); });
  bot.action('cmd:settings', (ctx) => { ctx.answerCbQuery(); return bot.handleUpdate({ ...ctx.update, message: { ...ctx.callbackQuery.message, text: '/settings', entities: [{ type: 'bot_command', offset: 0, length: 9 }] } }); });

  // ── Settings adjustments ──
  bot.action(/^set:(.+):(.+)$/, async (ctx) => {
    const [, key, dir] = ctx.match;
    const user = ctx.store.getUser(ctx.chat.id);
    const s = user.settings;

    if (key === 'slippage') {
      s.slippage = Math.max(0.1, Math.min(50, s.slippage + (dir === 'up' ? 0.5 : -0.5)));
    } else if (key === 'defaultBuy') {
      s.defaultBuy = Math.max(0.01, Math.min(100, s.defaultBuy + (dir === 'up' ? 0.1 : -0.1)));
      s.defaultBuy = Math.round(s.defaultBuy * 100) / 100;
    } else if (key === 'sniper') {
      s.sniperEnabled = !s.sniperEnabled;
    } else if (key === 'sniperAmt') {
      s.sniperAmount = Math.max(0.01, Math.min(10, (s.sniperAmount || 0.1) + (dir === 'up' ? 0.1 : -0.1)));
      s.sniperAmount = Math.round(s.sniperAmount * 100) / 100;
    }

    ctx.store.setUser(ctx.chat.id, { settings: s });

    await ctx.answerCbQuery(`Updated!`);
    return ctx.editMessageReplyMarkup(settingsKeyboard(s).reply_markup);
  });

  bot.action('noop', (ctx) => ctx.answerCbQuery());
}

module.exports = { registerCallbacks };
