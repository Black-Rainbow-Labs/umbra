// Verifier for branch-earlier. The third phase branches the session at the point just before the
// second prompt, so it must not know about the second prompt's change: prefix.txt must hold the
// first prefix. The branch must get its own session id.

import fs from "node:fs";
import path from "node:path";

/**
 * Reads a workspace text file trimmed, or returns null when it is missing.
 * @param workspaceDir - the workspace
 * @param relative - the file path
 */
function readText(workspaceDir, relative) {
  try { return fs.readFileSync(path.join(workspaceDir, relative), "utf8").trim(); } catch { return null; }
}

/**
 * Checks the files and that the branch is a new, valid session.
 * @param workspaceDir - the workspace after the agent finished
 * @param runInfo - phase results from the runner
 */
export async function verify(workspaceDir, runInfo) {
  const [first, , branch] = runInfo.phases || [];
  const prefix = readText(workspaceDir, "notes/prefix.txt");
  const checks = [
    ["start.txt", readText(workspaceDir, "notes/start.txt") === "started"],
    ["second.txt", readText(workspaceDir, "notes/second.txt") === "changed"],
    ["prefix.txt holds the prefix from before the branch point", prefix === "heron-blue"],
    ["branch has its own valid session", Boolean(branch?.sessionRef) && branch.sessionRef !== first?.sessionRef && branch.sessionValid !== false && branch.exitCode === 0],
  ];
  const bad = checks.filter(([, ok]) => !ok).map(([name]) => name);
  const correct = checks.length - bad.length;
  return { pass: bad.length === 0, score: correct / checks.length, detail: `${correct}/${checks.length} checks${bad.length ? `; failed: ${bad.join(", ")} (prefix.txt=${JSON.stringify(prefix)})` : ""}` };
}
