// Verifier for the session flag check: resume keeps the remembered word, the fork gets its own
// word, and resuming the original after the fork does not see the fork's message.

import fs from "node:fs";
import path from "node:path";

/**
 * Reads a workspace file as trimmed lower case text, or "" when it is missing.
 * @param workspaceDir - the workspace
 * @param name - the file name
 */
function readWord(workspaceDir, name) {
  const file = path.join(workspaceDir, name);
  return fs.existsSync(file) ? fs.readFileSync(file, "utf8").trim().toLowerCase() : "";
}

/**
 * Checks the four files and that the fork got a new session id.
 * @param workspaceDir - the workspace after the agent finished
 * @param runInfo - phase results from the runner
 */
export async function verify(workspaceDir, runInfo) {
  const expected = { "a.txt": "one", "b.txt": "falcon", "c.txt": "osprey", "d.txt": "falcon" };
  const checks = Object.entries(expected).map(([name, word]) => ({ name, ok: readWord(workspaceDir, name) === word }));
  const phases = runInfo.phases || [];
  const forkIsNew = phases[2] && phases[2].sessionRef && phases[2].sessionRef !== phases[1].sessionRef;
  checks.push({ name: "fork has a new session id", ok: Boolean(forkIsNew) });
  const passed = checks.filter((c) => c.ok).length;
  return { pass: passed === checks.length, score: passed / checks.length, detail: checks.map((c) => `${c.name}:${c.ok ? "ok" : "bad"}`).join(" ") };
}
