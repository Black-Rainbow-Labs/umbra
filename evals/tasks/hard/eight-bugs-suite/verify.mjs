// Verifier: eleven checks. Eight hidden test classes (one per bug), a class of behavior that must not
// change, the visible tests left unedited, and the visible test suite passing.

import fs from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";

const BUG_CLASSES = [
  ["Bug1DiscountRounding", "discount rounds half up"],
  ["Bug2BulkBoundaries", "bulk discount starts at 10 and 50 units"],
  ["Bug3BusinessDays", "business days skip the weekend"],
  ["Bug4PriceParsing", "prices parse to exact cents"],
  ["Bug5ReserveExactStock", "reserve can take all stock"],
  ["Bug6AuditList", "audit list is not shared"],
  ["Bug7ReportOrder", "report sorts by number then name"],
  ["Bug8Wrapping", "wrapped lines fit the width"],
  ["BehaviorThatMustNotChange", "other behavior unchanged"],
];

/**
 * Lists the files under a folder as relative paths with forward slashes.
 * @param root - the folder to walk
 */
function listFiles(root) {
  const out = [];
  const walk = (dir) => {
    for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
      if (entry.name === "__pycache__") continue;
      const full = path.join(dir, entry.name);
      if (entry.isDirectory()) walk(full);
      else out.push(path.relative(root, full).split(path.sep).join("/"));
    }
  };
  if (fs.existsSync(root)) walk(root);
  return out.sort();
}

/**
 * Checks that every file under the workspace tests folder is byte for byte the original.
 * @param workspaceDir - the workspace after the agent finished
 * @param taskDir - the task folder holding the original workspace
 */
function testsUnchanged(workspaceDir, taskDir) {
  const original = path.join(taskDir, "workspace", "tests");
  const current = path.join(workspaceDir, "tests");
  const names = listFiles(original);
  if (JSON.stringify(names) !== JSON.stringify(listFiles(current))) return false;
  return names.every((name) => fs.readFileSync(path.join(original, name)).equals(fs.readFileSync(path.join(current, name))));
}

/**
 * Runs python with the workspace as the working folder and importable package root.
 * @param args - python arguments
 * @param workspaceDir - the workspace folder
 */
function runPython(args, workspaceDir) {
  return spawnSync("python", args, { cwd: workspaceDir, env: { ...process.env, PYTHONPATH: workspaceDir, PYTHONDONTWRITEBYTECODE: "1" }, encoding: "utf8", timeout: 60000 });
}

/**
 * Runs the hidden tests and returns the per class counts, or null when they could not run.
 * @param workspaceDir - the workspace folder
 * @param taskDir - the task folder
 */
function runHidden(workspaceDir, taskDir) {
  const run = runPython([path.join(taskDir, "hidden", "run_hidden.py")], workspaceDir);
  try {
    return JSON.parse(run.stdout.trim().split("\n").pop());
  } catch {
    return null;
  }
}

/**
 * Runs the checks and returns { pass, score, detail }.
 * @param workspaceDir - the workspace after the agent finished
 * @param runInfo - run details, including taskDir
 */
export async function verify(workspaceDir, runInfo) {
  try {
    const results = [];
    const counts = runHidden(workspaceDir, runInfo.taskDir);
    for (const [name, label] of BUG_CLASSES) {
      const entry = counts?.[name];
      results.push({ label, ok: Boolean(entry) && entry.failed === 0 && entry.passed > 0 });
    }
    results.push({ label: "visible tests are unedited", ok: testsUnchanged(workspaceDir, runInfo.taskDir) });
    const visible = runPython(["-m", "unittest", "discover", "-s", "tests", "-t", "."], workspaceDir);
    results.push({ label: "visible tests pass", ok: visible.status === 0 && /Ran 36 tests/.test(visible.stderr || "") });
    const failed = results.filter((r) => !r.ok).map((r) => r.label);
    const score = (results.length - failed.length) / results.length;
    return { pass: failed.length === 0, score, detail: failed.length ? `failed checks: ${failed.join("; ")}` : `all ${results.length} checks passed` };
  } catch (error) {
    return { pass: false, score: 0, detail: `verifier error: ${error.message}` };
  }
}
