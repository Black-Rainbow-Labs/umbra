// Computes scorecard metrics from cell results and the proxy's request records. Every token
// number comes from requests.jsonl, never from an agent's own accounting.

import { sum, mean, ratio } from "../utils/stats.mjs";

/**
 * Returns the chat requests of a cell that got a usage record.
 * @param cell - { result, requests }
 */
function chatRequests(cell) {
  return cell.requests.filter((r) => r.kind === "chat");
}

/**
 * Returns the message count a request is compared with. Inside a phase that is the previous
 * request. The first request of a resumed or forked phase is compared with the last request of
 * the latest earlier phase that had the session it resumed, so starting a new session or
 * resuming an older one is not counted as a compaction.
 * @param chats - the chat requests of one cell, in order
 * @param i - index of the request
 * @param phases - the cell's phase results
 */
function previousMessageCount(chats, i, phases) {
  if (chats[i].phase === chats[i - 1].phase) return chats[i - 1].messageCount;
  const phase = phases?.[chats[i].phase];
  if (!phase?.resumedFrom) return null;
  const source = [...phases.slice(0, chats[i].phase)].reverse().find((p) => p.sessionRef === phase.resumedFrom);
  const last = source ? [...chats.slice(0, i)].reverse().find((r) => r.phase === source.index) : null;
  return last ? last.messageCount : null;
}

/**
 * Returns true when a request changed an earlier message of the request before it in the same
 * phase, instead of only adding messages at the end. That is what a compaction does, whether it
 * removes messages (a summary) or rewrites them in place (Umbra's mask pass), and it is what
 * makes the server's prompt cache stop matching. Needs the proxy's per message hashes.
 * @param previous - the earlier request record
 * @param current - the later request record
 */
function rewroteHistory(previous, current) {
  const before = previous.messageHashes;
  const after = current.messageHashes;
  if (!Array.isArray(before) || !Array.isArray(after)) return null;
  if (after.length < before.length) return true;
  return before.some((hash, index) => after[index] !== hash);
}

/**
 * Finds compactions in one cell. Inside a phase, a request that rewrote the history of the
 * request before it counts; records from before the proxy kept message hashes fall back to a
 * drop in the message count. Across a resume, a drop in the message count compared with the
 * resumed session counts. A run of consecutive compactions (a summary call followed by the
 * rebuilt history) counts once. Returns the index of each compaction.
 * @param chats - the chat requests of one cell, in order
 * @param phases - the cell's phase results
 */
export function compactionIndexes(chats, phases) {
  const indexes = [];
  for (let i = 1; i < chats.length; i += 1) {
    const samePhase = chats[i].phase === chats[i - 1].phase;
    const rewritten = samePhase ? rewroteHistory(chats[i - 1], chats[i]) : null;
    const previous = rewritten === null ? previousMessageCount(chats, i, phases) : null;
    const compacted = rewritten ?? (previous !== null && chats[i].messageCount < previous);
    if (compacted && indexes[indexes.length - 1] !== i - 1) indexes.push(i);
  }
  return indexes;
}

/**
 * Computes token and request totals over a set of cells.
 * @param cells - the cells
 */
function costMetrics(cells) {
  const chats = cells.flatMap(chatRequests);
  const usage = chats.map((r) => r.usage).filter(Boolean);
  const prompt = sum(usage.map((u) => u.prompt_tokens));
  const cached = sum(usage.map((u) => u.cached_tokens));
  const firstPrompts = cells.map((c) => chatRequests(c).find((r) => r.usage)?.usage.prompt_tokens);
  return {
    promptTokens: prompt, uncachedPromptTokens: prompt - cached, completionTokens: sum(usage.map((u) => u.completion_tokens)),
    reasoningTokens: sum(usage.map((u) => u.reasoning_tokens)), requests: chats.length,
    firstRequestPromptTokens: mean(firstPrompts), cacheHitRate: ratio(cached, prompt),
  };
}

/**
 * Computes wall time and model time over a set of cells.
 * @param cells - the cells
 */
function speedMetrics(cells) {
  const chats = cells.flatMap(chatRequests);
  return {
    wallSecondsPerTask: mean(cells.map((c) => c.result.wallMs / 1000)),
    meanTimeToFirstContentMs: mean(chats.map((r) => r.firstContentMs).filter((v) => v !== null)),
    totalModelSeconds: sum(chats.map((r) => r.durationMs)) / 1000,
  };
}

/**
 * Counts reliability problems over a set of cells.
 * @param cells - the cells
 */
function reliabilityMetrics(cells) {
  const chats = cells.flatMap(chatRequests);
  return {
    runsWithNoToolCall: cells.filter((c) => !chatRequests(c).some((r) => r.hasToolCall)).length,
    runsHitLimit: cells.filter((c) => c.result.timedOut || c.result.stepLimitHit).length,
    httpErrorRequests: chats.filter((r) => r.status >= 400 || (r.status === 0 && !r.aborted)).length,
    malformedToolCalls: sum(cells.map((c) => c.result.malformedToolCalls || 0)),
    nonzeroExits: cells.filter((c) => c.result.nonzeroExit).length,
    isolationBreaches: cells.filter((c) => c.result.isolationBreach).length,
  };
}

