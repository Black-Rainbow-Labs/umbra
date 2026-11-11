// proxycheck: checks that the measuring proxy streams SSE through without buffering and records
// usage, and optionally that every chat request of a finished run has usage recorded.

import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { startProxy } from "./proxy.mjs";
import { UPSTREAM, DEFAULT_MODEL, DEFAULT_SAMPLING } from "../utils/config.mjs";
import { readJsonl } from "../utils/files.mjs";
import { loadRunCells } from "../repositories/runRepository.mjs";

/**
 * Sends one streaming request through the proxy and records when each chunk arrived.
 * @param baseUrl - the proxy base URL
 */
async function streamThroughProxy(baseUrl) {
  const started = Date.now();
  const body = { model: DEFAULT_MODEL, stream: true, max_tokens: 400, messages: [{ role: "user", content: "Count from 1 to 120, separated by spaces." }] };
  const response = await fetch(`${baseUrl}/v1/chat/completions`, { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify(body) });
  const arrivals = [];
  const decoder = new TextDecoder();
  let text = "";
  for await (const part of response.body) {
    arrivals.push(Date.now() - started);
    text += decoder.decode(part, { stream: true });
  }
  return { arrivals, totalMs: Date.now() - started, sawDone: text.includes("[DONE]"), status: response.status };
}

/**
 * Runs the live streaming check against the model server.
 */
async function liveCheck() {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "umbra-evals-proxycheck-"));
  const requestsPath = path.join(dir, "requests.jsonl");
  const proxy = await startProxy({ upstreamHost: UPSTREAM.host, upstreamPort: UPSTREAM.port, sampling: { ...DEFAULT_SAMPLING, enableThinking: false }, requestsPath });
  const stream = await streamThroughProxy(proxy.baseUrl);
  await proxy.close();
  const record = readJsonl(requestsPath)[0] || {};
  fs.rmSync(dir, { recursive: true, force: true });
  const firstChunkShare = stream.arrivals.length ? stream.arrivals[0] / stream.totalMs : 1;
  const checks = [
    ["HTTP 200", stream.status === 200], ["stream ended with [DONE]", stream.sawDone],
    [`chunks arrived over time (${stream.arrivals.length} reads, first at ${stream.arrivals[0]} ms of ${stream.totalMs} ms)`, stream.arrivals.length > 5 && firstChunkShare < 0.5],
    [`usage recorded (${JSON.stringify(record.usage)})`, Boolean(record.usage && record.usage.prompt_tokens > 0 && record.usage.completion_tokens > 0)],
    ["stream_options.include_usage was added (agent did not ask for it)", Boolean(record.usage)],
    [`timings recorded`, Boolean(record.timings)],
  ];
  return checks;
}

/**
 * Checks that every chat request in a run's cells has usage, per agent.
 * @param runId - the run id
 */
function runCheck(runId) {
  const checks = [];
  const cells = loadRunCells(runId);
  for (const agent of [...new Set(cells.map((c) => c.result.agent))]) {
    const chats = cells.filter((c) => c.result.agent === agent).flatMap((c) => c.requests.filter((r) => r.kind === "chat" && !r.aborted));
    const streamed = chats.filter((r) => r.stream).length;
    checks.push([`${agent}: ${chats.length} chat requests (${streamed} streamed), all with usage`, chats.length > 0 && chats.every((r) => r.usage && r.usage.prompt_tokens > 0)]);
  }
  return checks;
}

/**
 * Runs the proxy checks and prints one line per check. Returns true when all pass.
 * @param runId - a finished run to scan for usage records, or null
 */
export async function runProxyCheck(runId) {
  const checks = [...(await liveCheck()), ...(runId ? runCheck(runId) : [])];
  for (const [label, ok] of checks) console.log(`${ok ? "ok  " : "FAIL"} ${label}`);
  return checks.every(([, ok]) => ok);
}
