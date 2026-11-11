// Builds scorecard.json and scorecard.md for a run, with a head to head table of Umbra and pi.

import fs from "node:fs";
import path from "node:path";
import { loadRunCells, runDir } from "../repositories/runRepository.mjs";
import { readJson, writeJson } from "../utils/files.mjs";
import { agentScorecard } from "./scorecardMetrics.mjs";
import { signTest } from "../utils/stats.mjs";
import { renderMarkdown } from "./scorecardMarkdown.mjs";

// Each head to head metric: where it lives, its label, and whether higher is better.
export const HEAD_TO_HEAD = [
  ["overall.passRate", "Pass rate", true], ["overall.meanScore", "Mean score", true],
  ["overall.promptTokens", "Prompt tokens, total", false], ["overall.uncachedPromptTokens", "Uncached prompt tokens, total", false],
  ["overall.completionTokens", "Completion tokens, total", false], ["overall.reasoningTokens", "Reasoning tokens, total", false],
  ["overall.requests", "Requests, total", false], ["overall.promptTokensPerPass", "Prompt tokens per passed task", false],
  ["overall.uncachedPromptTokensPerPass", "Uncached prompt tokens per passed task", false], ["overall.completionTokensPerPass", "Completion tokens per passed task", false],
  ["overall.requestsPerPass", "Requests per passed task", false], ["overall.firstRequestPromptTokens", "First request prompt tokens (fixed overhead)", false],
  ["overall.cacheHitRate", "Cache hit rate", true], ["overall.wallSecondsPerTask", "Wall seconds per task", false],
  ["overall.meanTimeToFirstContentMs", "Mean time to first content, ms", false], ["overall.totalModelSeconds", "Total model seconds", false],
  ["overall.runsWithNoToolCall", "Runs with no tool call", false], ["overall.runsHitLimit", "Runs that hit the step or time limit", false],
  ["overall.httpErrorRequests", "Requests with HTTP errors", false], ["overall.malformedToolCalls", "Malformed or unknown tool calls", false],
  ["overall.nonzeroExits", "Runs with a nonzero exit", false], ["compaction.compactionPassRate", "Compaction pass rate", true],
  ["compaction.compactionMeanScore", "Compaction mean score", true], ["compaction.windowExceededRequests", "Requests over the context window", false],
  ["compaction.cacheHitRateAfterCompaction", "Cache hit rate, 5 requests after a compaction", true], ["compaction.peakPromptTokensWithWindow", "Peak prompt tokens on window tasks", false],
];

/**
 * Reads a dotted path such as "overall.passRate" from an object.
 * @param object - the scorecard of one agent
 * @param dotted - the path
 */
function pick(object, dotted) {
  return dotted.split(".").reduce((value, key) => (value == null ? undefined : value[key]), object);
}

/**
 * Builds one head to head row: both values, which is better, and (Umbra - pi) / pi. A tie at
 * the best possible value (0 errors, a 100% rate) is marked, because neither agent can do better.
 * @param cards - { umbra, pi } scorecards
 * @param metric - [path, label, higherIsBetter]
 */
function headToHeadRow(cards, metric) {
  const [key, label, higherIsBetter] = metric;
  const umbra = pick(cards.umbra, key);
  const pi = pick(cards.pi, key);
  let better = "n/a";
  if (Number.isFinite(umbra) && Number.isFinite(pi)) better = umbra === pi ? "tie" : (umbra > pi) === higherIsBetter ? "umbra" : "pi";
  const relative = Number.isFinite(umbra) && Number.isFinite(pi) && pi !== 0 ? (umbra - pi) / Math.abs(pi) : null;
  const bestPossible = higherIsBetter ? /Rate|Score/.test(key) && umbra === 1 : umbra === 0;
  const tiedAtBest = better === "tie" && bestPossible;
  return { key, label, higherIsBetter, umbra: umbra ?? null, pi: pi ?? null, better, relative, tiedAtBest };
}

