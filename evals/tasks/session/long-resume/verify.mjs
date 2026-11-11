// Verifier for long-resume. The first phase must have built a history larger than 32768 prompt
// tokens; the second phase resumes it with a 32768 window reported to the agent. The passphrase
// exists only in the first prompt, so it must survive the resume. (The runner separately fails
// the cell if any request in the second phase went over the window.)

import fs from "node:fs";
import path from "node:path";
import { buildChapters, expectedSummary } from "./fixture.mjs";

/**
 * Parses summary.json, or returns null.
 * @param workspaceDir - the workspace
 */
function readSummary(workspaceDir) {
  try { return JSON.parse(fs.readFileSync(path.join(workspaceDir, "summary.json"), "utf8")); } catch { return null; }
}

/**
 * Lists problems with how the phases ran: the history size and the resume.
 * @param runInfo - phase results and request records from the runner
 */
function phaseProblems(runInfo) {
  if (runInfo.selftest) return [];
  const [first, second] = runInfo.phases || [];
  const peak = Math.max(0, ...(runInfo.requests || []).filter((r) => r.kind === "chat" && r.phase === 0 && r.usage).map((r) => r.usage.prompt_tokens));
  const problems = [];
  if (peak <= 32768) problems.push(`first phase history peaked at ${peak} prompt tokens, not over 32768`);
  if (!second || second.sessionRef !== first?.sessionRef || !second.sessionRef) problems.push("second phase did not resume the first session");
  return problems;
}

/**
 * Checks markers.txt, summary.json and the phases.
 * @param workspaceDir - the workspace after the agent finished
 * @param runInfo - phase results and request records from the runner
 */
export async function verify(workspaceDir, runInfo) {
  const chapters = buildChapters(runInfo.task?.seed ?? 2614);
  const expected = expectedSummary(chapters);
  const summary = readSummary(workspaceDir) || {};
  const wrong = Object.keys(expected).filter((key) => summary[key] !== expected[key]).map((key) => `${key}=${JSON.stringify(summary[key])}`);
  const markersFile = path.join(workspaceDir, "markers.txt");
  const markers = fs.existsSync(markersFile) ? fs.readFileSync(markersFile, "utf8").split(/\r?\n/).map((l) => l.trim()).filter(Boolean) : [];
  if (markers.join(",") !== chapters.map((c) => c.marker).join(",")) wrong.push(`markers.txt has ${markers.length} lines, not the 30 markers in order`);
  const problems = phaseProblems(runInfo);
  const items = Object.keys(expected).length + 1;
  return { pass: wrong.length === 0 && problems.length === 0, score: (items - wrong.length) / items, detail: `${items - wrong.length}/${items} items right${wrong.length ? `; wrong: ${wrong.join(", ")}` : ""}${problems.length ? `; ${problems.join("; ")}` : ""}` };
}
