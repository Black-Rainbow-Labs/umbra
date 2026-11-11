// Verifier: line endings, bytes outside the six functions, the six behaviors, and the test runner.

import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { buildModuleText, buildRunnerText, MODULE_NAME, RUNNER_NAME, TARGET_NAMES } from "./fixture.mjs";

const SEED = 2617;

/**
 * Counts line endings that are not CRLF (a lone LF or a lone CR).
 * @param buffer - the file bytes
 */
function countBadLineEndings(buffer) {
  let bad = 0;
  for (let index = 0; index < buffer.length; index += 1) {
    if (buffer[index] === 0x0a && buffer[index - 1] !== 0x0d) bad += 1;
    if (buffer[index] === 0x0d && buffer[index + 1] !== 0x0a) bad += 1;
  }
  return bad;
}

/**
 * Splits CRLF text into lines and returns the lines outside the six target function blocks.
 * @param text - file text with CRLF endings
 */
function splitOutside(text) {
  const lines = text.split("\r\n");
  const inBlock = new Array(lines.length).fill(false);
  for (const name of TARGET_NAMES) {
    const start = lines.findIndex((line) => line.startsWith(`def ${name}(`));
    if (start < 0) continue;
    let end = start + 1;
    while (end < lines.length && (lines[end] === "" || /^\s/.test(lines[end]))) end += 1;
    while (end > start + 1 && lines[end - 1] === "") end -= 1;
    for (let index = start; index < end; index += 1) inBlock[index] = true;
  }
  return lines.filter((_, index) => !inBlock[index]);
}

/**
 * Compares the lines outside the six functions with the original module.
 * @param actualText - the edited module text
 */
function outsideProblem(actualText) {
  const original = splitOutside(buildModuleText(SEED, false).replace(/\n/g, "\r\n"));
  const actual = splitOutside(actualText);
  if (original.length !== actual.length) return `lines outside the six functions changed: ${original.length} before, ${actual.length} after`;
  const index = original.findIndex((line, position) => line !== actual[position]);
  return index < 0 ? "" : `a line outside the six functions changed: ${JSON.stringify(actual[index]).slice(0, 80)}`;
}

/**
 * Runs a Python command in a scratch folder with a timeout and returns the spawn result.
 * @param args - python arguments
 * @param cwd - working folder
 */
function runPython(args, cwd) {
  return spawnSync("python", args, { cwd, encoding: "utf8", timeout: 60000, env: { ...process.env, PYTHONDONTWRITEBYTECODE: "1" } });
}

/**
 * Runs the hidden behavior checks against a copy of the edited module.
 * @param moduleBuffer - the edited module bytes
 * @param hiddenDir - the hidden folder
 */
function behaviorResults(moduleBuffer, hiddenDir) {
  const scratch = fs.mkdtempSync(path.join(os.tmpdir(), "eval-stockbook-"));
  try {
    fs.writeFileSync(path.join(scratch, MODULE_NAME), moduleBuffer);
    const run = runPython([path.join(hiddenDir, "checks.py"), scratch], scratch);
    try {
      return JSON.parse((run.stdout || "").trim().split("\n").pop());
    } catch {
      const reason = `${run.stderr || run.stdout || run.error?.message || "no output"}`.trim().split("\n").pop();
      return Object.fromEntries(TARGET_NAMES.map((name) => [name, `module did not load: ${reason}`.slice(0, 200)]));
    }
  } finally {
    fs.rmSync(scratch, { recursive: true, force: true });
  }
}

/**
 * Runs a test runner against the edited module in a scratch folder and returns { code, output }.
 * @param moduleBuffer - the edited module bytes
 * @param runnerBuffer - the runner bytes to execute
 */
function runRunner(moduleBuffer, runnerBuffer) {
  const scratch = fs.mkdtempSync(path.join(os.tmpdir(), "eval-runner-"));
  try {
    fs.writeFileSync(path.join(scratch, MODULE_NAME), moduleBuffer);
    fs.writeFileSync(path.join(scratch, RUNNER_NAME), runnerBuffer);
    const run = runPython([RUNNER_NAME], scratch);
    return { code: run.status, output: `${run.stdout || ""}${run.stderr || ""}`.trim().split("\n").slice(-3).join(" | ") };
  } finally {
    fs.rmSync(scratch, { recursive: true, force: true });
  }
}

