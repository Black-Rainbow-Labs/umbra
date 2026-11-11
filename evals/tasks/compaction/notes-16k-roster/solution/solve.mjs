// Reference solution: writes roster.txt and summary.json.

import fs from "node:fs";
import path from "node:path";
import { buildNotes, rosterLine, FACTS } from "../fixture.mjs";

/**
 * Writes the expected roster and summary.
 * @param workspaceDir - the prepared workspace
 * @param options - { seed }
 */
export async function solve(workspaceDir, { seed }) {
  const notes = buildNotes(seed);
  const lines = notes.map((note, i) => rosterLine(i + 1, note.first, note.last));
  fs.writeFileSync(path.join(workspaceDir, "roster.txt"), lines.join("\n") + "\n");
  fs.writeFileSync(path.join(workspaceDir, "summary.json"), JSON.stringify({ ...FACTS, attendee_count: notes.length }, null, 2));
}
