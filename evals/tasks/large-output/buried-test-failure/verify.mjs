// Verifier for the buried test failure task: runs hidden checks against the workspace's parseDuration.

import path from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const HIDDEN_TEST = path.join(path.dirname(fileURLToPath(import.meta.url)), "hidden", "hidden-test.mjs");

/**
 * Runs the hidden checks and the workspace's own test runner, and scores the hidden checks.
 * @param workspaceDir - the workspace after the agent finished
 */
export async function verify(workspaceDir) {
  const run = spawnSync("node", [HIDDEN_TEST, workspaceDir], { encoding: "utf8", timeout: 60000 });
  if (run.status !== 0) return { pass: false, score: 0, detail: `hidden checks did not run: ${(run.stderr || "").trim().split("\n").pop()}` };
  const outcome = JSON.parse(run.stdout.trim().split("\n").pop());
  const ownRun = spawnSync("node", [path.join(workspaceDir, "run-tests.mjs")], { encoding: "utf8", timeout: 60000, maxBuffer: 64 * 1024 * 1024 });
  const ownPasses = ownRun.status === 0;
  const pass = outcome.failures.length === 0 && ownPasses;
  const detail = outcome.failures.length ? `hidden check failed: ${outcome.failures[0]}` : ownPasses ? "all checks pass" : "hidden checks pass but node run-tests.mjs still fails (cases or runner were changed)";
  return { pass, score: outcome.passed / outcome.total * (ownPasses ? 1 : 0.9), detail };
}