/**
 * Names of the test functions defined in a runner text.
 * @param text - runner text
 */
function testNames(text) {
  return [...text.matchAll(/^def (test_\w+)\(/gm)].map((match) => match[1]);
}

/**
 * Checks that each of the six functions has a new test and that the runner keeps the original tests and CRLF.
 * @param runnerText - the edited runner text
 * @param runnerBuffer - the edited runner bytes
 */
function runnerProblem(runnerText, runnerBuffer) {
  const originalNames = testNames(buildRunnerText(false));
  const names = testNames(runnerText);
  const missingOld = originalNames.filter((name) => !names.includes(name));
  if (missingOld.length) return `original tests removed: ${missingOld.join(", ")}`;
  const added = names.filter((name) => !originalNames.includes(name));
  const uncovered = TARGET_NAMES.filter((target) => !added.some((name) => name.includes(target)));
  if (uncovered.length) return `no new test for: ${uncovered.join(", ")}`;
  const bad = countBadLineEndings(runnerBuffer);
  return bad ? `${bad} line endings in run_tests.py are not CRLF` : "";
}

/**
 * Reads a workspace file as a buffer, or an empty buffer when it is missing.
 * @param file - the file path
 */
function readOrEmpty(file) {
  return fs.existsSync(file) ? fs.readFileSync(file) : Buffer.alloc(0);
}

/**
 * Builds the list of named checks, each { name, ok, why }.
 * @param workspaceDir - the workspace after the agent finished
 * @param hiddenDir - the hidden folder
 */
function runChecks(workspaceDir, hiddenDir) {
  const checks = [];
  const add = (name, why) => checks.push({ name, ok: !why, why: why || "" });
  const moduleBuffer = readOrEmpty(path.join(workspaceDir, MODULE_NAME));
  const runnerBuffer = readOrEmpty(path.join(workspaceDir, RUNNER_NAME));
  const bad = countBadLineEndings(moduleBuffer);
  add("stockbook.py all CRLF", !moduleBuffer.length ? "stockbook.py is missing" : bad ? `${bad} line endings are not CRLF` : moduleBuffer.subarray(-2).toString() !== "\r\n" ? "file does not end with CRLF" : "");
  add("bytes outside the six functions", outsideProblem(moduleBuffer.toString("utf8")));
  const behavior = behaviorResults(moduleBuffer, hiddenDir);
  for (const name of TARGET_NAMES) add(`${name} behavior`, behavior[name]);
  const original = runRunner(moduleBuffer, Buffer.from(buildRunnerText(false).replace(/\n/g, "\r\n")));
  add("original tests pass against edited module", original.code === 0 ? "" : original.output);
  add("a new test for each function, CRLF kept", runnerProblem(runnerBuffer.toString("utf8"), runnerBuffer));
  const own = runRunner(moduleBuffer, runnerBuffer);
  add("python run_tests.py exits 0", own.code === 0 ? "" : own.output);
  return checks;
}

/**
 * Verifies the six edits, the line endings and the test runner.
 * @param workspaceDir - the workspace after the agent finished
 * @param runInfo - run information, including taskDir
 */
export async function verify(workspaceDir, runInfo) {
  const hiddenDir = path.join(runInfo.taskDir, "hidden");
  const checks = runChecks(workspaceDir, hiddenDir);
  const passed = checks.filter((check) => check.ok).length;
  const failed = checks.filter((check) => !check.ok);
  const detail = failed.length ? `${passed}/${checks.length} checks; failed: ${failed.slice(0, 3).map((check) => `${check.name} (${check.why.slice(0, 100)})`).join("; ")}` : `all ${checks.length} checks passed`;
  return { pass: failed.length === 0, score: passed / checks.length, detail };
}
