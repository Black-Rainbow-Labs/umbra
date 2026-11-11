// Verifier: runs the hidden node tests under three process time zones. A hidden test counts as
// passed only when it passes in every time zone, and each test is one check.

import fs from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";

const ZONES = ["UTC", "America/Los_Angeles", "Pacific/Auckland"];

/**
 * Runs the hidden tests once with the given TZ and returns the top level TAP results.
 * @param workspaceDir - the workspace after the agent finished
 * @param files - absolute paths of the hidden test files
 * @param zone - the TZ value for the child process
 */
function runInZone(workspaceDir, files, zone) {
  const run = spawnSync("node", ["--test", "--test-reporter=tap", ...files], {
    cwd: workspaceDir, env: { ...process.env, WORKSPACE_DIR: workspaceDir, TZ: zone }, encoding: "utf8", timeout: 60000,
  });
  const output = `${run.stdout || ""}${run.stderr || ""}`;
  const results = [];
  for (const line of output.split(/\r?\n/)) {
    const match = /^(not )?ok \d+ - (.+?)(?: # .*)?$/.exec(line);
    if (match) results.push({ ok: !match[1], name: match[2] });
  }
  return { results, output, error: run.error };
}

/**
 * Verifies the workspace: every hidden test must pass in UTC, America/Los_Angeles and Pacific/Auckland.
 * @param workspaceDir - the workspace after the agent finished
 * @param runInfo - run details, including taskDir
 */
export async function verify(workspaceDir, runInfo) {
  try {
    const hiddenDir = path.join(runInfo.taskDir, "hidden");
    if (!fs.existsSync(workspaceDir)) return { pass: false, score: 0, detail: "workspace is missing" };
    const files = fs.readdirSync(hiddenDir).filter((name) => name.endsWith(".test.mjs")).map((name) => path.join(hiddenDir, name));
    const verdicts = new Map();
    let lastOutput = "";
    for (const zone of ZONES) {
      const outcome = runInZone(workspaceDir, files, zone);
      lastOutput = outcome.output;
      for (const result of outcome.results) {
        const entry = verdicts.get(result.name) || { failedIn: [] };
        if (!result.ok) entry.failedIn.push(zone);
        verdicts.set(result.name, entry);
      }
    }
    if (verdicts.size === 0) return { pass: false, score: 0, detail: `hidden tests did not run: ${lastOutput.trim().slice(-300)}` };
    const failed = [...verdicts.entries()].filter(([, entry]) => entry.failedIn.length > 0);
    const total = verdicts.size;
    if (failed.length === 0) return { pass: true, score: 1, detail: `all ${total} checks passed in ${ZONES.length} time zones` };
    const names = failed.slice(0, 3).map(([name, entry]) => `${name} (${entry.failedIn.join(", ")})`).join("; ");
    return { pass: false, score: (total - failed.length) / total, detail: `${total - failed.length} of ${total} checks passed; failed: ${names}` };
  } catch (error) {
    return { pass: false, score: 0, detail: `verifier error: ${error.message}` };
  }
}
