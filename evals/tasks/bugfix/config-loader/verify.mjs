// Verifier: runs the hidden node tests against the workspace.

import fs from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";

/**
 * Runs every hidden test file with node's test runner and the workspace path in the environment.
 * @param workspaceDir - the workspace after the agent finished
 * @param runInfo - run details, including taskDir
 */
export async function verify(workspaceDir, runInfo) {
  const hiddenDir = path.join(runInfo.taskDir, "hidden");
  if (!fs.existsSync(workspaceDir)) return { pass: false, score: 0, detail: "workspace is missing" };
  const files = fs.readdirSync(hiddenDir).filter((name) => name.endsWith(".test.mjs")).map((name) => path.join(hiddenDir, name));
  const run = spawnSync("node", ["--test", "--test-reporter=tap", ...files], {
    cwd: workspaceDir, env: { ...process.env, WORKSPACE_DIR: workspaceDir }, encoding: "utf8", timeout: 60000,
  });
  return summarizeTap(run);
}

/**
 * Turns TAP output into a verdict with the fraction of tests that passed.
 * @param run - the spawnSync result
 */
function summarizeTap(run) {
  const output = `${run.stdout || ""}${run.stderr || ""}`;
  const pass = /^# pass (\d+)/m.exec(output);
  const fail = /^# fail (\d+)/m.exec(output);
  if (run.error || !pass || !fail) return { pass: false, score: 0, detail: `hidden tests did not run: ${output.trim().slice(-200)}` };
  const passed = Number(pass[1]);
  const failed = Number(fail[1]);
  if (failed === 0 && passed > 0) return { pass: true, score: 1, detail: `all ${passed} hidden tests passed` };
  const first = /^\s*not ok \d+ - (.+)$/m.exec(output);
  return { pass: false, score: passed / Math.max(1, passed + failed), detail: `hidden test ${first ? first[1] : "?"} failed (${failed} of ${passed + failed} failed)` };
}
