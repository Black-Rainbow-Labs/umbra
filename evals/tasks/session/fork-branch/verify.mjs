// Verifier for fork-branch. The fork must use the region it was given, the resumed original must
// still use the first region (so it did not see the fork's message), the fork must have its own
// session id, and the original must resume under its first id.

import fs from "node:fs";
import path from "node:path";
import { isDeepStrictEqual } from "node:util";

/**
 * Parses a workspace JSON file, or returns undefined when missing or invalid.
 * @param workspaceDir - the workspace
 * @param relative - the file path
 */
function readJsonFile(workspaceDir, relative) {
  try { return JSON.parse(fs.readFileSync(path.join(workspaceDir, relative), "utf8")); } catch { return undefined; }
}

/**
 * Lists problems with the session ids of the three phases.
 * @param runInfo - phase results from the runner
 */
function sessionProblems(runInfo) {
  const [base, fork, original] = runInfo.phases || [];
  const problems = [];
  if (!fork?.sessionRef || fork.sessionRef === base?.sessionRef) problems.push("fork did not get a new session id");
  if (!original?.sessionRef || original.sessionRef !== base?.sessionRef) problems.push("original did not resume under its own id");
  if (original && !original.sessionValid) problems.push("original session invalid after resume");
  return problems;
}

/**
 * Checks the three config files and the session ids.
 * @param workspaceDir - the workspace after the agent finished
 * @param runInfo - phase results from the runner
 */
export async function verify(workspaceDir, runInfo) {
  const checks = [
    ["base.json", isDeepStrictEqual(readJsonFile(workspaceDir, "config/base.json"), { name: "atlas", port: 8080 })],
    ["region-trial.json (fork)", isDeepStrictEqual(readJsonFile(workspaceDir, "config/region-trial.json"), { region: "ap-south-1" })],
    ["region.json (original)", isDeepStrictEqual(readJsonFile(workspaceDir, "config/region.json"), { region: "eu-west-2" })],
  ];
  const problems = sessionProblems(runInfo);
  const bad = checks.filter(([, ok]) => !ok).map(([name]) => name);
  const region = readJsonFile(workspaceDir, "config/region.json");
  const correct = checks.length - bad.length;
  return { pass: bad.length === 0 && problems.length === 0, score: correct / checks.length, detail: `${correct}/3 files right${bad.length ? `; wrong: ${bad.join(", ")} (region.json=${JSON.stringify(region)})` : ""}${problems.length ? `; ${problems.join("; ")}` : ""}` };
}
