// Verifier for notes-16k-roster: 24 roster lines in the exact form plus the two early facts and
// the count in summary.json. Score is the fraction of the 27 items right.

import fs from "node:fs";
import path from "node:path";
import { buildNotes, rosterLine, FACTS } from "./fixture.mjs";

/**
 * Reads summary.json, returning an empty object when it is missing or does not parse.
 * @param workspaceDir - the workspace
 */
function readSummary(workspaceDir) {
  try { return JSON.parse(fs.readFileSync(path.join(workspaceDir, "summary.json"), "utf8")) || {}; } catch { return {}; }
}

/**
 * Reads the non empty lines of roster.txt, or an empty list when the file is missing.
 * @param workspaceDir - the workspace
 */
function readRoster(workspaceDir) {
  try { return fs.readFileSync(path.join(workspaceDir, "roster.txt"), "utf8").split(/\r?\n/).map((l) => l.trim()).filter(Boolean); } catch { return []; }
}

/**
 * Scores the roster and the summary.
 * @param workspaceDir - the workspace after the agent finished
 * @param runInfo - run information
 */
export async function verify(workspaceDir, runInfo) {
  const notes = buildNotes(runInfo.task?.seed ?? 2614);
  const lines = readRoster(workspaceDir);
  const badLines = [];
  notes.forEach((note, i) => { if (lines[i] !== rosterLine(i + 1, note.first, note.last)) badLines.push(`${i + 1}: ${JSON.stringify((lines[i] || "missing").slice(0, 40))}`); });
  const summary = readSummary(workspaceDir);
  const wrongFacts = Object.keys(FACTS).filter((key) => typeof summary[key] !== "string" || summary[key].trim().toLowerCase() !== FACTS[key].toLowerCase());
  const countOk = summary.attendee_count === notes.length && lines.length === notes.length;
  const correct = notes.length - badLines.length + (Object.keys(FACTS).length - wrongFacts.length) + (countOk ? 1 : 0);
  const total = notes.length + Object.keys(FACTS).length + 1;
  const detail = `roster ${notes.length - badLines.length}/${notes.length}${badLines.length ? ` (bad ${badLines.slice(0, 3).join("; ")})` : ""}; facts wrong: ${wrongFacts.map((k) => `${k}=${JSON.stringify(summary[k])}`).join(", ") || "none"}; count ${countOk ? "ok" : "bad"}`;
  return { pass: correct === total, score: correct / total, detail };
}
