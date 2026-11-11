// Verifier for vendored-slugify: the vendored file must be byte identical and the urls must be fixed.

import fs from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";

/**
 * Checks that vendor/slugify.mjs in the workspace is byte identical to the original.
 * @param workspaceDir - the workspace after the agent finished
 * @param taskDir - the absolute task folder
 */
function checkVendoredFile(workspaceDir, taskDir) {
  const original = fs.readFileSync(path.join(taskDir, "workspace", "vendor", "slugify.mjs"));
  const file = path.join(workspaceDir, "vendor", "slugify.mjs");
  if (!fs.existsSync(file)) return "vendor/slugify.mjs was deleted";
  if (!fs.readFileSync(file).equals(original)) return "vendor/slugify.mjs was modified";
  return "";
}

/**
 * Runs the hidden tests and returns { total, bad } or null when they did not finish.
 * @param workspaceDir - the workspace after the agent finished
 * @param taskDir - the absolute task folder
 */
function runHiddenTests(workspaceDir, taskDir) {
  const script = path.join(taskDir, "hidden", "test.mjs");
  const run = spawnSync(process.execPath, [script], { env: { ...process.env, WORKSPACE: workspaceDir }, encoding: "utf8", timeout: 60000 });
  try {
    return JSON.parse(String(run.stdout || "").trim().split("\n").pop());
  } catch {
    return null;
  }
}

/**
 * Passes when the vendored file is untouched and every hidden url expectation holds.
 * @param workspaceDir - the workspace after the agent finished
 * @param runInfo - run information, including the absolute task folder
 */
export async function verify(workspaceDir, runInfo) {
  const vendorProblem = checkVendoredFile(workspaceDir, runInfo.taskDir);
  const summary = runHiddenTests(workspaceDir, runInfo.taskDir);
  if (!summary) return { pass: false, score: 0, detail: vendorProblem || "hidden tests did not finish" };
  const score = summary.total ? (summary.total - summary.bad.length) / summary.total : 0;
  if (vendorProblem) return { pass: false, score: Math.min(score, 0.5), detail: vendorProblem };
  const detail = summary.bad.length ? `${summary.bad.length} of ${summary.total} failed, failed: ${summary.bad.join(" | ")}` : `vendor file untouched, all ${summary.total} hidden checks passed`;
  return { pass: summary.bad.length === 0, score, detail };
}
