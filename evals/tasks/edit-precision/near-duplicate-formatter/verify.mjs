// Verifier: only formatShipmentRow changed, and it now reports kilograms.

import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { buildRowsModule, targetRange } from "./fixture.mjs";

const SAMPLE_ROWS = [
  { id: 7, name: "  Pallet of tiles ", amount: 12345, weight: 1234, status: "late", date: "2026-03-04T10:00:00Z", priority: 2, tags: ["a", "b"], owner: "Kim", region: "emea", note: "fragile   goods" },
  { id: 12, name: "Box", amount: -500, weight: 987, status: "ok", date: "2026-01-09", owner: "", region: "", note: "" },
  { id: 3, name: "Crate", amount: 99, weight: 15600, status: "new", date: "2025-12-31", tags: [], owner: "Lee", region: "apac" },
];

/**
 * Imports a module from a temp copy so repeated imports are not cached.
 * @param text - the module source
 * @param tempDir - folder to write the copy into
 * @param name - file name for the copy
 */
async function importCopy(text, tempDir, name) {
  const file = path.join(tempDir, name);
  fs.writeFileSync(file, text);
  return import(pathToFileURL(file).href);
}

/**
 * Computes the expected output of the new formatShipmentRow from the original one.
 * @param original - the original module
 * @param row - a sample row
 */
function expectedLine(original, row) {
  const base = original.formatShipmentRow(row);
  const grams = `${Math.round(Number(row.weight))} g`;
  return base.replace(grams, `${(Number(row.weight) / 1000).toFixed(1)} kg`);
}

/**
 * Checks that everything outside formatShipmentRow is byte identical.
 * @param originalBuffer - the original file bytes
 * @param newBuffer - the edited file bytes
 * @param originalText - the original text
 */
function outsideUnchanged(originalBuffer, newBuffer, originalText) {
  const [start, end] = targetRange(originalText);
  const prefix = originalBuffer.subarray(0, Buffer.byteLength(originalText.slice(0, start)));
  const suffix = originalBuffer.subarray(Buffer.byteLength(originalText.slice(0, end)));
  return newBuffer.length >= prefix.length + suffix.length && newBuffer.subarray(0, prefix.length).equals(prefix) && newBuffer.subarray(newBuffer.length - suffix.length).equals(suffix);
}

/**
 * Compares the edited formatter with the expected output on the sample rows.
 * @param original - the original module
 * @param edited - the edited module
 */
function behaviorMismatch(original, edited) {
  for (const row of SAMPLE_ROWS) {
    const got = edited.formatShipmentRow(row);
    const want = expectedLine(original, row);
    if (got !== want) return `formatShipmentRow returned ${JSON.stringify(got)}, expected ${JSON.stringify(want)}`;
  }
  return "";
}

/**
 * Verifies the edit: bytes outside the target function and the behavior of the target.
 * @param workspaceDir - the workspace after the agent finished
 * @param runInfo - run information
 */
export async function verify(workspaceDir, runInfo) {
  const file = path.join(workspaceDir, "src", "rows.mjs");
  if (!fs.existsSync(file)) return { pass: false, score: 0, detail: "src/rows.mjs is missing" };
  const originalText = buildRowsModule(2614);
  const originalBuffer = Buffer.from(originalText);
  const newBuffer = fs.readFileSync(file);
  if (!outsideUnchanged(originalBuffer, newBuffer, originalText)) return { pass: false, score: 0, detail: "bytes outside formatShipmentRow changed" };
  const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), "eval-rows-"));
  try {
    const original = await importCopy(originalText, tempDir, "original.mjs");
    const edited = await importCopy(newBuffer.toString("utf8"), tempDir, "edited.mjs");
    const mismatch = behaviorMismatch(original, edited);
    if (mismatch) return { pass: false, score: 0, detail: mismatch };
    return { pass: true, score: 1, detail: "formatShipmentRow reports kilograms, nothing else changed" };
  } catch (error) {
    return { pass: false, score: 0, detail: `module failed: ${error.message}` };
  } finally {
    fs.rmSync(tempDir, { recursive: true, force: true });
  }
}
