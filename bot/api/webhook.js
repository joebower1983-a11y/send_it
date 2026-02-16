let handleUpdate;
try {
  handleUpdate = require("../bot-logic").handleUpdate;
} catch (err) {
  console.error("FATAL: Failed to load bot-logic:", err.message, err.stack);
}

module.exports = async function handler(req, res) {
  if (req.method !== "POST") {
    return res.status(200).json({ ok: true, method: req.method, botLoaded: !!handleUpdate, hasRedisUrl: !!process.env.UPSTASH_REDIS_REST_URL });
  }

  if (!handleUpdate) {
    console.error("handleUpdate not loaded — bot-logic import failed");
    return res.status(200).json({ ok: false, error: "bot-logic not loaded" });
  }

  try {
    const update = req.body;
    console.log("Received update:", JSON.stringify(update).slice(0, 200));
    await handleUpdate(update);
    console.log("Update processed OK");
    return res.status(200).json({ ok: true });
  } catch (err) {
    console.error("Webhook error:", err.message, err.stack);
    return res.status(200).json({ ok: false, error: err.message });
  }
};
