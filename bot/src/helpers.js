function shortAddr(addr) {
  if (!addr) return '—';
  return addr.slice(0, 4) + '...' + addr.slice(-4);
}

function solFmt(n) {
  return Number(n).toFixed(4);
}

function usdFmt(n) {
  return '$' + Number(n).toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 });
}

function pctFmt(n) {
  const sign = n >= 0 ? '+' : '';
  return sign + n.toFixed(2) + '%';
}

function pnlEmoji(pnl) {
  if (pnl > 0) return '🟢';
  if (pnl < 0) return '🔴';
  return '⚪';
}

function buyButtons(mint) {
  return {
    reply_markup: {
      inline_keyboard: [
        [
          { text: '🛒 0.1 SOL', callback_data: `buy:${mint}:0.1` },
          { text: '🛒 0.5 SOL', callback_data: `buy:${mint}:0.5` },
          { text: '🛒 1 SOL', callback_data: `buy:${mint}:1` },
        ],
        [
          { text: '📈 Price', callback_data: `price:${mint}` },
          { text: '💰 Sell All', callback_data: `sell_all:${mint}` },
        ],
      ],
    },
  };
}

function settingsKeyboard(settings) {
  return {
    reply_markup: {
      inline_keyboard: [
        [
          { text: `Slippage: ${settings.slippage}%`, callback_data: 'noop' },
          { text: '⬇️', callback_data: 'set:slippage:down' },
          { text: '⬆️', callback_data: 'set:slippage:up' },
        ],
        [
          { text: `Default Buy: ${settings.defaultBuy} SOL`, callback_data: 'noop' },
          { text: '⬇️', callback_data: 'set:defaultBuy:down' },
          { text: '⬆️', callback_data: 'set:defaultBuy:up' },
        ],
        [
          { text: `🎯 Sniper: ${settings.sniperEnabled ? 'ON ✅' : 'OFF ❌'}`, callback_data: 'set:sniper:toggle' },
          { text: `Amount: ${settings.sniperAmount} SOL`, callback_data: 'noop' },
        ],
        [
          { text: '⬇️ Sniper Amt', callback_data: 'set:sniperAmt:down' },
          { text: '⬆️ Sniper Amt', callback_data: 'set:sniperAmt:up' },
        ],
      ],
    },
  };
}

module.exports = { shortAddr, solFmt, usdFmt, pctFmt, pnlEmoji, buyButtons, settingsKeyboard };