/**
 * Computes the session cost measurements from phases that recorded them: the time from launch
 * to the first model request when resuming a long session, and the bytes the agent wrote to its
 * session store divided by the steps of the phase. A step is a model request or a tool call,
 * whichever count is larger, so a reply with several tool calls counts each one (task-2643).
 * @param cells - all cells of one agent
 */
function sessionCostMetrics(cells) {
  const phases = cells.flatMap((c) => c.result.phases || []);
  const startups = phases.filter((p) => p.measureResumeStartup && Number.isFinite(p.firstRequestAfterMs)).map((p) => p.firstRequestAfterMs);
  const steps = (p) => Math.max(p.requests || 0, p.toolCalls || 0);
  const stores = phases.filter((p) => Number.isFinite(p.storeBytesWritten) && steps(p) > 0);
  return {
    resumeStartupMs: mean(startups),
    storeBytesPerStep: stores.length ? sum(stores.map((p) => p.storeBytesWritten)) / sum(stores.map(steps)) : null,
  };
}

/**
 * Computes the compaction metrics: pass rate on compaction tasks, window overruns, compactions,
 * cache hit rate on the five requests after each compaction, and peak prompt tokens.
 * @param cells - all cells of one agent
 */
function compactionMetrics(cells) {
  const compactionCells = cells.filter((c) => c.result.category === "compaction");
  let compactions = 0;
  const after = [];
  for (const cell of cells) {
    const chats = chatRequests(cell);
    const indexes = compactionIndexes(chats, cell.result.phases);
    compactions += indexes.length;
    for (const index of indexes) after.push(...chats.slice(index, index + 5).map((r) => r.usage).filter(Boolean));
  }
  const windowCells = cells.filter((c) => c.result.contextWindow || c.result.phases?.some((p) => p.contextWindow));
  return {
    compactionPassRate: ratio(compactionCells.filter((c) => c.result.pass).length, compactionCells.length),
    compactionMeanScore: mean(compactionCells.map((c) => c.result.score)),
    windowExceededRequests: sum(cells.map((c) => c.requests.filter((r) => r.windowExceeded).length)),
    compactions,
    cacheHitRateAfterCompaction: ratio(sum(after.map((u) => u.cached_tokens)), sum(after.map((u) => u.prompt_tokens))),
    peakPromptTokensWithWindow: Math.max(0, ...windowCells.flatMap((c) => chatRequests(c).map((r) => r.usage?.prompt_tokens || 0))),
    agentReportedCompactions: sum(cells.map((c) => c.result.agentCompactions || 0)),
  };
}

/**
 * Computes every metric for one set of cells (one agent, overall or one category).
 * @param cells - the cells
 */
export function metricsFor(cells) {
  const passed = cells.filter((c) => c.result.pass);
  const cost = costMetrics(cells);
  const passedCost = costMetrics(passed);
  return {
    cells: cells.length, passed: passed.length,
    passRate: ratio(passed.length, cells.length), meanScore: mean(cells.map((c) => c.result.score)),
    ...cost,
    promptTokensPerPass: ratio(cost.promptTokens, passed.length), uncachedPromptTokensPerPass: ratio(cost.uncachedPromptTokens, passed.length),
    completionTokensPerPass: ratio(cost.completionTokens, passed.length), requestsPerPass: ratio(cost.requests, passed.length),
    passedOnlyPromptTokensPerTask: ratio(passedCost.promptTokens, passed.length),
    ...speedMetrics(cells), ...reliabilityMetrics(cells),
  };
}

/**
 * Computes the session metrics: pass rate per session scenario.
 * @param cells - all cells of one agent
 */
function sessionMetrics(cells) {
  const out = {};
  for (const cell of cells.filter((c) => c.result.category === "session")) {
    const name = cell.result.scenario || cell.result.taskId;
    out[name] = out[name] || { cells: 0, passed: 0 };
    out[name].cells += 1;
    if (cell.result.pass) out[name].passed += 1;
  }
  for (const value of Object.values(out)) value.passRate = ratio(value.passed, value.cells);
  return out;
}

/**
 * Computes the whole scorecard for one agent: overall, per category, compaction and sessions.
 * @param cells - all cells of one agent
 */
export function agentScorecard(cells) {
  const categories = [...new Set(cells.map((c) => c.result.category))];
  const perCategory = Object.fromEntries(categories.map((category) => [category, metricsFor(cells.filter((c) => c.result.category === category))]));
  const hardCells = cells.filter((c) => c.result.category === "hard");
  return { overall: metricsFor(cells), perCategory, compaction: compactionMetrics(cells), sessions: sessionMetrics(cells), sessionCost: sessionCostMetrics(cells), hard: hardCells.length ? metricsFor(hardCells) : {} };
}
