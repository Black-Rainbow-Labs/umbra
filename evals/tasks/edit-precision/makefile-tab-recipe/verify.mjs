// Verifier: billing-package gained one tab indented recipe line, and nothing else changed.

import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { buildMakefile, targetBlock, TARGET_NAME } from "./fixture.mjs";

/**
 * Runs make -n for a target in a folder and returns stdout and stderr.
 * @param folder - folder containing the Makefile
 * @param target - target name
 */
function dryRun(folder, target) {
  const result = spawnSync("make", ["-n", "--no-print-directory", "-C", folder, target], { encoding: "utf8", timeout: 30000 });
  return { ok: result.status === 0, text: `${result.stdout || ""}`.replace(/\r\n/g, "\n"), error: `${result.stderr || ""}`.trim() };
}

/**
 * Checks that the bytes before and after the target block are unchanged.
 * @param originalText - the original Makefile text
 * @param newBuffer - the edited file bytes
 */
function outsideUnchanged(originalText, newBuffer) {
  const [start, end] = targetBlock(originalText);
  const prefix = Buffer.from(originalText.slice(0, start));
  const block = Buffer.from(originalText.slice(start, end));
  const suffix = Buffer.from(originalText.slice(end));
  if (newBuffer.length < prefix.length + block.length + suffix.length) return "the file got shorter";
  if (!newBuffer.subarray(0, prefix.length).equals(prefix)) return "bytes before billing-package changed";
  if (!newBuffer.subarray(newBuffer.length - suffix.length).equals(suffix)) return "bytes after billing-package changed";
  const middle = newBuffer.subarray(prefix.length, newBuffer.length - suffix.length);
  if (!middle.subarray(0, block.length).equals(block)) return "existing lines of billing-package changed";
  return "";
}

/**
 * Compares make -n output of the edited Makefile with the original plus the new command.
 * @param originalText - the original Makefile text
 * @param newBuffer - the edited file bytes
 * @param tempDir - folder to hold the original copy
 * @param workspaceDir - the workspace
 */
function dryRunMismatch(originalText, newBuffer, tempDir, workspaceDir) {
  fs.writeFileSync(path.join(tempDir, "Makefile"), originalText);
  const before = dryRun(tempDir, TARGET_NAME);
  const after = dryRun(workspaceDir, TARGET_NAME);
  if (!after.ok) return `make -n ${TARGET_NAME} failed: ${after.error.slice(0, 120)}`;
  const want = before.text.replace(/\n$/, "") + '\necho "billing package signed"\n';
  return after.text === want ? "" : `make -n ${TARGET_NAME} printed ${JSON.stringify(after.text.slice(-120))}`;
}

/**
 * Verifies the Makefile edit by bytes outside the region and by make -n.
 * @param workspaceDir - the workspace after the agent finished
 * @param runInfo - run information
 */
export async function verify(workspaceDir, runInfo) {
  const file = path.join(workspaceDir, "Makefile");
  if (!fs.existsSync(file)) return { pass: false, score: 0, detail: "Makefile is missing" };
  const originalText = buildMakefile(2614);
  const newBuffer = fs.readFileSync(file);
  const bytesProblem = outsideUnchanged(originalText, newBuffer);
  if (bytesProblem) return { pass: false, score: 0, detail: bytesProblem };
  const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), "eval-make-"));
  try {
    const mismatch = dryRunMismatch(originalText, newBuffer, tempDir, workspaceDir);
    if (mismatch) return { pass: false, score: 0, detail: mismatch };
    return { pass: true, score: 1, detail: "recipe line added with a tab, nothing else changed" };
  } finally {
    fs.rmSync(tempDir, { recursive: true, force: true });
  }
}
