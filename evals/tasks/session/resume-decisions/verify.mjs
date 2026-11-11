// Verifier for resume-decisions. All ten triage files must be right. The second half needs the
// Lisbon rule, which only the first phase's prompt states, so the resumed session must carry it.
// The first phase must have stopped after the first half, and the second phase must have resumed
// the first phase's session.

import fs from "node:fs";
import path from "node:path";
import { EXPECTED } from "./fixture.mjs";

/**
 * Reads a triage file as one normalized line, or "" when missing.
 * @param workspaceDir - the workspace
 * @param name - the message file name
 */
function readTriage(workspaceDir, name) {
  const file = path.join(workspaceDir, "triage", name);
  return fs.existsSync(file) ? fs.readFileSync(file, "utf8").trim().replace(/\s+/g, " ").toUpperCase() : "";
}

/**
 * Checks how the phases ran: phase 0 left the second half undone and phase 1 resumed it.
 * @param runInfo - phase results from the runner
 */
function phaseProblems(runInfo) {
  const [first, second] = runInfo.phases || [];
  const problems = [];
  if (!second || second.mode !== "resume" || !second.sessionRef || second.sessionRef !== first?.sessionRef) problems.push("second phase did not resume the first session");
  const doneEarly = (first?.workspaceFiles || []).filter((f) => /^triage\/msg-(0[6-9]|10)\.txt$/.test(f));
  if (doneEarly.length) problems.push(`first phase already wrote ${doneEarly.length} second half files`);
  return problems;
}

/**
 * Scores the ten triage files and checks the phases.
 * @param workspaceDir - the workspace after the agent finished
 * @param runInfo - phase results from the runner
 */
export async function verify(workspaceDir, runInfo) {
  const wrong = Object.entries(EXPECTED).filter(([name, line]) => readTriage(workspaceDir, name) !== line).map(([name]) => `${name}=${JSON.stringify(readTriage(workspaceDir, name))}`);
  const problems = phaseProblems(runInfo);
  const correct = Object.keys(EXPECTED).length - wrong.length;
  const pass = wrong.length === 0 && problems.length === 0;
  return { pass, score: correct / Object.keys(EXPECTED).length, detail: `${correct}/10 triage files right${wrong.length ? `; wrong: ${wrong.join(", ")}` : ""}${problems.length ? `; ${problems.join("; ")}` : ""}` };
}
