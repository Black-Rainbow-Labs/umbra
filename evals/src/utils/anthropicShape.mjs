// Helpers that read and adjust Anthropic Messages API requests and responses (task-2643). They
// record the same fields the OpenAI helpers in chatShape.mjs record, so the scorecard reads both.

import { messageHashes, contentText } from "./chatShape.mjs";
import { CLAUDE_CODE_IDENTITY } from "../repositories/anthropicCredentials.mjs";

/**
 * Returns true for the request path of a Messages API call.
 * @param method - the HTTP method
 * @param url - the request URL
 */
export function isMessagesRequest(method, url) {
  return method === "POST" && /\/v1\/messages$/.test(url.split("?")[0]);
}

/**
 * Measures the parts of a Messages request the scorecard reports. The system prompt is the
 * top level `system` field, so it counts as one leading message for the message hashes.
 * @param body - the parsed request JSON
 */
export function describeAnthropicRequest(body) {
  const messages = Array.isArray(body.messages) ? body.messages : [];
  const tools = Array.isArray(body.tools) ? body.tools : [];
  const systemText = typeof body.system === "string" ? body.system : contentText(body.system);
  const hashed = body.system ? [{ role: "system", content: body.system }, ...messages] : messages;
  return {
    messageCount: hashed.length,
    messageHashes: messageHashes(hashed),
    toolCount: tools.length,
    systemBytes: Buffer.byteLength(systemText, "utf8"),
    toolsBytes: tools.length ? Buffer.byteLength(JSON.stringify(tools), "utf8") : 0,
    stream: body.stream === true,
  };
}

/**
 * Records what the agent itself asked for, before the proxy changes it.
 * @param body - the parsed request JSON
 */
export function anthropicAgentSettings(body) {
  return { temperature: body.temperature ?? null, top_p: body.top_p ?? null, top_k: body.top_k ?? null, max_tokens: body.max_tokens ?? null, thinking: body.thinking ?? null, effort: body.output_config?.effort ?? null, cacheControl: body.cache_control ?? null };
}

/**
 * Applies the run's model settings to a Messages request: no sampling fields (Claude models
 * reject anything but their defaults), adaptive thinking at the model's default effort for both
 * agents, and the Claude Code identity first in the system prompt when a login token is used.
 * Messages, tools, cache markers and max_tokens are left as the agent sent them.
 * @param body - the parsed request JSON, changed in place
 * @param sampling - the AnthropicSampling from config.mjs
 * @param credential - the credential the proxy sends
 */
export function applyAnthropicPolicy(body, sampling, credential) {
  delete body.temperature;
  delete body.top_p;
  delete body.top_k;
  if (sampling.thinking) body.thinking = { ...sampling.thinking };
  if (sampling.dropEffort && body.output_config) {
    delete body.output_config.effort;
    if (!Object.keys(body.output_config).length) delete body.output_config;
  }
  if (credential?.kind === "oauth") body.system = withIdentity(body.system);
  return body;
}

/**
 * Puts the Claude Code identity block first in a system prompt that does not start with it.
 * @param system - the request's system field: a string, a list of blocks, or undefined
 */
function withIdentity(system) {
  const identity = { type: "text", text: CLAUDE_CODE_IDENTITY };
  if (!system) return [identity];
  const blocks = typeof system === "string" ? [{ type: "text", text: system }] : system;
  if (blocks[0]?.text === CLAUDE_CODE_IDENTITY) return blocks;
  return [identity, ...blocks];
}

/**
 * Normalizes Anthropic usage into the scorecard's numbers. Prompt tokens are every input token,
 * cached or not; cached tokens are the cache reads. The API does not report thinking tokens on
 * their own (they are inside output_tokens), so reasoning_tokens is null.
 * @param usage - the merged usage of a response, or undefined
 */
export function normalizeAnthropicUsage(usage) {
  if (!usage) return null;
  const read = usage.cache_read_input_tokens ?? 0;
  const write = usage.cache_creation_input_tokens ?? 0;
  return {
    prompt_tokens: (usage.input_tokens ?? 0) + read + write,
    cached_tokens: read,
    cache_write_tokens: write,
    completion_tokens: usage.output_tokens ?? 0,
    reasoning_tokens: null,
  };
}

/**
 * Creates a stream reader for the Messages API's SSE events that keeps the facts the proxy
 * records. First content is the first content block the model starts, of any kind.
 * @param onFirstContent - called once when the first content block starts
 */
export function createAnthropicSseTracker(onFirstContent) {
  const state = { buffer: "", usage: null, timings: null, finishReason: null, hasToolCall: false, sawContent: false, chunks: 0, streamError: null };
  const push = (text) => {
    state.buffer += text;
    let index = state.buffer.indexOf("\n");
    while (index >= 0) {
      const line = state.buffer.slice(0, index).trim();
      state.buffer = state.buffer.slice(index + 1);
      if (line.startsWith("data:")) readAnthropicEvent(state, line.slice(5).trim(), onFirstContent);
      index = state.buffer.indexOf("\n");
    }
  };
  return { push, state };
}

/**
 * Reads one SSE data payload and updates the tracker state.
 * @param state - the tracker state
 * @param data - the text after "data:"
 * @param onFirstContent - called once on the first content block
 */
function readAnthropicEvent(state, data, onFirstContent) {
  let event;
  try { event = JSON.parse(data); } catch { return; }
  state.chunks += 1;
  if (event.type === "message_start") state.usage = { ...(event.message?.usage || {}) };
  if (event.type === "message_delta") {
    state.usage = { ...(state.usage || {}), ...(event.usage || {}) };
    if (event.delta?.stop_reason) state.finishReason = event.delta.stop_reason;
  }
  if (event.type === "content_block_start") {
    if (event.content_block?.type === "tool_use") state.hasToolCall = true;
    if (!state.sawContent) { state.sawContent = true; onFirstContent(); }
  }
  if (event.type === "error") state.streamError = event.error?.type || "error";
}

/**
 * Reads the recorded facts from a complete, non streaming Messages response.
 * @param json - the parsed response JSON
 */
export function describeAnthropicJson(json) {
  const content = Array.isArray(json?.content) ? json.content : [];
  return { usage: json?.usage || null, timings: null, finishReason: json?.stop_reason || null, hasToolCall: content.some((block) => block.type === "tool_use") };
}
