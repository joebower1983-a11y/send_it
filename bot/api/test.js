module.exports = async function handler(req, res) {
  const BOT_TOKEN = process.env.TELEGRAM_BOT_TOKEN || "8562369283:AAEG2hfV6vOCzSwcxEmpHtVBYxRxBYS_ejI";
  const BASE = `https://api.telegram.org/bot${BOT_TOKEN}`;

  try {
    const r = await fetch(`${BASE}/sendMessage`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ chat_id: 7920028061, text: "🧪 Test from Vercel function" })
    });
    const data = await r.json();
    return res.status(200).json({ ok: true, telegram: data });
  } catch (err) {
    return res.status(200).json({ ok: false, error: err.message });
  }
};
