// The local model server has one slot. Before a batch and before every cell the runner sends one
// tiny request and waits until it answers quickly, so two agent runs never share the slot. On
// the Anthropic upstream (task-2643) the same check confirms the credential works and the API
// answers.

import { UPSTREAM } from "../utils/config.mjs";
import { readAnthropicCredential, withCredential } from "../repositories/anthropicCredentials.mjs";
import { applyAnthropicPolicy } from "../utils/anthropicShape.mjs";

/**
 * Sends one tiny chat request straight to the server and returns how long it took.
 * @param model - the model id
 * @param timeoutMs - give up after this many milliseconds
 */
export async function pingServer(model, timeoutMs) {
  if (UPSTREAM.kind === "anthropic") return pingAnthropic(model, timeoutMs);
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
 * Sends one tiny Messages request to the Anthropic API with the proxy's credential.
 * @param model - the model id
 * @param timeoutMs - give up after this many milliseconds
 */
async function pingAnthropic(model, timeoutMs) {
  const started = Date.now();
  const credential = readAnthropicCredential();
  if (!credential) return { ok: false, ms: 0, status: 0, error: "no Anthropic credential" };
  const body = applyAnthropicPolicy({ model, max_tokens: 16, messages: [{ role: "user", content: "Say hi" }] }, { thinking: null }, credential);
  const headers = withCredential({ "content-type": "application/json", "anthropic-version": "2023-06-01" }, credential);
  try {
    const response = await fetch(`https://${UPSTREAM.host}/v1/messages`, { method: "POST", headers, body: JSON.stringify(body), signal: AbortSignal.timeout(timeoutMs) });
    const text = await response.text();
    return { ok: response.ok, ms: Date.now() - started, status: response.status, error: response.ok ? "" : text.slice(0, 200) };
  } catch (error) {
    return { ok: false, ms: Date.now() - started, status: 0, error: error.message };
  }
}

/**
 * Waits until a tiny request answers within `fastMs`. Logs each slow attempt and waits 30
 * seconds before trying again. The Anthropic API serves many requests at once, so there only an
 * error counts as busy.
 * @param model - the model id
 * @param fastMs - the answer time that counts as idle
 */
export async function waitForIdleServer(model, fastMs = 5000) {
  for (let attempt = 1; ; attempt += 1) {
    const ping = await pingServer(model, 60000);
    if (ping.ok && (UPSTREAM.kind === "anthropic" || ping.ms <= fastMs)) return ping;
    console.log(`  server busy or down (attempt ${attempt}: status ${ping.status}, ${ping.ms} ms ${ping.error || ""}); waiting 30 s`);
    await new Promise((resolve) => setTimeout(resolve, 30000));
  }
}
