const { buyToken } = require('./solana');

class SniperEngine {
  constructor(store) {
    this.store = store;
    this._interval = null;
  }

  start() {
    // In production: subscribe to on-chain program logs via WebSocket
    // connection.onLogs(PROGRAM_ID, ...) to detect new token launches in real-time
    // For now, poll every 5s as a placeholder
    this._interval = setInterval(() => this._poll(), 5000);
  }

  stop() {
    if (this._interval) clearInterval(this._interval);
  }

  async _poll() {
    // TODO: Check for new token creation events on-chain
    // For each new token, check which users have sniper enabled and auto-buy
  }

  async snipe(chatId, mint) {
    const user = this.store.getUser(chatId);
    if (!user.wallet || !user.settings.sniperEnabled) return null;
    const amount = user.settings.sniperAmount || 0.1;
    try {
      const result = await buyToken(user, mint, amount);
      if (result.success) {
        // Update holdings
        if (!user.holdings[mint]) user.holdings[mint] = { amount: 0, avgCost: 0 };
        user.holdings[mint].amount += result.tokensReceived;
        user.holdings[mint].avgCost = amount / result.tokensReceived;
        this.store.setUser(chatId, { holdings: user.holdings });
        this.store.addTrade({ chatId, mint, side: 'buy', sol: amount, tokens: result.tokensReceived, price: result.price, txSig: result.txSig });
      }
      return result;
    } catch (e) {
      console.error('Snipe failed:', e.message);
      return null;
    }
  }
}

module.exports = { SniperEngine };
