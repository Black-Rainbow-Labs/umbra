// Verifier: runs the hidden node tests against the workspace; every hidden test is one check.

import fs from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";

/**
 * Runs every hidden test file with node's test runner and the workspace path in the environment.
 * @param workspaceDir - the workspace after the agent finished
 * @param runInfo - run details, including taskDir
 */
export async function verify(workspaceDir, runInfo) {
  try {
    const hiddenDir = path.join(runInfo.taskDir, "hidden");
    if (!fs.existsSync(workspaceDir)) return { pass: false, score: 0, detail: "workspace is missing" };
    const files = fs.readdirSync(hiddenDir).filter((name) => name.endsWith(".test.mjs")).map((name) => path.join(hiddenDir, name));
    const run = spawnSync("node", ["--test", "--test-reporter=tap", ...files], {
      cwd: workspaceDir, env: { ...process.env, WORKSPACE_DIR: workspaceDir }, encoding: "utf8", timeout: 60000,
    });
    return summarize(parseTap(`${run.stdout || ""}${run.stderr || ""}`), run);
  } catch (error) {
    return { pass: false, score: 0, detail: `verifier error: ${error.message}` };
  }
}

/**
 * Extracts the top level test results from TAP output.
 * @param output - the TAP text
 */
function parseTap(output) {
  const results = [];
  for (const line of output.split(/\r?\n/)) {
    const match = /^(not )?ok \d+ - (.+?)(?: # .*)?$/.exec(line);
    if (match) results.push({ ok: !match[1], name: match[2] });
  }
  return { results, output };
}

/**
 * Builds the verdict: pass only when every hidden test passed, score is the fraction that passed.
 * @param parsed - { results, output } from parseTap
 * @param run - the spawnSync result
 */
function summarize(parsed, run) {
  const total = parsed.results.length;
  if (run.error || total === 0) return { pass: false, score: 0, detail: `hidden tests did not run: ${parsed.output.trim().slice(-300)}` };
  const failed = parsed.results.filter((result) => !result.ok);
  const passed = total - failed.length;
  if (failed.length === 0) return { pass: true, score: 1, detail: `all ${total} checks passed` };
  const names = failed.slice(0, 3).map((result) => result.name).join("; ");
  return { pass: false, score: passed / total, detail: `${passed} of ${total} checks passed; failed: ${names}` };
}
