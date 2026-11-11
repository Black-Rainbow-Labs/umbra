// Verifier: backoffDelayMs is capped, every line ending is still CRLF, nothing else changed.

import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { buildRetryModule, FILE_NAME, TARGET_TEXT } from "./fixture.mjs";

const EXPECTED_DELAYS = [[0, 250], [1, 500], [3, 2000], [7, 30000], [8, 30000], [20, 30000], [-2, 250]];

/**
 * Counts line endings that are not CRLF (a lone LF or a lone CR).
 * @param buffer - the file bytes
 */
function countBadLineEndings(buffer) {
  let bad = 0;
  for (let index = 0; index < buffer.length; index += 1) {
    const byte = buffer[index];
    if (byte === 0x0a && buffer[index - 1] !== 0x0d) bad += 1;
    if (byte === 0x0d && buffer[index + 1] !== 0x0a) bad += 1;
  }
  return bad;
}

/**
 * Checks the bytes before and after the target function are unchanged.
 * @param originalText - the original file text
 * @param newBuffer - the edited file bytes
 */
function outsideProblem(originalText, newBuffer) {
  const start = originalText.indexOf(TARGET_TEXT);
  const prefix = Buffer.from(originalText.slice(0, start));
  const suffix = Buffer.from(originalText.slice(start + TARGET_TEXT.length));
  if (newBuffer.length < prefix.length + suffix.length) return "file is shorter than the unchanged parts";
  if (!newBuffer.subarray(0, prefix.length).equals(prefix)) return "bytes before backoffDelayMs changed";
  if (!newBuffer.subarray(newBuffer.length - suffix.length).equals(suffix)) return "bytes after backoffDelayMs changed";
  return "";
}

/**
 * Calls the edited function and compares with the expected delays.
 * @param newBuffer - the edited file bytes
 */
async function behaviorProblem(newBuffer) {
  const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), "eval-crlf-"));
  try {
    const file = path.join(tempDir, FILE_NAME);
    fs.writeFileSync(file, newBuffer);
    const module = await import(pathToFileURL(file).href);
    for (const [attempt, want] of EXPECTED_DELAYS) {
      const got = module.backoffDelayMs(attempt);
      if (got !== want) return `backoffDelayMs(${attempt}) returned ${got}, expected ${want}`;
    }
    return "";
  } catch (error) {
    return `module failed: ${error.message}`;
  } finally {
    fs.rmSync(tempDir, { recursive: true, force: true });
  }
}

/**
 * Verifies line endings, untouched bytes and behavior.
 * @param workspaceDir - the workspace after the agent finished
 * @param runInfo - run information
 */
export async function verify(workspaceDir, runInfo) {
  const file = path.join(workspaceDir, FILE_NAME);
  if (!fs.existsSync(file)) return { pass: false, score: 0, detail: `${FILE_NAME} is missing` };
  const newBuffer = fs.readFileSync(file);
  const bad = countBadLineEndings(newBuffer);
  if (bad > 0) return { pass: false, score: 0, detail: `${bad} line endings are not CRLF` };
  const outside = outsideProblem(buildRetryModule(2614), newBuffer);
  if (outside) return { pass: false, score: 0, detail: outside };
  const behavior = await behaviorProblem(newBuffer);
  if (behavior) return { pass: false, score: 0, detail: behavior };
  return { pass: true, score: 1, detail: "backoffDelayMs capped, CRLF kept, nothing else changed" };
}
