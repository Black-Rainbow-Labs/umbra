// Verifier for facts-report: 30 keyword lines in order plus the five early facts in report.json.
// Score is the fraction of the 36 items right (30 keywords, 5 facts, the count).

import fs from "node:fs";
import path from "node:path";
import { buildNotes, FACTS } from "./fixture.mjs";

/**
 * Compares two values loosely for strings (case and surrounding space ignored) and exactly for numbers.
 * @param actual - the value in report.json
 * @param expected - the expected value
 */
function sameValue(actual, expected) {
  if (typeof expected === "number") return typeof actual === "number" && actual === expected;
  return typeof actual === "string" && actual.trim().toLowerCase() === expected.toLowerCase();
}

/**
 * Reads report.json, returning null when it is missing or does not parse.
 * @param workspaceDir - the workspace
 */
function readReport(workspaceDir) {
  try { return JSON.parse(fs.readFileSync(path.join(workspaceDir, "report.json"), "utf8")); } catch { return null; }
}

/**
 * Scores the keywords file and the report.
 * @param workspaceDir - the workspace after the agent finished
 * @param runInfo - run information
 */
export async function verify(workspaceDir, runInfo) {
  const notes = buildNotes(runInfo.task?.seed ?? 2614);
  const keywordFile = path.join(workspaceDir, "keywords.txt");
  const lines = fs.existsSync(keywordFile) ? fs.readFileSync(keywordFile, "utf8").split(/\r?\n/).map((l) => l.trim()).filter(Boolean) : [];
  const keywordHits = notes.filter((note, i) => lines[i] && lines[i].toLowerCase() === note.keyword).length;
  const report = readReport(workspaceDir) || {};
  const wrongFacts = Object.keys(FACTS).filter((key) => !sameValue(report[key], FACTS[key]));
  const countOk = report.keyword_count === notes.length && lines.length === notes.length;
  const correct = keywordHits + (Object.keys(FACTS).length - wrongFacts.length) + (countOk ? 1 : 0);
  const total = notes.length + Object.keys(FACTS).length + 1;
  const detail = `keywords ${keywordHits}/${notes.length} (lines ${lines.length}); facts wrong: ${wrongFacts.map((k) => `${k}=${JSON.stringify(report[k])}`).join(", ") || "none"}; count ${countOk ? "ok" : "bad"}`;
  return { pass: correct === total, score: correct / total, detail };
}
