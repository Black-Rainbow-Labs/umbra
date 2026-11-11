// Reference solution: writes keywords.txt and report.json.

import fs from "node:fs";
import path from "node:path";
import { buildNotes, FACTS } from "../fixture.mjs";

/**
 * Writes the expected keywords file and report.
 * @param workspaceDir - the prepared workspace
 * @param options - { seed }
 */
export async function solve(workspaceDir, { seed }) {
  const notes = buildNotes(seed);
  fs.writeFileSync(path.join(workspaceDir, "keywords.txt"), notes.map((n) => n.keyword).join("\n") + "\n");
  fs.writeFileSync(path.join(workspaceDir, "report.json"), JSON.stringify({ ...FACTS, keyword_count: notes.length }, null, 2));
}
