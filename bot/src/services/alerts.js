const { getTokenPrice } = require('./solana');

const GRADUATION_THRESHOLD = 85; // alert when token is 85%+ to graduation

class AlertEngine {
  constructor(bot, store) {
    this.bot = bot;
    this.store = store;
    this._interval = null;
    this._alerted = new Set(); // track already-alerted mints
  }

  start() {
    this._interval = setInterval(() => this._check(), 30000); // check every 30s
  }

  stop() {
    if (this._interval) clearInterval(this._interval);
  }

  async _check() {
    const tokens = this.store.allTokens().filter(t => !t.graduated);
    for (const token of tokens) {
      try {
        const info = await getTokenPrice(token.mint);
        this.store.updateToken(token.mint, { volume24h: info.volume24h, mcap: info.mcap });

        if (info.graduationPct >= GRADUATION_THRESHOLD && !this._alerted.has(token.mint)) {
          this._alerted.add(token.mint);
          await this._notifyAll(token, info);
        }
        if (info.graduationPct >= 100) {
          this.store.updateToken(token.mint, { graduated: true });
        }
      } catch {}
    }
  }

  async _notifyAll(token, info) {
    const msg = [
      `🎓 <b>GRADUATION ALERT!</b>`,
      ``,
      `<b>${token.name}</b> ($${token.symbol}) is at <b>${info.graduationPct}%</b> to graduation!`,
      `💰 MCap: $${(info.mcap || 0).toLocaleString()}`,
      `📊 24h Vol: $${(info.volume24h || 0).toLocaleString()}`,
      ``,
      `⚡ This token is about to hit Raydium!`,
    ].join('\n');

    for (const user of this.store.allUsers()) {
      try {
        await this.bot.telegram.sendMessage(user.chatId, msg, { parse_mode: 'HTML' });
      } catch {}
    }
  }
}

module.exports = { AlertEngine };
