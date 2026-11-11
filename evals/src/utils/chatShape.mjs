// Helpers that read and adjust OpenAI chat completion requests and responses.

import { createHash } from "node:crypto";

/**
 * Returns a short hash of every message in a request, in order. Comparing these between two
 * requests of one session shows whether the later one only added messages at the end (the
 * server's prompt cache can reuse everything) or changed an earlier message (a compaction).
 * @param messages - the request's messages
 */
export function messageHashes(messages) {
  return messages.map((message) => createHash("sha1").update(JSON.stringify(message)).digest("hex").slice(0, 12));
}

/**
 * Applies the run's sampling settings to a chat request body. Messages and tools are left alone.
 * @param body - the parsed request JSON, changed in place
 * @param sampling - the SamplingConfig to apply
 */
export function applySampling(body, sampling) {
  body.temperature = sampling.temperature;
  body.top_p = sampling.top_p;
  body.top_k = sampling.top_k;
  if (sampling.enableThinking !== null && sampling.enableThinking !== undefined) body.chat_template_kwargs = { ...(body.chat_template_kwargs || {}), enable_thinking: sampling.enableThinking };
  if (body.stream === true) body.stream_options = { ...(body.stream_options || {}), include_usage: true };
  return body;
}

/**
 * Measures the parts of a chat request the scorecard reports: message count, tool count and byte sizes.
 * @param body - the parsed request JSON
 */
export function describeRequest(body) {
  const messages = Array.isArray(body.messages) ? body.messages : [];
  const tools = Array.isArray(body.tools) ? body.tools : [];
  const systemText = messages.filter((m) => m.role === "system" || m.role === "developer").map((m) => contentText(m.content)).join("");
  return {
    messageCount: messages.length,
    messageHashes: messageHashes(messages),
    toolCount: tools.length,
    systemBytes: Buffer.byteLength(systemText, "utf8"),
    toolsBytes: tools.length ? Buffer.byteLength(JSON.stringify(tools), "utf8") : 0,
    stream: body.stream === true,
  };
}

/**
 * Turns message content, which may be a string or a list of parts, into plain text.
 * @param content - the message content
 */
export function contentText(content) {
  if (typeof content === "string") return content;
  if (Array.isArray(content)) return content.map((part) => (typeof part === "string" ? part : part.text || "")).join("");
  return "";
}

/**
 * Normalizes the server's usage object into the four numbers the scorecard uses.
 * @param usage - the usage object from a response, or undefined
 */
export function normalizeUsage(usage) {
  if (!usage) return null;
  return {
    prompt_tokens: usage.prompt_tokens ?? 0,
    cached_tokens: usage.prompt_tokens_details?.cached_tokens ?? 0,
    completion_tokens: usage.completion_tokens ?? 0,
    reasoning_tokens: usage.completion_tokens_details?.reasoning_tokens ?? 0,
  };
}

/**
 * Creates a stream reader that consumes SSE text and keeps the facts the proxy records.
 * @param onFirstContent - called once when the first content, reasoning or tool call delta arrives
 */
export function createSseTracker(onFirstContent) {
  const state = { buffer: "", usage: null, timings: null, finishReason: null, hasToolCall: false, sawContent: false, chunks: 0 };
  const push = (text) => {
    state.buffer += text;
    let index = state.buffer.indexOf("\n");
    while (index >= 0) {
      const line = state.buffer.slice(0, index).trim();
      state.buffer = state.buffer.slice(index + 1);
      if (line.startsWith("data:")) readSseData(state, line.slice(5).trim(), onFirstContent);
      index = state.buffer.indexOf("\n");
    }
  };
  return { push, state };
}

/**
 * Reads one SSE data payload and updates the tracker state.
 * @param state - the tracker state
 * @param data - the text after "data:"
 * @param onFirstContent - called once on the first content delta
 */
function readSseData(state, data, onFirstContent) {
  if (!data || data === "[DONE]") return;
  let chunk;
  try { chunk = JSON.parse(data); } catch { return; }
  state.chunks += 1;
  if (chunk.usage) state.usage = chunk.usage;
  if (chunk.timings) state.timings = chunk.timings;
  for (const choice of chunk.choices || []) {
    const delta = choice.delta || {};
    if (choice.finish_reason) state.finishReason = choice.finish_reason;
    if (Array.isArray(delta.tool_calls) && delta.tool_calls.length) state.hasToolCall = true;
    const hasText = Boolean(delta.content || delta.reasoning_content || delta.reasoning || state.hasToolCall);
    if (hasText && !state.sawContent) { state.sawContent = true; onFirstContent(); }
  }
}

/**
 * Reads the recorded facts from a complete, non streaming chat response.
 * @param json - the parsed response JSON
 */
export function describeJsonResponse(json) {
  const choice = (json && json.choices && json.choices[0]) || {};
  const toolCalls = choice.message?.tool_calls;
  return {
    usage: json?.usage || null,
    timings: json?.timings || null,
    finishReason: choice.finish_reason || null,
    hasToolCall: Array.isArray(toolCalls) && toolCalls.length > 0,
  };
}

/**
 * Rewrites every context size field in a metadata response to the given window.
 * @param value - parsed JSON from /props, /v1/models or similar, changed in place
 * @param contextWindow - the window to report
 */
export function rewriteContextFields(value, contextWindow) {
  const keys = new Set(["n_ctx", "max_model_len", "context_length", "context_window", "n_ctx_train"]);
  if (Array.isArray(value)) { value.forEach((item) => rewriteContextFields(item, contextWindow)); return value; }
  if (value && typeof value === "object") {
    for (const key of Object.keys(value)) {
      if (keys.has(key) && typeof value[key] === "number") value[key] = contextWindow;
      else rewriteContextFields(value[key], contextWindow);
    }
  }
  return value;
}
