// Verifier: runs the hidden tests; each requirement R1 to R12 is one check and passes only when every test named for it passes.

import fs from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";

/**
 * Runs the hidden test file against the workspace and groups the TAP results by requirement tag.
 * @param workspaceDir - the workspace after the agent finished
 * @param runInfo - run details, including taskDir
 */
export async function verify(workspaceDir, runInfo) {
  const file = path.join(runInfo.taskDir, "hidden", "limiter.test.mjs");
  if (!fs.existsSync(path.join(workspaceDir, "src", "rate-limiter.mjs"))) return { pass: false, score: 0, detail: "src/rate-limiter.mjs is missing" };
  const run = spawnSync("node", ["--test", "--test-reporter=tap", file], { cwd: workspaceDir, env: { ...process.env, WORKSPACE_DIR: workspaceDir }, encoding: "utf8", timeout: 60000 });
  const output = `${run.stdout || ""}${run.stderr || ""}`;
  const groups = new Map();
  for (let index = 1; index <= 12; index += 1) groups.set(`R${index}`, true);
  let seen = 0;
  for (const match of output.matchAll(/^(not ok|ok) \d+ - (R\d+) (.*)$/gm)) {
    seen += 1;
    if (match[1] === "not ok") groups.set(match[2], false);
  }
  if (seen === 0) return { pass: false, score: 0, detail: `hidden tests did not run: ${output.trim().slice(-200)}` };
  const failed = [...groups].filter(([, ok]) => !ok).map(([tag]) => tag);
  const missing = 12 - new Set([...output.matchAll(/^(?:not ok|ok) \d+ - (R\d+) /gm)].map((m) => m[1])).size;
  const passed = 12 - failed.length - missing;
  return { pass: failed.length === 0 && missing === 0, score: Math.max(0, passed) / 12, detail: failed.length || missing ? `requirements failed: ${failed.join(", ") || "none"}${missing ? `; ${missing} not run` : ""}` : "all 12 requirements passed" };
}
