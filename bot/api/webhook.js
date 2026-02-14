const { handleUpdate } = require("../bot-logic");

module.exports = async function handler(req, res) {
  if (req.method !== "POST") {
    return res.status(200).json({ ok: true, method: req.method });
  }

  try {
    const update = req.body;
    await handleUpdate(update);
  } catch (err) {
    console.error("Webhook error:", err);
  }

  // Always return 200 so Telegram doesn't retry
  res.status(200).json({ ok: true });
};
