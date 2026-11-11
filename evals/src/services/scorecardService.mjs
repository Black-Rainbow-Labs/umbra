// Builds scorecard.json and scorecard.md for a run, with a head to head table of Umbra and pi.

import fs from "node:fs";
import path from "node:path";
import { runDir } from "../repositories/runRepository.mjs";
import { readJson, writeJson } from "../utils/files.mjs";
import { agentScorecard } from "./scorecardMetrics.mjs";
import { signTest, mean } from "../utils/stats.mjs";
import { scorecardCells } from "./scorecardCells.mjs";
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
  ["overall.isolationBreaches", "Runs that reached evals/tasks", false], ["hard.passRate", "Hard tier pass rate", true], ["hard.meanScore", "Hard tier mean score", true],
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
 * Returns a metric's value for each rep of one agent, leaving out reps where it is undefined.
 * @param repCards - the agent's scorecard for each rep
 * @param key - the metric path
 */
function perRepValues(repCards, key) {
  return (repCards || []).map((card) => pick(card, key)).filter(Number.isFinite);
}

/**
 * Builds one head to head row. Each agent's value is the mean of its per rep values, so a run of
 * 1 rep and a run of 3 reps are on the same scale. The noise band is the spread (largest minus
 * smallest) of pi's per rep values; a difference inside it is a tie. A tie at the best possible
 * value (0 errors, a 100% rate) is marked, because neither agent can do better.
 * @param repCards - { umbra: scorecard per rep, pi: scorecard per rep }
 * @param metric - [path, label, higherIsBetter]
 */
function headToHeadRow(repCards, metric) {
  const [key, label, higherIsBetter] = metric;
  const piValues = perRepValues(repCards.pi, key);
  const umbra = mean(perRepValues(repCards.umbra, key));
  const pi = mean(piValues);
  const noise = piValues.length > 1 ? Math.max(...piValues) - Math.min(...piValues) : null;
  let better = "n/a";
  if (Number.isFinite(umbra) && Number.isFinite(pi)) {
    const difference = umbra - pi;
    if (difference === 0) better = "tie";
    else if (noise !== null && Math.abs(difference) <= noise) better = "tie (noise)";
    else better = (difference > 0) === higherIsBetter ? "umbra" : "pi";
  }
  const relative = Number.isFinite(umbra) && Number.isFinite(pi) && pi !== 0 ? (umbra - pi) / Math.abs(pi) : null;
  const bestPossible = higherIsBetter ? /Rate|Score/i.test(key) && umbra === 1 : umbra === 0;
  const tiedAtBest = better === "tie" && bestPossible;
  return { key, label, higherIsBetter, umbra: umbra ?? null, pi: pi ?? null, noise, better, relative, tiedAtBest };
}

/**
 * Adds one row per session scenario, and one per session cost measurement, to the head to head
 * table.
 * @param cards - { umbra, pi } scorecards over all reps
 * @param repCards - { umbra, pi } scorecards per rep
 */
function sessionRows(cards, repCards) {
  const names = [...new Set([...Object.keys(cards.umbra?.sessions || {}), ...Object.keys(cards.pi?.sessions || {})])].sort();
  const rows = names.map((name) => headToHeadRow(repCards, [`sessions.${name}.passRate`, `Session pass rate: ${name}`, true]));
  const costs = [["sessionCost.resumeStartupMs", "Resume startup ms, long session", false], ["sessionCost.storeBytesPerStep", "Session store bytes written per step", false]];
  const measured = costs.filter(([key]) => Number.isFinite(pick(cards.umbra, key)) || Number.isFinite(pick(cards.pi, key)));
  return rows.concat(measured.map((metric) => headToHeadRow(repCards, metric)));
}

/**
 * Builds each agent's scorecard once per rep.
 * @param cells - every cell
 * @param agents - the agent names
 */
function scorecardsPerRep(cells, agents) {
  return Object.fromEntries(agents.map((agent) => {
    const own = cells.filter((c) => c.result.agent === agent);
    const reps = [...new Set(own.map((c) => c.result.rep))].sort((a, b) => a - b);
    return [agent, reps.map((rep) => agentScorecard(own.filter((c) => c.result.rep === rep)))];
  }));
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
 * Computes the scorecard object of a run. A run that measured Umbra only is compared with pi's
 * cells from the reference run.
 * @param runId - the run id
 * @param referenceRunId - the pi reference run id, or null for the default
 */
export function buildScorecard(runId, referenceRunId = null) {
  const { cells, referenceRunId: reference } = scorecardCells(runId, referenceRunId);
  const meta = readJson(path.join(runDir(runId), "run.json"), {});
  const referenceMeta = reference ? readJson(path.join(runDir(reference), "run.json"), {}) : null;
  const agents = [...new Set(cells.map((c) => c.result.agent))].sort();
  const cards = Object.fromEntries(agents.map((agent) => [agent, agentScorecard(cells.filter((c) => c.result.agent === agent))]));
  const repCards = scorecardsPerRep(cells, agents);
  const rows = cards.umbra && cards.pi ? [...HEAD_TO_HEAD.map((metric) => headToHeadRow(repCards, metric)), ...sessionRows(cards, repCards)] : [];
  const reps = Object.fromEntries(agents.map((agent) => [agent, new Set(cells.filter((c) => c.result.agent === agent).map((c) => c.result.rep)).size]));
  const multiRep = Math.max(0, ...Object.values(reps)) > 1;
  const hardCells = cells.filter((c) => c.result.category === "hard");
  return {
    runId, meta, referenceRunId: reference, referenceMeta, cellCount: cells.length, agents, reps, cards, headToHead: rows,
    signTest: multiRep ? pairedSignTest(cells) : null, hardSignTest: multiRep && hardCells.length ? pairedSignTest(hardCells) : null,
    umbraNotBetter: rows.filter((row) => row.better !== "umbra" && row.better !== "n/a" && !row.tiedAtBest).map((row) => row.label), failed: failedCells(cells),
  };
}

/**
 * Builds the scorecard, writes scorecard.json and scorecard.md into the run folder, and returns
 * the markdown. With a comparison run, its head to head values are shown beside this run's.
 * @param runId - the run id
 * @param compareRunId - another run id, or null
 * @param referenceRunId - the pi reference run id, or null for the default
 */
export function writeReport(runId, compareRunId, referenceRunId = null) {
  const scorecard = buildScorecard(runId, referenceRunId);
  const comparison = compareRunId ? buildScorecard(compareRunId, referenceRunId) : null;
  const markdown = renderMarkdown(scorecard, comparison);
  writeJson(path.join(runDir(runId), "scorecard.json"), comparison ? { ...scorecard, comparedWith: comparison } : scorecard);
  fs.writeFileSync(path.join(runDir(runId), "scorecard.md"), markdown);
  return markdown;
}
