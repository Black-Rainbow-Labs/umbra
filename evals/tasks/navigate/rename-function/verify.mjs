// Verifier for the rename task: runs the entry script and the tests and checks that only the intended files changed.

import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { buildRepo } from "./fixture.mjs";
import { writeFiles, readFilesAsBuffers } from "./gen-utils.mjs";

/**
 * Runs node with arguments in a folder and returns { status, stdout, stderr }.
 * @param cwd - working folder
 * @param args - node arguments
 */
function runNode(cwd, args) {
  const result = spawnSync("node", args, { cwd, encoding: "utf8", timeout: 60000 });
  return { status: result.status, stdout: result.stdout || "", stderr: result.stderr || "" };
}

/**
 * Counts whole word matches of the old function name in a text.
 * @param text - the text to scan
 */
function countOldName(text) {
  return (text.match(/\bcalcDiscount\b/g) || []).length;
}

/**
 * Checks that files outside the touched set are identical and that no file was added or removed.
 * @param original - the original repository build
 * @param actual - Map of path to Buffer from the workspace
 */
function checkUnrelatedFiles(original, actual) {
  const expectedPaths = [...original.files.keys()].sort().join("\n");
  const actualPaths = [...actual.keys()].sort().join("\n");
  if (expectedPaths !== actualPaths) return "the set of files changed (a file was added, removed or moved)";
  for (const [file, text] of original.files) {
    if (original.touched.has(file)) continue;
    if (!actual.get(file).equals(Buffer.from(text))) return `${file} was changed but has nothing to rename`;
  }
  return "";
}

/**
 * Checks that no code use of the old name is left and that every decoy mention is still present.
 * @param original - the original repository build
 * @param actual - Map of path to Buffer from the workspace
 */
function checkNames(original, actual) {
  let remaining = 0;
  let allText = "";
  for (const buffer of actual.values()) {
    const text = buffer.toString("utf8");
    remaining += countOldName(text);
    allText += text + "\n";
  }
  const allowed = original.decoyLines.reduce((sum, line) => sum + countOldName(line), 0);
  if (remaining > allowed) return `${remaining - allowed} use(s) of calcDiscount are still in the code`;
  if (remaining < allowed) return "a comment, string or doc mention of calcDiscount was changed";
  const lost = original.decoyLines.find((line) => !allText.includes(line));
  return lost ? `an unrelated line was changed: ${lost.trim().slice(0, 70)}` : "";
}

/**
 * Checks the exports of the pricing barrel by importing it in a child process.
 * @param workspaceDir - the workspace
 */
function checkExports(workspaceDir) {
  const script = 'const m = await import("./src/pricing/index.mjs"); console.log(JSON.stringify([typeof m.computeDiscount, typeof m.calcDiscount, typeof m.calcDiscountRate, typeof m.calcDiscountedTotal, m.computeDiscount?.(2000, 10), m.calcDiscountedTotal?.(1000, 25)]));';
  const run = runNode(workspaceDir, ["--input-type=module", "-e", script]);
  if (run.status !== 0) return "src/pricing/index.mjs could not be imported";
  const [fnType, oldType, rateType, totalType, value, total] = JSON.parse(run.stdout.trim());
  if (fnType !== "function") return "computeDiscount is not exported from src/pricing/index.mjs";
  if (oldType !== "undefined") return "calcDiscount is still exported";
  if (rateType !== "function" || totalType !== "function") return "calcDiscountRate or calcDiscountedTotal was lost";
  return value === 200 && total === 750 ? "" : "computeDiscount or calcDiscountedTotal returns wrong values";
}

/**
 * Runs main.mjs on the original build and on the workspace and compares the printed JSON.
 * @param seed - the fixed seed
 * @param workspaceDir - the workspace
 */
function checkMainOutput(seed, workspaceDir) {
  const temp = fs.mkdtempSync(path.join(os.tmpdir(), "rename-original-"));
  try {
    writeFiles(temp, buildRepo(seed).files);
    const expected = runNode(temp, ["main.mjs"]);
    const actual = runNode(workspaceDir, ["main.mjs"]);
    if (actual.status !== 0) return `node main.mjs failed: ${actual.stderr.trim().split("\n").pop()}`;
    return actual.stdout.trim() === expected.stdout.trim() ? "" : "node main.mjs prints different output than before";
  } finally {
    fs.rmSync(temp, { recursive: true, force: true });
  }
}

/**
 * Runs the repository's own tests in the workspace.
 * @param workspaceDir - the workspace
 */
function checkTests(workspaceDir) {
  const run = runNode(workspaceDir, ["--test"]);
  return run.status === 0 ? "" : "node --test fails";
}

/**
 * Scores the rename: one point for each of five checks.
 * @param workspaceDir - the workspace after the agent finished
 * @param runInfo - run information, used for the task seed
 */
export async function verify(workspaceDir, runInfo) {
  const seed = runInfo?.task?.seed ?? 2614;
  const original = buildRepo(seed);
  const actual = readFilesAsBuffers(workspaceDir);
  const results = [];
  const run = (label, fn) => {
    try { results.push([label, fn()]); } catch (error) { results.push([label, `${label} check crashed: ${error.message}`]); }
  };
  run("files", () => checkUnrelatedFiles(original, actual));
  run("names", () => checkNames(original, actual));
  run("exports", () => checkExports(workspaceDir));
  run("main", () => checkMainOutput(seed, workspaceDir));
  run("tests", () => checkTests(workspaceDir));
  const failures = results.filter(([, problem]) => problem);
  const score = (results.length - failures.length) / results.length;
  return { pass: failures.length === 0, score, detail: failures.length ? failures.map(([, problem]) => problem).join("; ") : "rename is complete and nothing else changed" };
}
