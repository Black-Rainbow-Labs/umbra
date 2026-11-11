// Verifier: the 40 old tests still pass, the four new modules export the stated API and share one
// state, the old entry is a re-export, and the import rules and size limits hold.

import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

const MODULES = ["money", "validate", "pricing", "report"];
const ALLOWED_IMPORTS = { money: [], validate: ["money"], pricing: ["money", "validate"], report: ["money", "validate", "pricing"] };

/**
 * Runs node's test runner on files and returns the TAP counts.
 * @param files - absolute test file paths
 * @param cwd - working directory
 * @param workspaceDir - value for WORKSPACE_DIR
 */
function runTests(files, cwd, workspaceDir) {
  const run = spawnSync("node", ["--test", "--test-reporter=tap", ...files], { cwd, env: { ...process.env, WORKSPACE_DIR: workspaceDir }, encoding: "utf8", timeout: 60000 });
  const output = `${run.stdout || ""}${run.stderr || ""}`;
  const pass = /^# pass (\d+)/m.exec(output);
  const fail = /^# fail (\d+)/m.exec(output);
  const first = /^\s*not ok \d+ - (.+)$/m.exec(output);
  return { passed: pass ? Number(pass[1]) : 0, failed: fail ? Number(fail[1]) : -1, first: first ? first[1] : output.trim().slice(-150) };
}

/**
 * Check: the original 40 tests, copied from the hidden folder, pass against the workspace src folder.
 * @param workspaceDir - the workspace
 * @param hiddenDir - the hidden folder
 */
function checkOldTests(workspaceDir, hiddenDir) {
  const temp = fs.mkdtempSync(path.join(os.tmpdir(), "eval-orderkit-"));
  try {
    fs.cpSync(path.join(workspaceDir, "src"), path.join(temp, "src"), { recursive: true });
    fs.mkdirSync(path.join(temp, "test"));
    fs.copyFileSync(path.join(hiddenDir, "old", "orderkit.test.mjs"), path.join(temp, "test", "orderkit.test.mjs"));
    const result = runTests([path.join(temp, "test", "orderkit.test.mjs")], temp, workspaceDir);
    if (result.passed === 40 && result.failed === 0) return "";
    return `old tests: ${result.passed} passed, ${result.failed} failed (${result.first})`;
  } finally {
    fs.rmSync(temp, { recursive: true, force: true });
  }
}

/**
 * Check: one hidden test file passes completely.
 * @param name - the file name in hidden/
 * @param workspaceDir - the workspace
 * @param hiddenDir - the hidden folder
 */
function checkHiddenFile(name, workspaceDir, hiddenDir) {
  const result = runTests([path.join(hiddenDir, name)], hiddenDir, workspaceDir);
  if (result.failed === 0 && result.passed > 0) return "";
  return `${name}: ${result.first}`;
}

/**
 * Removes comments from JavaScript source text.
 * @param text - the source
 */
function stripComments(text) {
  return text.replace(/\/\*[\s\S]*?\*\//g, "").replace(/^\s*\/\/.*$/gm, "");
}

/**
 * Check: src/orderkit.mjs only has export-from statements and is short.
 * @param workspaceDir - the workspace
 */
function checkEntryIsReexport(workspaceDir) {
  const text = fs.readFileSync(path.join(workspaceDir, "src", "orderkit.mjs"), "utf8");
  const code = stripComments(text);
  if (code.split("\n").length > 60) return "src/orderkit.mjs is longer than 60 lines";
  if (/\b(function|class|const|let|var|return|if|for)\b/.test(code.replace(/export\s*\{[^}]*\}\s*from/g, "").replace(/export\s*\*\s*from/g, ""))) {
    return "src/orderkit.mjs contains code other than export-from statements";
  }
  return "";
}

/**
 * Reads the relative module names a file imports or re-exports.
 * @param text - the file text
 */
