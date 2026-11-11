// Verifier: runs the hidden node tests against the migrated workspace.

import fs from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";

/**
 * Runs the hidden test file with node's test runner and the workspace path in the environment.
 * @param workspaceDir - the workspace after the agent finished
 * @param runInfo - run details, including taskDir
 */
export async function verify(workspaceDir, runInfo) {
  if (!fs.existsSync(workspaceDir)) return { pass: false, score: 0, detail: "workspace is missing" };
  const file = path.join(runInfo.taskDir, "hidden", "esm.test.mjs");
  const run = spawnSync("node", ["--test", "--test-reporter=tap", file], {
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
  if (failed === 0 && passed > 0) return { pass: true, score: 1, detail: `all ${passed} hidden checks passed` };
  const firsts = [...output.matchAll(/^\s*not ok \d+ - (.+)$/gm)].slice(0, 3).map((m) => m[1]);
  return { pass: false, score: passed / Math.max(1, passed + failed), detail: `${failed} of ${passed + failed} checks failed: ${firsts.join("; ")}` };
}
