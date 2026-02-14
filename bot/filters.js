const BOT_TOKEN = process.env.TELEGRAM_BOT_TOKEN || "8562369283:AAEG2hfV6vOCzSwcxEmpHtVBYxRxBYS_ejI";
const BASE = `https://api.telegram.org/bot${BOT_TOKEN}`;

const MINT = "F8qWTN8JfyDCvj4RoCHuvNMVbTV9XQksLuziA8PYpump";

const responses = {
  "/price": `📊 *SENDIT Token*\n\n• Mint: \`${MINT}\`\n• Chain: Solana\n• Platform: Pump\\.fun\n\n[View on Pump\\.fun](https://pump.fun/coin/${MINT})\n[View on DexScreener](https://dexscreener.com/solana/${MINT})`,

  "/links": `🔗 *Official Links*\n\n🟢 [Pump\\.fun Token](https://pump.fun/coin/${MINT})\n📦 [GitHub](https://github.com/joebower1983-a11y/send_it)\n🌐 [Live Demo](https://send-it-seven-sigma.vercel.app)\n💬 [Discord](https://discord.gg/vKRTyG85)\n🐦 [Twitter](https://twitter.com/SendItSolana420)\n📱 [Telegram](https://t.me/+Xw4E2sJ0Z3Q5ZDYx)`,

  "/tokeninfo": `💰 *SENDIT Token Info*\n\n• Name: Send It\n• Ticker: SENDIT\n• Chain: Solana\n• Mint: \`${MINT}\`\n\n*Fee Structure \\(launchpad\\):*\n• 1% platform fee → treasury\n• 1% creator fee → token creators\n• Holder rewards → redistributed\n\n*Modules:* 29 on\\-chain \\| 13k\\+ lines of Rust`,

  "/rules": `📜 *Group Rules*\n\n1️⃣ Be respectful\n2️⃣ No scams, phishing, or unsolicited DMs\n3️⃣ No shilling other projects\n4️⃣ Nothing here is financial advice — DYOR\n5️⃣ English only\n6️⃣ No spam\n7️⃣ Have fun and send it\\! 🚀\n\n_Breaking rules \\= warn → mute → ban_`,

  "/website": `🌐 *Send\\.it Website*\n\n• Main: [send\\-it\\-seven\\-sigma\\.vercel\\.app](https://send-it-seven-sigma.vercel.app)\n• GitHub Pages: [joebower1983\\-a11y\\.github\\.io/send\\_it](https://joebower1983-a11y.github.io/send_it/)\n• Pitch Deck: [View](https://joebower1983-a11y.github.io/send_it/pitch-deck.html)`,

  "/chart": `📈 *SENDIT Chart*\n\n[DexScreener](https://dexscreener.com/solana/${MINT})\n[Pump\\.fun](https://pump.fun/coin/${MINT})\n[Birdeye](https://birdeye.so/token/${MINT}?chain=solana)`,

  "/buy": `🛒 *How to Buy SENDIT*\n\n1\\. Get a Solana wallet \\(Phantom, Solflare\\)\n2\\. Fund it with SOL\n3\\. Go to [Pump\\.fun](https://pump.fun/coin/${MINT})\n4\\. Connect wallet and buy\\!\n\n⚠️ _DYOR \\- This is not financial advice_`,

  "/socials": `📱 *Send\\.it Socials*\n\n🐦 Twitter: [@SendItSolana420](https://twitter.com/SendItSolana420)\n💬 Discord: [discord\\.gg/vKRTyG85](https://discord.gg/vKRTyG85)\n📱 Telegram: [Join Group](https://t.me/+Xw4E2sJ0Z3Q5ZDYx)\n📦 GitHub: [send\\_it](https://github.com/joebower1983-a11y/send_it)`,

  "/whitepaper": `📄 *Send\\.it Whitepaper v2\\.0*\n\nRead the full whitepaper covering all 29 modules:\n[View on GitHub](https://github.com/joebower1983-a11y/send_it/blob/main/docs/WHITEPAPER.md)`,

  "/ca": `📋 *Contract Address*\n\n\`F8qWTN8JfyDCvj4RoCHuvNMVbTV9XQksLuziA8PYpump\`\n\n[Buy on Pump\\.fun](https://pump.fun/coin/F8qWTN8JfyDCvj4RoCHuvNMVbTV9XQksLuziA8PYpump)`,

  "/filters": `🤖 *Bot Commands*\n\n/price — Token price \\& stats\n/ca — Contract address\n/links — Official links\n/tokeninfo — Contract \\& fee info\n/rules — Group rules\n/website — Send\\.it website\n/chart — Price charts\n/buy — How to buy SENDIT\n/socials — Social media links\n/whitepaper — Read the whitepaper\n/roadmap — Project roadmap\n/filters — This list`,

  "/roadmap": `🗺️ *Send\\.it Roadmap*\n\n*Q1 2026* ← WE ARE HERE\n• Core program \\+ community building\n• Token launch on Pump\\.fun ✅\n• Grant applications ✅\n\n*Q2 2026*\n• Mainnet deployment\n• First token launches\n• Mobile PWA\n\n*Q3 2026*\n• DeFi suite live \\(staking, lending, perps\\)\n• Solana dApp Store\n\n*Q4 2026*\n• Cross\\-chain bridge\n• DAO governance\n• Ecosystem partnerships`
};

// Anti-spam: block common scam patterns
const SPAM_PATTERNS = [
  /airdrop.*claim/i,
  /connect.*wallet.*verify/i,
  /dm me for/i,
  /send \d+ sol/i,
  /free (nft|token|sol|crypto)/i,
  /t\.me\/(?!.*SendIt)/i, // other telegram links
  /bit\.ly|tinyurl/i,
];

let offset = 0;

async function poll() {
  try {
    const res = await fetch(`${BASE}/getUpdates?offset=${offset}&timeout=30`);
    const data = await res.json();
    
    if (!data.ok) return;
    
    for (const update of data.result) {
      offset = update.update_id + 1;
      const msg = update.message;
      if (!msg || !msg.text) continue;
      
      const text = msg.text.trim();
      const chatId = msg.chat.id;
      
      // Check for spam
      if (SPAM_PATTERNS.some(p => p.test(text))) {
        try {
          await fetch(`${BASE}/deleteMessage`, {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ chat_id: chatId, message_id: msg.message_id })
          });
          console.log(`Deleted spam from ${msg.from?.username || msg.from?.id}`);
        } catch (e) {}
        continue;
      }
      
      // Check for commands
      const cmd = text.split("@")[0].split(" ")[0].toLowerCase();
      if (responses[cmd]) {
        await fetch(`${BASE}/sendMessage`, {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({
            chat_id: chatId,
            text: responses[cmd],
            parse_mode: "MarkdownV2",
            disable_web_page_preview: true,
            reply_to_message_id: msg.message_id
          })
        });
        console.log(`Replied to ${cmd} from ${msg.from?.username || msg.from?.id}`);
      }
    }
  } catch (err) {
    console.error("Poll error:", err.message);
  }
}

console.log("🤖 Send.it Bot running with filters...");
setInterval(poll, 1000);
poll();
