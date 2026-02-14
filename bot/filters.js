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

// Captcha system for new members
const pendingCaptcha = new Map(); // userId -> { chatId, msgId, answer, timeout, joinMsgId }

function generateCaptcha() {
  const a = Math.floor(Math.random() * 10) + 1;
  const b = Math.floor(Math.random() * 10) + 1;
  return { question: `${a} + ${b}`, answer: String(a + b) };
}

async function handleNewMember(msg) {
  const chatId = msg.chat.id;
  for (const member of msg.new_chat_members || []) {
    if (member.is_bot) continue;
    
    const userId = member.id;
    const name = member.first_name || "New member";
    const captcha = generateCaptcha();
    
    // Restrict user until they solve captcha
    try {
      await fetch(`${BASE}/restrictChatMember`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          chat_id: chatId,
          user_id: userId,
          permissions: {
            can_send_messages: false,
            can_send_audios: false,
            can_send_documents: false,
            can_send_photos: false,
            can_send_videos: false,
            can_send_video_notes: false,
            can_send_voice_notes: false,
            can_send_polls: false,
            can_send_other_messages: false,
            can_add_web_page_previews: false,
            can_invite_users: false
          }
        })
      });
    } catch (e) {}
    
    // Send captcha challenge
    const res = await fetch(`${BASE}/sendMessage`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        chat_id: chatId,
        text: `👋 Welcome ${name}\\!\n\n🔒 To verify you're human, solve this:\n\n*What is ${captcha.question.replace('+', '\\+')} \\?*\n\nReply with the answer within 60 seconds or you'll be removed\\.`,
        parse_mode: "MarkdownV2"
      })
    });
    const data = await res.json();
    const captchaMsgId = data.ok ? data.result.message_id : null;
    
    // Set timeout to kick if not solved in 60s
    const timeout = setTimeout(async () => {
      if (pendingCaptcha.has(userId)) {
        pendingCaptcha.delete(userId);
        try {
          await fetch(`${BASE}/banChatMember`, {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ chat_id: chatId, user_id: userId, until_date: Math.floor(Date.now()/1000) + 60 })
          });
          if (captchaMsgId) {
            await fetch(`${BASE}/deleteMessage`, {
              method: "POST",
              headers: { "Content-Type": "application/json" },
              body: JSON.stringify({ chat_id: chatId, message_id: captchaMsgId })
            });
          }
          console.log(`Kicked ${name} (${userId}) - captcha timeout`);
        } catch (e) {}
      }
    }, 60000);
    
    pendingCaptcha.set(userId, { chatId, msgId: captchaMsgId, answer: captcha.answer, timeout });
    console.log(`Captcha sent to ${name} (${userId}): ${captcha.question} = ${captcha.answer}`);
  }
}

async function checkCaptchaAnswer(msg) {
  const userId = msg.from.id;
  if (!pendingCaptcha.has(userId)) return false;
  
  const { chatId, msgId, answer, timeout } = pendingCaptcha.get(userId);
  const text = msg.text.trim();
  
  if (text === answer) {
    // Correct - unrestrict user
    pendingCaptcha.delete(userId);
    clearTimeout(timeout);
    
    // Restore permissions
    await fetch(`${BASE}/restrictChatMember`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        chat_id: chatId,
        user_id: userId,
        permissions: {
          can_send_messages: true,
          can_send_audios: false,
          can_send_documents: false,
          can_send_photos: true,
          can_send_videos: false,
          can_send_video_notes: false,
          can_send_voice_notes: false,
          can_send_polls: false,
          can_send_other_messages: true,
          can_add_web_page_previews: true,
          can_invite_users: true
        }
      })
    });
    
    // Delete captcha message and answer
    try {
      if (msgId) await fetch(`${BASE}/deleteMessage`, { method: "POST", headers: {"Content-Type":"application/json"}, body: JSON.stringify({chat_id: chatId, message_id: msgId}) });
      await fetch(`${BASE}/deleteMessage`, { method: "POST", headers: {"Content-Type":"application/json"}, body: JSON.stringify({chat_id: chatId, message_id: msg.message_id}) });
    } catch (e) {}
    
    // Welcome them
    const welcomeRes = await fetch(`${BASE}/sendMessage`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        chat_id: chatId,
        text: `✅ *Verified\\!* Welcome to Send\\.it, ${(msg.from.first_name || "anon").replace(/[_*[\]()~`>#+\-=|{}.!]/g, '\\$&')}\\! 🚀\n\nType /filters to see available commands\\.`,
        parse_mode: "MarkdownV2"
      })
    });
    
    console.log(`${msg.from.first_name} (${userId}) passed captcha`);
    return true;
  } else {
    // Wrong answer - delete their message, let them try again
    try {
      await fetch(`${BASE}/deleteMessage`, { method: "POST", headers: {"Content-Type":"application/json"}, body: JSON.stringify({chat_id: chatId, message_id: msg.message_id}) });
    } catch (e) {}
    return true;
  }
}

let offset = 0;

async function poll() {
  try {
    const res = await fetch(`${BASE}/getUpdates?offset=${offset}&timeout=30`);
    const data = await res.json();
    
    if (!data.ok) return;
    
    for (const update of data.result) {
      offset = update.update_id + 1;
      const msg = update.message;
      if (!msg) continue;
      
      // Handle new members
      if (msg.new_chat_members && msg.new_chat_members.length > 0) {
        await handleNewMember(msg);
        continue;
      }
      
      if (!msg.text) continue;
      
      // Check captcha answers first
      if (pendingCaptcha.has(msg.from?.id)) {
        await checkCaptchaAnswer(msg);
        continue;
      }
      
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
