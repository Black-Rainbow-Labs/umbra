// Renders a scorecard object as markdown.

/**
 * Formats a metric value for a table: rates as percentages, large numbers with separators.
 * @param value - the number, or null
 * @param key - the metric path, used to tell rates from counts
 */
export function formatValue(value, key) {
  if (value === null || value === undefined || Number.isNaN(value)) return "n/a";
  if (/Rate|meanScore|MeanScore/.test(key)) return `${(value * 100).toFixed(1)}%`;
  if (/Seconds|Ms/.test(key)) return value.toLocaleString("en-US", { maximumFractionDigits: 1 });
  return Math.round(value).toLocaleString("en-US");
}

/**
 * Formats (Umbra - pi) / pi as a signed percentage.
 * @param relative - the ratio, or null
 */
function formatRelative(relative) {
  if (relative === null) return "n/a";
  return `${relative >= 0 ? "+" : ""}${(relative * 100).toFixed(1)}%`;
}

/**
 * Renders the head to head table, with the comparison run's values when given.
 * @param scorecard - this run's scorecard
 * @param comparison - another run's scorecard, or null
 */
function headToHeadTable(scorecard, comparison) {
  const extra = comparison ? ` Umbra (${comparison.runId}) | pi (${comparison.runId}) |` : "";
  const lines = [`| Metric | Umbra | pi | Better | Umbra vs pi |${extra}`, `|---|---|---|---|---|${comparison ? "---|---|" : ""}`];
  for (const row of scorecard.headToHead) {
    const other = comparison?.headToHead.find((r) => r.key === row.key);
    const otherCells = comparison ? ` ${formatValue(other?.umbra, row.key)} | ${formatValue(other?.pi, row.key)} |` : "";
    lines.push(`| ${row.label} | ${formatValue(row.umbra, row.key)} | ${formatValue(row.pi, row.key)} | ${row.better} | ${formatRelative(row.relative)} |${otherCells}`);
  }
  return lines.join("\n");
}

/**
 * Renders the per category table for every agent.
 * @param scorecard - the scorecard
 */
function categoryTable(scorecard) {
  const lines = ["| Category | Agent | Cells | Pass rate | Mean score | Prompt tokens | Uncached prompt | Completion | Requests | Cache hit | Wall s per task |", "|---|---|---|---|---|---|---|---|---|---|---|"];
  const categories = [...new Set(scorecard.agents.flatMap((agent) => Object.keys(scorecard.cards[agent].perCategory)))];
  for (const category of categories) {
    for (const agent of scorecard.agents) {
      const m = scorecard.cards[agent].perCategory[category];
      if (!m) continue;
      lines.push(`| ${category} | ${agent} | ${m.cells} | ${formatValue(m.passRate, "passRate")} | ${formatValue(m.meanScore, "meanScore")} | ${formatValue(m.promptTokens, "t")} | ${formatValue(m.uncachedPromptTokens, "t")} | ${formatValue(m.completionTokens, "t")} | ${formatValue(m.requests, "t")} | ${formatValue(m.cacheHitRate, "cacheHitRate")} | ${formatValue(m.wallSecondsPerTask, "Seconds")} |`);
    }
  }
  return lines.join("\n");
}

/**
 * Renders the compaction and session details for every agent.
 * @param scorecard - the scorecard
 */
function compactionAndSessions(scorecard) {
  const lines = ["| Agent | Compaction pass rate | Compaction mean score | Requests over window | Compactions (message count drops) | Compactions the agent reported | Cache hit after compaction | Peak prompt tokens on window tasks |", "|---|---|---|---|---|---|---|---|"];
  for (const agent of scorecard.agents) {
    const c = scorecard.cards[agent].compaction;
    lines.push(`| ${agent} | ${formatValue(c.compactionPassRate, "Rate")} | ${formatValue(c.compactionMeanScore, "MeanScore")} | ${c.windowExceededRequests} | ${c.compactions} | ${c.agentReportedCompactions} | ${formatValue(c.cacheHitRateAfterCompaction, "Rate")} | ${formatValue(c.peakPromptTokensWithWindow, "t")} |`);
  }
  lines.push("", "| Agent | Session scenario | Passed | Cells |", "|---|---|---|---|");
  for (const agent of scorecard.agents) {
    for (const [name, s] of Object.entries(scorecard.cards[agent].sessions)) lines.push(`| ${agent} | ${name} | ${s.passed} | ${s.cells} |`);
  }
  return lines.join("\n");
}

/**
 * Renders the failed cell table.
 * @param scorecard - the scorecard
 */
function failedTable(scorecard) {
  if (!scorecard.failed.length) return "No failed cells.";
  const escape = (text) => text.replace(/\|/g, "\\|");
  return ["| Task | Agent | Rep | Score | Reason |", "|---|---|---|---|---|", ...scorecard.failed.map((f) => `| ${f.taskId} | ${f.agent} | ${f.rep} | ${f.score.toFixed(2)} | ${escape(f.reason)} |`)].join("\n");
}

/**
 * Renders the whole scorecard as markdown.
 * @param scorecard - the scorecard
 * @param comparison - another run's scorecard, or null
 */
export function renderMarkdown(scorecard, comparison) {
  const meta = scorecard.meta || {};
  const sign = scorecard.signTest;
  const parts = [
    `# Scorecard: ${scorecard.runId}`, "",
    `Cells: ${scorecard.cellCount}. Agents: ${scorecard.agents.join(", ")}. Model: ${meta.model || "?"}. Versions: umbra ${meta.versions?.umbra || "?"}, pi ${meta.versions?.pi || "?"}. Sampling: ${JSON.stringify(meta.sampling || {})}.`, "",
    "All token numbers come from the measuring proxy (requests.jsonl). \"Umbra vs pi\" is (Umbra - pi) / pi.", "",
    "## Head to head", "", scorecard.headToHead.length ? headToHeadTable(scorecard, comparison) : "Both agents are needed for the head to head table.", "",
    sign ? `Paired sign test over tasks (pass rate across reps): Umbra better on ${sign.umbraBetterTasks}, pi better on ${sign.piBetterTasks}, tied on ${sign.tiedTasks}, two sided p = ${sign.pValue.toFixed(3)}.\n` : "",
    "## Metrics where Umbra is not strictly better than pi", "", scorecard.umbraNotBetter.length ? scorecard.umbraNotBetter.map((label) => `- ${label}`).join("\n") : "None.", "",
    "## Per category", "", categoryTable(scorecard), "",
    "## Compaction and sessions", "", compactionAndSessions(scorecard), "",
    "## Failed cells", "", failedTable(scorecard), "",
  ];
  return parts.join("\n");
}
