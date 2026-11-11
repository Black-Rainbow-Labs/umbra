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
  const lines = [`| Metric | Umbra | pi | pi noise band | Better | Umbra vs pi |${extra}`, `|---|---|---|---|---|---|${comparison ? "---|---|" : ""}`];
  for (const row of scorecard.headToHead) {
    const other = comparison?.headToHead.find((r) => r.key === row.key);
    const otherCells = comparison ? ` ${formatValue(other?.umbra, row.key)} | ${formatValue(other?.pi, row.key)} |` : "";
    lines.push(`| ${row.label} | ${formatValue(row.umbra, row.key)} | ${formatValue(row.pi, row.key)} | ${row.noise === null || row.noise === undefined ? "n/a" : formatValue(row.noise, row.key)} | ${row.better} | ${formatRelative(row.relative)} |${otherCells}`);
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
  const hardSign = scorecard.hardSignTest;
  const reference = scorecard.referenceRunId ? `pi cells come from the reference run ${scorecard.referenceRunId} (pi ${scorecard.referenceMeta?.versions?.pi || "?"}). ` : "";
  const reps = Object.entries(scorecard.reps || {}).map(([agent, n]) => `${agent} ${n}`).join(", ");
  const build = meta.umbraBuild ? `Umbra build: ${meta.umbraBuild.bin}, commit ${meta.umbraBuild.commit}${meta.umbraBuild.dirty ? " with uncommitted changes" : ""}${meta.umbraBuild.settings?.length ? `, settings: ${meta.umbraBuild.settings.join("; ")}` : ""}. ` : "";
  const signLine = (label, s) => s ? `${label}: Umbra better on ${s.umbraBetterTasks}, pi better on ${s.piBetterTasks}, tied on ${s.tiedTasks}, two sided p = ${s.pValue.toFixed(3)}.\n` : "";
  const parts = [
    `# Scorecard: ${scorecard.runId}`, "",
    `Cells: ${scorecard.cellCount}. Reps: ${reps}. Agents: ${scorecard.agents.join(", ")}. Model: ${meta.model || "?"}. Versions: umbra ${meta.versions?.umbra || "?"}, pi ${scorecard.referenceMeta?.versions?.pi || meta.versions?.pi || "?"}. Sampling: ${JSON.stringify(meta.sampling || {})}.`, "",
    `${build}${reference}`, "",
    "All token numbers come from the measuring proxy (requests.jsonl). Each value is the mean of the agent's per rep values, so totals are totals per rep. The pi noise band is the largest minus the smallest of pi's per rep values; a difference inside it is a tie (noise). \"Umbra vs pi\" is (Umbra - pi) / pi.", "",
    "## Head to head", "", scorecard.headToHead.length ? headToHeadTable(scorecard, comparison) : "Both agents are needed for the head to head table.", "",
    signLine("Paired sign test over tasks (pass rate across reps)", sign), signLine("Paired sign test over hard tier tasks", hardSign),
    "## Metrics where Umbra is not strictly better than pi", "", scorecard.umbraNotBetter.length ? scorecard.umbraNotBetter.map((label) => `- ${label}`).join("\n") : "None.", "",
    "## Per category", "", categoryTable(scorecard), "",
    "## Compaction and sessions", "", compactionAndSessions(scorecard), "",
    "## Failed cells", "", failedTable(scorecard), "",
  ];
  return parts.join("\n");
}