function importedModules(text) {
  const names = [];
  for (const match of stripComments(text).matchAll(/(?:import|export)\s[^;]*?from\s*["']\.\/([\w-]+)\.mjs["']/g)) names.push(match[1]);
  for (const match of stripComments(text).matchAll(/import\s*["']\.\/([\w-]+)\.mjs["']/g)) names.push(match[1]);
  return names;
}

/**
 * Check: each new module imports only what the dependency rule allows.
 * @param workspaceDir - the workspace
 */
function checkImportRules(workspaceDir) {
  for (const name of MODULES) {
    const text = fs.readFileSync(path.join(workspaceDir, "src", `${name}.mjs`), "utf8");
    for (const imported of importedModules(text)) {
      if (!ALLOWED_IMPORTS[name].includes(imported)) return `src/${name}.mjs imports ./${imported}.mjs, which the rule does not allow`;
    }
  }
  return "";
}

/**
 * Check: every new module is under 300 lines and each of the 49 public names is defined in one file only.
 * @param workspaceDir - the workspace
 * @param names - all 49 public names
 */
function checkSplitQuality(workspaceDir, names) {
  const texts = {};
  for (const name of MODULES) {
    texts[name] = stripComments(fs.readFileSync(path.join(workspaceDir, "src", `${name}.mjs`), "utf8"));
    const lines = fs.readFileSync(path.join(workspaceDir, "src", `${name}.mjs`), "utf8").split("\n").length;
    if (lines >= 300) return `src/${name}.mjs has ${lines} lines, limit is 299`;
  }
  for (const exported of names) {
    const pattern = new RegExp(`^\\s*(?:export\\s+)?(?:async\\s+)?(?:function|class|const)\\s+${exported}\\b`, "m");
    const holders = MODULES.filter((name) => pattern.test(texts[name]));
    if (holders.length !== 1) return `${exported} is defined in ${holders.length} modules`;
  }
  return "";
}

/**
 * Check: the test file in the workspace is byte for byte the original.
 * @param workspaceDir - the workspace
 * @param hiddenDir - the hidden folder
 */
function checkTestsUnchanged(workspaceDir, hiddenDir) {
  const mine = fs.readFileSync(path.join(workspaceDir, "test", "orderkit.test.mjs"));
  const original = fs.readFileSync(path.join(hiddenDir, "old", "orderkit.test.mjs"));
  return mine.equals(original) ? "" : "test/orderkit.test.mjs was changed";
}

/**
 * Runs one check, turning any thrown error into a failure message.
 * @param fn - function returning "" on success or a message
 */
function guarded(fn) {
  try {
    return fn();
  } catch (error) {
    return `error: ${error.message.slice(0, 120)}`;
  }
}

/**
 * Runs all checks and returns the verdict.
 * @param workspaceDir - the workspace after the agent finished
 * @param runInfo - run details, including taskDir
 */
export async function verify(workspaceDir, runInfo) {
  const hiddenDir = path.join(runInfo.taskDir, "hidden");
  if (!fs.existsSync(workspaceDir)) return { pass: false, score: 0, detail: "workspace is missing" };
  const names = [];
  const checks = [
    ["old tests", () => checkOldTests(workspaceDir, hiddenDir)],
    ["money module", () => checkHiddenFile("money.test.mjs", workspaceDir, hiddenDir)],
    ["validate module", () => checkHiddenFile("validate.test.mjs", workspaceDir, hiddenDir)],
    ["pricing module", () => checkHiddenFile("pricing.test.mjs", workspaceDir, hiddenDir)],
    ["report module", () => checkHiddenFile("report.test.mjs", workspaceDir, hiddenDir)],
    ["entry exports and shared state", () => checkHiddenFile("entry.test.mjs", workspaceDir, hiddenDir)],
    ["entry is a re-export", () => checkEntryIsReexport(workspaceDir)],
    ["import rules", () => checkImportRules(workspaceDir)],
    ["split quality", () => checkSplitQuality(workspaceDir, publicNames(hiddenDir, names))],
    ["tests unchanged", () => checkTestsUnchanged(workspaceDir, hiddenDir)],
  ];
  const failures = [];
  for (const [label, fn] of checks) {
    const problem = guarded(fn);
    if (problem) failures.push(`${label}: ${problem}`);
  }
  const passed = checks.length - failures.length;
  return { pass: failures.length === 0, score: passed / checks.length, detail: failures.length ? failures.slice(0, 3).join(" | ") : `all ${checks.length} checks passed` };
}

/**
 * Reads the list of 49 public names from the hidden helper file.
 * @param hiddenDir - the hidden folder
 * @param cache - an array used to remember the result
 */
function publicNames(hiddenDir, cache) {
  if (cache.length) return cache;
  const text = fs.readFileSync(path.join(hiddenDir, "_load.mjs"), "utf8");
  for (const list of text.matchAll(/^\s+\w+: \[(.*)\],?$/gm)) {
    for (const item of list[1].matchAll(/"(\w+)"/g)) cache.push(item[1]);
  }
  return cache;
}
