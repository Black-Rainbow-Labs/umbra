// Verifier for long-300-resume. The first phase builds a session of about 300 model requests,
// one echo per request, while the runner measures the bytes the agent writes to its session
// store. The second phase resumes it, and the runner measures the time from launch to the first
// model request. The verifier checks the log, that the session really was long, and the answer.

import fs from "node:fs";
import path from "node:path";

/**
 * Reads a workspace text file, or returns null when it is missing.
 * @param workspaceDir - the workspace
 * @param relative - the file path
 */
function readText(workspaceDir, relative) {
  try { return fs.readFileSync(path.join(workspaceDir, relative), "utf8"); } catch { return null; }
}

/**
 * Checks log.txt, the length of the first phase, and answer.txt.
 * @param workspaceDir - the workspace after the agent finished
 * @param runInfo - phase results from the runner
 */
export async function verify(workspaceDir, runInfo) {
  const [build, resumed] = runInfo.phases || [];
  const lines = (readText(workspaceDir, "log.txt") || "").split(/\r?\n/).filter((line) => line.trim());
  const expected = Array.from({ length: 300 }, (_, i) => `step ${i + 1}`);
  const answer = (readText(workspaceDir, "answer.txt") || "").split(/\r?\n/).map((line) => line.trim()).filter(Boolean);
  const longEnough = runInfo.selftest || (build?.requests ?? 0) >= 250;
  const checks = [
    ["log.txt has steps 1 to 300 in order", lines.length === 300 && lines.every((line, i) => line.trim() === expected[i])],
    ["the first phase made at least 250 model requests", longEnough],
    ["answer.txt line 1 is the audit key", answer[0] === "slate-otter-77"],
    ["answer.txt line 2 is 300", answer[1] === "300"],
    ["resume exited cleanly in the same session", runInfo.selftest || (resumed?.exitCode === 0 && resumed?.sessionRef === build?.sessionRef)],
  ];
  const bad = checks.filter(([, ok]) => !ok).map(([name]) => name);
  const correct = checks.length - bad.length;
  return { pass: bad.length === 0, score: correct / checks.length, detail: `${correct}/${checks.length} checks; ${lines.length} log lines, ${build?.requests ?? "?"} requests in phase 0${bad.length ? `; failed: ${bad.join(", ")}` : ""}` };
}