/**
 * Adds one row per session scenario to the head to head table.
 * @param cards - { umbra, pi } scorecards
 */
function sessionRows(cards) {
  const names = [...new Set([...Object.keys(cards.umbra?.sessions || {}), ...Object.keys(cards.pi?.sessions || {})])].sort();
  return names.map((name) => headToHeadRow(cards, [`sessions.${name}.passRate`, `Session pass rate: ${name}`, true]));
}

/**
 * Paired sign test over tasks: per task, compares the two agents' pass rates across reps.
 * @param cells - every cell of the run
 */
function pairedSignTest(cells) {
  const byTask = {};
  for (const cell of cells) {
    const entry = (byTask[cell.result.taskId] ||= { umbra: [], pi: [] });
    entry[cell.result.agent]?.push(cell.result.pass ? 1 : 0);
  }
  let wins = 0, losses = 0, ties = 0;
  for (const entry of Object.values(byTask)) {
    if (!entry.umbra.length || !entry.pi.length) continue;
    const difference = entry.umbra.reduce((a, b) => a + b, 0) / entry.umbra.length - entry.pi.reduce((a, b) => a + b, 0) / entry.pi.length;
    if (difference > 0) wins += 1; else if (difference < 0) losses += 1; else ties += 1;
  }
  return { umbraBetterTasks: wins, piBetterTasks: losses, tiedTasks: ties, pValue: signTest(wins, losses) };
}

/**
 * Lists the failed cells with a one line reason each.
 * @param cells - every cell of the run
 */
function failedCells(cells) {
  return cells.filter((c) => !c.result.pass).map((c) => {
    const r = c.result;
    const flags = [r.timedOut ? "timed out" : "", r.stepLimitHit ? "step limit" : "", r.windowExceededCount ? `${r.windowExceededCount} requests over window` : ""].filter(Boolean);
    return { taskId: r.taskId, agent: r.agent, rep: r.rep, score: r.score, reason: `${flags.length ? `[${flags.join(", ")}] ` : ""}${r.detail}`.replace(/\s+/g, " ").slice(0, 300) };
  }).sort((a, b) => a.taskId.localeCompare(b.taskId) || a.agent.localeCompare(b.agent));
}

/**
 * Computes the scorecard object of a run.
 * @param runId - the run id
 */
export function buildScorecard(runId) {
  const cells = loadRunCells(runId);
  const meta = readJson(path.join(runDir(runId), "run.json"), {});
  const agents = [...new Set(cells.map((c) => c.result.agent))].sort();
  const cards = Object.fromEntries(agents.map((agent) => [agent, agentScorecard(cells.filter((c) => c.result.agent === agent))]));
  const rows = cards.umbra && cards.pi ? [...HEAD_TO_HEAD.map((metric) => headToHeadRow(cards, metric)), ...sessionRows(cards)] : [];
  const reps = Math.max(0, ...cells.map((c) => c.result.rep));
  return { runId, meta, cellCount: cells.length, agents, cards, headToHead: rows, signTest: reps > 1 ? pairedSignTest(cells) : null, umbraNotBetter: rows.filter((row) => row.better !== "umbra" && row.better !== "n/a" && !row.tiedAtBest).map((row) => row.label), failed: failedCells(cells) };
}

/**
 * Builds the scorecard, writes scorecard.json and scorecard.md into the run folder, and returns
 * the markdown. With a comparison run, its head to head values are shown beside this run's.
 * @param runId - the run id
 * @param compareRunId - another run id, or null
 */
export function writeReport(runId, compareRunId) {
  const scorecard = buildScorecard(runId);
  const comparison = compareRunId ? buildScorecard(compareRunId) : null;
  const markdown = renderMarkdown(scorecard, comparison);
  writeJson(path.join(runDir(runId), "scorecard.json"), comparison ? { ...scorecard, comparedWith: comparison } : scorecard);
  fs.writeFileSync(path.join(runDir(runId), "scorecard.md"), markdown);
  return markdown;
}
