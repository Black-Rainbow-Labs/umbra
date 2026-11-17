// The model server has one slot. Before a batch and before every cell the runner sends one tiny
// request and waits until it answers quickly, so two agent runs never share the slot.

import os from "node:os";
import { spawnSync } from "node:child_process";
import { UPSTREAM } from "../utils/config.mjs";

/**
 * Returns the free commit charge in MB on Windows (what is left before the system commit limit,
 * past which new processes fail to start with 0xC0000142), or Infinity elsewhere. A failed query
 * counts as 0, because a query that cannot start means processes cannot start either.
 */
export function freeCommitMb() {
  if (process.platform !== "win32") return Infinity;
  const out = spawnSync("powershell", ["-NoProfile", "-Command", "(Get-CimInstance Win32_OperatingSystem).FreeVirtualMemory"], { encoding: "utf8", windowsHide: true, timeout: 60000 });
  const kb = Number((out.stdout || "").trim());
  return out.status === 0 && Number.isFinite(kb) && kb > 0 ? Math.round(kb / 1024) : 0;
}

/**
 * Waits while the machine has less free physical memory than `EVAL_MIN_FREE_MB` (6000 by
 * default) or, on Windows, less free commit charge than `EVAL_MIN_FREE_COMMIT_MB` (16000 by
 * default), so a cell never starts while new processes would fail to start. Logs each wait and
 * checks again every 60 seconds.
 */
export async function waitForFreeMemory() {
  const minMb = Number(process.env.EVAL_MIN_FREE_MB || 6000);
  const minCommitMb = Number(process.env.EVAL_MIN_FREE_COMMIT_MB || 16000);
  for (let attempt = 1; ; attempt += 1) {
    const freeMb = Math.round(os.freemem() / 1048576);
    const commitMb = freeMb >= minMb ? freeCommitMb() : 0;
    if (freeMb >= minMb && commitMb >= minCommitMb) return freeMb;
    console.log(`  low memory (attempt ${attempt}: ${freeMb} MB free, need ${minMb}; ${commitMb === Infinity ? "n/a" : commitMb} MB commit free, need ${minCommitMb}); waiting 60 s`);
    await new Promise((resolve) => setTimeout(resolve, 60000));
  }
}

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
