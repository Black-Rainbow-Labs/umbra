// Verifier for contracts-64k-clauses: report.txt must have the header, one exact line per
// contract (the clause id, or EXEMPT for the five exempt contracts) and the TOTAL footer.
// Score is the fraction of the 38 lines right; extra lines are reported but do not change the score,
// except that pass also requires exactly 38 lines.

import fs from "node:fs";
import path from "node:path";
import { buildContracts, expectedReport } from "./fixture.mjs";

/**
 * Reads the non empty, trimmed lines of report.txt, or an empty list when it is missing.
 * @param workspaceDir - the workspace
 */
function readReportLines(workspaceDir) {
  try { return fs.readFileSync(path.join(workspaceDir, "report.txt"), "utf8").split(/\r?\n/).map((l) => l.trim()).filter(Boolean); } catch { return []; }
}

/**
 * Scores report.txt line by line against the expected report.
 * @param workspaceDir - the workspace after the agent finished
 * @param runInfo - run information
 */
export async function verify(workspaceDir, runInfo) {
  const expected = expectedReport(buildContracts(runInfo.task?.seed ?? 2614));
  const lines = readReportLines(workspaceDir);
  const bad = [];
  expected.forEach((line, i) => { if (lines[i] !== line) bad.push(`line ${i + 1}: got ${JSON.stringify((lines[i] ?? "missing").slice(0, 40))}, expected ${JSON.stringify(line)}`); });
  const correct = expected.length - bad.length;
  const detail = `${correct}/${expected.length} lines right (file has ${lines.length})${bad.length ? `; ${bad.slice(0, 3).join("; ")}` : ""}`;
  return { pass: bad.length === 0 && lines.length === expected.length, score: correct / expected.length, detail };
}
