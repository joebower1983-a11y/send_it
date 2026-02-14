const { BOT_TOKEN } = require("../bot-logic");

module.exports = async function handler(req, res) {
  // Derive the webhook URL from the Vercel deployment URL
  const host = req.headers["x-forwarded-host"] || req.headers.host;
  const proto = req.headers["x-forwarded-proto"] || "https";
  const webhookUrl = `${proto}://${host}/api/webhook`;

  const tgRes = await fetch(
    `https://api.telegram.org/bot${BOT_TOKEN}/setWebhook`,
    {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        url: webhookUrl,
        allowed_updates: ["message"],
        drop_pending_updates: true,
      }),
    }
  );
  const data = await tgRes.json();

  res.status(200).json({
    webhookUrl,
    telegram: data,
  });
};
