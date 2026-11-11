// Verifier for records-threshold: each record must keep its original text and end with exactly
// the expected status line. Score is the fraction of the 40 records done right, so a loss of
// early context shows as wrong decisions or a missing tag late in the batch.

import fs from "node:fs";
import path from "node:path";
import { buildRecords, expectedStatusLine } from "./fixture.mjs";

/**
 * Checks one record file and returns "" when it is right, or a short reason.
 * @param workspaceDir - the workspace
 * @param record - { path, text, value }
 * @param threshold - the batch threshold
 */
function checkRecord(workspaceDir, record, threshold) {
  const file = path.join(workspaceDir, record.path);
  if (!fs.existsSync(file)) return "missing";
  const text = fs.readFileSync(file, "utf8");
  const original = record.text.replace(/\n$/, "");
  if (!text.startsWith(original)) return "original text changed";
  const added = text.slice(original.length).trim();
  const expected = expectedStatusLine(record.value, threshold);
  if (added === expected) return "";
  return added ? `wrong line ${JSON.stringify(added.slice(0, 60))}` : "no status line";
}

/**
 * Scores all 40 records.
 * @param workspaceDir - the workspace after the agent finished
 * @param runInfo - run information (unused)
 */
export async function verify(workspaceDir, runInfo) {
  const { threshold, records } = buildRecords(runInfo.task?.seed ?? 2614);
  const problems = [];
  let correct = 0;
  records.forEach((record, index) => {
    const problem = checkRecord(workspaceDir, record, threshold);
    if (problem) problems.push(`rec-${String(index + 1).padStart(2, "0")}: ${problem}`);
    else correct += 1;
  });
  const firstBad = problems.slice(0, 4).join("; ");
  return { pass: correct === records.length, score: correct / records.length, detail: `${correct}/${records.length} records right${firstBad ? `; ${firstBad}` : ""}` };
}
