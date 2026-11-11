// Verifier for cut-resume. Before the second phase the runner cuts the last 400 bytes off the
// session file, as a crash in the middle of a write would. The agent must still resume the
// session and remember the code name from the first prompt.

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
 * Checks the files of both phases and that the resumed phase ran in the same session.
 * @param workspaceDir - the workspace after the agent finished
 * @param runInfo - phase results from the runner
 */
export async function verify(workspaceDir, runInfo) {
  const [work, resumed] = runInfo.phases || [];
  const release = readText(workspaceDir, "release.txt");
  const checks = [
    ["data files", ["a", "b", "c"].every((name, i) => readText(workspaceDir, `data/${name}.txt`) === ["alpha", "beta", "gamma"][i])],
    ["release.txt holds the code name", release === "amber-falcon-31"],
    ["resume exited cleanly", resumed?.exitCode === 0],
    ["resumed the same session", Boolean(resumed?.sessionRef) && resumed.sessionRef === work?.sessionRef],
  ];
  const bad = checks.filter(([, ok]) => !ok).map(([name]) => name);
  const correct = checks.length - bad.length;
  return { pass: bad.length === 0, score: correct / checks.length, detail: `${correct}/${checks.length} checks${bad.length ? `; failed: ${bad.join(", ")} (release.txt=${JSON.stringify(release)}, resume exit ${resumed?.exitCode})` : ""}` };
}
