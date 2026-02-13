const fs = require('fs');
const path = require('path');

const DB_PATH = path.join(__dirname, '..', 'data', 'db.json');

class Store {
  constructor() {
    this._data = { users: {}, tokens: {}, trades: [] };
    this._load();
  }

  _load() {
    try {
      if (fs.existsSync(DB_PATH)) {
        this._data = JSON.parse(fs.readFileSync(DB_PATH, 'utf8'));
      }
    } catch {}
  }

  _save() {
    const dir = path.dirname(DB_PATH);
    if (!fs.existsSync(dir)) fs.mkdirSync(dir, { recursive: true });
    fs.writeFileSync(DB_PATH, JSON.stringify(this._data, null, 2));
  }

  // ── Users ──
  getUser(chatId) {
    const id = String(chatId);
    if (!this._data.users[id]) {
      this._data.users[id] = {
        chatId: id,
        wallet: null,
        privateKey: null,
        settings: { slippage: 1, defaultBuy: 0.1, sniperEnabled: false, sniperAmount: 0.1 },
        holdings: {},
        createdAt: Date.now(),
      };
      this._save();
    }
    return this._data.users[id];
  }

  setUser(chatId, patch) {
    const user = this.getUser(chatId);
    Object.assign(user, patch);
    this._save();
    return user;
  }

  allUsers() {
    return Object.values(this._data.users);
  }

  // ── Tokens ──
  addToken(token) {
    this._data.tokens[token.mint] = { ...token, createdAt: Date.now(), volume24h: 0, mcap: 0, holders: 0, graduated: false };
    this._save();
    return this._data.tokens[token.mint];
  }

  getToken(mint) {
    return this._data.tokens[mint] || null;
  }

  allTokens() {
    return Object.values(this._data.tokens);
  }

  updateToken(mint, patch) {
    if (this._data.tokens[mint]) {
      Object.assign(this._data.tokens[mint], patch);
      this._save();
    }
    return this._data.tokens[mint];
  }

  // ── Trades ──
  addTrade(trade) {
    this._data.trades.push({ ...trade, ts: Date.now() });
    this._save();
  }

  tradesForUser(chatId) {
    return this._data.trades.filter(t => String(t.chatId) === String(chatId));
  }

  // ── Trending / New ──
  trending(limit = 10) {
    return this.allTokens()
      .sort((a, b) => (b.volume24h || 0) - (a.volume24h || 0))
      .slice(0, limit);
  }

  newest(limit = 10) {
    return this.allTokens()
      .sort((a, b) => b.createdAt - a.createdAt)
      .slice(0, limit);
  }
}

module.exports = { Store };
