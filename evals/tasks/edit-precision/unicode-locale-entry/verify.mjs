// Verifier: one value changed, every other byte of the localization file is identical.

import fs from "node:fs";
import path from "node:path";
import { buildLocales, FILE_NAME, NEW_VALUE, TARGET_KEY, TARGET_LOCALE, targetValueByteRange } from "./fixture.mjs";

/**
 * Compares the edited bytes with the original outside the target value and returns a problem text.
 * @param originalBuffer - the original file bytes
 * @param newBuffer - the edited file bytes
 * @param range - [start, end) byte range of the target value in the original
 */
function outsideProblem(originalBuffer, newBuffer, range) {
  const prefix = originalBuffer.subarray(0, range[0]);
  const suffix = originalBuffer.subarray(range[1]);
  if (newBuffer.length < prefix.length + suffix.length) return "the file is shorter than its unchanged parts";
  if (!newBuffer.subarray(0, prefix.length).equals(prefix)) return "bytes before the edited entry changed";
  if (!newBuffer.subarray(newBuffer.length - suffix.length).equals(suffix)) return "bytes after the edited entry changed";
  return "";
}

/**
 * Checks the edited entry's raw bytes and the parsed JSON value.
 * @param originalBuffer - the original file bytes
 * @param newBuffer - the edited file bytes
 * @param range - [start, end) byte range of the target value in the original
 */
function entryProblem(originalBuffer, newBuffer, range) {
  const suffixLength = originalBuffer.length - range[1];
  const middle = newBuffer.subarray(range[0], newBuffer.length - suffixLength);
  const want = Buffer.from(JSON.stringify(NEW_VALUE), "utf8");
  if (!middle.equals(want)) return `edited value is ${JSON.stringify(middle.toString("utf8"))}, expected raw ${JSON.stringify(NEW_VALUE)}`;
  try {
    const parsed = JSON.parse(newBuffer.toString("utf8"));
    if (parsed[TARGET_LOCALE][TARGET_KEY] !== NEW_VALUE) return "parsed value does not match";
  } catch (error) {
    return `file is not valid JSON: ${error.message}`;
  }
  return "";
}

/**
 * Verifies the localization edit on raw bytes.
 * @param workspaceDir - the workspace after the agent finished
 * @param runInfo - run information
 */
export async function verify(workspaceDir, runInfo) {
  const file = path.join(workspaceDir, FILE_NAME);
  if (!fs.existsSync(file)) return { pass: false, score: 0, detail: `${FILE_NAME} is missing` };
  const originalText = buildLocales(2614);
  const originalBuffer = Buffer.from(originalText, "utf8");
  const newBuffer = fs.readFileSync(file);
  const range = targetValueByteRange(originalText);
  const outside = outsideProblem(originalBuffer, newBuffer, range);
  if (outside) return { pass: false, score: 0, detail: outside };
  const entry = entryProblem(originalBuffer, newBuffer, range);
  if (entry) return { pass: false, score: 0, detail: entry };
  return { pass: true, score: 1, detail: "ja settings.language.title updated, all other bytes identical" };
}
