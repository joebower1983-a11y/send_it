require('dotenv').config();
const { Telegraf } = require('telegraf');
const { registerCommands } = require('./commands');
const { registerCallbacks } = require('./callbacks');
const { SniperEngine } = require('./services/sniper');
const { AlertEngine } = require('./services/alerts');
const { Store } = require('./store');

const bot = new Telegraf(process.env.BOT_TOKEN);
const store = new Store();
const sniper = new SniperEngine(store);
const alerts = new AlertEngine(bot, store);

// Attach shared state to context
bot.use((ctx, next) => {
  ctx.store = store;
  ctx.sniper = sniper;
  ctx.alerts = alerts;
  return next();
});

registerCommands(bot);
registerCallbacks(bot);

// Graceful shutdown
process.once('SIGINT', () => { sniper.stop(); alerts.stop(); bot.stop('SIGINT'); });
process.once('SIGTERM', () => { sniper.stop(); alerts.stop(); bot.stop('SIGTERM'); });

bot.launch().then(() => {
  console.log('🚀 Send.it bot is live!');
  alerts.start();
});
