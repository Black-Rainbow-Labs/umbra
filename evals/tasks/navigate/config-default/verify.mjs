// Verifier for the config default task: calls every settings reader and checks that exactly one default changed.

import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { buildRepo, TARGET_FILE } from "./fixture.mjs";
import { writeFiles, readFilesAsBuffers } from "./gen-utils.mjs";

const DUMP_SCRIPT = path.join(path.dirname(fileURLToPath(import.meta.url)), "hidden", "dump_defaults.py");
const TARGET_KEY = "shopkit.export.scheduler.retry_limit";

/**
 * Runs the dump script against a package root and returns the parsed JSON, or null with an error text.
 * @param root - folder that contains the shopkit package
 * @param overrides - optional settings object to configure before calling the readers
 */
function dumpDefaults(root, overrides) {
  const args = ["-I", "-B", DUMP_SCRIPT, root];
  if (overrides) args.push(JSON.stringify(overrides));
  const run = spawnSync("python", args, { encoding: "utf8", timeout: 60000 });
  if (run.status !== 0) return { error: (run.stderr || "python failed").trim().split("\n").pop() };
  return { values: JSON.parse(run.stdout) };
}

/**
 * Checks that every file except the scheduler module is byte identical and the scheduler changed on one line only.
 * @param original - the original build
 * @param actual - Map of path to Buffer from the workspace
 */
function checkFiles(original, actual) {
  const expectedPaths = [...original.files.keys()].sort().join("\n");
  if (expectedPaths !== [...actual.keys()].sort().join("\n")) return describeFileSetChange(original.files, actual);
  for (const [file, text] of original.files) {
    if (file === TARGET_FILE) continue;
    if (!actual.get(file).equals(Buffer.from(text))) return `${file} was changed`;
  }
  const before = original.files.get(TARGET_FILE).split("\n");
  const after = actual.get(TARGET_FILE).toString("utf8").split("\n");
  if (before.length !== after.length) return `${TARGET_FILE} has a different number of lines`;
  const changed = before.filter((line, index) => line !== after[index]);
  return changed.length === 1 ? "" : `${TARGET_FILE} has ${changed.length} changed lines instead of 1`;
}

/**
 * Compares the defaults of the workspace against the original build and checks the one expected change.
 * @param original - the original build, written to a temp folder
 * @param workspaceDir - the workspace
 */
function checkDefaults(original, workspaceDir) {
  const temp = fs.mkdtempSync(path.join(os.tmpdir(), "config-original-"));
  try {
    writeFiles(temp, original.files);
    const before = dumpDefaults(temp);
    const after = dumpDefaults(workspaceDir);
    if (after.error) return `the package does not import or run: ${after.error}`;
    if (after.values[TARGET_KEY] !== 5) return `export.max_retries default is ${after.values[TARGET_KEY]}, expected 5`;
    const different = Object.keys(before.values).filter((key) => key !== TARGET_KEY && before.values[key] !== after.values[key]);
    if (different.length) return `another setting changed: ${different[0]}`;
    return "";
  } finally {
    fs.rmSync(temp, { recursive: true, force: true });
  }
}

/**
 * Checks that a configured value still wins over the new default.
 * @param workspaceDir - the workspace
 */
function checkOverride(workspaceDir) {
  const result = dumpDefaults(workspaceDir, { "export.max_retries": 9 });
  if (result.error) return `the package does not run with settings configured: ${result.error}`;
  return result.values[TARGET_KEY] === 9 ? "" : "a configured export.max_retries no longer wins";
}

/**
 * Scores the task: files, defaults and override checks.
 * @param workspaceDir - the workspace after the agent finished
 * @param runInfo - run information, used for the task seed
 */
export async function verify(workspaceDir, runInfo) {
  const seed = runInfo?.task?.seed ?? 2614;
  const original = buildRepo(seed);
  const results = [];
  const run = (fn) => {
    try { results.push(fn()); } catch (error) { results.push(`check crashed: ${error.message}`); }
  };
  run(() => checkFiles(original, readFilesAsBuffers(workspaceDir)));
  run(() => checkDefaults(original, workspaceDir));
  run(() => checkOverride(workspaceDir));
  const failures = results.filter(Boolean);
  const score = (results.length - failures.length) / results.length;
  return { pass: failures.length === 0, score, detail: failures.length ? failures.join("; ") : "export.max_retries defaults to 5 and nothing else changed" };
}

/**
 * Names the files that were added or removed compared with the original repository.
 * @param originalFiles - Map of the original paths
 * @param actual - Map of the paths in the workspace
 */
function describeFileSetChange(originalFiles, actual) {
  const added = [...actual.keys()].filter((p) => !originalFiles.has(p));
  const removed = [...originalFiles.keys()].filter((p) => !actual.has(p));
  return `the set of files changed: added ${added.slice(0, 5).join(", ") || "none"}; removed ${removed.slice(0, 5).join(", ") || "none"}`;
}
