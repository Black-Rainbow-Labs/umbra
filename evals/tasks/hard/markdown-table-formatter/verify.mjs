// Verifier: runs the hidden unittest file against a copy of the workspace; rules R1 to R11 are each one check
// that passes only when every test named for the rule passes.

import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

const RULE_COUNT = 11;

/**
 * Copies the workspace to a temp folder, adds the hidden test file and runs unittest verbosely.
 * @param workspaceDir - the workspace after the agent finished
 * @param taskDir - the task folder
 */
function runHiddenTests(workspaceDir, taskDir) {
  const temp = fs.mkdtempSync(path.join(os.tmpdir(), "eval-mdtable-"));
  try {
    fs.cpSync(workspaceDir, temp, { recursive: true, filter: (source) => path.basename(source) !== "__pycache__" });
    fs.copyFileSync(path.join(taskDir, "hidden", "test_rules_hidden.py"), path.join(temp, "test_rules_hidden.py"));
    const run = spawnSync("python", ["-m", "unittest", "-v", "test_rules_hidden"], { cwd: temp, encoding: "utf8", timeout: 60000, env: { ...process.env, PYTHONDONTWRITEBYTECODE: "1" } });
    return `${run.stdout || ""}${run.stderr || ""}`;
  } finally {
    fs.rmSync(temp, { recursive: true, force: true });
  }
}

/**
 * Turns unittest -v output into a verdict with one check per rule.
 * @param workspaceDir - the workspace after the agent finished
 * @param runInfo - run details, including taskDir
 */
export async function verify(workspaceDir, runInfo) {
  if (!fs.existsSync(path.join(workspaceDir, "mdtable.py"))) return { pass: false, score: 0, detail: "mdtable.py is missing" };
  const output = runHiddenTests(workspaceDir, runInfo.taskDir);
  const state = new Map();
  for (const match of output.matchAll(/^test_(R\d+)_\w+ .*\.\.\. (ok|FAIL|ERROR)\s*$/gm)) {
    if (!state.has(match[1])) state.set(match[1], true);
    if (match[2] !== "ok") state.set(match[1], false);
  }
  if (state.size === 0) return { pass: false, score: 0, detail: `hidden tests did not run: ${output.trim().slice(-200)}` };
  const failed = [];
  for (let rule = 1; rule <= RULE_COUNT; rule += 1) if (state.get(`R${rule}`) !== true) failed.push(`R${rule}`);
  const firstProblem = /^(?:FAIL|ERROR): test_(R\d+_\w+)[^\n]*\n[-=]+\n[\s\S]*?\n((?:\w*Error|AssertionError)[^\n]*)/m.exec(output);
  return {
    pass: failed.length === 0,
    score: (RULE_COUNT - failed.length) / RULE_COUNT,
    detail: failed.length ? `rules failed: ${failed.join(", ")}${firstProblem ? ` (${firstProblem[1]}: ${firstProblem[2].slice(0, 120)})` : ""}` : `all ${RULE_COUNT} rules passed`,
  };
}
