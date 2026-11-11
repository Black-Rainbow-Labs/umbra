// The model server has one slot. Before a batch and before every cell the runner sends one tiny
// request and waits until it answers quickly, so two agent runs never share the slot.

import { UPSTREAM } from "../utils/config.mjs";

/**
 * Sends one tiny chat request straight to the server and returns how long it took.
 * @param model - the model id
 * @param timeoutMs - give up after this many milliseconds
 */
export async function pingServer(model, timeoutMs) {
  const started = Date.now();
  const body = { model, messages: [{ role: "user", content: "Say hi" }], max_tokens: 4, chat_template_kwargs: { enable_thinking: false } };
  try {
    const response = await fetch(`http://${UPSTREAM.host}:${UPSTREAM.port}/v1/chat/completions`, { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify(body), signal: AbortSignal.timeout(timeoutMs) });
    await response.text();
    return { ok: response.ok, ms: Date.now() - started, status: response.status };
  } catch (error) {
    return { ok: false, ms: Date.now() - started, status: 0, error: error.message };
  }
}

/**
 * Waits until a tiny request answers within `fastMs`. Logs each slow attempt and waits 30
 * seconds before trying again.
 * @param model - the model id
 * @param fastMs - the answer time that counts as idle
 */
export async function waitForIdleServer(model, fastMs = 5000) {
  for (let attempt = 1; ; attempt += 1) {
    const ping = await pingServer(model, 60000);
    if (ping.ok && ping.ms <= fastMs) return ping;
    console.log(`  server busy or down (attempt ${attempt}: status ${ping.status}, ${ping.ms} ms ${ping.error || ""}); waiting 30 s`);
    await new Promise((resolve) => setTimeout(resolve, 30000));
  }
}
