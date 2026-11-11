// Verifier: runs vm.mjs from the workspace on hidden programs. One check per group of cases; a group passes
// when every case matches standard output, standard error and the exit code exactly.

import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { pathToFileURL } from "node:url";

/**
 * Runs one case in a fresh temporary folder and returns null when it matches, or a short reason.
 * @param vmPath - absolute path of vm.mjs in the workspace
 * @param testCase - { program, args, noFile, stdout, stderr, code }
 */
function runCase(vmPath, testCase) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "svm-hidden-"));
  try {
    let args = testCase.args;
    if (!testCase.noFile) {
      fs.writeFileSync(path.join(dir, "p.svm"), testCase.program);
      args = [path.join(dir, "p.svm")];
    }
    const run = spawnSync("node", [vmPath, ...args], { cwd: dir, encoding: "utf8", timeout: 30000, maxBuffer: 64 * 1024 * 1024 });
    if (run.error) return `${testCase.name}: ${run.error.message}`;
    if (run.stdout !== testCase.stdout) return `${testCase.name}: stdout was ${JSON.stringify((run.stdout || "").slice(0, 60))}`;
    if (run.stderr !== testCase.stderr) return `${testCase.name}: stderr was ${JSON.stringify((run.stderr || "").slice(0, 100))}`;
    if (run.status !== testCase.code) return `${testCase.name}: exit code ${run.status}`;
    return null;
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
}

/**
 * Runs every group and returns { pass, score, detail }.
 * @param workspaceDir - the workspace after the agent finished
 * @param runInfo - run details, including taskDir
 */
export async function verify(workspaceDir, runInfo) {
  try {
    const vmPath = path.join(workspaceDir, "vm.mjs");
    if (!fs.existsSync(vmPath)) return { pass: false, score: 0, detail: "vm.mjs is missing" };
    const { GROUPS } = await import(pathToFileURL(path.join(runInfo.taskDir, "hidden", "cases.mjs")).href);
    const failures = [];
    for (const group of GROUPS) {
      const reasons = group.cases.map((testCase) => runCase(vmPath, testCase)).filter(Boolean);
      if (reasons.length) failures.push(`${group.name} (${reasons.length} of ${group.cases.length} cases, first: ${reasons[0]})`);
    }
    const score = (GROUPS.length - failures.length) / GROUPS.length;
    return { pass: failures.length === 0, score, detail: failures.length ? `failed groups: ${failures.slice(0, 3).join(" | ")}` : `all ${GROUPS.length} groups passed` };
  } catch (error) {
    return { pass: false, score: 0, detail: `verifier error: ${error.message}` };
  }
}
