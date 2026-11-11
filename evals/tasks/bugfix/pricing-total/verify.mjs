// Verifier: runs the hidden unittest suite against the workspace.

import fs from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";

/**
 * Runs the hidden Python tests with the workspace on the import path.
 * @param workspaceDir - the workspace after the agent finished
 * @param runInfo - run details, including taskDir
 */
export async function verify(workspaceDir, runInfo) {
  const hiddenDir = path.join(runInfo.taskDir, "hidden");
  if (!fs.existsSync(workspaceDir)) return { pass: false, score: 0, detail: "workspace is missing" };
  const run = spawnSync("python", ["-B", "-m", "unittest", "discover", "-s", hiddenDir, "-p", "test_*.py"], {
    cwd: workspaceDir, env: { ...process.env, WORKSPACE_DIR: workspaceDir, PYTHONDONTWRITEBYTECODE: "1" }, encoding: "utf8", timeout: 60000,
  });
  return summarizeUnittest(run);
}

/**
 * Turns unittest output into a verdict with the fraction of tests that passed.
 * @param run - the spawnSync result
 */
function summarizeUnittest(run) {
  const output = `${run.stderr || ""}${run.stdout || ""}`;
  const ran = /Ran (\d+) tests?/.exec(output);
  if (run.error || !ran) return { pass: false, score: 0, detail: `hidden tests did not run: ${output.trim().split("\n").slice(-3).join(" | ").slice(0, 200)}` };
  const total = Number(ran[1]);
  const failed = (output.match(/^(FAIL|ERROR): /gm) || []).length;
  const first = /^(FAIL|ERROR): (\S+)/m.exec(output);
  if (run.status === 0) return { pass: true, score: 1, detail: `all ${total} hidden tests passed` };
  const detail = first ? `hidden test ${first[2]} failed (${failed} of ${total} failed)` : `hidden tests failed: ${output.trim().slice(-150)}`;
  return { pass: false, score: Math.max(0, (total - failed) / total), detail };
}
