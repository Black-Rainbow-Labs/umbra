// Fixed settings of the suite: the model server, the model, and the default sampling.

import os from "node:os";
import path from "node:path";

// EVAL_UPSTREAM=anthropic sends every chat request to the Anthropic Messages API (task-2643).
// Unset, the suite uses the local model server on port 8080, as every run before task-2643 did.
export const UPSTREAM_KIND = process.env.EVAL_UPSTREAM === "anthropic" ? "anthropic" : "local";
export const UPSTREAM = UPSTREAM_KIND === "anthropic" ? { kind: "anthropic", host: "api.anthropic.com", port: 443, tls: true } : { kind: "local", host: "127.0.0.1", port: 8080, tls: false };
export const DEFAULT_MODEL = process.env.EVAL_MODEL || (UPSTREAM_KIND === "anthropic" ? "claude-haiku-5-5" : "ninfer-qwen3.8-27b-coletti-v9");
// EVAL_THINKING_OVERRIDE=0 leaves enable_thinking to the agent, for the thinking policy arms
// (task-2617, E12). Those runs are reported apart from the main comparison.
const THINKING_OVERRIDE = process.env.EVAL_THINKING_OVERRIDE === "0" ? null : true;
// Claude models accept only their default sampling, so the Anthropic upstream sets none of it
// and instead gives both agents adaptive thinking at the model's default effort.
export const DEFAULT_SAMPLING = UPSTREAM_KIND === "anthropic"
  ? { kind: "anthropic", thinking: THINKING_OVERRIDE ? { type: "adaptive" } : null, dropEffort: Boolean(THINKING_OVERRIDE) }
  : { temperature: 0.6, top_p: 0.95, top_k: 20, enableThinking: THINKING_OVERRIDE };
export const WORK_ROOT = process.env.EVAL_WORK_DIR || path.join(os.homedir(), "umbra-evals-work");
