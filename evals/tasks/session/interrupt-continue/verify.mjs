// Verifier for interrupt-continue. The runner killed the agent 20 seconds into the first phase
// and resumed the session with "continue". Pass needs all eleven output files right, the first
// phase to have really been interrupted, and the resumed session to be the same, readable one.

import fs from "node:fs";
import path from "node:path";
import { isDeepStrictEqual } from "node:util";
import { buildTables, expectedIndex } from "./fixture.mjs";

/**
 * Parses a workspace JSON file, or returns undefined when missing or invalid.
 * @param workspaceDir - the workspace
 * @param relative - the file path
 */
function readJsonFile(workspaceDir, relative) {
  try { return JSON.parse(fs.readFileSync(path.join(workspaceDir, relative), "utf8")); } catch { return undefined; }
}

/**
 * Lists problems with how the phases ran.
 * @param runInfo - phase results from the runner
 */
function phaseProblems(runInfo) {
  const [first, second] = runInfo.phases || [];
  const problems = [];
  if (first?.killReason !== "planned-interrupt") problems.push(`first phase was not interrupted (${first?.killReason || "ended on its own"})`);
  if (!second || second.sessionRef !== first?.sessionRef || !second.sessionRef) problems.push("second phase did not resume the first session");
  if (second && !second.sessionValid) problems.push(`session invalid after resume: ${second.sessionDetail || "unknown"}`);
  if (second && second.exitCode !== 0) problems.push(`resumed phase exit ${second.exitCode} ${second.killReason || ""}`);
  return problems;
}

/**
 * Scores the eleven output files and checks the phases.
 * @param workspaceDir - the workspace after the agent finished
 * @param runInfo - phase results from the runner
 */
export async function verify(workspaceDir, runInfo) {
  const tables = buildTables(runInfo.task?.seed ?? 2614);
  const wrong = tables.filter((t) => !isDeepStrictEqual(readJsonFile(workspaceDir, `out/${t.name}.json`), t.rows)).map((t) => t.name);
  const indexOk = isDeepStrictEqual(readJsonFile(workspaceDir, "out/index.json"), expectedIndex(tables));
  if (!indexOk) wrong.push("index");
  const problems = phaseProblems(runInfo);
  const correct = tables.length + 1 - wrong.length;
  return { pass: wrong.length === 0 && problems.length === 0, score: correct / (tables.length + 1), detail: `${correct}/11 files right${wrong.length ? `; wrong: ${wrong.join(", ")}` : ""}${problems.length ? `; ${problems.join("; ")}` : ""}` };
}
