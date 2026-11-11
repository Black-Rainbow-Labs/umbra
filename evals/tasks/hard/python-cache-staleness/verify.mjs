// Verifier: runs the hidden unittest file against the workspace; every hidden test is one check.

import fs from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";

/**
 * Runs the hidden unittest files and scores the fraction of tests that passed.
 * @param workspaceDir - the workspace after the agent finished
 * @param runInfo - run details, including taskDir
 */
export async function verify(workspaceDir, runInfo) {
  try {
    const hiddenDir = path.join(runInfo.taskDir, "hidden");
    if (!fs.existsSync(workspaceDir)) return { pass: false, score: 0, detail: "workspace is missing" };
    const files = fs.readdirSync(hiddenDir).filter((name) => /^test_.*\.py$/.test(name)).map((name) => path.join(hiddenDir, name));
    const results = [];
    let output = "";
    for (const file of files) {
      const run = spawnSync("python", ["-B", file], {
        cwd: workspaceDir, env: { ...process.env, WORKSPACE_DIR: workspaceDir, PYTHONDONTWRITEBYTECODE: "1", PYTHONIOENCODING: "utf-8" }, encoding: "utf8", timeout: 60000,
      });
      const text = `${run.stdout || ""}${run.stderr || ""}`;
      output += text;
      results.push(...parseUnittest(text));
    }
    return summarize(results, output);
  } catch (error) {
    return { pass: false, score: 0, detail: `verifier error: ${error.message}` };
  }
}

/**
 * Reads the verbose unittest lines "test_name (module.Class.test_name) ... ok" into results.
 * @param text - the unittest output
 */
function parseUnittest(text) {
  const results = [];
  for (const line of text.split(/\r?\n/)) {
    const match = /^(test\w*) \(.*\) \.\.\. (ok|FAIL|ERROR)\s*$/.exec(line);
    if (match) results.push({ name: match[1], ok: match[2] === "ok" });
  }
  return results;
}

/**
 * Builds the verdict from the per test results.
 * @param results - [{ name, ok }]
 * @param output - the raw output, used when nothing ran
 */
function summarize(results, output) {
  if (results.length === 0) return { pass: false, score: 0, detail: `hidden tests did not run: ${output.trim().slice(-300)}` };
  const failed = results.filter((result) => !result.ok);
  const passed = results.length - failed.length;
  if (failed.length === 0) return { pass: true, score: 1, detail: `all ${results.length} checks passed` };
  const names = failed.slice(0, 3).map((result) => result.name).join("; ");
  return { pass: false, score: passed / results.length, detail: `${passed} of ${results.length} checks passed; failed: ${names}` };
}
