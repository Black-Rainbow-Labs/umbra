// `node evals/run.mjs profile <run id> [--ref <pi run>]`: where each agent's reasoning tokens go
// (task-2712). Prints, per agent, reasoning tokens by category and by task, per request
// percentiles, the share held by large replies, totals by request position in the cell, requests
// per passed cell by category, and tool calls per reply.

import fs from "node:fs";
import path from "node:path";
import { scorecardCells } from "./scorecardCells.mjs";

const THRESHOLDS = [1000, 2000, 3000, 4000];
const LIMITS = [2000, 3000, 4000, 6000, 8000];
const POSITIONS = [["1", 1, 1], ["2 to 5", 2, 5], ["6 to 20", 6, 20], ["21 and later", 21, Infinity]];

/**
 * Returns the chat requests of a cell with their reasoning tokens.
 * @param cell - { requests }
 */
function chats(cell) {
  return cell.requests.filter((r) => r.kind === "chat").map((r) => r.usage?.reasoning_tokens || 0);
}

/**
 * Returns the value at a percentile of a sorted list.
 * @param sorted - numbers in ascending order
 * @param p - the percentile, 0 to 100
 */
function percentile(sorted, p) {
  if (!sorted.length) return 0;
  return sorted[Math.min(sorted.length - 1, Math.floor((p / 100) * sorted.length))];
}

/**
 * Counts the tool calls of each assistant reply in a cell's transcripts: Umbra's trajectory, or
 * pi's message_end events.
 * @param dir - the cell folder
 */
function toolCallsPerReply(dir) {
  const folders = [dir, ...fs.readdirSync(dir).filter((n) => /^phase-\d+$/.test(n)).map((n) => path.join(dir, n))];
  const counts = [];
  for (const folder of folders) {
    const trajectory = path.join(folder, "trajectory.json");
    const stdout = path.join(folder, "stdout.log");
    if (fs.existsSync(trajectory)) {
      for (const m of JSON.parse(fs.readFileSync(trajectory, "utf8")).messages || []) if (m.role === "assistant") counts.push((m.tool_calls || []).length);
    } else if (fs.existsSync(stdout)) {
      for (const line of fs.readFileSync(stdout, "utf8").split("\n")) {
        if (!line.includes('"type":"message_end"')) continue;
        try { const e = JSON.parse(line); if (e.message?.role === "assistant") counts.push((e.message.content || []).filter((c) => c.type === "toolCall").length); } catch { /* a cut line */ }
      }
    }
  }
  return counts;
}

/**
 * Groups the cells of one agent and sums reasoning tokens per rep under a key.
 * @param cells - one agent's cells
 * @param keyOf - (cell) => group name
 * @param reps - the agent's rep count
 */
function perRepByKey(cells, keyOf, reps) {
  const out = {};
  for (const cell of cells) {
    const entry = (out[keyOf(cell)] ||= { reasoning: 0, requests: 0, cells: 0, passed: 0 });
    entry.reasoning += chats(cell).reduce((a, b) => a + b, 0) / reps;
    entry.requests += chats(cell).length;
    entry.cells += 1;
    if (cell.result.pass) entry.passed += 1;
  }
  return out;
}

/**
 * Computes the whole profile of one agent.
 * @param cells - the agent's cells
 */
export function agentProfile(cells) {
  const reps = new Set(cells.map((c) => c.result.rep)).size || 1;
  const all = cells.flatMap(chats);
  const total = all.reduce((a, b) => a + b, 0);
  const sorted = [...all].sort((a, b) => a - b);
  const above = THRESHOLDS.map((t) => { const big = all.filter((v) => v > t); return { threshold: t, requests: big.length, share: total ? big.reduce((a, b) => a + b, 0) / total : 0 }; });
  const byPosition = POSITIONS.map(([label, from, to]) => ({ label, reasoning: cells.reduce((sum, c) => sum + chats(c).filter((_, i) => i + 1 >= from && i + 1 <= to).reduce((a, b) => a + b, 0), 0) / reps }));
  const usage = cells.flatMap((c) => c.requests.filter((r) => r.kind === "chat" && r.usage).map((r) => r.usage));
  const replyLimits = LIMITS.map((limit) => ({ limit, savedPerRep: usage.reduce((s, u) => s + Math.max(0, (u.reasoning_tokens || 0) - limit), 0) / reps, cutWhileThinking: usage.filter((u) => (u.reasoning_tokens || 0) > limit).length, cutAfterThinking: usage.filter((u) => (u.reasoning_tokens || 0) <= limit && (u.completion_tokens || 0) > limit).length, replies: usage.length }));
  const replies = cells.flatMap((c) => toolCallsPerReply(c.dir));
  const toolReplies = replies.filter((n) => n > 0);
  return {
    reps, cells: cells.length, requests: all.length, reasoningPerRep: total / reps,
    percentiles: Object.fromEntries([50, 90, 95, 99, 100].map((p) => [p, percentile(sorted, p)])), above, byPosition, replyLimits,
    byCategory: perRepByKey(cells, (c) => c.result.category, reps), byTask: perRepByKey(cells, (c) => c.result.taskId, reps),
    toolCalls: { replies: replies.length, withTools: toolReplies.length, multi: toolReplies.filter((n) => n > 1).length, mean: toolReplies.length ? toolReplies.reduce((a, b) => a + b, 0) / toolReplies.length : 0 },
  };
}

