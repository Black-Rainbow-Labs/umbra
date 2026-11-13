// The measuring proxy. Both agents talk to it, it forwards to the model server, applies the
// same sampling settings to every chat request, and writes one JSON line per request.

import http from "node:http";
import fs from "node:fs";
import { applySampling, describeRequest, normalizeUsage, createSseTracker, describeJsonResponse, rewriteContextFields } from "../utils/chatShape.mjs";

/**
 * Starts a proxy on a free local port. The returned state object can be changed while it runs
 * (phase index and context window), and `onChatRequest` is called with the running chat count.
 * @param options - { upstreamHost, upstreamPort, sampling, requestsPath, onChatRequest }
 */
export async function startProxy(options) {
  const state = { phase: 0, contextWindow: null, chatCount: 0, phaseChatCount: 0, inFlight: 0 };
  const log = fs.createWriteStream(options.requestsPath, { flags: "a" });
  const server = http.createServer((req, res) => handleRequest({ req, res, options, state, log }));
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const port = server.address().port;
  const close = () => closeProxy(server, log);
  return { port, baseUrl: `http://127.0.0.1:${port}`, state, close };
}

/**
 * Stops the server and flushes the request log.
 * @param server - the http server
 * @param log - the requests.jsonl write stream
 */
async function closeProxy(server, log) {
  server.closeAllConnections?.();
  await new Promise((resolve) => server.close(() => resolve()));
  await new Promise((resolve) => log.end(resolve));
}

/**
 * Reads a whole request body into a buffer.
 * @param req - the incoming request
 */
function readBody(req) {
  return new Promise((resolve, reject) => {
    const parts = [];
    req.on("data", (part) => parts.push(part));
    req.on("end", () => resolve(Buffer.concat(parts)));
    req.on("error", reject);
  });
}

/**
 * Handles one request: adjusts chat bodies, forwards, and records the result.
 * @param ctx - { req, res, options, state, log }
 */
async function handleRequest(ctx) {
  const { req, state } = ctx;
  const started = Date.now();
  let body = await readBody(req);
  const isChat = req.method === "POST" && /\/chat\/completions$/.test(req.url.split("?")[0]);
  const record = baseRecord(req, state, started, isChat);
  if (isChat) {
    body = prepareChatBody(body, ctx.options.sampling, record);
    state.chatCount += 1;
    state.phaseChatCount += 1;
    if (state.phaseChatCount === 1) state.phaseFirstChatAt = started;
    ctx.options.onChatRequest?.(state.phaseChatCount);
  }
  state.inFlight += 1;
  forward({ ...ctx, body, record, started, isChat });
}

/**
 * Builds the fields every request record has before the response arrives.
 * @param req - the incoming request
 * @param state - proxy state
 * @param started - arrival time in ms
 * @param isChat - true for a chat completion request
 */
function baseRecord(req, state, started, isChat) {
  return {
    kind: isChat ? "chat" : "other", method: req.method, path: req.url, phase: state.phase,
    startedAt: new Date(started).toISOString(), firstByteMs: null, firstContentMs: null, durationMs: 0,
    status: 0, usage: null, timings: null, messageCount: 0, toolCount: 0, systemBytes: 0, toolsBytes: 0,
    finishReason: null, hasToolCall: false, windowExceeded: false, contextWindow: state.contextWindow, aborted: false,
  };
}

/**
 * Parses a chat body, applies sampling, and copies its shape into the record.
 * @param raw - the raw request body
 * @param sampling - the SamplingConfig
 * @param record - the request record, changed in place
 */
function prepareChatBody(raw, sampling, record) {
  let parsed;
  try { parsed = JSON.parse(raw.toString("utf8")); } catch { return raw; }
  Object.assign(record, describeRequest(parsed));
  record.agentSampling = { temperature: parsed.temperature ?? null, top_p: parsed.top_p ?? null, top_k: parsed.top_k ?? null, max_tokens: parsed.max_tokens ?? parsed.max_completion_tokens ?? null, enable_thinking: parsed.chat_template_kwargs?.enable_thinking ?? null };
  return Buffer.from(JSON.stringify(applySampling(parsed, sampling)), "utf8");
}

/**
 * Sends the request upstream and relays the response back to the agent.
 * @param ctx - request context with body, record, started and isChat
 */
