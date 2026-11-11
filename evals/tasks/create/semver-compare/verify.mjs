// Verifier for semver-compare: runs the hidden Node test script against the workspace.

import path from "node:path";
import { spawnSync } from "node:child_process";

/**
 * Runs the hidden Node test script and turns its JSON summary line into a verdict.
 * @param workspaceDir - the workspace after the agent finished
 * @param runInfo - run information, including the absolute task folder
 */
export async function verify(workspaceDir, runInfo) {
  const script = path.join(runInfo.taskDir, "hidden", "test.mjs");
  const run = spawnSync(process.execPath, [script], { env: { ...process.env, WORKSPACE: workspaceDir }, encoding: "utf8", timeout: 60000 });
  const lastLine = String(run.stdout || "").trim().split("\n").pop();
  let summary;
  try {
    summary = JSON.parse(lastLine);
  } catch {
    return { pass: false, score: 0, detail: `hidden tests did not finish: ${String(run.stderr || run.error || "").slice(0, 200)}` };
  }
  const score = summary.total ? Math.max(0, summary.total - summary.bad.length) / summary.total : 0;
  const detail = summary.bad.length ? `${summary.bad.length} of ${summary.total} failed, failed: ${summary.bad.join(" | ")}` : `all ${summary.total} hidden tests passed`;
  return { pass: summary.bad.length === 0, score, detail };
}