/**
 * Formats a number with separators and no decimals.
 * @param n - the number
 */
function n0(n) {
  return Math.round(n).toLocaleString("en-US");
}

/**
 * Renders the tables of one agent's profile as markdown lines.
 * @param agent - the agent name
 * @param p - the profile
 * @param topTasks - how many tasks to list
 */
function renderAgent(agent, p, topTasks) {
  const lines = [`## ${agent}`, "", `${p.cells} cells, ${p.reps} reps, ${n0(p.requests)} requests, ${n0(p.reasoningPerRep)} reasoning tokens per rep.`, ""];
  lines.push(`Reasoning tokens per request: median ${n0(p.percentiles[50])}, 90th ${n0(p.percentiles[90])}, 95th ${n0(p.percentiles[95])}, 99th ${n0(p.percentiles[99])}, largest ${n0(p.percentiles[100])}.`, "");
  lines.push("| Requests above | Requests | Share of reasoning tokens |", "|---|---|---|", ...p.above.map((a) => `| ${n0(a.threshold)} | ${a.requests} | ${(a.share * 100).toFixed(1)}% |`), "");
  lines.push("| Reply limit | Reasoning above the limit, per rep | Replies still thinking at the limit | Replies cut after thinking ended |", "|---|---|---|---|", ...p.replyLimits.map((r) => `| ${n0(r.limit)} | ${n0(r.savedPerRep)} | ${r.cutWhileThinking} | ${r.cutAfterThinking} (${((r.cutAfterThinking / Math.max(1, r.replies)) * 100).toFixed(1)}%) |`), "");
  lines.push("| Request position in the cell | Reasoning tokens per rep |", "|---|---|", ...p.byPosition.map((b) => `| ${b.label} | ${n0(b.reasoning)} |`), "");
  lines.push("| Category | Reasoning per rep | Requests per passed cell | Passed |", "|---|---|---|---|");
  for (const [name, c] of Object.entries(p.byCategory).sort((a, b) => b[1].reasoning - a[1].reasoning)) lines.push(`| ${name} | ${n0(c.reasoning)} | ${c.passed ? (c.requests / c.passed).toFixed(1) : "n/a"} | ${c.passed} of ${c.cells} |`);
  lines.push("", `| Task (top ${topTasks} by reasoning) | Reasoning per rep | Requests per cell | Passed |`, "|---|---|---|---|");
  for (const [name, c] of Object.entries(p.byTask).sort((a, b) => b[1].reasoning - a[1].reasoning).slice(0, topTasks)) lines.push(`| ${name} | ${n0(c.reasoning)} | ${(c.requests / c.cells).toFixed(1)} | ${c.passed} of ${c.cells} |`);
  const t = p.toolCalls;
  lines.push("", `Tool calls per reply: ${t.withTools} of ${t.replies} replies have a tool call, ${t.multi} (${t.withTools ? ((t.multi / t.withTools) * 100).toFixed(1) : 0}% of those) have two or more, mean ${t.mean.toFixed(2)} per reply with a tool call.`, "");
  return lines;
}

/**
 * Builds and prints the profile of a run, comparing with the pi reference when the run is Umbra
 * only, and writes profile.md and profile.json into the run folder.
 * @param runId - the run id
 * @param referenceRunId - the pi reference, or null for the default
 * @param runFolder - the run folder to write into
 */
export function writeProfile(runId, referenceRunId, runFolder) {
  const { cells } = scorecardCells(runId, referenceRunId);
  const agents = [...new Set(cells.map((c) => c.result.agent))].sort();
  const profiles = Object.fromEntries(agents.map((agent) => [agent, agentProfile(cells.filter((c) => c.result.agent === agent))]));
  const lines = [`# Reasoning profile: ${runId}`, "", ...agents.flatMap((agent) => renderAgent(agent, profiles[agent], 15))];
  fs.writeFileSync(path.join(runFolder, "profile.json"), JSON.stringify(profiles, null, 2));
  fs.writeFileSync(path.join(runFolder, "profile.md"), lines.join("\n"));
  return lines.join("\n");
}