function forward(ctx) {
  const { req, res, options, body } = ctx;
  const headers = { ...req.headers, host: `${options.upstreamHost}:${options.upstreamPort}`, "content-length": String(body.length) };
  delete headers["accept-encoding"];
  const upstream = http.request({ host: options.upstreamHost, port: options.upstreamPort, method: req.method, path: req.url, headers }, (up) => relayResponse(ctx, up));
  upstream.on("error", (error) => failRequest(ctx, error));
  res.on("close", () => {
    if (res.writableFinished) return;
    ctx.record.aborted = true;
    upstream.destroy();
    finishRecord(ctx);
  });
  upstream.end(body);
}

/**
 * Reports an upstream connection error to the agent as HTTP 502 and records it.
 * @param ctx - request context
 * @param error - the connection error
 */
function failRequest(ctx, error) {
  if (!ctx.res.headersSent) { ctx.res.writeHead(502, { "content-type": "application/json" }); }
  ctx.res.end(JSON.stringify({ error: { message: `proxy upstream error: ${error.message}` } }));
  ctx.record.status = ctx.record.status || 502;
  ctx.record.error = error.message;
  finishRecord(ctx);
}

/**
 * Relays an upstream response. Streams are passed through chunk by chunk, JSON is buffered so
 * that metadata responses can have their context size rewritten.
 * @param ctx - request context
 * @param up - the upstream response
 */
function relayResponse(ctx, up) {
  const { res, record, started } = ctx;
  record.status = up.statusCode;
  const isSse = String(up.headers["content-type"] || "").includes("text/event-stream");
  if (isSse) return relayStream(ctx, up);
  const parts = [];
  up.on("data", (part) => { if (record.firstByteMs === null) record.firstByteMs = Date.now() - started; parts.push(part); });
  up.on("end", () => {
    const out = finishJsonBody(ctx, Buffer.concat(parts), up.headers);
    const headers = { ...up.headers, "content-length": String(out.length) };
    res.writeHead(up.statusCode, headers);
    res.end(out);
    finishRecord(ctx);
  });
  up.on("error", (error) => failRequest(ctx, error));
}

/**
 * Passes an SSE stream through without buffering while tracking usage and first content time.
 * @param ctx - request context
 * @param up - the upstream response
 */
function relayStream(ctx, up) {
  const { res, record, started } = ctx;
  const tracker = createSseTracker(() => { record.firstContentMs = Date.now() - started; });
  res.writeHead(up.statusCode, up.headers);
  res.flushHeaders?.();
  up.on("data", (part) => {
    if (record.firstByteMs === null) record.firstByteMs = Date.now() - started;
    tracker.push(part.toString("utf8"));
    res.write(part);
  });
  up.on("end", () => {
    tracker.push("\n");
    Object.assign(record, { usage: tracker.state.usage, timings: tracker.state.timings, finishReason: tracker.state.finishReason, hasToolCall: tracker.state.hasToolCall });
    res.end();
    finishRecord(ctx);
  });
  up.on("error", (error) => failRequest(ctx, error));
}

/**
 * Reads facts from a buffered JSON body and rewrites context sizes on metadata responses.
 * @param ctx - request context
 * @param raw - the upstream body
 * @param headers - the upstream headers
 */
function finishJsonBody(ctx, raw, headers) {
  const { record, state, isChat } = ctx;
  if (!String(headers["content-type"] || "").includes("json")) return raw;
  let json;
  try { json = JSON.parse(raw.toString("utf8")); } catch { return raw; }
  if (isChat) {
    Object.assign(record, describeJsonResponse(json));
    if (record.firstContentMs === null) record.firstContentMs = record.firstByteMs;
    return raw;
  }
  if (ctx.req.method === "GET" && state.contextWindow) {
    record.rewroteContext = true;
    return Buffer.from(JSON.stringify(rewriteContextFields(json, state.contextWindow)), "utf8");
  }
  return raw;
}

/**
 * Completes a record (duration, normalized usage, window check) and writes it as one JSON line.
 * @param ctx - request context
 */
function finishRecord(ctx) {
  const { record, state, log, started } = ctx;
  if (ctx.finished) return;
  ctx.finished = true;
  state.inFlight -= 1;
  record.durationMs = Date.now() - started;
  record.usage = normalizeUsage(record.usage);
  if (record.usage && record.contextWindow) record.windowExceeded = record.usage.prompt_tokens > record.contextWindow;
  log.write(JSON.stringify(record) + "\n");
}
