// Verifier for the buried log line task: the named record must be quarantined and nothing else may change.

import fs from "node:fs";
import path from "node:path";
import { buildRecords, chooseTarget } from "./fixture.mjs";

/**
 * Reads and parses data/records.json from the workspace.
 * @param workspaceDir - the workspace
 */
function readRecords(workspaceDir) {
  return JSON.parse(fs.readFileSync(path.join(workspaceDir, "data", "records.json"), "utf8"));
}

/**
 * Compares the workspace records to the originals; only the target record may differ, and only in status.
 * @param original - the original records
 * @param actual - the records in the workspace
 * @param targetId - the id of the corrupt record
 */
function compareRecords(original, actual, targetId) {
  if (!Array.isArray(actual) || actual.length !== original.length) return { pass: false, score: 0, detail: "data/records.json has a different number of records" };
  const wrong = [];
  for (let index = 0; index < original.length; index += 1) {
    const expected = original[index].id === targetId ? { ...original[index], status: "quarantined" } : original[index];
    if (JSON.stringify(expected) !== JSON.stringify(actual[index])) wrong.push(original[index].id);
  }
  if (!wrong.length) return { pass: true, score: 1, detail: `${targetId} is quarantined and nothing else changed` };
  const targetWrong = wrong.includes(targetId);
  const detail = targetWrong ? `${targetId} was not quarantined correctly` : "";
  const others = wrong.filter((id) => id !== targetId);
  return { pass: false, score: targetWrong ? 0 : 0.5, detail: [detail, others.length ? `${others.length} other record(s) changed, first ${others[0]}` : ""].filter(Boolean).join("; ") };
}

/**
 * Verifies the data file against the original records.
 * @param workspaceDir - the workspace after the agent finished
 * @param runInfo - run information, used for the task seed
 */
export async function verify(workspaceDir, runInfo) {
  const seed = runInfo?.task?.seed ?? 2614;
  let actual;
  try { actual = readRecords(workspaceDir); } catch (error) { return { pass: false, score: 0, detail: `data/records.json cannot be read: ${error.message}` }; }
  return compareRecords(buildRecords(seed), actual, chooseTarget(seed).recordId);
}
