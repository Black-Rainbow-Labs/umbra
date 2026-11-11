// Verifier for ledger-balance. Each day is scored on its own: its BALANCE must equal the
// balance the agent wrote for the day before (the true balance when that day has none, the
// opening balance for day 1) plus that day's
// change. One arithmetic slip therefore costs one day, not every later day.

import fs from "node:fs";
import path from "node:path";
import { buildDays, dayDelta } from "./fixture.mjs";

/**
 * Reads the BALANCE the agent appended to a day, or null when the file was changed elsewhere
 * or has no single BALANCE line at the end.
 * @param workspaceDir - the workspace
 * @param day - the day from the fixture
 */
function readBalance(workspaceDir, day) {
  const file = path.join(workspaceDir, day.path);
  if (!fs.existsSync(file)) return { value: null, problem: "missing" };
  const text = fs.readFileSync(file, "utf8");
  const original = day.text.replace(/\n$/, "");
  if (!text.startsWith(original)) return { value: null, problem: "original text changed" };
  const match = /^BALANCE: (-?\d+)$/.exec(text.slice(original.length).trim());
  return match ? { value: Number(match[1]), problem: "" } : { value: null, problem: "no BALANCE line" };
}

/**
 * Scores the 35 days.
 * @param workspaceDir - the workspace after the agent finished
 * @param runInfo - run information
 */
export async function verify(workspaceDir, runInfo) {
  const { opening, days } = buildDays(runInfo.task?.seed ?? 2614);
  let previous = opening;
  let correct = 0;
  const problems = [];
  days.forEach((day, i) => {
    const { value, problem } = readBalance(workspaceDir, day);
    const expected = previous + dayDelta(day.transactions);
    if (value !== null && value === expected) correct += 1;
    else problems.push(`day ${i + 1}: ${problem || `got ${value}, expected ${expected}`}`);
    previous = value ?? day.closing;
  });
  return { pass: correct === days.length, score: correct / days.length, detail: `${correct}/${days.length} days right${problems.length ? `; ${problems.slice(0, 4).join("; ")}` : ""}` };
}
