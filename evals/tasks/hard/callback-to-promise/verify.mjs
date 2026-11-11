// Verifier: the 18 existing tests still pass (run from a hidden copy), each of requirements R1 to R9 is one check
// from the hidden promise tests, and package.json was not changed.

import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

/**
 * Runs node's test runner and returns the TAP output.
 * @param files - absolute test files
 * @param cwd - working directory
 * @param workspaceDir - value of WORKSPACE_DIR
 */
function runTap(files, cwd, workspaceDir) {
  const run = spawnSync("node", ["--test", "--test-reporter=tap", ...files], { cwd, env: { ...process.env, WORKSPACE_DIR: workspaceDir }, encoding: "utf8", timeout: 60000 });
  return `${run.stdout || ""}${run.stderr || ""}`;
}

/**
 * Check: the original callback tests, copied from hidden/old, pass against the workspace src folder.
 * @param workspaceDir - the workspace
 * @param taskDir - the task folder
 */
function checkOldTests(workspaceDir, taskDir) {
  const temp = fs.mkdtempSync(path.join(os.tmpdir(), "eval-a2p-"));
  try {
    fs.cpSync(path.join(workspaceDir, "src"), path.join(temp, "src"), { recursive: true });
    fs.mkdirSync(path.join(temp, "test"));
    const names = fs.readdirSync(path.join(taskDir, "hidden", "old"));
    for (const name of names) fs.copyFileSync(path.join(taskDir, "hidden", "old", name), path.join(temp, "test", name));
    const output = runTap(names.map((name) => path.join(temp, "test", name)), temp, workspaceDir);
    const pass = /^# pass (\d+)/m.exec(output);
    const fail = /^# fail (\d+)/m.exec(output);
    if (pass && fail && Number(pass[1]) === 18 && Number(fail[1]) === 0) return "";
    return `existing tests: ${pass ? pass[1] : "?"} passed, ${fail ? fail[1] : "?"} failed`;
  } finally {
    fs.rmSync(temp, { recursive: true, force: true });
  }
}

/**
 * Check: package.json is byte for byte unchanged, so no dependency was added.
 * @param workspaceDir - the workspace
 * @param taskDir - the task folder
 */
function checkPackageUnchanged(workspaceDir, taskDir) {
  const original = fs.readFileSync(path.join(taskDir, "workspace", "package.json"));
  return fs.readFileSync(path.join(workspaceDir, "package.json")).equals(original) ? "" : "package.json was changed";
}

/**
 * Runs all checks and returns the verdict.
 * @param workspaceDir - the workspace after the agent finished
 * @param runInfo - run details, including taskDir
 */
export async function verify(workspaceDir, runInfo) {
  const taskDir = runInfo.taskDir;
  if (!fs.existsSync(path.join(workspaceDir, "src"))) return { pass: false, score: 0, detail: "src is missing" };
  const results = [];
  const guarded = (label, fn) => {
    try {
      const problem = fn();
      results.push([label, problem]);
    } catch (error) {
      results.push([label, `error: ${error.message.slice(0, 100)}`]);
    }
  };
  guarded("existing tests", () => checkOldTests(workspaceDir, taskDir));
  guarded("package.json", () => checkPackageUnchanged(workspaceDir, taskDir));
  const output = runTap([path.join(taskDir, "hidden", "promises.test.mjs")], workspaceDir, workspaceDir);
  const state = new Map();
  for (const match of output.matchAll(/^(not ok|ok) \d+ - (R\d+) (.*)$/gm)) {
    if (!state.has(match[2])) state.set(match[2], true);
    if (match[1] === "not ok") state.set(match[2], false);
  }
  for (let index = 1; index <= 9; index += 1) {
    const tag = `R${index}`;
    results.push([tag, state.get(tag) === true ? "" : state.has(tag) ? "failed" : `not run: ${output.trim().slice(-120)}`]);
  }
  const failures = results.filter(([, problem]) => problem).map(([label, problem]) => `${label}: ${problem}`);
  const passed = results.length - failures.length;
  return { pass: failures.length === 0, score: passed / results.length, detail: failures.length ? failures.slice(0, 4).join(" | ") : `all ${results.length} checks passed` };
}
